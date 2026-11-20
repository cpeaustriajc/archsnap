use crate::reader::{Reader, at, attribute, name_of, path_text, unwrapped};
use earnest_ast::{
    COLORED_PARTS, Draft, LabelDraft, TemplatePiece, Term, TermKind, ViewDraft, is_color,
};
use proc_macro2::Span;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{
    Attribute, Block, Expr, ImplItem, ImplItemFn, ItemImpl, LitStr, Macro, Path, Stmt, Token,
};

const LABELS: [&str; 2] = ["state", "group"];
const STATE: &str = "self";
const SHAPE: &str = "write the view as `#[view(title = \"...\", step(action = \"...\"), \
                     colors(broken = \"#c62828\"))]`, with any of the three parts left out";
const ONLY_LABELS: &str = "a view's block holds only `#[state]` and `#[group]` methods";
const A_LABEL: &str = "a label is `format!(\"...\", values)`, or one value followed by \
                       `.to_string()` or `.to_owned()`";

struct Written {
    title: Option<(Span, LitStr)>,
    steps: Vec<(Path, LitStr)>,
    colors: Vec<(Path, LitStr)>,
}

pub(crate) struct StepPlace {
    pub(crate) action: String,
    pub(crate) span: Span,
}

fn nested(
    meta: &syn::meta::ParseNestedMeta<'_>,
    found: &mut Vec<(Path, LitStr)>,
) -> syn::Result<()> {
    meta.parse_nested_meta(|entry| {
        let value: LitStr = entry.value()?.parse()?;
        found.push((entry.path, value));
        Ok(())
    })
}

fn written(attribute: &Attribute) -> syn::Result<Written> {
    let mut found = Written {
        title: None,
        steps: Vec::new(),
        colors: Vec::new(),
    };
    if matches!(attribute.meta, syn::Meta::Path(_)) {
        return Ok(found);
    }
    attribute.parse_nested_meta(|meta| {
        if meta.path.is_ident("title") {
            let value: LitStr = meta.value()?.parse()?;
            if found.title.is_some() {
                return Err(meta.error("`title` is given twice"));
            }
            found.title = Some((meta.path.span(), value));
            Ok(())
        } else if meta.path.is_ident("step") {
            nested(&meta, &mut found.steps)
        } else if meta.path.is_ident("colors") {
            nested(&meta, &mut found.colors)
        } else {
            Err(meta.error(SHAPE))
        }
    })?;
    Ok(found)
}

impl Reader<'_> {
    pub(crate) fn view(&mut self, block: &ItemImpl, draft: &mut Draft) -> Vec<StepPlace> {
        let Some(marked) = attribute(&block.attrs, "view") else {
            return Vec::new();
        };
        if let Some((_, implemented, _)) = &block.trait_ {
            self.refuse(
                implemented.span(),
                &path_text(implemented),
                "`#[view]` goes on an `impl` block of the model's own type, not on a trait's",
            );
        }
        self.refuse_conditional(&block.attrs);
        let mut places = Vec::new();
        let Ok(parts) = written(marked) else {
            let feature = self.short_text(marked.span());
            self.refuse(marked.span(), &feature, SHAPE);
            return places;
        };
        let mut view = ViewDraft::default();
        if let Some((_, title)) = parts.title {
            view.title = Some(title.value());
        }
        self.view_steps(&parts.steps, &mut view, &mut places);
        self.view_colors(&parts.colors, &mut view);
        for item in &block.items {
            let ImplItem::Fn(method) = item else {
                let feature = self.short_text(item.span());
                self.refuse(item.span(), &feature, ONLY_LABELS);
                continue;
            };
            self.view_label(method, &mut view);
        }
        draft.view = view;
        places
    }

    fn view_steps(
        &mut self,
        steps: &[(Path, LitStr)],
        view: &mut ViewDraft,
        places: &mut Vec<StepPlace>,
    ) {
        for (path, label) in steps {
            let action = path_text(path);
            if view.steps.iter().any(|(seen, _)| *seen == action) {
                self.refuse(
                    path.span(),
                    &action,
                    "this action is labelled twice in the view: give each action one `step(action = \"label\")`",
                );
                continue;
            }
            places.push(StepPlace {
                action: action.clone(),
                span: path.span(),
            });
            view.steps.push((action, label.value()));
        }
    }

    fn view_colors(&mut self, colors: &[(Path, LitStr)], view: &mut ViewDraft) {
        for (path, color) in colors {
            let part = path_text(path);
            let value = color.value();
            if !COLORED_PARTS.contains(&part.as_str()) {
                self.refuse(
                    path.span(),
                    &part,
                    "`colors` colors `broken`, `start` and `end`, and no other part",
                );
            } else if view.colors.iter().any(|(seen, _)| *seen == part) {
                self.refuse(path.span(), &part, "this part is colored twice in the view: give each of `broken`, `start` and `end` one color");
            } else if !is_color(&value) {
                self.refuse(
                    color.span(),
                    &format!("\"{value}\""),
                    "a color is `#` and 3, 4, 6 or 8 hexadecimal digits, or a color name of \
                     letters such as `red`; the chart would leave anything else out",
                );
            }
            view.colors.push((part, value));
        }
    }

    fn view_label(&mut self, method: &ImplItemFn, view: &mut ViewDraft) {
        self.refuse_conditional(&method.attrs);
        let signature = &method.sig;
        let marks: Vec<&'static str> = LABELS
            .iter()
            .copied()
            .filter(|label| attribute(&method.attrs, label).is_some())
            .collect();
        for found in method.attrs.iter().filter(|found| {
            found.path().segments.len() > 1
                && found
                    .path()
                    .segments
                    .last()
                    .is_some_and(|last| LABELS.contains(&name_of(&last.ident).as_str()))
        }) {
            self.refuse(
                found.pound_token.span,
                &format!("#[{}]", path_text(found.path())),
                "`#[view]` knows a label only by the plain attribute: write `#[state]` or \
                 `#[group]`",
            );
        }
        let [label] = marks.as_slice() else {
            self.refuse(
                signature.ident.span(),
                &format!("fn {}", signature.ident),
                ONLY_LABELS,
            );
            return;
        };
        self.plain_function(signature);
        self.reads_state(signature);
        self.returns(signature, "String", "a label");
        if let Some(extra) = signature.inputs.iter().nth(1) {
            self.refuse(
                extra.span(),
                &signature.ident.to_string(),
                "a label looks only at the state: it takes `&self` and nothing else, so read a value from a field, or call a helper with it",
            );
        }
        let Some(term) = self.label_body(&method.block) else {
            return;
        };
        let read = LabelDraft {
            state_name: Some(STATE.to_owned()),
            term,
        };
        let slot = if *label == "state" {
            &mut view.state
        } else {
            &mut view.group
        };
        if slot.is_some() {
            self.refuse(
                signature.ident.span(),
                &format!("#[{label}]"),
                "a view has one `#[state]` label and one `#[group]` label at most: merge the two methods into one `format!`",
            );
        } else {
            *slot = Some(read);
        }
    }

    fn label_body(&mut self, block: &Block) -> Option<Term> {
        let [Stmt::Expr(expr, None)] = block.stmts.as_slice() else {
            self.refuse(
                block.brace_token.span.open(),
                "{",
                &format!("a label body is one expression: {A_LABEL}"),
            );
            return None;
        };
        match unwrapped(expr) {
            Expr::Macro(called) if is_format(&called.mac) => self.format_label(&called.mac),
            Expr::MethodCall(call)
                if call.args.is_empty() && is_conversion(&name_of(&call.method)) =>
            {
                let value = self.term(&call.receiver);
                let piece = if name_of(&call.method) == "to_string" {
                    TemplatePiece::Value(value)
                } else {
                    TemplatePiece::Owned(value)
                };
                Some(at(call.method.span(), TermKind::Template(vec![piece])))
            }
            other => {
                let feature = self.short_text(other.span());
                self.refuse(other.span(), &feature, A_LABEL);
                None
            }
        }
    }

    fn format_label(&mut self, called: &Macro) -> Option<Term> {
        let span = called.path.span();
        let Ok((format, values)) = called.parse_body_with(|input: syn::parse::ParseStream<'_>| {
            let format: LitStr = input.parse()?;
            let values = if input.parse::<Option<Token![,]>>()?.is_some() {
                Punctuated::<Expr, Token![,]>::parse_terminated(input)?
            } else {
                Punctuated::new()
            };
            Ok((format, values))
        }) else {
            self.refuse(
                span,
                "format!",
                "write `format!(\"text {}\", value)`: a string literal, then each value",
            );
            return None;
        };
        let values: Vec<&Expr> = values.iter().collect();
        let pieces = self.template_pieces(&format, &values)?;
        Some(at(span, TermKind::Template(pieces)))
    }

    fn template_pieces(&mut self, format: &LitStr, values: &[&Expr]) -> Option<Vec<TemplatePiece>> {
        let mut pieces = Vec::new();
        let mut text = String::new();
        let mut holes = 0;
        let written = format.value();
        let mut characters = written.chars().peekable();
        while let Some(character) = characters.next() {
            match character {
                '{' if characters.peek() == Some(&'{') => {
                    characters.next();
                    text.push('{');
                }
                '}' if characters.peek() == Some(&'}') => {
                    characters.next();
                    text.push('}');
                }
                '{' => {
                    let mut spec = String::new();
                    let mut closed = false;
                    for inside in characters.by_ref() {
                        if inside == '}' {
                            closed = true;
                            break;
                        }
                        spec.push(inside);
                    }
                    if !closed {
                        self.refuse(
                            format.span(),
                            &format!("{{{spec}"),
                            "this `{` is never closed, and Rust refuses the text: write a \
                             literal `{` as `{{`",
                        );
                        return None;
                    }
                    if !spec.is_empty() && spec != ":?" {
                        self.refuse(
                            format.span(),
                            &format!("{{{spec}}}"),
                            "a hole is `{}` or `{:?}`, filled from the values after the text: \
                             the chart prints each value as it is",
                        );
                        return None;
                    }
                    let Some(value) = values.get(holes) else {
                        self.refuse(
                            format.span(),
                            "{}",
                            "the text has more holes than values after it",
                        );
                        return None;
                    };
                    holes += 1;
                    if !text.is_empty() {
                        pieces.push(TemplatePiece::Text(std::mem::take(&mut text)));
                    }
                    let term = self.term(value);
                    pieces.push(if spec.is_empty() {
                        TemplatePiece::Value(term)
                    } else {
                        TemplatePiece::Debug(term)
                    });
                }
                '}' => {
                    self.refuse(format.span(), "}", "write a literal `}` as `}}`");
                    return None;
                }
                other => text.push(other),
            }
        }
        if holes < values.len() {
            self.refuse(
                format.span(),
                "format!",
                "the text has fewer holes than values after it",
            );
            return None;
        }
        if !text.is_empty() {
            pieces.push(TemplatePiece::Text(text));
        }
        Some(pieces)
    }
}

fn is_format(called: &Macro) -> bool {
    called.path.is_ident("format")
}

fn is_conversion(method: &str) -> bool {
    matches!(method, "to_string" | "to_owned")
}
