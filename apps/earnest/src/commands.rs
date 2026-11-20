use crate::arguments::{self, Chart, Confirm, Models, Report, Tla};
use crate::changes::{Compare, Difference, check_base, rule_changes};
use crate::confirm::{Checker, Tool, confirm_lines, differences};
use crate::discovery::discover;
use crate::print::{problem_lines, skipped_line, to_stderr, to_stdout, unreadable_lines};
use crate::reading::{ModelFile, load};
use crate::run::{Bounds, Graphs, Ran, Search, checked, explore_each};
use crate::select::{Target, select, select_with_removed};
use earnest_chart::{RuleChange, RuleChanges, Section, Unreadable, chart, report, skipped_section};
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
}

pub(crate) fn gather(models: &Models) -> Result<Gathered, String> {
    let discovered = discover(&models.paths)?;
    Ok(Gathered {
        files: discovered.found.into_iter().map(load).collect(),
        skipped: discovered.skipped,
    })
}

fn say_what_was_skipped(skipped: &[String]) {
    if let Some(line) = skipped_line(skipped.len()) {
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
    let Gathered { files, skipped } = gather(&arguments.selection.models)?;
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
    say_what_was_skipped(&skipped);
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
    let Gathered { files, skipped } = gather(&arguments.selection.models)?;
    let rule = arguments.selection.rule.as_deref();
    let selected = select_with_removed(
        &files,
        arguments.selection.models.model.as_deref(),
        rule,
        arguments.base.as_deref(),
    )?;
    let targets = selected.targets;
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
    let mut written = report(&sections, &unreadable, since.as_ref());
    if let Some(section) = skipped_section(&skipped) {
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
    let Gathered { files, skipped } = gather(&arguments.models)?;
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
    say_what_was_skipped(&skipped);
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
    let Gathered { files, skipped } = gather(&arguments.models)?;
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
    say_what_was_skipped(&skipped);
    let unreadable = warn_of_unreadable(&files);
    Ok(if everything_agrees {
        unreadable
    } else {
        Exit::Failed
    })
}
