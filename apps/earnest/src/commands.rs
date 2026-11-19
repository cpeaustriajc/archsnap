use crate::arguments::{self, Chart, Confirm, Models, Report, Tla};
use crate::changes::{Difference, check_base, rule_changes};
use crate::confirm::{Tlc, confirm_lines, differences};
use crate::discovery::discover;
use crate::print::{to_stderr, to_stdout, unreadable_lines};
use crate::reading::{ModelFile, load};
use crate::run::{Bounds, Graphs, Ran, Search, checked, explore_each};
use crate::select::{Target, select};
use earnest_chart::{RuleChange, RuleChanges, Section, chart, report};
use earnest_check::Status;
use earnest_tla::{Files, write};
use std::fs;
use std::path::Path;
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

pub(crate) fn gather(models: &Models) -> Result<Vec<ModelFile>, String> {
    Ok(discover(&models.paths)?.into_iter().map(load).collect())
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
    let files = gather(&arguments.selection.models)?;
    let rule = arguments.selection.rule.as_deref();
    let targets = select(&files, arguments.selection.models.model.as_deref(), rule)?;
    let ran = explore_each(
        &searched(&targets, rule),
        limits_of(&arguments.bounds),
        Graphs::Kept,
    );
    let charts: Vec<String> = ran
        .iter()
        .filter_map(|ran| ran.outcome.graph.as_ref())
        .map(chart)
        .collect();
    to_stdout(&charts.join("\n\n"));
    Ok(warn_of_unreadable(&files))
}

fn every_model_passed(ran: &[Ran]) -> bool {
    ran.iter().all(|ran| ran.outcome.status == Status::Pass)
}

pub(crate) fn report_command(arguments: &Report) -> Result<Exit, String> {
    let bounds = limits_of(&arguments.bounds);
    if let Some(base) = &arguments.base {
        check_base(base)?;
    }
    let files = gather(&arguments.selection.models)?;
    let rule = arguments.selection.rule.as_deref();
    let targets = select(&files, arguments.selection.models.model.as_deref(), rule)?;
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
    let compared = arguments
        .base
        .as_deref()
        .map(|base| (base, rule_changes(base, &targets, bounds)));
    let changes: Vec<RuleChange<'_>> = compared
        .iter()
        .flat_map(|(_, compared)| compared.changes.iter().map(Difference::for_the_chart))
        .collect();
    let since = compared.as_ref().map(|(base, _)| RuleChanges {
        base,
        changes: &changes,
    });
    to_stdout(&report(&sections, since.as_ref()));
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
    let files = gather(&arguments.models)?;
    let targets = select(&files, arguments.models.model.as_deref(), None)?;
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
    Ok(warn_of_unreadable(&files))
}

pub(crate) fn confirm_command(arguments: &Confirm) -> Result<Exit, String> {
    let bounds = limits_of(&arguments.bounds);
    let jar = fs::canonicalize(&arguments.tlc).map_err(|error| {
        format!(
            "the TLC jar {} can't be read: {error}",
            arguments.tlc.to_string_lossy()
        )
    })?;
    let files = gather(&arguments.models)?;
    let targets = select(&files, arguments.models.model.as_deref(), None)?;
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
        let tlc = Tlc {
            jar: &jar,
            folder: Path::new(CONFIRM_FOLDER).join(&exported.name),
            workers: bounds.jobs,
        };
        let rule_ids: Vec<&str> = model.rules.iter().map(|rule| rule.id.as_str()).collect();
        let found = match tlc.check(exported, &rule_ids) {
            Ok((counting, rules)) => differences(&ours, &counting, &rules),
            Err(error) => vec![error],
        };
        everything_agrees &= found.is_empty();
        to_stdout(&confirm_lines(&ours, &found).join("\n"));
    }
    let unreadable = warn_of_unreadable(&files);
    Ok(if everything_agrees {
        unreadable
    } else {
        Exit::Failed
    })
}
