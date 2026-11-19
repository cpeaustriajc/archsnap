mod diag;
mod diff;
mod git;
mod report;
mod scan;
mod store;
mod system;

use std::io::{self, Write};
use std::panic::PanicHookInfo;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use diag::{BUG_REPORT_HELP, Fatal, INTERNAL_ERROR_EXIT, Warning};
use git::{Git, WeekPoint};
use report::WeekView;
use scan::Snapshot;
use store::Store;

const MAX_PRINTED_WARNINGS: usize = 10;
const SHORT_SHA_LEN: usize = 7;

#[derive(Parser)]
#[command(
    version,
    about = "Weekly architecture snapshots of a TypeScript/JavaScript repo"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Snapshot each week of git history and write the HTML report.
    Run(RunArgs),
}

#[derive(clap::Args)]
struct RunArgs {
    /// Repository to scan (defaults to the current directory).
    #[arg(long)]
    repo: Option<PathBuf>,
    /// Output folder (defaults to <repo>/.archsnap).
    #[arg(long)]
    out: Option<PathBuf>,
    /// How many weeks that had commits to snapshot, newest first.
    #[arg(long, default_value_t = 12, value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..))]
    weeks: usize,
    /// Folder levels per area below each package (after src/), e.g. 1 turns apps/web/src/billing/api/x.ts into apps/web/src/billing.
    #[arg(long, default_value_t = 1, value_parser = clap::builder::RangedU64ValueParser::<usize>::new().range(1..))]
    depth: usize,
    /// Exit 1 when there are warnings (the report is still written).
    #[arg(long)]
    strict: bool,
    /// Print every git call.
    #[arg(long)]
    verbose: bool,
}

fn main() -> ExitCode {
    std::panic::set_hook(Box::new(report_panic));
    let Cmd::Run(args) = Cli::parse().command;
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fatal) => ExitCode::from(report_fatal(fatal).exit_code()),
    }
}

fn report_panic(panic: &PanicHookInfo) {
    let mut stderr = io::stderr().lock();
    let reported = writeln!(stderr, "archsnap: internal error: {panic}")
        .and_then(|()| writeln!(stderr, "  help: {BUG_REPORT_HELP}"));
    let exit_code = match reported {
        Ok(()) => INTERNAL_ERROR_EXIT,
        Err(unwritable) => Fatal::StderrNotWritable(unwritable).exit_code(),
    };
    std::process::exit(i32::from(exit_code));
}

fn report_fatal(fatal: Fatal) -> Fatal {
    match fatal.print() {
        Ok(()) => fatal,
        Err(unprintable) => unprintable,
    }
}

fn run(args: RunArgs) -> Result<(), Fatal> {
    let dir = args
        .repo
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| Fatal::NotGitRepo(PathBuf::from(".")))?;
    if !dir.is_dir() {
        return Err(Fatal::NotGitRepo(dir));
    }
    let git = Git::open(&dir, args.verbose)?;
    let out = args.out.unwrap_or_else(|| git.root.join(".archsnap"));
    let store = Store::open(&out)?;

    let mut warnings: Vec<Warning> = Vec::new();
    let shallow = git.is_shallow()?;
    if shallow {
        warnings.push(Warning::new(
            "shallow_clone",
            "this is a shallow clone, so only the current week can be snapshotted",
            "in GitHub Actions, set `fetch-depth: 0` on actions/checkout to get full history",
        ));
    }
    let mut points = git.weekly_points(if shallow { 1 } else { args.weeks })?;
    points.reverse();

    let weekly = WeeklyScan {
        git: &git,
        store: &store,
        depth: args.depth,
        total: points.len(),
    };
    let mut snapshots = Vec::with_capacity(points.len());
    for (position, point) in (1..).zip(&points) {
        snapshots.push(weekly.snapshot(position, point, &mut warnings)?);
    }
    let weeks = week_views(snapshots);
    print_warnings(&warnings)?;

    let repo_name = git
        .root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let html = report::render(&report::Report {
        repo: repo_name,
        shallow,
        weeks,
        warnings: &warnings,
    })?;
    let path = store.write_report(&html)?;
    writeln!(io::stderr().lock(), "→ {}", path.display()).map_err(Fatal::StderrNotWritable)?;

    if args.strict && !warnings.is_empty() {
        return Err(Fatal::Strict(warnings.len()));
    }
    Ok(())
}

struct WeeklyScan<'a> {
    git: &'a Git,
    store: &'a Store,
    depth: usize,
    total: usize,
}

impl WeeklyScan<'_> {
    fn snapshot(
        &self,
        position: usize,
        point: &WeekPoint,
        warnings: &mut Vec<Warning>,
    ) -> Result<Snapshot, Fatal> {
        let newest = position == self.total;
        let cached = match self.store.load(&point.week, &point.sha, self.depth) {
            Ok(cached) => cached,
            Err(corrupt) => {
                warnings.push(*corrupt);
                None
            }
        };
        let progress = format!(
            "[{position}/{}] ✔ {} ({})",
            self.total,
            point.week,
            short_sha(&point.sha)
        );
        // The newest week is always rescanned so its warnings are reported on every run.
        if let Some(snapshot) = cached.filter(|_| !newest) {
            writeln!(io::stderr().lock(), "{progress} cached").map_err(Fatal::StderrNotWritable)?;
            return Ok(snapshot);
        }
        let mut found = Vec::new();
        let snapshot = scan::scan(self.git, point, self.depth, &mut found)?;
        self.store.save(&snapshot)?;
        writeln!(
            io::stderr().lock(),
            "{progress} {} files, {} areas, {} warnings",
            snapshot.stats.files,
            snapshot.modules.len(),
            found.len()
        )
        .map_err(Fatal::StderrNotWritable)?;
        if newest {
            warnings.extend(found);
        }
        Ok(snapshot)
    }
}

fn short_sha(sha: &str) -> &str {
    sha.get(..SHORT_SHA_LEN).unwrap_or(sha)
}

fn week_views(snapshots: Vec<Snapshot>) -> Vec<WeekView> {
    let previous = std::iter::once(None).chain(snapshots.iter().map(Some));
    let diffs: Vec<diff::Diff> = previous
        .zip(&snapshots)
        .map(|(before, after)| diff::diff(before, after))
        .collect();
    let mut weeks: Vec<WeekView> = snapshots
        .into_iter()
        .zip(diffs)
        .map(|(snapshot, diff)| WeekView { snapshot, diff })
        .collect();
    weeks.reverse();
    // File-level links only for the newest week: on big repos every week's links would bloat the page.
    for older in weeks.iter_mut().skip(1) {
        older
            .snapshot
            .edges
            .iter_mut()
            .for_each(|edge| edge.links.clear());
    }
    weeks
}

fn print_warnings(warnings: &[Warning]) -> Result<(), Fatal> {
    for warning in warnings.iter().take(MAX_PRINTED_WARNINGS) {
        warning.print()?;
    }
    let unprinted = warnings.iter().skip(MAX_PRINTED_WARNINGS).len();
    if unprinted > 0 {
        writeln!(
            io::stderr().lock(),
            "…and {unprinted} more warnings (listed in the report)"
        )
        .map_err(Fatal::StderrNotWritable)?;
    }
    Ok(())
}
