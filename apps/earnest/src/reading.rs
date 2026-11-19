use crate::discovery::{Found, Language};
use earnest_ast::{Draft, Reading};
use earnest_diagnostics::Problem;
use earnest_ir::Model;
use earnest_semantic::lower;

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

#[derive(Debug)]
pub(crate) struct ModelFile {
    pub(crate) path: String,
    pub(crate) language: Language,
    pub(crate) source: String,
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
    let content = understand(read(found.language, &found.path, &found.source));
    ModelFile {
        path: found.path,
        language: found.language,
        source: found.source,
        content,
    }
}

#[cfg(test)]
mod tests {
    use super::{Content, ModelFile, load};
    use crate::discovery::{Found, Language};

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
        load(Found {
            path: path.to_owned(),
            language,
            source: source.to_owned(),
        })
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
}
