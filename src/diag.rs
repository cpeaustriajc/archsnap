use std::io::IsTerminal;
use std::path::PathBuf;

use oxc_diagnostics::{GraphicalReportHandler, GraphicalTheme, NamedSource, OxcDiagnostic};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Fatal {
    #[error("{} is not inside a git repository", .0.display())]
    NotGitRepo(PathBuf),
    #[error("{} has no commits yet", .0.display())]
    NoCommits(PathBuf),
    #[error("git is not installed or not on PATH")]
    GitMissing,
    #[error("`git {args}` failed: {stderr}")]
    GitFailed { args: String, stderr: String },
    #[error("can't write to {}: {source}", path.display())]
    OutputNotWritable { path: PathBuf, source: std::io::Error },
    #[error("{0} warning(s) and --strict is set")]
    Strict(usize),
}

impl Fatal {
    pub fn exit_code(&self) -> i32 {
        match self {
            Fatal::Strict(_) => 1,
            Fatal::NotGitRepo(_) | Fatal::NoCommits(_) | Fatal::GitMissing => 2,
            Fatal::GitFailed { .. } | Fatal::OutputNotWritable { .. } => 3,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Fatal::NotGitRepo(_) => "not_git_repo",
            Fatal::NoCommits(_) => "no_commits",
            Fatal::GitMissing => "git_missing",
            Fatal::GitFailed { .. } => "git_failed",
            Fatal::OutputNotWritable { .. } => "output_not_writable",
            Fatal::Strict(_) => "strict",
        }
    }

    fn help(&self) -> &'static str {
        match self {
            Fatal::NotGitRepo(_) => "run archsnap inside a git repo, or pass --repo <path>",
            Fatal::NoCommits(_) => "commit at least once, then re-run",
            Fatal::GitMissing => "install git and make sure `git --version` works",
            Fatal::GitFailed { .. } => "re-run with --verbose to see every git call",
            Fatal::OutputNotWritable { .. } => {
                "check the folder exists and is writable, or pass --out <dir>"
            }
            Fatal::Strict(_) => "the report was still written; drop --strict to exit 0 on warnings",
        }
    }

    pub fn print(&self) {
        let d = OxcDiagnostic::error(self.to_string())
            .with_error_code("archsnap", self.code())
            .with_help(self.help());
        eprint!("{}", render(d, None));
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Warning {
    pub code: &'static str,
    pub message: String,
    pub file: Option<String>,
    pub help: String,
    #[serde(skip)]
    pub source: Option<Source>,
}

#[derive(Debug, Clone)]
pub struct Source {
    pub text: String,
    pub labels: Vec<(u32, u32, Option<String>)>,
}

impl Warning {
    pub fn new(code: &'static str, message: impl Into<String>, help: impl Into<String>) -> Self {
        Warning { code, message: message.into(), file: None, help: help.into(), source: None }
    }

    pub fn in_file(mut self, file: &str, source: Source) -> Self {
        self.file = Some(file.to_string());
        self.source = Some(source);
        self
    }

    pub fn print(&self) {
        let mut d = OxcDiagnostic::warn(self.message.clone())
            .with_error_code("archsnap", self.code)
            .with_help(self.help.clone());
        let src = match (&self.file, &self.source) {
            (Some(file), Some(src)) => {
                for (start, end, label) in &src.labels {
                    let span = oxc_span::Span::new(*start, *end);
                    d = d.and_label(oxc_diagnostics::LabeledSpan::new_with_span(label.clone(), span));
                }
                Some((file.as_str(), src.text.clone()))
            }
            _ => None,
        };
        eprint!("{}", render(d, src));
    }
}

fn render(d: OxcDiagnostic, source: Option<(&str, String)>) -> String {
    let color = std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none();
    let theme = if color { GraphicalTheme::unicode() } else { GraphicalTheme::unicode_nocolor() };
    let handler = GraphicalReportHandler::new_themed(theme).with_links(false);
    let mut out = String::new();
    let _ = match source {
        Some((name, text)) => {
            let boxed = d.with_source_code(NamedSource::new(name, text));
            handler.render_report(&mut out, boxed.as_ref())
        }
        None => handler.render_report(&mut out, &d),
    };
    out
}
