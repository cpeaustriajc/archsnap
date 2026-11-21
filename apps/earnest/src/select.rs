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

fn unreadable_files(files: &[ModelFile]) -> Vec<String> {
    files
        .iter()
        .filter(|file| !file.problems().is_empty())
        .map(|file| format!("`{}`", file.path))
        .collect()
}

fn might_be_in(files: &[ModelFile]) -> String {
    let unreadable = unreadable_files(files);
    match unreadable.as_slice() {
        [] => String::new(),
        [only] => format!("; the unreadable file {only} may declare it"),
        many => format!("; the unreadable files {} may declare it", many.join(", ")),
    }
}

pub(crate) fn held_back(
    files: &[ModelFile],
    model: Option<&str>,
    rule: Option<&str>,
) -> Option<String> {
    let holding: Vec<String> = files
        .iter()
        .filter(|file| file.declares_unread(model, rule))
        .map(|file| format!("`{}`", file.path))
        .collect();
    if model.is_none() && rule.is_none() || holding.is_empty() {
        return None;
    }
    let asked = match (model, rule) {
        (Some(name), Some(id)) => format!("the model `{name}` with the rule `{id}`"),
        (Some(name), None) => format!("the model `{name}`"),
        (None, Some(id)) => format!("a model with the rule `{id}`"),
        (None, None) => String::new(),
    };
    let (files, noun) = if let [only] = holding.as_slice() {
        (only.clone(), "which is unreadable")
    } else {
        (holding.join(", "), "which are unreadable")
    };
    Some(format!(
        "error: {asked} is declared in {files}, {noun}, so it was not checked: the problems of \
         the unreadable files are listed with the results"
    ))
}

pub(crate) struct Selected<'a> {
    pub(crate) targets: Vec<Target<'a>>,
    pub(crate) unmatched: Option<String>,
}

pub(crate) fn select_with_removed<'a>(
    files: &'a [ModelFile],
    model: Option<&str>,
    rule: Option<&str>,
    base: Option<&str>,
) -> Result<Selected<'a>, String> {
    match select(files, model, rule) {
        Ok(targets) => Ok(Selected {
            targets,
            unmatched: None,
        }),
        Err(error) => {
            let has_the_model = |name: &str| {
                files
                    .iter()
                    .flat_map(ModelFile::models)
                    .any(|readable| readable.model.name == name)
            };
            if base.is_some() && model.is_some_and(|name| !has_the_model(name)) {
                Ok(Selected {
                    targets: Vec::new(),
                    unmatched: Some(error),
                })
            } else {
                Err(error)
            }
        }
    }
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
        if files.iter().any(|file| file.declares_unread(model, None)) {
            return Ok(named);
        }
        return Err(format!("no model is named `{name}`{}", might_be_in(files)));
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
        (0, None)
            if files
                .iter()
                .any(|file| file.declares_unread(None, Some(id))) =>
        {
            Ok(with_rule)
        }
        (0, None) => Err(format!("no model has a rule `{id}`{}", might_be_in(files))),
        (1, _) | (_, Some(_)) => Ok(with_rule),
        (_, None) => Err(format!(
            "the rule `{id}` is in the models {}: name one with `--model`",
            listed(&with_rule)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{held_back, select, select_with_removed};
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
                    "export const {rule} = m.rule({name}, \"{rule} of {name}\", (s) => !s.on);"
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
    fn with_a_base_a_model_that_is_gone_is_left_for_the_comparison_to_list() {
        let kept = files();
        let selected = select_with_removed(&kept, Some("Cellar"), None, Some("HEAD"))
            .expect("the base may have the model");
        assert!(selected.targets.is_empty(), "nothing is checked");
        assert_eq!(
            selected.unmatched.as_deref(),
            Some("no model is named `Cellar`"),
            "the error stays for when the base has none either"
        );
        assert!(
            select_with_removed(&kept, Some("Cellar"), None, None).is_err(),
            "without a base the name must match a model"
        );
        let found = select_with_removed(&kept, Some("Hall"), None, Some("HEAD"))
            .expect("the model is there");
        assert_eq!(found.targets.len(), 1, "the model of that name");
        assert_eq!(found.unmatched, None, "nothing unmatched");
        assert!(
            select_with_removed(&kept, Some("Porch"), Some("dim"), Some("HEAD")).is_err(),
            "a rule the model lacks is still an error"
        );
    }

    fn unreadable(path: &str, declares: &str) -> ModelFile {
        let source = format!(
            "import {{ m }} from \"@earnest/model\";\n\
             export const {declares} = m.model(\"{declares}\", {{ on: m.boolean().initial(false) }});\n\
             export const lit = m.rule({declares}, \"lit\", (s) => s.on === Date.now());\n"
        );
        let loaded = load(Found::model(path, Language::TypeScript, &source));
        assert!(!loaded.problems().is_empty(), "the test file is unreadable");
        loaded
    }

    #[test]
    fn a_name_an_unreadable_file_declares_selects_nothing_and_is_no_error() {
        let mut kept = files();
        kept.push(unreadable("c.model.ts", "Cellar"));
        assert!(
            select(&kept, Some("Cellar"), None)
                .expect("the file declares it")
                .is_empty(),
            "nothing is checked"
        );
        assert!(
            select(&kept, None, Some("lit"))
                .expect("the file has the rule")
                .is_empty(),
            "nothing is checked"
        );
        assert_eq!(
            held_back(&kept, Some("Cellar"), None).as_deref(),
            Some(
                "error: the model `Cellar` is declared in `c.model.ts`, which is unreadable, \
                 so it was not checked: the problems of the unreadable files are listed with the results"
            ),
            "the sentence says where it is"
        );
        assert_eq!(
            held_back(&kept, Some("Porch"), None),
            None,
            "a readable model"
        );
    }

    #[test]
    fn a_name_nobody_declares_names_the_unreadable_files_that_may_hold_it() {
        let mut kept = files();
        kept.push(unreadable("c.model.ts", "Cellar"));
        let error = select(&kept, Some("Garage"), None).expect_err("no such model");
        assert_eq!(
            error,
            "no model is named `Garage`; the unreadable file `c.model.ts` may declare it"
        );
        let plain = select(&files(), Some("Garage"), None).expect_err("no such model");
        assert_eq!(plain, "no model is named `Garage`", "no unreadable file");
    }

    #[test]
    fn no_files_select_nothing() {
        assert!(names(&[], None, None).is_empty(), "nothing to select");
    }
}
