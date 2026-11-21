use crate::arguments::{self, Chart, Confirm, Models, Report, Tla};
use crate::changes::{Compare, Difference, check_base, rule_changes};
use crate::confirm::{Checker, Tool, confirm_lines, differences};
use crate::discovery::{discover, language_of};
use crate::print::{
    no_model_line, problem_lines, skipped_line, to_stderr, to_stdout, unreadable_lines,
};
use crate::reading::{ModelFile, load, read};
use crate::run::{Bounds, Graphs, Ran, Search, checked, explore_each};
use crate::select::{Target, held_back, select, select_with_removed};
use earnest_chart::{
    RuleChange, RuleChanges, Section, Unreadable, chart_with_limit, one_line, report_with_limit,
    skipped_section,
};
use earnest_check::Status;
use earnest_tla::{BEYOND_32_BITS_COMPUTED, Files, limits, write};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const CONFIRM_FOLDER: &str = ".earnest/confirm";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Exit {
    Passed,
    Failed,
    Misused,
}

impl From<Exit> for ExitCode {
    fn from(exit: Exit) -> Self {
        match exit {
            Exit::Passed => Self::SUCCESS,
            Exit::Failed => Self::FAILURE,
            Exit::Misused => Self::from(2),
        }
    }
}

pub(crate) fn limits_of(bounds: &arguments::Bounds) -> Bounds {
    Bounds::new(bounds.max_states, bounds.time_limit, bounds.jobs)
}

#[derive(Debug)]
pub(crate) struct Gathered {
    pub(crate) files: Vec<ModelFile>,
    pub(crate) skipped: Vec<String>,
    pub(crate) no_model: Vec<String>,
}

pub(crate) fn gather(models: &Models) -> Result<Gathered, String> {
    let discovered = discover(&models.paths)?;
    Ok(Gathered {
        files: discovered.found.into_iter().map(load).collect(),
        skipped: discovered.skipped,
        no_model: discovered.no_model,
    })
}

fn holds_the_asked_for(path: &str, model: Option<&str>, rule: Option<&str>) -> bool {
    let Some(language) = language_of(Path::new(path)) else {
        return false;
    };
    let Ok(source) = fs::read_to_string(path) else {
        return false;
    };
    read(language, path, &source).drafts.iter().any(|draft| {
        model.is_none_or(|name| draft.name == name)
            && rule.is_none_or(|id| draft.rules.iter().any(|each| each.id == id))
    })
}

pub(crate) fn say_what_is_unread(
    files: &[ModelFile],
    model: Option<&str>,
    rule: Option<&str>,
    targets: &[Target<'_>],
) {
    if targets.is_empty()
        && let Some(said) = held_back(files, model, rule)
    {
        to_stderr(&said);
    }
}

fn naming_the_files_that_declare_no_model(error: String, no_model: &[String]) -> String {
    if no_model.is_empty() {
        return error;
    }
    let listed: Vec<String> = no_model.iter().map(|path| format!("`{path}`")).collect();
    let (files, verb) = if let [only] = listed.as_slice() {
        (
            format!("the file {only} uses the library and declares"),
            "was",
        )
    } else {
        (
            format!(
                "the files {} use the library and declare",
                listed.join(", ")
            ),
            "were",
        )
    };
    format!("{error}; {files} no model, and {verb} not read")
}

pub(crate) fn refusing(
    skipped: &[String],
    no_model: &[String],
    model: Option<&str>,
    rule: Option<&str>,
    error: String,
) -> String {
    say_what_was_skipped(skipped, no_model);
    let error = naming_the_test_files_that_were_skipped(skipped, model, rule, error);
    naming_the_files_that_declare_no_model(error, no_model)
}

fn naming_the_test_files_that_were_skipped(
    skipped: &[String],
    model: Option<&str>,
    rule: Option<&str>,
    error: String,
) -> String {
    if model.is_none() && rule.is_none() {
        return error;
    }
    let holding: Vec<String> = skipped
        .iter()
        .filter(|path| holds_the_asked_for(path, model, rule))
        .map(|path| format!("`{path}`"))
        .collect();
    if holding.is_empty() {
        return error;
    }
    let asked = match (model, rule) {
        (Some(name), Some(id)) => format!("the model `{name}` with the rule `{id}`"),
        (Some(name), None) => format!("the model `{name}`"),
        (None, Some(id)) => format!("a model with the rule `{id}`"),
        (None, None) => String::new(),
    };
    let (noun, files) = if let [only] = holding.as_slice() {
        ("a test file that uses", only.clone())
    } else {
        ("test files that use", holding.join(", "))
    };
    format!(
        "{error}; {asked} is in {files}, {noun} the library and was skipped: name the file, or \
         search its folder, to read it"
    )
}

fn say_what_was_skipped(skipped: &[String], no_model: &[String]) {
    for line in skipped_line(skipped.len())
        .into_iter()
        .chain(no_model_line(no_model.len()))
    {
        to_stderr(&line);
    }
}

fn warn_of_unreadable(files: &[ModelFile]) -> Exit {
    let lines: Vec<String> = files
        .iter()
        .filter(|file| !file.problems().is_empty())
        .flat_map(unreadable_lines)
        .collect();
    to_stderr(&lines.join("\n"));
    if lines.is_empty() {
        Exit::Passed
    } else {
        Exit::Failed
    }
}

fn searched<'a>(targets: &[Target<'a>], rule: Option<&'a str>) -> Vec<Search<'a>> {
    targets
        .iter()
        .map(|target| Search {
            model: &target.readable.model,
            only_rule: rule,
        })
        .collect()
}

pub(crate) fn chart_command(arguments: &Chart) -> Result<Exit, String> {
    let Gathered {
        files,
        skipped,
        no_model,
    } = gather(&arguments.selection.models)?;
    let rule = arguments.selection.rule.as_deref();
    let model = arguments.selection.models.model.as_deref();
    let targets = select(&files, model, rule)
        .map_err(|error| refusing(&skipped, &no_model, model, rule, error))?;
    say_what_is_unread(&files, model, rule, &targets);
    let ran = explore_each(
        &searched(&targets, rule),
        limits_of(&arguments.bounds),
        Graphs::Kept,
    );
    let charts: Vec<String> = ran
        .iter()
        .filter_map(|ran| ran.outcome.graph.as_ref())
        .map(|graph| chart_with_limit(graph, arguments.arrows.max_arrows.get()))
        .collect();
    to_stdout(&charts.join("\n\n"));
    say_what_was_skipped(&skipped, &no_model);
    Ok(warn_of_unreadable(&files))
}

fn no_model_section(no_model: &[String]) -> String {
    let (noun, verb) = if no_model.len() == 1 {
        ("file that uses the library and declares", "was")
    } else {
        ("files that use the library and declare", "were")
    };
    let lines: Vec<String> = no_model.iter().map(|path| one_line(path)).collect();
    format!(
        "## Files that declare no model\n\n{} {noun} no model {verb} not read:\n\n```\n{}\n```",
        no_model.len(),
        lines.join("\n")
    )
}

fn every_model_passed(ran: &[Ran]) -> bool {
    ran.iter().all(|ran| ran.outcome.status == Status::Pass)
}

pub(crate) fn report_command(arguments: &Report) -> Result<Exit, String> {
    let bounds = limits_of(&arguments.bounds);
    if let Some(base) = &arguments.base {
        check_base(base)?;
    }
    let Gathered {
        files,
        skipped,
        no_model,
    } = gather(&arguments.selection.models)?;
    let rule = arguments.selection.rule.as_deref();
    let model = arguments.selection.models.model.as_deref();
    let selected = select_with_removed(&files, model, rule, arguments.base.as_deref())
        .map_err(|error| refusing(&skipped, &no_model, model, rule, error))?;
    let targets = selected.targets;
    say_what_is_unread(&files, model, rule, &targets);
    let ran = explore_each(&searched(&targets, rule), bounds, Graphs::Kept);
    let sections: Vec<Section<'_>> = targets
        .iter()
        .zip(&ran)
        .map(|(target, ran)| Section {
            name: &target.readable.model.name,
            file: &target.file.path,
            outcome: &ran.outcome,
        })
        .collect();
    let compared = arguments.base.as_deref().map(|base| {
        let compare = Compare {
            base,
            roots: &arguments.selection.models.paths,
            files: &files,
            model: arguments.selection.models.model.as_deref(),
            rule,
            bounds,
        };
        (base, rule_changes(&compare, &targets))
    });
    let changes: Vec<RuleChange<'_>> = compared
        .iter()
        .flat_map(|(_, compared)| compared.changes.iter().map(Difference::for_the_chart))
        .collect();
    let since = compared.as_ref().map(|(base, _)| RuleChanges {
        base,
        changes: &changes,
    });
    if let Some(error) = selected.unmatched
        && changes.is_empty()
    {
        return Err(error);
    }
    let problems: Vec<(&str, Vec<String>)> = files
        .iter()
        .filter(|file| !file.problems().is_empty())
        .map(|file| (file.path.as_str(), problem_lines(file)))
        .collect();
    let unreadable: Vec<Unreadable<'_>> = problems
        .iter()
        .map(|(file, lines)| Unreadable {
            file,
            problems: lines,
        })
        .collect();
    let mut written = report_with_limit(
        &sections,
        &unreadable,
        since.as_ref(),
        arguments.arrows.max_arrows.get(),
    );
    let left_out = (!no_model.is_empty()).then(|| no_model_section(&no_model));
    for section in skipped_section(&skipped).into_iter().chain(left_out) {
        if !written.is_empty() {
            written.push_str("\n\n");
        }
        written.push_str(&section);
    }
    to_stdout(&written);
    if let Some((base, compared)) = &compared {
        to_stderr(&compared.warnings(base).join("\n"));
    }
    let unreadable = warn_of_unreadable(&files);
    Ok(if every_model_passed(&ran) {
        unreadable
    } else {
        Exit::Failed
    })
}

fn exported(targets: &[Target<'_>]) -> Result<Vec<Files>, String> {
    let all: Vec<Files> = targets
        .iter()
        .map(|target| write(&target.readable.model))
        .collect();
    for (position, files) in all.iter().enumerate() {
        if all
            .iter()
            .take(position)
            .any(|earlier| earlier.name == files.name)
        {
            return Err(format!(
                "two models export as the module `{}`: name one file, or one model with `--model`",
                files.name
            ));
        }
    }
    Ok(all)
}

fn write_file(path: &Path, text: &str) -> Result<(), String> {
    fs::write(path, text)
        .map_err(|error| format!("{} can't be written: {error}", path.to_string_lossy()))
}

pub(crate) fn tla_command(arguments: &Tla) -> Result<Exit, String> {
    let Gathered {
        files,
        skipped,
        no_model,
    } = gather(&arguments.models)?;
    let model = arguments.models.model.as_deref();
    let targets = select(&files, model, None)
        .map_err(|error| refusing(&skipped, &no_model, model, None, error))?;
    say_what_is_unread(&files, model, None, &targets);
    let all = exported(&targets)?;
    let out = &arguments.out;
    if !all.is_empty()
        && let Err(error) = fs::create_dir_all(out)
    {
        to_stderr(&format!(
            "error: the folder {} can't be made: {error}",
            out.to_string_lossy()
        ));
        return Ok(Exit::Failed);
    }
    for target in &targets {
        for why in limits(&target.readable.model) {
            to_stderr(&format!(
                "warning: TLC can't confirm the model `{}`: {why}",
                target.readable.model.name
            ));
        }
    }
    let mut written = Vec::new();
    for each in &all {
        let pair = [
            (out.join(format!("{}.tla", each.name)), &each.module),
            (out.join(format!("{}.cfg", each.name)), &each.config),
        ];
        for (path, text) in pair {
            if let Err(error) = write_file(&path, text) {
                to_stderr(&format!("error: {error}"));
                return Ok(Exit::Failed);
            }
            written.push(format!("wrote  {}", path.to_string_lossy()));
        }
    }
    to_stdout(&written.join("\n"));
    say_what_was_skipped(&skipped, &no_model);
    Ok(warn_of_unreadable(&files))
}

fn program_path(path: &Path, what: &str) -> Result<PathBuf, String> {
    fs::canonicalize(path)
        .map_err(|error| format!("{what} {} can't be read: {error}", path.to_string_lossy()))
}

pub(crate) fn confirm_command(arguments: &Confirm) -> Result<Exit, String> {
    let bounds = limits_of(&arguments.bounds);
    let tools: Vec<(Tool, PathBuf)> = [
        (Tool::Tlc, &arguments.tlc, "the TLC jar"),
        (Tool::TlaRs, &arguments.tla_rs, "the tla-rs program"),
    ]
    .into_iter()
    .filter_map(|(tool, path, what)| {
        path.as_deref()
            .map(|path| program_path(path, what).map(|found| (tool, found)))
    })
    .collect::<Result<_, _>>()?;
    let Gathered {
        files,
        skipped,
        no_model,
    } = gather(&arguments.models)?;
    let model = arguments.models.model.as_deref();
    let targets = select(&files, model, None)
        .map_err(|error| refusing(&skipped, &no_model, model, None, error))?;
    say_what_is_unread(&files, model, None, &targets);
    let all = exported(&targets)?;
    let ran = explore_each(&searched(&targets, None), bounds, Graphs::Dropped);
    let mut everything_agrees = true;
    for ((target, ran), exported) in targets.iter().zip(&ran).zip(&all) {
        let model = &target.readable.model;
        let ours = match checked(model, ran) {
            Ok(ours) => ours,
            Err(error) => {
                to_stderr(&format!("error: {error}"));
                return Ok(Exit::Failed);
            }
        };
        for (tool, program) in &tools {
            let folder = match tool {
                Tool::Tlc => Path::new(CONFIRM_FOLDER).join(&exported.name),
                Tool::TlaRs => Path::new(CONFIRM_FOLDER)
                    .join(tool.name())
                    .join(&exported.name),
            };
            let checker = Checker {
                tool: *tool,
                program,
                folder,
                workers: bounds.jobs,
            };
            let found = match checker.check(exported) {
                Ok((counting, rules)) => differences(&ours, &counting, &rules, *tool),
                Err(error) => vec![error],
            };
            let mut beyond = Vec::new();
            if *tool == Tool::Tlc {
                beyond = limits(model);
                if ran.outcome.computes_beyond_32_bits {
                    beyond.push(BEYOND_32_BITS_COMPUTED.to_owned());
                }
            }
            everything_agrees &= found.is_empty();
            to_stdout(&confirm_lines(&ours, &found, &beyond, *tool).join("\n"));
        }
    }
    say_what_was_skipped(&skipped, &no_model);
    let unreadable = warn_of_unreadable(&files);
    Ok(if everything_agrees {
        unreadable
    } else {
        Exit::Failed
    })
}

#[cfg(test)]
mod tests {
    use super::refusing;
    use std::fs;

    const CLIMB: &str = "import { m } from \"@earnest/model\";\n\
        export const Climb = m.model(\"Climb\", { n: m.int(0, 3).initial(0) });\n\
        export const low = m.rule(Climb, \"low\", (s) => s.n <= 1);\n";
    const NO_MODEL: &str = "no model is named `Climb`";

    fn skipped_files(files: &[(&str, &str)]) -> (tempfile::TempDir, Vec<String>) {
        let folder = tempfile::tempdir().expect("a temporary folder");
        let mut paths = Vec::new();
        for (name, text) in files {
            let path = folder.path().join(name);
            fs::write(&path, text).expect("a file can be written");
            paths.push(path.to_string_lossy().into_owned());
        }
        (folder, paths)
    }

    #[test]
    fn a_model_that_is_only_in_a_skipped_test_file_is_named_with_the_file() {
        let (_folder, skipped) = skipped_files(&[("one.ts", CLIMB)]);
        let said = refusing(&skipped, &[], Some("Climb"), None, NO_MODEL.to_owned());
        assert!(
            said.starts_with(NO_MODEL) && said.contains(&skipped[0]) && said.contains("skipped"),
            "{said}"
        );
        assert!(said.contains("a test file that uses the library"), "{said}");
    }

    #[test]
    fn two_skipped_files_with_the_model_are_both_named() {
        let (_folder, skipped) = skipped_files(&[("one.ts", CLIMB), ("two.ts", CLIMB)]);
        let said = refusing(&skipped, &[], Some("Climb"), None, NO_MODEL.to_owned());
        assert!(
            said.contains(&skipped[0])
                && said.contains(&skipped[1])
                && said.contains("test files that use the library"),
            "{said}"
        );
    }

    #[test]
    fn a_rule_that_only_a_skipped_model_has_is_named_with_the_file() {
        let (_folder, skipped) = skipped_files(&[("one.ts", CLIMB)]);
        let said = refusing(
            &skipped,
            &[],
            None,
            Some("low"),
            "no model has a rule `low`".to_owned(),
        );
        assert!(
            said.contains(&skipped[0]) && said.contains("`low`"),
            "{said}"
        );
        let both = refusing(
            &skipped,
            &[],
            Some("Climb"),
            Some("low"),
            "the model `Climb` has no rule `low`".to_owned(),
        );
        assert!(both.contains(&skipped[0]), "{both}");
    }

    #[test]
    fn files_that_declare_no_model_are_named_in_the_error() {
        let one = refusing(
            &[],
            &["helper.rs".to_owned()],
            Some("Climb"),
            None,
            NO_MODEL.to_owned(),
        );
        assert_eq!(
            one,
            format!(
                "{NO_MODEL}; the file `helper.rs` uses the library and declares no model, and was not read"
            )
        );
        let two = refusing(
            &[],
            &["a.rs".to_owned(), "b.rs".to_owned()],
            None,
            None,
            "an error".to_owned(),
        );
        assert_eq!(
            two,
            "an error; the files `a.rs`, `b.rs` use the library and declare no model, and were not read"
        );
    }

    #[test]
    fn a_name_no_skipped_file_has_leaves_the_error_as_it_was() {
        let (_folder, skipped) =
            skipped_files(&[("one.ts", CLIMB), ("two.ts", "export const a = 1;\n")]);
        for (model, rule) in [(Some("Other"), None), (None, Some("high")), (None, None)] {
            assert_eq!(
                refusing(&skipped, &[], model, rule, "an error".to_owned()),
                "an error",
                "{model:?} {rule:?}"
            );
        }
        assert_eq!(
            refusing(&[], &[], Some("Climb"), None, NO_MODEL.to_owned()),
            NO_MODEL,
            "no skipped file"
        );
        assert_eq!(
            refusing(
                &["/nowhere/at/all.ts".to_owned()],
                &[],
                Some("Climb"),
                None,
                NO_MODEL.to_owned()
            ),
            NO_MODEL,
            "a skipped file that is gone"
        );
    }
}
