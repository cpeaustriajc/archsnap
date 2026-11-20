use crate::bodies::Body;
use crate::reader::{Reader, attribute, name_of, path_text, place_of, why_outside};
use crate::ty::Ty;
use crate::typing::plain;
use earnest_ast::{
    ActionBody, ActionDraft, Draft, EndDraft, FieldDraft, HelperDraft, HelperKind, Language,
    Quoting, Reading, RuleDraft, Shape, Term, TermKind, ViewDraft, condition_text, nesting_problem,
};
use proc_macro2::{Ident, Span};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Attribute, Block, Expr, Field, Fields, FieldsNamed, FnArg, ImplItem, ImplItemFn, Item,
    ItemEnum, ItemFn, ItemImpl, ItemStruct, ItemUse, LitStr, Pat, PatType, Path, ReturnType,
    Signature, Token, Type, UseTree, Visibility,
};

const ROLES: [&str; 3] = ["action", "rule", "end"];
const NEEDS_A_START: &str = "write the starting value in the brackets, `#[initial(value)]`, or \
    each value the field starts at, `#[initial(A, B)]`";
const LIBRARY_NAMES: [&str; 7] = ["forall", "exists", "Some", "None", "Set", "Seq", "Map"];
const STATE: &str = "self";
const WAIT: &str = "waiting is outside the library: model it with a `Request` or a `Timer`";
const ONLY_MODELS: &str = "a model file holds only models, the types they use, constants and \
    helper functions: move anything else to another file, which the checker does not read as a \
    model unless it derives `Model`";

#[derive(Default)]
struct Found<'a> {
    models: Vec<&'a ItemStruct>,
    blocks: Vec<&'a ItemImpl>,
    views: Vec<&'a ItemImpl>,
    constants: Vec<(String, &'a Type, &'a Expr)>,
    functions: Vec<&'a ItemFn>,
}

fn derives(attributes: &[Attribute], wanted: &str) -> bool {
    attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("derive"))
        .any(|attribute| {
            attribute
                .parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
                .is_ok_and(|listed| {
                    listed.iter().any(|path| {
                        path.segments
                            .last()
                            .is_some_and(|segment| name_of(&segment.ident) == wanted)
                    })
                })
        })
}

fn is_test_only(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("cfg")
            && attribute
                .parse_args::<Path>()
                .is_ok_and(|condition| condition.is_ident("test"))
    })
}

fn is_part_of_a_model(item: &Item) -> bool {
    match item {
        Item::Struct(found) => derives(&found.attrs, "Model") || derives(&found.attrs, "Domain"),
        Item::Enum(found) => derives(&found.attrs, "Domain"),
        Item::Impl(found) => marks_model(&found.attrs) || marks_view(&found.attrs),
        _ => false,
    }
}

fn names_a_role(attribute: &Attribute) -> bool {
    let path = attribute.path();
    path.segments.len() > 1
        && path
            .segments
            .last()
            .is_some_and(|segment| ROLES.contains(&name_of(&segment.ident).as_str()))
}

fn marks_model(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| name_of(&segment.ident) == "model")
    })
}

fn marks_view(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| name_of(&segment.ident) == "view")
    })
}

fn attributes_of(item: &Item) -> &[Attribute] {
    match item {
        Item::Const(found) => &found.attrs,
        Item::Enum(found) => &found.attrs,
        Item::Fn(found) => &found.attrs,
        Item::Impl(found) => &found.attrs,
        Item::Mod(found) => &found.attrs,
        Item::Struct(found) => &found.attrs,
        Item::Use(found) => &found.attrs,
        _ => &[],
    }
}

fn keyword(item: &Item) -> (Span, &'static str) {
    match item {
        Item::ExternCrate(found) => (found.extern_token.span, "extern crate"),
        Item::ForeignMod(found) => (found.abi.extern_token.span, "extern"),
        Item::Macro(found) => (found.mac.path.span(), "a macro"),
        Item::Mod(found) => (found.mod_token.span, "mod"),
        Item::Static(found) => (found.static_token.span, "static"),
        Item::Trait(found) => (found.trait_token.span, "trait"),
        Item::TraitAlias(found) => (found.trait_token.span, "trait"),
        Item::Type(found) => (found.type_token.span, "type"),
        Item::Union(found) => (found.union_token.span, "union"),
        other => (other.span(), "an item"),
    }
}

fn owner_of(block: &ItemImpl) -> Option<String> {
    match block.self_ty.as_ref() {
        Type::Path(named) if named.qself.is_none() => named.path.get_ident().map(name_of),
        _ => None,
    }
}

const BYTE_ORDER_MARK: char = '\u{feff}';

pub(crate) fn read_file(file: &str, source: &str) -> Reading {
    let source = source.strip_prefix(BYTE_ORDER_MARK).unwrap_or(source);
    if let Some(problem) = nesting_problem(file, source, Quoting::Rust) {
        return Reading {
            drafts: Vec::new(),
            problems: vec![problem],
        };
    }
    let mut reader = Reader::new(file, source);
    let drafts = match syn::parse_file(source) {
        Ok(parsed) => reader.drafts(&parsed.items),
        Err(refused) => {
            let seen = reader.short_text(refused.span());
            let feature = if seen.is_empty() {
                "the end of the file".to_owned()
            } else {
                seen
            };
            reader.refuse(
                refused.span(),
                &feature,
                &format!("this is not Rust the reader can parse: {refused}"),
            );
            Vec::new()
        }
    };
    let mut problems = reader.problems;
    problems.sort_by_key(earnest_diagnostics::Problem::place);
    problems.dedup();
    if problems.is_empty() {
        Reading { drafts, problems }
    } else {
        Reading {
            drafts: Vec::new(),
            problems,
        }
    }
}

impl Reader<'_> {
    fn drafts(&mut self, items: &[Item]) -> Vec<Draft> {
        let found = self.declare(items);
        let constants: Vec<(String, Term)> = found
            .constants
            .iter()
            .map(|(name, ty, value)| {
                self.walk_constant(ty, value, "the value of this constant");
                let read = self.term(value);
                (name.clone(), self.of_type(ty, read))
            })
            .collect();
        let helpers: Vec<HelperDraft> = found
            .functions
            .iter()
            .map(|function| self.function(function))
            .collect();
        let models: Vec<String> = found
            .models
            .iter()
            .map(|state| name_of(&state.ident))
            .collect();
        for block in &found.blocks {
            if !owner_of(block).is_some_and(|owner| models.contains(&owner)) {
                let feature = format!("impl {}", self.short_text(block.self_ty.span()));
                self.refuse(
                    block.self_ty.span(),
                    &feature,
                    "`#[model]` goes on the `impl` block of a struct that derives `Model`",
                );
            }
        }
        for block in &found.views {
            if !owner_of(block).is_some_and(|owner| models.contains(&owner)) {
                let feature = format!("impl {}", self.short_text(block.self_ty.span()));
                self.refuse(
                    block.self_ty.span(),
                    &feature,
                    "`#[view]` goes on the `impl` block of a struct that derives `Model`",
                );
            }
        }
        found
            .models
            .iter()
            .map(|state| self.model(state, &found, &constants, &helpers))
            .collect()
    }

    fn declare<'b>(&mut self, items: &'b [Item]) -> Found<'b> {
        let mut found = Found::default();
        let mut types: Vec<(String, Span)> = Vec::new();
        let mut values: Vec<(String, Span)> = Vec::new();
        for item in items {
            let attributes = attributes_of(item);
            if is_test_only(attributes) && !is_part_of_a_model(item) {
                continue;
            }
            self.refuse_conditional(attributes);
            match item {
                Item::Use(import) => self.import(import),
                Item::Enum(choices) => {
                    if derives(&choices.attrs, "Copy") {
                        self.copies.insert(name_of(&choices.ident));
                    }
                    types.push((name_of(&choices.ident), choices.ident.span()));
                    self.refuse_library_name(&choices.ident, "enum");
                    self.declare_enum(choices);
                }
                Item::Struct(state) if derives(&state.attrs, "Model") => {
                    self.note_struct(state);
                    types.push((name_of(&state.ident), state.ident.span()));
                    self.refuse_library_name(&state.ident, "struct");
                    self.name_of(state);
                    self.models.insert(name_of(&state.ident));
                    found.models.push(state);
                }
                Item::Struct(record) => {
                    self.note_struct(record);
                    types.push((name_of(&record.ident), record.ident.span()));
                    self.refuse_library_name(&record.ident, "struct");
                    self.declare_record(record);
                }
                Item::Impl(block) if marks_model(&block.attrs) => found.blocks.push(block),
                Item::Impl(block) if marks_view(&block.attrs) => found.views.push(block),
                Item::Impl(block) => self.refuse(block.impl_token.span, "impl", ONLY_MODELS),
                Item::Const(constant) => {
                    let name = name_of(&constant.ident);
                    values.push((name.clone(), constant.ident.span()));
                    self.refuse_library_name(&constant.ident, "const");
                    self.const_types
                        .insert(name.clone(), constant.ty.as_ref().clone());
                    self.const_values
                        .insert(name.clone(), constant.expr.as_ref().clone());
                    if let Some(number) = self.whole_number(&constant.expr) {
                        self.numbers.insert(name.clone(), number);
                    }
                    found
                        .constants
                        .push((name, constant.ty.as_ref(), constant.expr.as_ref()));
                }
                Item::Fn(function) => {
                    let name = name_of(&function.sig.ident);
                    values.push((name.clone(), function.sig.ident.span()));
                    self.refuse_library_name(&function.sig.ident, "fn");
                    if let ReturnType::Type(_, ty) = &function.sig.output {
                        self.fn_types.insert(name.clone(), ty.as_ref().clone());
                    }
                    self.fn_params
                        .insert(name.clone(), typed_parameter_types(&function.sig));
                    self.functions.insert(name);
                    found.functions.push(function);
                }
                other => {
                    let (span, feature) = keyword(other);
                    self.refuse(span, feature, ONLY_MODELS);
                }
            }
        }
        self.refuse_repeats(&types, "type", "E0428");
        self.refuse_repeats(&values, "item", "E0428");
        found
    }

    fn note_struct(&mut self, item: &ItemStruct) {
        let name = name_of(&item.ident);
        if derives(&item.attrs, "Copy") {
            self.copies.insert(name.clone());
        }
        if let Fields::Named(fields) = &item.fields {
            let listed = fields
                .named
                .iter()
                .filter_map(|field| Some((name_of(field.ident.as_ref()?), field.ty.clone())))
                .collect();
            self.structs.insert(name, listed);
        }
    }

    fn import(&mut self, import: &ItemUse) {
        let root = match &import.tree {
            UseTree::Path(path) => Some(&path.ident),
            UseTree::Name(name) => Some(&name.ident),
            UseTree::Rename(renamed) => Some(&renamed.ident),
            UseTree::Glob(_) | UseTree::Group(_) => None,
        };
        if root.is_some_and(|name| name_of(name) == "earnest") {
            if !matches!(import.vis, Visibility::Inherited) {
                let path = self.short_text(import.tree.span()).replace(' ', "");
                self.refuse(
                    import.tree.span(),
                    &format!("pub use {path}"),
                    "a model file imports the library directly, with a plain `use earnest::...`: \
                     another file that re-exports it would hide the library from the checker",
                );
            }
            self.refuse_renames(&import.tree);
            return;
        }
        let path = self.short_text(import.tree.span()).replace(' ', "");
        self.refuse(import.tree.span(), &path, why_outside(&path));
    }

    fn refuse_renames(&mut self, tree: &UseTree) {
        match tree {
            UseTree::Path(path) => self.refuse_renames(&path.tree),
            UseTree::Group(group) => {
                for item in &group.items {
                    self.refuse_renames(item);
                }
            }
            UseTree::Rename(renamed) if renamed.rename != "_" => {
                let feature = format!("{} as {}", renamed.ident, renamed.rename);
                self.refuse(
                    renamed.rename.span(),
                    &feature,
                    "the checker reads a library name by the name it is written with, so an \
                     item of the library can't be imported under another name",
                );
            }
            UseTree::Rename(_) | UseTree::Name(_) | UseTree::Glob(_) => {}
        }
    }

    fn refuse_library_name(&mut self, name: &Ident, what: &str) {
        if LIBRARY_NAMES.contains(&name_of(name).as_str()) {
            self.refuse(
                name.span(),
                &format!("{what} {name}"),
                &format!(
                    "the checker reads `{name}` as the library's, so a {what} of the file can't \
                     have this name"
                ),
            );
        }
    }

    fn name_of(&mut self, state: &ItemStruct) {
        if !state.generics.params.is_empty() {
            self.refuse(
                state.generics.span(),
                "<>",
                "a model and the types it uses can't be generic, because every value must be known: write each concrete type out, one struct for each",
            );
        }
    }

    fn declare_enum(&mut self, choices: &ItemEnum) {
        let name = name_of(&choices.ident);
        if !derives(&choices.attrs, "Domain") {
            self.refuse(
                choices.enum_token.span,
                &format!("enum {name}"),
                "derive `Domain` on it, so the checker knows its values",
            );
            return;
        }
        let mut variants = Vec::with_capacity(choices.variants.len());
        let written: Vec<(String, Span)> = choices
            .variants
            .iter()
            .map(|variant| (name_of(&variant.ident), variant.ident.span()))
            .collect();
        self.refuse_repeats(&written, "variant", "E0428");
        for variant in &choices.variants {
            self.refuse_conditional(&variant.attrs);
            if !matches!(variant.fields, Fields::Unit) {
                self.refuse(
                    variant.span(),
                    &format!("{name}::{}", variant.ident),
                    "a variant with data has no one name to be known by: keep every variant plain, and write the data as fields of a struct that derives `Domain`, or as one plain variant for each case",
                );
            }
            variants.push(name_of(&variant.ident));
        }
        self.enums.insert(name, variants);
    }

    fn declare_record(&mut self, record: &ItemStruct) {
        let name = name_of(&record.ident);
        if !derives(&record.attrs, "Domain") {
            self.refuse(
                record.struct_token.span,
                &format!("struct {name}"),
                "derive `Model` on the model's state, or `Domain` on a record it holds",
            );
            return;
        }
        self.name_of(record);
        match &record.fields {
            Fields::Named(fields) => {
                self.refuse_repeated_fields(fields);
                for field in &fields.named {
                    self.refuse_conditional(&field.attrs);
                }
                self.records.insert(name, fields.clone());
            }
            other => self.refuse(
                other.span(),
                &format!("struct {name}"),
                "a record has named fields: write `struct Name { field: Type }` and derive `Domain`",
            ),
        }
    }

    fn model(
        &mut self,
        state: &ItemStruct,
        found: &Found<'_>,
        constants: &[(String, Term)],
        helpers: &[HelperDraft],
    ) -> Draft {
        let name = name_of(&state.ident);
        let fields = match &state.fields {
            Fields::Named(named) => {
                self.refuse_repeated_fields(named);
                named
                    .named
                    .iter()
                    .filter_map(|field| self.field(field))
                    .collect()
            }
            other => {
                self.refuse(
                    other.span(),
                    &format!("struct {name}"),
                    "a model's state has named fields: write `struct Name { field: Type }` and derive `Model`",
                );
                Vec::new()
            }
        };
        let own: Vec<&ImplItemFn> = found
            .blocks
            .iter()
            .filter(|block| owner_of(block).as_deref() == Some(name.as_str()))
            .flat_map(|block| self.methods_of(block))
            .collect();
        let written: Vec<(String, Span)> = own
            .iter()
            .map(|method| (name_of(&method.sig.ident), method.sig.ident.span()))
            .collect();
        self.refuse_repeats(&written, "method", "E0592");
        self.methods = own
            .iter()
            .filter(|method| role_of(&method.attrs).is_none())
            .map(|method| name_of(&method.sig.ident))
            .collect();
        self.model_name = Some(name.clone());
        self.method_params = own
            .iter()
            .map(|method| {
                (
                    name_of(&method.sig.ident),
                    typed_parameter_types(&method.sig),
                )
            })
            .collect();
        self.method_types = own
            .iter()
            .filter_map(|method| match &method.sig.output {
                ReturnType::Type(_, ty) => Some((name_of(&method.sig.ident), ty.as_ref().clone())),
                ReturnType::Default => None,
            })
            .collect();
        let mut draft = Draft {
            name,
            file: self.file.to_owned(),
            language: Language::Rust,
            place: place_of(state.struct_token.span),
            fields,
            actions: Vec::new(),
            rules: Vec::new(),
            ends: Vec::new(),
            view: ViewDraft::default(),
            constants: constants.to_vec(),
            helpers: helpers.to_vec(),
        };
        for method in own {
            self.method(method, &mut draft);
        }
        self.view_of(state, found, &mut draft);
        self.methods.clear();
        self.method_types.clear();
        self.method_params.clear();
        self.model_name = None;
        draft
    }

    fn view_of(&mut self, state: &ItemStruct, found: &Found<'_>, draft: &mut Draft) {
        let name = name_of(&state.ident);
        let mut places = Vec::new();
        let mut seen = false;
        for block in found
            .views
            .iter()
            .filter(|block| owner_of(block).as_deref() == Some(name.as_str()))
        {
            if seen {
                self.refuse(
                    block.impl_token.span,
                    "#[view]",
                    "a model has one view: put every label in one `#[view]` block",
                );
                continue;
            }
            seen = true;
            places = self.view(block, draft);
        }
        for step in places {
            if !draft.actions.iter().any(|action| action.id == step.action) {
                self.refuse(
                    step.span,
                    &step.action,
                    "a view's `step` labels an action of the model by its id, and this model \
                     has no action with this id: the chart would leave the label out",
                );
            }
        }
    }

    fn field(&mut self, field: &Field) -> Option<FieldDraft> {
        let name = name_of(field.ident.as_ref()?);
        self.refuse_conditional(&field.attrs);
        let mut shape = self.shape(&field.ty, &field.attrs);
        if let Some(initial) = attribute(&field.attrs, "initial") {
            if let Ok(listed) =
                initial.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated)
            {
                for value in &listed {
                    self.walk_constant(&field.ty, value, "the starting value of this field");
                }
            }
            shape.initial = Some(
                match initial.parse_args_with(Punctuated::<Expr, Token![,]>::parse_terminated) {
                    Ok(listed) if listed.len() == 1 => match listed.iter().next() {
                        Some(value) => self.term(value),
                        None => self.nothing(initial.span(), "#[initial]", NEEDS_A_START),
                    },
                    Ok(listed) if !listed.is_empty() => {
                        let values: Vec<Term> =
                            listed.iter().map(|value| self.term(value)).collect();
                        Term::new(place_of(initial.span()), TermKind::Choices(values))
                    }
                    _ => self.nothing(initial.span(), "#[initial]", NEEDS_A_START),
                },
            );
        }
        Some(FieldDraft { name, shape })
    }

    fn methods_of<'b>(&mut self, block: &'b ItemImpl) -> Vec<&'b ImplItemFn> {
        if let Some((_, implemented, _)) = &block.trait_ {
            self.refuse(
                implemented.span(),
                &path_text(implemented),
                "`#[model]` goes on the model's own `impl` block, not on a trait's",
            );
        }
        let mut methods = Vec::with_capacity(block.items.len());
        for item in &block.items {
            if let ImplItem::Fn(method) = item {
                methods.push(method);
            } else {
                let feature = self.short_text(item.span());
                self.refuse(item.span(), &feature, "a model's block holds only methods: move a constant, a type or a function to the top of the file");
            }
        }
        methods
    }

    fn method(&mut self, method: &ImplItemFn, draft: &mut Draft) {
        let signature = &method.sig;
        let id = name_of(&signature.ident);
        let place = place_of(signature.fn_token.span);
        self.refuse_conditional(&method.attrs);
        for attribute in method.attrs.iter().filter(|found| names_a_role(found)) {
            let feature = format!("#[{}]", path_text(attribute.path()));
            self.refuse(
                attribute.pound_token.span,
                &feature,
                "`#[model]` knows an action, a rule and an end only by the plain attribute: \
                 write `#[action(\"...\")]`, `#[rule(\"...\")]` or `#[end(\"...\")]`",
            );
        }
        let Some((role, sentence)) = role_of(&method.attrs) else {
            self.reads_state(signature);
            let helper = self.helper(HelperKind::Method, signature, &method.block);
            draft.helpers.push(helper);
            return;
        };
        let inputs = self.inputs(signature);
        let says = self.sentence(role, sentence);
        let returns = if role == "action" {
            self.returns_of(signature)
        } else {
            Ty::Bool
        };
        self.walk_body(signature, &method.block, returns);
        if role == "action" {
            let returns_self = self.action_returns(signature);
            let taken: Vec<String> = inputs.iter().map(|(name, _)| name.clone()).collect();
            let typed_inputs: Vec<(String, Type)> = signature
                .inputs
                .iter()
                .filter_map(|input| match input {
                    FnArg::Typed(typed) => match typed.pat.as_ref() {
                        Pat::Ident(named) => {
                            Some((name_of(&named.ident), typed.ty.as_ref().clone()))
                        }
                        _ => None,
                    },
                    FnArg::Receiver(_) => None,
                })
                .collect();
            let body = |term: Term| ActionBody {
                state_name: Some(STATE.to_owned()),
                input_name: None,
                taken: taken.clone(),
                term,
            };
            let kept = self.bound.len();
            self.bound.extend(
                typed_inputs
                    .iter()
                    .map(|(name, ty)| plain(name.clone(), Some(ty.clone()))),
            );
            let guard = self.guard(&method.block.stmts).map(body);
            let close = method.block.brace_token.span.close();
            self.returns_self = returns_self;
            let change = body(self.rest(&method.block.stmts, Body::Action, close));
            self.returns_self = None;
            self.bound.truncate(kept);
            draft.actions.push(ActionDraft {
                id,
                label: says,
                place,
                inputs,
                guard,
                change,
            });
            return;
        }
        self.returns(signature, "bool", "a rule or an end");
        if let Some(FnArg::Typed(extra)) = signature.inputs.iter().nth(1) {
            self.refuse(
                extra.span(),
                &id,
                "a rule or an end looks only at the state: it takes `&self` and nothing else, so read a value from a field, or call a helper with it",
            );
        }
        let condition = self.value_of(&method.block);
        let state_name = Some(STATE.to_owned());
        if role == "rule" {
            draft.rules.push(RuleDraft {
                id,
                says,
                place,
                state_name,
                holds: condition,
                text: self.body_text(&method.block),
            });
        } else {
            draft.ends.push(EndDraft {
                id,
                says,
                place,
                state_name,
                when: condition,
            });
        }
    }

    fn body_text(&self, block: &Block) -> String {
        let open = block.brace_token.span.open().byte_range().end;
        let close = block.brace_token.span.close().byte_range().start;
        condition_text(
            self.source.get(open..close).unwrap_or_default(),
            Quoting::Rust,
        )
    }

    fn sentence(&mut self, role: &str, attribute: &Attribute) -> String {
        if let Ok(sentence) = attribute.parse_args::<LitStr>() {
            sentence.value()
        } else {
            self.refuse(
                attribute.span(),
                &format!("#[{role}]"),
                &format!("write the sentence in quotes: `#[{role}(\"...\")]`"),
            );
            String::new()
        }
    }

    fn action_returns(&mut self, signature: &Signature) -> Option<bool> {
        let given = match &signature.output {
            ReturnType::Default => String::new(),
            ReturnType::Type(_, ty) => self.text(ty.span()).replace(' ', ""),
        };
        if given == "Self" {
            return Some(true);
        }
        let before = self.problems.len();
        self.returns(signature, "Option<Self>", "an action");
        (self.problems.len() == before).then_some(false)
    }

    pub(crate) fn returns(&mut self, signature: &Signature, wanted: &str, what: &str) {
        let (span, given) = match &signature.output {
            ReturnType::Default => (signature.ident.span(), String::new()),
            ReturnType::Type(_, ty) => (ty.span(), self.text(ty.span()).replace(' ', "")),
        };
        if given != wanted {
            self.refuse(
                span,
                &format!("fn {}", signature.ident),
                &format!("{what} returns `{wanted}`"),
            );
        }
    }

    pub(crate) fn reads_state(&mut self, signature: &Signature) {
        let reads_state = match signature.inputs.first() {
            Some(FnArg::Receiver(receiver)) => {
                receiver.reference.is_some()
                    && receiver.mutability.is_none()
                    && receiver.colon_token.is_none()
            }
            _ => false,
        };
        if !reads_state {
            let span = signature
                .inputs
                .first()
                .map_or(signature.paren_token.span.open(), Spanned::span);
            let seen = self.short_text(span);
            let feature = if seen.is_empty() {
                format!("fn {}", signature.ident)
            } else {
                seen
            };
            self.refuse(
                span,
                &feature,
                "a method of a model takes `&self` first: it reads the state and never changes it",
            );
        }
    }

    fn inputs(&mut self, signature: &Signature) -> Vec<(String, Shape)> {
        self.plain_function(signature);
        self.reads_state(signature);
        signature
            .inputs
            .iter()
            .filter_map(|input| match input {
                FnArg::Typed(typed) => Some(self.input(typed)),
                FnArg::Receiver(_) => None,
            })
            .collect()
    }

    fn input(&mut self, typed: &PatType) -> (String, Shape) {
        let name = match typed.pat.as_ref() {
            Pat::Ident(named) if named.subpat.is_none() && named.by_ref.is_none() => {
                if let Some(mutable) = named.mutability {
                    self.refuse(
                        mutable.span,
                        "mut",
                        "an input can't change: drop the `mut`, and write `let next = value + 1;` under a new name",
                    );
                }
                self.refuse_constant_name(&named.ident);
                name_of(&named.ident)
            }
            other => {
                let feature = self.short_text(other.span());
                self.refuse(
                    other.span(),
                    &feature,
                    "an input is one plain name: take a pair or a tuple as one struct that derives `Domain`, and read its fields",
                );
                "_".to_owned()
            }
        };
        self.refuse_conditional(&typed.attrs);
        (name, self.shape(&typed.ty, &typed.attrs))
    }

    fn refuse_repeated_fields(&mut self, fields: &FieldsNamed) {
        let written: Vec<(String, Span)> = fields
            .named
            .iter()
            .filter_map(|field| field.ident.as_ref())
            .map(|ident| (name_of(ident), ident.span()))
            .collect();
        self.refuse_repeats(&written, "field", "E0124");
    }

    pub(crate) fn plain_function(&mut self, signature: &Signature) {
        let written: Vec<(String, Span)> = signature
            .inputs
            .iter()
            .filter_map(|input| match input {
                FnArg::Typed(typed) => match typed.pat.as_ref() {
                    Pat::Ident(named) => Some((name_of(&named.ident), named.ident.span())),
                    _ => None,
                },
                FnArg::Receiver(_) => None,
            })
            .collect();
        self.refuse_repeats(&written, "parameter", "E0415");
        if let Some(waits) = signature.asyncness {
            self.refuse(waits.span, "async", WAIT);
        }
        if let Some(unchecked) = signature.unsafety {
            self.refuse(
                unchecked.span,
                "unsafe",
                "unsafe code is outside the library",
            );
        }
        if !signature.generics.params.is_empty() {
            self.refuse(
                signature.generics.span(),
                &format!("fn {}", signature.ident),
                "a function of a model can't be generic, because every value must be known: write the concrete type",
            );
        }
    }

    fn function(&mut self, function: &ItemFn) -> HelperDraft {
        for input in &function.sig.inputs {
            if let FnArg::Receiver(receiver) = input {
                self.refuse(
                    receiver.span(),
                    STATE,
                    "a function outside the model's block has no state to read",
                );
            }
        }
        self.helper(HelperKind::Function, &function.sig, &function.block)
    }

    fn is_the_state(&self, ty: &Type) -> bool {
        match ty {
            Type::Reference(borrowed) => self.is_the_state(&borrowed.elem),
            Type::Paren(inner) => self.is_the_state(&inner.elem),
            Type::Group(inner) => self.is_the_state(&inner.elem),
            Type::Path(named) => named.path.get_ident().is_some_and(|name| {
                name_of(name) == "Self" || self.models.contains(&name_of(name))
            }),
            _ => false,
        }
    }

    fn names_the_state(&self, ty: &Type) -> bool {
        match ty {
            Type::Paren(inner) => self.names_the_state(&inner.elem),
            Type::Group(inner) => self.names_the_state(&inner.elem),
            Type::Path(named) => named.qself.is_none() && self.is_the_state(ty),
            _ => false,
        }
    }

    fn helper(&mut self, kind: HelperKind, signature: &Signature, block: &Block) -> HelperDraft {
        self.plain_function(signature);
        let mut parameters = Vec::with_capacity(signature.inputs.len());
        let mut typed_parameters = Vec::new();
        for input in &signature.inputs {
            let typed = match input {
                FnArg::Typed(typed) => typed,
                FnArg::Receiver(_) => {
                    parameters.push(STATE.to_owned());
                    continue;
                }
            };
            self.refuse_conditional(&typed.attrs);
            match typed.pat.as_ref() {
                Pat::Ident(named) if named.subpat.is_none() && named.by_ref.is_none() => {
                    let name = name_of(&named.ident);
                    self.refuse_constant_name(&named.ident);
                    if !self.is_the_state(&typed.ty) {
                        typed_parameters.push((name.clone(), named.ident.span(), &typed.ty));
                    }
                    parameters.push(name);
                }
                other => {
                    let feature = self.short_text(other.span());
                    self.refuse(
                        other.span(),
                        &feature,
                        "a parameter is one plain name: take the pair or the tuple as one struct that derives `Domain`, and read its fields",
                    );
                }
            }
        }
        let kept = self.bound.len();
        let receiver = self
            .model_name
            .clone()
            .map(|model| syn::parse_str::<Type>(&format!("&{model}")).ok());
        for input in &signature.inputs {
            match input {
                FnArg::Receiver(_) => {
                    self.bound
                        .push(plain(STATE.to_owned(), receiver.clone().flatten()));
                }
                FnArg::Typed(typed) => {
                    if let Pat::Ident(named) = typed.pat.as_ref() {
                        self.bound.push(plain(
                            name_of(&named.ident),
                            Some(typed.ty.as_ref().clone()),
                        ));
                    }
                }
            }
        }
        let returns = self.returns_of(signature);
        self.walk_body(signature, block, returns);
        let mut body = self.value_of(block);
        self.bound.truncate(kept);
        if let ReturnType::Type(_, ty) = &signature.output {
            body = if self.names_the_state(ty) {
                Term::new(body.place, TermKind::Updates(Box::new(body)))
            } else {
                if self.is_the_state(ty) {
                    let feature = self.short_text(ty.span());
                    self.refuse(
                        ty.span(),
                        &feature,
                        "a helper gives the state back by value: write `Self` as the result, \
                         and build it as `Self { changed fields, ..self.clone() }`",
                    );
                }
                self.of_type(ty, body)
            };
        }
        for (name, span, ty) in typed_parameters.into_iter().rev() {
            let given = Term::new(place_of(span), TermKind::Name(name.clone()));
            let value = self.of_type(ty, given);
            body = Term::new(
                body.place,
                TermKind::Bind {
                    name,
                    value: Box::new(value),
                    body: Box::new(body),
                },
            );
        }
        HelperDraft {
            kind,
            name: name_of(&signature.ident),
            parameters,
            body,
        }
    }
}

fn typed_parameter_types(signature: &Signature) -> Vec<Type> {
    signature
        .inputs
        .iter()
        .filter_map(|input| match input {
            FnArg::Typed(typed) => Some(typed.ty.as_ref().clone()),
            FnArg::Receiver(_) => None,
        })
        .collect()
}

fn role_of(attributes: &[Attribute]) -> Option<(&'static str, &Attribute)> {
    ROLES
        .iter()
        .find_map(|role| attribute(attributes, role).map(|found| (*role, found)))
}
