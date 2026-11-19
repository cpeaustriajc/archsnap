use crate::reading::{ModelFile, Readable};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Target<'a> {
    pub(crate) file: &'a ModelFile,
    pub(crate) readable: &'a Readable,
}

fn has_rule(target: &Target<'_>, id: &str) -> bool {
    target.readable.model.rules.iter().any(|rule| rule.id == id)
}

fn listed(targets: &[Target<'_>]) -> String {
    targets
        .iter()
        .map(|target| format!("`{}`", target.readable.model.name))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn select<'a>(
    files: &'a [ModelFile],
    model: Option<&str>,
    rule: Option<&str>,
) -> Result<Vec<Target<'a>>, String> {
    let named: Vec<Target<'a>> = files
        .iter()
        .flat_map(|file| {
            file.models()
                .iter()
                .map(move |readable| Target { file, readable })
        })
        .filter(|target| model.is_none_or(|name| target.readable.model.name == name))
        .collect();
    if let Some(name) = model
        && named.is_empty()
    {
        return Err(format!("no model is named `{name}`"));
    }
    let Some(id) = rule else {
        return Ok(named);
    };
    let with_rule: Vec<Target<'a>> = named
        .into_iter()
        .filter(|target| has_rule(target, id))
        .collect();
    match (with_rule.len(), model) {
        (0, Some(name)) => Err(format!("the model `{name}` has no rule `{id}`")),
        (0, None) => Err(format!("no model has a rule `{id}`")),
        (1, _) | (_, Some(_)) => Ok(with_rule),
        (_, None) => Err(format!(
            "the rule `{id}` is in the models {}: name one with `--model`",
            listed(&with_rule)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::select;
    use crate::discovery::{Found, Language};
    use crate::reading::{ModelFile, load};
    use std::fmt::Write;

    fn file(path: &str, models: &[(&str, &[&str])]) -> ModelFile {
        let mut source = String::from("import { m } from \"@earnest/model\";\n");
        for (name, rules) in models {
            writeln!(
                source,
                "export const {name} = m.model(\"{name}\", {{ on: m.boolean().initial(false) }});"
            )
            .unwrap();
            for rule in *rules {
                writeln!(
                    source,
                    "export const {rule} = {name}.rule({{ says: \"{rule} of {name}\", holds: (s) => !s.on }});"
                    )
                    .unwrap();
            }
        }
        let loaded = load(Found::model(path, Language::TypeScript, &source));
        assert!(
            loaded.problems().is_empty(),
            "the test file is readable: {:?}",
            loaded.problems()
        );
        loaded
    }

    fn files() -> Vec<ModelFile> {
        vec![
            file(
                "a.model.ts",
                &[("Porch", &["dark"]), ("Hall", &["dim", "shared"])],
            ),
            file("b.model.ts", &[("Attic", &["shared"])]),
        ]
    }

    fn names(files: &[ModelFile], model: Option<&str>, rule: Option<&str>) -> Vec<String> {
        select(files, model, rule)
            .expect("the selection names a model")
            .iter()
            .map(|target| target.readable.model.name.clone())
            .collect()
    }

    #[test]
    fn without_a_name_every_model_is_selected_in_file_then_position_order() {
        assert_eq!(names(&files(), None, None), ["Porch", "Hall", "Attic"]);
    }

    #[test]
    fn a_model_name_selects_that_model() {
        assert_eq!(names(&files(), Some("Hall"), None), ["Hall"]);
    }

    #[test]
    fn a_model_name_no_model_has_is_an_error() {
        let error = select(&files(), Some("Cellar"), None).expect_err("no such model");
        assert!(
            error.contains("`Cellar`"),
            "the error names the model: {error}"
        );
    }

    #[test]
    fn a_rule_one_model_has_selects_that_model() {
        assert_eq!(names(&files(), None, Some("dim")), ["Hall"]);
    }

    #[test]
    fn a_rule_two_models_share_needs_the_model_name() {
        let error = select(&files(), None, Some("shared")).expect_err("two models have it");
        assert!(
            error.contains("`Hall`") && error.contains("`Attic`") && error.contains("--model"),
            "the error names both models and the flag: {error}"
        );
        assert_eq!(names(&files(), Some("Attic"), Some("shared")), ["Attic"]);
    }

    #[test]
    fn a_rule_no_selected_model_has_is_an_error() {
        let nowhere = select(&files(), None, Some("missing")).expect_err("no such rule");
        assert!(
            nowhere.contains("`missing`"),
            "the error names the rule: {nowhere}"
        );
        let elsewhere = select(&files(), Some("Porch"), Some("dim")).expect_err("another model's");
        assert!(
            elsewhere.contains("`Porch`") && elsewhere.contains("`dim`"),
            "the error names the model and the rule: {elsewhere}"
        );
    }

    #[test]
    fn no_files_select_nothing() {
        assert!(names(&[], None, None).is_empty(), "nothing to select");
    }
}
