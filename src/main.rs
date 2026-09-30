mod diag;
mod diff;
mod git;
mod report;
mod scan;
mod store;
mod system;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use diag::{Fatal, Warning};
use git::Git;
use store::Store;

const MAX_PRINTED_WARNINGS: usize = 10;

#[derive(Parser)]
#[command(version, about = "Weekly architecture snapshots of a TypeScript/JavaScript repo")]
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
    std::panic::set_hook(Box::new(|info| {
        eprintln!("archsnap: internal error: {info}");
        eprintln!("  help: this is a bug in archsnap; please report it with the command you ran");
        std::process::exit(70);
    }));
    let Cmd::Run(args) = Cli::parse().command;
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fatal) => {
            fatal.print();
            ExitCode::from(fatal.exit_code() as u8)
        }
    }
}

fn run(args: RunArgs) -> Result<(), Fatal> {
    let dir = match args.repo {
        Some(p) => p,
        None => std::env::current_dir().map_err(|_| Fatal::NotGitRepo(PathBuf::from(".")))?,
    };
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

    let total = points.len();
    let mut snapshots = Vec::with_capacity(total);
    for (i, point) in points.iter().enumerate() {
        let newest = i + 1 == total;
        let short = &point.sha[..7.min(point.sha.len())];
        let cached = match store.load(&point.week, &point.sha, args.depth) {
            Ok(c) => c,
            Err(w) => {
                warnings.push(*w);
                None
            }
        };
        // The newest week is always rescanned so its warnings are reported on every run.
        let snap = match cached {
            Some(s) if !newest => {
                eprintln!("[{}/{total}] ✔ {} ({short}) cached", i + 1, point.week);
                s
            }
            _ => {
                let mut found = Vec::new();
                let s = scan::scan(&git, point, args.depth, &mut found)?;
                store.save(&s)?;
                eprintln!(
                    "[{}/{total}] ✔ {} ({short}) {} files, {} areas, {} warnings",
                    i + 1,
                    point.week,
                    s.stats.files,
                    s.modules.len(),
                    found.len()
                );
                if newest {
                    warnings.extend(found);
                }
                s
            }
        };
        snapshots.push(snap);
    }

    let mut weeks = Vec::with_capacity(total);
    for i in (0..snapshots.len()).rev() {
        let before = if i == 0 { None } else { Some(&snapshots[i - 1]) };
        let mut snapshot = snapshots[i].clone();
        // File-level links only for the newest week: on big repos every week's links would bloat the page.
        if i + 1 != snapshots.len() {
            snapshot.edges.iter_mut().for_each(|e| e.links.clear());
        }
        weeks.push(report::WeekView { diff: diff::diff(before, &snapshots[i]), snapshot });
    }

    for w in warnings.iter().take(MAX_PRINTED_WARNINGS) {
        w.print();
    }
    if warnings.len() > MAX_PRINTED_WARNINGS {
        eprintln!("…and {} more warnings (listed in the report)", warnings.len() - MAX_PRINTED_WARNINGS);
    }

    let repo_name = git.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let html = report::render(&report::Report { repo: repo_name, shallow, weeks, warnings: &warnings });
    let path = store.write_report(&html)?;
    eprintln!("→ {}", path.display());

    if args.strict && !warnings.is_empty() {
        return Err(Fatal::Strict(warnings.len()));
    }
    Ok(())
}
