use crate::discovery::{Found, Language, Text};
use earnest_ast::{Draft, Reading};
use earnest_diagnostics::Problem;
use earnest_ir::Model;
use earnest_semantic::lower;
use syn::{Attribute, ImplItem, Item, Type};

#[derive(Debug)]
pub(crate) struct Readable {
    pub(crate) draft: Draft,
    pub(crate) model: Model,
}

#[derive(Debug)]
pub(crate) enum Content {
    Models(Vec<Readable>),
    Unreadable(Vec<Problem>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Declared {
    pub(crate) name: String,
    pub(crate) rules: Vec<String>,
}

#[derive(Debug)]
pub(crate) struct ModelFile {
    pub(crate) path: String,
    pub(crate) source: String,
    pub(crate) declared: Vec<Declared>,
    pub(crate) content: Content,
}

impl ModelFile {
    pub(crate) fn models(&self) -> &[Readable] {
        match &self.content {
            Content::Models(models) => models,
            Content::Unreadable(_) => &[],
        }
    }

    pub(crate) fn problems(&self) -> &[Problem] {
        match &self.content {
            Content::Models(_) => &[],
            Content::Unreadable(problems) => problems,
        }
    }

    pub(crate) fn declares_unread(&self, model: Option<&str>, rule: Option<&str>) -> bool {
        !self.problems().is_empty()
            && self.declared.iter().any(|declared| {
                model.is_none_or(|name| declared.name == name)
                    && rule.is_none_or(|id| declared.rules.iter().any(|each| each == id))
            })
    }
}

fn derives_model(attributes: &[Attribute]) -> bool {
    let mut found = false;
    for attribute in attributes
        .iter()
        .filter(|each| each.path().is_ident("derive"))
    {
        let parsed = attribute.parse_nested_meta(|meta| {
            found |= meta
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == "Model");
            Ok(())
        });
        if parsed.is_err() {
            return found;
        }
    }
    found
}

fn declared_in_rust(items: &[Item], declared: &mut Vec<Declared>) {
    for item in items {
        match item {
            Item::Struct(each) if derives_model(&each.attrs) => declared.push(Declared {
                name: each.ident.to_string(),
                rules: Vec::new(),
            }),
            Item::Mod(each) => {
                if let Some((_, inner)) = &each.content {
                    declared_in_rust(inner, declared);
                }
            }
            _ => {}
        }
    }
    for item in items {
        let Item::Impl(block) = item else {
            continue;
        };
        let Type::Path(owner) = &*block.self_ty else {
            continue;
        };
        let Some(name) = owner.path.get_ident().map(ToString::to_string) else {
            continue;
        };
        for rule in block.items.iter().filter_map(|member| match member {
            ImplItem::Fn(function)
                if function
                    .attrs
                    .iter()
                    .any(|attribute| attribute.path().is_ident("rule")) =>
            {
                Some(function.sig.ident.to_string())
            }
            _ => None,
        }) {
            if let Some(found) = declared.iter_mut().find(|each| each.name == name) {
                found.rules.push(rule);
            }
        }
    }
}

fn declared_by(language: Language, source: &str, reading: &Reading) -> Vec<Declared> {
    if reading.drafts.is_empty() && language == Language::Rust && !reading.problems.is_empty() {
        let mut declared = Vec::new();
        if let Ok(parsed) = syn::parse_file(source) {
            declared_in_rust(&parsed.items, &mut declared);
        }
        return declared;
    }
    reading
        .drafts
        .iter()
        .map(|draft| Declared {
            name: draft.name.clone(),
            rules: draft.rules.iter().map(|rule| rule.id.clone()).collect(),
        })
        .collect()
}

pub(crate) fn read(language: Language, path: &str, source: &str) -> Reading {
    match language {
        Language::TypeScript => earnest_ts::read(path, source),
        Language::Rust => earnest_rs::read(path, source),
    }
}

fn understand(reading: Reading) -> Content {
    if !reading.problems.is_empty() {
        return Content::Unreadable(reading.problems);
    }
    let mut models = Vec::with_capacity(reading.drafts.len());
    let mut problems = Vec::new();
    for draft in reading.drafts {
        match lower(&draft) {
            Ok(model) => models.push(Readable { draft, model }),
            Err(found) => problems.extend(found),
        }
    }
    if problems.is_empty() {
        Content::Models(models)
    } else {
        problems.sort_by_key(Problem::place);
        Content::Unreadable(problems)
    }
}

pub(crate) fn load(found: Found) -> ModelFile {
    match found.text {
        Text::Model { language, source } => {
            let reading = read(language, &found.path, &source);
            let declared = declared_by(language, &source, &reading);
            ModelFile {
                path: found.path,
                source,
                declared,
                content: understand(reading),
            }
        }
        Text::Unread(problem) => ModelFile {
            path: found.path,
            source: String::new(),
            declared: Vec::new(),
            content: Content::Unreadable(vec![problem]),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{Content, Declared, ModelFile, load};
    use crate::discovery::{Found, Language, Text};
    use earnest_diagnostics::{Place, Problem};

    const COUNTER: &str = "\
import { m } from \"@earnest/model\";

export const Counter = m.model(\"Counter\", {
\tcount: m.int(0, 3).initial(0),
});

export const bounded = Counter.rule({
\tsays: \"The count is never above three\",
\tholds: (s) => s.count <= LIMIT,
});
";

    fn loaded(path: &str, language: Language, source: &str) -> ModelFile {
        load(Found::model(path, language, source))
    }

    #[test]
    fn a_file_whose_names_all_resolve_gives_its_models() {
        let source = COUNTER.replace("LIMIT", "3");
        let file = loaded("src/counter.model.ts", Language::TypeScript, &source);
        let names: Vec<&str> = file
            .models()
            .iter()
            .map(|readable| readable.model.name.as_str())
            .collect();
        assert_eq!(names, ["Counter"], "the model of the file");
        assert!(file.problems().is_empty(), "a readable file has no problem");
        assert_eq!(
            file.models()[0].model.file,
            "src/counter.model.ts",
            "the model carries the path it was found at"
        );
    }

    #[test]
    fn a_name_the_file_does_not_define_makes_the_whole_file_unreadable() {
        let file = loaded("src/counter.model.ts", Language::TypeScript, COUNTER);
        assert!(file.models().is_empty(), "no model of the file is checked");
        let Content::Unreadable(problems) = &file.content else {
            panic!("the file is unreadable, not {:?}", file.content);
        };
        let features: Vec<&str> = problems
            .iter()
            .map(|problem| problem.feature.as_str())
            .collect();
        assert_eq!(features, ["`LIMIT`"], "the undefined name in backticks");
        assert_eq!(problems[0].line, 9, "the line of the name");
        assert_eq!(problems[0].file, "src/counter.model.ts", "the file");
    }

    #[test]
    fn a_construct_the_reader_refuses_makes_the_file_unreadable() {
        let source = COUNTER.replace("LIMIT", "Date.now()");
        let file = loaded("clock.model.ts", Language::TypeScript, &source);
        assert!(file.models().is_empty(), "no model of the file is checked");
        assert_eq!(
            file.problems()
                .iter()
                .map(|problem| problem.feature.as_str())
                .collect::<Vec<_>>(),
            ["`Date.now`"],
            "the reader's problem"
        );
    }

    #[test]
    fn the_language_picks_the_reader() {
        let source = "\
use earnest::model::{Model, model};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Lamp {
    #[initial(false)]
    lit: bool,
}

#[model]
impl Lamp {
    #[action(\"someone flips the switch\")]
    fn flip(&self) -> Option<Self> {
        Some(Self { lit: !self.lit, ..self.clone() })
    }
}
";
        let file = loaded("lamp.rs", Language::Rust, source);
        assert!(
            file.problems().is_empty(),
            "the Rust reader reads it: {:?}",
            file.problems()
        );
        assert_eq!(file.models().len(), 1, "one Rust model");
    }

    #[test]
    fn an_unreadable_typescript_file_still_declares_the_models_the_reader_got() {
        let file = loaded("src/counter.model.ts", Language::TypeScript, COUNTER);
        assert!(!file.problems().is_empty(), "the file is unreadable");
        assert!(file.declares_unread(Some("Counter"), None), "the model");
        assert!(file.declares_unread(None, Some("bounded")), "its rule");
        assert!(!file.declares_unread(Some("Other"), None), "no other model");
    }

    #[test]
    fn an_unreadable_rust_file_declares_its_derived_models_and_their_rules() {
        let source = "\
use earnest::model::{Model, model};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Lamp {
    #[initial(false)]
    lit: bool,
}

#[model]
impl Lamp {
    #[action(\"someone flips the switch\")]
    fn flip(&self) -> Option<Self> {
        loop {}
    }

    #[rule(\"The lamp is never lit\")]
    fn dark(&self) -> bool {
        !self.lit
    }
}
";
        let file = loaded("lamp.rs", Language::Rust, source);
        assert!(!file.problems().is_empty(), "the file is unreadable");
        assert_eq!(
            file.declared,
            [Declared {
                name: "Lamp".to_owned(),
                rules: vec!["dark".to_owned()]
            }],
            "the struct and its rule"
        );
        assert!(file.declares_unread(Some("Lamp"), Some("dark")));
        assert!(
            !file.declares_unread(Some("Lamp"), Some("flip")),
            "an action"
        );
    }

    #[test]
    fn a_readable_file_declares_nothing_unread() {
        let source = COUNTER.replace("LIMIT", "3");
        let file = loaded("src/counter.model.ts", Language::TypeScript, &source);
        assert!(!file.declares_unread(Some("Counter"), None), "it is read");
    }

    #[test]
    fn a_file_that_could_not_be_read_is_unreadable_with_the_problem_it_was_found_with() {
        let problem = Problem::new(
            "src/stray.model.ts",
            Place {
                line: 7,
                column: 62,
            },
            "`byte 0xE9`",
            "the file is not UTF-8 text",
        );
        let file = load(Found {
            path: "src/stray.model.ts".to_owned(),
            text: Text::Unread(problem.clone()),
        });
        assert!(file.models().is_empty(), "no model is checked");
        assert_eq!(file.problems(), [problem], "the problem is kept as it is");
        assert_eq!(file.path, "src/stray.model.ts", "under the file's path");
    }

    const ITEMS: &str = "\
import { m } from \"@earnest/model\";

const Guest = m.enum([\"ann\", \"bob\"]);
const Item = m.record({ qty: m.int(0, 2), who: Guest.optional() });

export const Pick = m.model(\"Pick\", {
\tchosen: Item.optional().initial(null),
});

export const pick = Pick.action(\"a guest picks an item\", {
\tinput: { g: Guest },
\twhen: (s) => s.chosen === null,
\tthen: (_, { g }) => THEN,
});
";

    fn exported_update(then: &str) -> String {
        let source = ITEMS.replace("THEN", then);
        let file = loaded("pick.model.ts", Language::TypeScript, &source);
        assert!(
            file.problems().is_empty(),
            "the file is readable: {:?}",
            file.problems()
        );
        let module = earnest_tla::write(&file.models()[0].model).module;
        module
            .lines()
            .find_map(|line| line.trim().strip_prefix("THEN /\\ chosen' = "))
            .unwrap_or_else(|| panic!("the action sets `chosen`: {module}"))
            .to_owned()
    }

    #[test]
    fn a_record_that_gives_an_optional_field_a_value_exports_as_the_declared_record() {
        assert_eq!(
            exported_update(
                "({ chosen: g === \"ann\" ? { qty: 0, who: g } : { qty: 1, who: null } })"
            ),
            "<<(IF (g = \"ann\") THEN [qty |-> 0, who |-> <<g>>] ELSE [qty |-> 1, who |-> <<>>])>>",
            "`who` is the tuple of an optional in both branches, as the field declares it"
        );
        assert_eq!(
            exported_update(
                "{ const fresh = { qty: 0, who: g }; return { chosen: { ...fresh, qty: 1 } }; }"
            ),
            "(LET local == [qty |-> 0, who |-> g] IN <<[[local EXCEPT !.qty = 1] EXCEPT !.who = \
             <<[local EXCEPT !.qty = 1].who>>]>>)",
            "the `who` of the bound record is wrapped where the record meets its declaration"
        );
    }

    #[test]
    fn the_smallest_i8_is_exported_as_one_number_of_its_type() {
        let source = "\
use earnest::model::{Model, model};

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Model)]
pub struct Min {
    #[range(-128, -127)]
    #[initial(-128)]
    y: i8,
}

#[model]
impl Min {
    #[rule(\"The number is the smallest i8\")]
    fn least(&self) -> bool {
        self.y >= -128
    }
}
";
        let file = loaded("min.rs", Language::Rust, source);
        assert!(
            file.problems().is_empty(),
            "the file is readable: {:?}",
            file.problems()
        );
        let module = earnest_tla::write(&file.models()[0].model).module;
        assert!(
            module.contains("/\\ y = (-128)"),
            "the start is -128, not the negation of 128: {module}"
        );
        assert!(
            module.contains("(y >= (-128))") && !module.contains("128 \\in"),
            "the rule compares with -128 and no 128 is tested for membership of an `i8`: {module}"
        );
    }
}
