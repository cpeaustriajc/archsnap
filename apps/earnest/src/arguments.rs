use clap::{Args, Parser, Subcommand};
use std::num::NonZeroUsize;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "earnest",
    version,
    about = "Checks models written in TypeScript or Rust by visiting every reachable state"
)]
pub(crate) struct Arguments {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    #[command(about = "Check every model and print one result line for each")]
    Verify(Verify),
    #[command(about = "Print each model as a Mermaid statechart")]
    Chart(Chart),
    #[command(about = "Print Markdown for a PR body: each model's result, chart and paths")]
    Report(Report),
    #[command(about = "Write each model as a TLA+ module and its TLC configuration")]
    Tla(Tla),
    #[command(about = "Run TLC or tla-rs on each exported model and compare it with the checker")]
    Confirm(Confirm),
}

#[derive(Debug, Args)]
pub(crate) struct Models {
    #[arg(
        value_name = "PATH",
        help = "Files or folders to search for models [default: the current directory]"
    )]
    pub(crate) paths: Vec<PathBuf>,
    #[arg(
        long,
        value_name = "NAME",
        help = "Check only the model with this name"
    )]
    pub(crate) model: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct Selection {
    #[command(flatten)]
    pub(crate) models: Models,
    #[arg(
        long,
        value_name = "ID",
        help = "Check only this rule (needs --model when two models share the id)"
    )]
    pub(crate) rule: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct Bounds {
    #[arg(
        long,
        value_name = "N",
        default_value_t = 5_000_000,
        help = "Stop a model after this many distinct states"
    )]
    pub(crate) max_states: usize,
    #[arg(long, value_name = "SECONDS", help = "Stop a model after this long")]
    pub(crate) time_limit: Option<u64>,
    #[arg(long, value_name = "N", help = "Worker threads [default: all cores]")]
    pub(crate) jobs: Option<NonZeroUsize>,
}

#[derive(Debug, Args)]
pub(crate) struct Verify {
    #[command(flatten)]
    pub(crate) selection: Selection,
    #[command(flatten)]
    pub(crate) bounds: Bounds,
    #[arg(long, help = "Print the results as JSON instead of text")]
    pub(crate) json: bool,
    #[arg(long, help = "Check every model again, even when unchanged")]
    pub(crate) no_cache: bool,
    #[arg(
        long,
        value_name = "GIT REF",
        help = "Compare each model's rules with the same file at this commit"
    )]
    pub(crate) base: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct Arrows {
    #[arg(
        long,
        value_name = "N",
        default_value = "20",
        help = "Draw at most this many arrows in a chart (at least 1)"
    )]
    pub(crate) max_arrows: NonZeroUsize,
}

#[derive(Debug, Args)]
pub(crate) struct Chart {
    #[command(flatten)]
    pub(crate) selection: Selection,
    #[command(flatten)]
    pub(crate) bounds: Bounds,
    #[command(flatten)]
    pub(crate) arrows: Arrows,
}

#[derive(Debug, Args)]
pub(crate) struct Report {
    #[command(flatten)]
    pub(crate) selection: Selection,
    #[command(flatten)]
    pub(crate) bounds: Bounds,
    #[command(flatten)]
    pub(crate) arrows: Arrows,
    #[arg(
        long,
        value_name = "GIT REF",
        help = "Compare each model's rules with the same file at this commit"
    )]
    pub(crate) base: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct Tla {
    #[command(flatten)]
    pub(crate) models: Models,
    #[arg(
        long,
        value_name = "FOLDER",
        help = "Where to write <Model>.tla and <Model>.cfg"
    )]
    pub(crate) out: PathBuf,
}

#[derive(Debug, Args)]
pub(crate) struct Confirm {
    #[command(flatten)]
    pub(crate) models: Models,
    #[command(flatten)]
    pub(crate) bounds: Bounds,
    #[arg(
        long,
        value_name = "JAR",
        required_unless_present = "tla_rs",
        help = "The path of tla2tools.jar: check each model with TLC, which needs Java"
    )]
    pub(crate) tlc: Option<PathBuf>,
    #[arg(
        long = "tla-rs",
        value_name = "PATH",
        required_unless_present = "tlc",
        help = "The path of the tla-rs program: check each model with it, which needs no Java"
    )]
    pub(crate) tla_rs: Option<PathBuf>,
}
