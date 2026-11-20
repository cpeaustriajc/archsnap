use crate::bodies::Body;
use crate::reader::{Reader, attribute, path_text, place_of, why_outside};
use earnest_ast::{
    ActionBody, ActionDraft, Draft, EndDraft, FieldDraft, HelperDraft, HelperKind, Language,
    Reading, RuleDraft, Shape, Term, TermKind, ViewDraft, condition_text, nesting_problem,
};
use proc_macro2::{Ident, Span};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Attribute, Block, Expr, Field, Fields, FnArg, ImplItem, ImplItemFn, Item, ItemEnum, ItemFn,
    ItemImpl, ItemStruct, ItemUse, LitStr, Pat, PatType, Path, ReturnType, Signature, Token, Type,
    UseTree,
};

const ROLES: [&str; 3] = ["action", "rule", "end"];
const LIBRARY_NAMES: [&str; 7] = ["forall", "exists", "Some", "None", "Set", "Seq", "Map"];
const STATE: &str = "self";
const ONLY_MODELS: &str =
    "a model file holds only models, the types they use, constants and helper functions";

#[derive(Default)]
struct Found<'a> {
    models: Vec<&'a ItemStruct>,
    blocks: Vec<&'a ItemImpl>,
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
                            .is_some_and(|segment| segment.ident == wanted)
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
        Item::Impl(found) => marks_model(&found.attrs),
        _ => false,
    }
}

fn names_a_role(attribute: &Attribute) -> bool {
    let path = attribute.path();
    path.segments.len() > 1
        && path
            .segments
            .last()
            .is_some_and(|segment| ROLES.iter().any(|role| segment.ident == role))
}

fn marks_model(attributes: &[Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute
            .path()
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "model")
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
        Type::Path(named) if named.qself.is_none() => {
            named.path.get_ident().map(std::string::ToString::to_string)
        }
        _ => None,
    }
}

const BYTE_ORDER_MARK: char = '\u{feff}';

pub(crate) fn read_file(file: &str, source: &str) -> Reading {
    let source = source.strip_prefix(BYTE_ORDER_MARK).unwrap_or(source);
    if let Some(problem) = nesting_problem(file, source) {
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
            .map(|state| state.ident.to_string())
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
        found
            .models
            .iter()
            .map(|state| self.model(state, &found.blocks, &constants, &helpers))
            .collect()
    }

    fn declare<'b>(&mut self, items: &'b [Item]) -> Found<'b> {
        let mut found = Found::default();
        for item in items {
            let attributes = attributes_of(item);
            if is_test_only(attributes) && !is_part_of_a_model(item) {
                continue;
            }
            self.refuse_conditional(attributes);
            match item {
                Item::Use(import) => self.import(import),
                Item::Enum(choices) => {
                    self.refuse_library_name(&choices.ident, "enum");
                    self.declare_enum(choices);
                }
                Item::Struct(state) if derives(&state.attrs, "Model") => {
                    self.refuse_library_name(&state.ident, "struct");
                    self.plain(state);
                    self.models.insert(state.ident.to_string());
                    found.models.push(state);
                }
                Item::Struct(record) => {
                    self.refuse_library_name(&record.ident, "struct");
                    self.declare_record(record);
                }
                Item::Impl(block) if marks_model(&block.attrs) => found.blocks.push(block),
                Item::Impl(block) => self.refuse(block.impl_token.span, "impl", ONLY_MODELS),
                Item::Const(constant) => {
                    let name = constant.ident.to_string();
                    self.refuse_library_name(&constant.ident, "const");
                    if let Some(number) = self.whole_number(&constant.expr) {
                        self.numbers.insert(name.clone(), number);
                    }
                    found
                        .constants
                        .push((name, constant.ty.as_ref(), constant.expr.as_ref()));
                }
                Item::Fn(function) => {
                    let name = function.sig.ident.to_string();
                    self.refuse_library_name(&function.sig.ident, "fn");
                    self.functions.insert(name);
                    found.functions.push(function);
                }
                other => {
                    let (span, feature) = keyword(other);
                    self.refuse(span, feature, ONLY_MODELS);
                }
            }
        }
        found
    }

    fn import(&mut self, import: &ItemUse) {
        let root = match &import.tree {
            UseTree::Path(path) => Some(&path.ident),
            UseTree::Name(name) => Some(&name.ident),
            UseTree::Rename(renamed) => Some(&renamed.ident),
            UseTree::Glob(_) | UseTree::Group(_) => None,
        };
        if root.is_some_and(|name| name == "earnest") {
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
        if LIBRARY_NAMES.contains(&name.to_string().as_str()) {
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

    fn plain(&mut self, state: &ItemStruct) {
        if !state.generics.params.is_empty() {
            self.refuse(
                state.generics.span(),
                "<>",
                "a model and the types it uses can't be generic: every value must be known",
            );
        }
    }

    fn declare_enum(&mut self, choices: &ItemEnum) {
        let name = choices.ident.to_string();
        if !derives(&choices.attrs, "Domain") {
            self.refuse(
                choices.enum_token.span,
                &format!("enum {name}"),
                "derive `Domain` on it, so the checker knows its values",
            );
            return;
        }
        let mut variants = Vec::with_capacity(choices.variants.len());
        for variant in &choices.variants {
            self.refuse_conditional(&variant.attrs);
            if !matches!(variant.fields, Fields::Unit) {
                self.refuse(
                    variant.span(),
                    &format!("{name}::{}", variant.ident),
                    "a variant with data has no one name to be known by: keep every variant plain",
                );
            }
            variants.push(variant.ident.to_string());
        }
        self.enums.insert(name, variants);
    }

    fn declare_record(&mut self, record: &ItemStruct) {
        let name = record.ident.to_string();
        if !derives(&record.attrs, "Domain") {
            self.refuse(
                record.struct_token.span,
                &format!("struct {name}"),
                "derive `Model` on the model's state, or `Domain` on a record it holds",
            );
            return;
        }
        self.plain(record);
        match &record.fields {
            Fields::Named(fields) => {
                for field in &fields.named {
                    self.refuse_conditional(&field.attrs);
                }
                self.records.insert(name, fields.clone());
            }
            other => self.refuse(
                other.span(),
                &format!("struct {name}"),
                "a record has named fields",
            ),
        }
    }

    fn model(
        &mut self,
        state: &ItemStruct,
        blocks: &[&ItemImpl],
        constants: &[(String, Term)],
        helpers: &[HelperDraft],
    ) -> Draft {
        let name = state.ident.to_string();
        let fields = match &state.fields {
            Fields::Named(named) => named
                .named
                .iter()
                .filter_map(|field| self.field(field))
                .collect(),
            other => {
                self.refuse(
                    other.span(),
                    &format!("struct {name}"),
                    "a model's state has named fields",
                );
                Vec::new()
            }
        };
        let own: Vec<&ImplItemFn> = blocks
            .iter()
            .filter(|block| owner_of(block).as_deref() == Some(name.as_str()))
            .flat_map(|block| self.methods_of(block))
            .collect();
        self.methods = own
            .iter()
            .filter(|method| role_of(&method.attrs).is_none())
            .map(|method| method.sig.ident.to_string())
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
        self.methods.clear();
        draft
    }

    fn field(&mut self, field: &Field) -> Option<FieldDraft> {
        let name = field.ident.as_ref()?.to_string();
        self.refuse_conditional(&field.attrs);
        let mut shape = self.shape(&field.ty, &field.attrs);
        if let Some(initial) = attribute(&field.attrs, "initial") {
            shape.initial = Some(match initial.parse_args::<Expr>() {
                Ok(value) => self.term(&value),
                Err(_) => self.nothing(
                    initial.span(),
                    "#[initial]",
                    "write the starting value in the brackets: `#[initial(value)]`",
                ),
            });
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
                self.refuse(item.span(), &feature, "a model's block holds only methods");
            }
        }
        methods
    }

    fn method(&mut self, method: &ImplItemFn, draft: &mut Draft) {
        let signature = &method.sig;
        let id = signature.ident.to_string();
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
        if role == "action" {
            self.returns(signature, "Option<Self>", "an action");
            let taken: Vec<String> = inputs.iter().map(|(name, _)| name.clone()).collect();
            let body = |term: Term| ActionBody {
                state_name: Some(STATE.to_owned()),
                input_name: None,
                taken: taken.clone(),
                term,
            };
            let kept = self.bound.len();
            self.bound.extend(taken.iter().cloned());
            let guard = self.guard(&method.block.stmts).map(body);
            let close = method.block.brace_token.span.close();
            let change = body(self.rest(&method.block.stmts, Body::Action, close));
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
                "a rule or an end looks only at the state: it takes `&self` and nothing else",
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
        condition_text(self.source.get(open..close).unwrap_or_default())
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

    fn returns(&mut self, signature: &Signature, wanted: &str, what: &str) {
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

    fn reads_state(&mut self, signature: &Signature) {
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
                        "an input can't change: every operation gives back a new value",
                    );
                }
                named.ident.to_string()
            }
            other => {
                let feature = self.short_text(other.span());
                self.refuse(other.span(), &feature, "an input is one plain name");
                "_".to_owned()
            }
        };
        self.refuse_conditional(&typed.attrs);
        (name, self.shape(&typed.ty, &typed.attrs))
    }

    fn plain_function(&mut self, signature: &Signature) {
        const WAIT: &str = "waiting is outside the library: model it with a `Request` or a `Timer`";
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
                "a function of a model can't be generic: every value must be known",
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
            Type::Path(named) => named
                .path
                .get_ident()
                .is_some_and(|name| name == "Self" || self.models.contains(&name.to_string())),
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
                    let name = named.ident.to_string();
                    if !self.is_the_state(&typed.ty) {
                        typed_parameters.push((name.clone(), named.ident.span(), &typed.ty));
                    }
                    parameters.push(name);
                }
                other => {
                    let feature = self.short_text(other.span());
                    self.refuse(other.span(), &feature, "a parameter is one plain name");
                }
            }
        }
        let kept = self.bound.len();
        self.bound.extend(parameters.iter().cloned());
        let mut body = self.value_of(block);
        self.bound.truncate(kept);
        if let ReturnType::Type(_, ty) = &signature.output {
            body = self.of_type(ty, body);
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
            name: signature.ident.to_string(),
            parameters,
            body,
        }
    }
}

fn role_of(attributes: &[Attribute]) -> Option<(&'static str, &Attribute)> {
    ROLES
        .iter()
        .find_map(|role| attribute(attributes, role).map(|found| (*role, found)))
}
