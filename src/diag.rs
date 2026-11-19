use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

use oxc_diagnostics::{
    GraphicalReportHandler, GraphicalTheme, LabeledSpan, NamedSource, OxcDiagnostic,
};
use serde::Serialize;

pub(crate) const INTERNAL_ERROR_EXIT: u8 = 70;
pub(crate) const BUG_REPORT_HELP: &str =
    "this is a bug in earnest; please report it with the command you ran";

#[derive(Debug, thiserror::Error)]
pub(crate) enum Fatal {
    #[error("{} is not inside a git repository", .0.display())]
    NotGitRepo(PathBuf),
    #[error("{} has no commits yet", .0.display())]
    NoCommits(PathBuf),
    #[error("git is not installed or not on PATH")]
    GitMissing,
    #[error("`git {args}` failed: {stderr}")]
    GitFailed { args: String, stderr: String },
    #[error("can't write to {}: {source}", path.display())]
    OutputNotWritable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("can't write to stderr: {0}")]
    StderrNotWritable(std::io::Error),
    #[error("internal error: {context}: {detail}")]
    Internal {
        context: &'static str,
        detail: String,
    },
    #[error("{0} warning(s) and --strict is set")]
    Strict(usize),
}

impl Fatal {
    pub(crate) fn exit_code(&self) -> u8 {
        match self {
            Fatal::Strict(_) => 1,
            Fatal::NotGitRepo(_) | Fatal::NoCommits(_) | Fatal::GitMissing => 2,
            Fatal::GitFailed { .. }
            | Fatal::OutputNotWritable { .. }
            | Fatal::StderrNotWritable(_) => 3,
            Fatal::Internal { .. } => INTERNAL_ERROR_EXIT,
        }
    }

    pub(crate) fn internal(context: &'static str, error: &impl std::fmt::Display) -> Fatal {
        Fatal::Internal {
            context,
            detail: error.to_string(),
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Fatal::NotGitRepo(_) => "not_git_repo",
            Fatal::NoCommits(_) => "no_commits",
            Fatal::GitMissing => "git_missing",
            Fatal::GitFailed { .. } => "git_failed",
            Fatal::OutputNotWritable { .. } => "output_not_writable",
            Fatal::StderrNotWritable(_) => "stderr_not_writable",
            Fatal::Internal { .. } => "internal_error",
            Fatal::Strict(_) => "strict",
        }
    }

    fn help(&self) -> &'static str {
        match self {
            Fatal::NotGitRepo(_) => "run earnest inside a git repo, or pass --repo <path>",
            Fatal::NoCommits(_) => "commit at least once, then re-run",
            Fatal::GitMissing => "install git and make sure `git --version` works",
            Fatal::GitFailed { .. } => "re-run with --verbose to see every git call",
            Fatal::OutputNotWritable { .. } => {
                "check the folder exists and is writable, or pass --out <dir>"
            }
            Fatal::StderrNotWritable(_) => "keep stderr open until earnest exits",
            Fatal::Internal { .. } => BUG_REPORT_HELP,
            Fatal::Strict(_) => "the report was still written; drop --strict to exit 0 on warnings",
        }
    }

    pub(crate) fn print(&self) -> Result<(), Fatal> {
        let diagnostic = OxcDiagnostic::error(self.to_string())
            .with_error_code("earnest", self.code())
            .with_help(self.help());
        write_stderr(&render(diagnostic, None)?)
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Warning {
    pub(crate) code: &'static str,
    pub(crate) message: String,
    pub(crate) file: Option<String>,
    pub(crate) help: String,
    #[serde(skip)]
    pub(crate) source: Option<Source>,
}

#[derive(Debug, Clone)]
pub(crate) struct Source {
    pub(crate) text: String,
    pub(crate) labels: Vec<(u32, u32, Option<String>)>,
}

impl Source {
    fn labeled_spans(&self) -> impl Iterator<Item = LabeledSpan> {
        self.labels.iter().map(|(start, end, label)| {
            LabeledSpan::new_with_span(label.clone(), oxc_span::Span::new(*start, *end))
        })
    }
}

impl Warning {
    pub(crate) fn new(
        code: &'static str,
        message: impl Into<String>,
        help: impl Into<String>,
    ) -> Self {
        Warning {
            code,
            message: message.into(),
            file: None,
            help: help.into(),
            source: None,
        }
    }

    pub(crate) fn in_file(mut self, file: &str, source: Source) -> Self {
        self.file = Some(file.to_string());
        self.source = Some(source);
        self
    }

    pub(crate) fn print(&self) -> Result<(), Fatal> {
        let diagnostic = OxcDiagnostic::warn(self.message.clone())
            .with_error_code("earnest", self.code)
            .with_help(self.help.clone());
        let rendered = match (&self.file, &self.source) {
            (Some(file), Some(source)) => render(
                diagnostic.and_labels(source.labeled_spans()),
                Some((file.as_str(), source.text.clone())),
            )?,
            (None, _) | (_, None) => render(diagnostic, None)?,
        };
        write_stderr(&rendered)
    }
}

fn write_stderr(text: &str) -> Result<(), Fatal> {
    io::stderr()
        .lock()
        .write_all(text.as_bytes())
        .map_err(Fatal::StderrNotWritable)
}

fn render(diagnostic: OxcDiagnostic, source: Option<(&str, String)>) -> Result<String, Fatal> {
    let color = io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let theme = if color {
        GraphicalTheme::unicode()
    } else {
        GraphicalTheme::unicode_nocolor()
    };
    let handler = GraphicalReportHandler::new_themed(theme).with_links(false);
    let mut out = String::new();
    let rendered = match source {
        Some((name, text)) => {
            let with_source = diagnostic.with_source_code(NamedSource::new(name, text));
            handler.render_report(&mut out, with_source.as_ref())
        }
        None => handler.render_report(&mut out, &diagnostic),
    };
    rendered.map_err(|error| Fatal::internal("rendering a diagnostic", &error))?;
    Ok(out)
}
