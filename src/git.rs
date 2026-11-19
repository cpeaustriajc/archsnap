use std::collections::BTreeSet;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{ChildStdin, Command, Stdio};
use std::thread::JoinHandle;

use crate::diag::Fatal;

const SYMLINK_MODE: &str = "120000";

pub(crate) struct Git {
    pub(crate) root: PathBuf,
    verbose: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct WeekPoint {
    pub(crate) week: String,
    pub(crate) sha: String,
    pub(crate) date: String,
}

pub(crate) struct TreeEntry {
    pub(crate) path: String,
    pub(crate) oid: String,
    pub(crate) size: u64,
}

impl Git {
    pub(crate) fn open(dir: &Path, verbose: bool) -> Result<Git, Fatal> {
        let out = raw(dir, &["rev-parse", "--show-toplevel"], verbose)?;
        if !out.status.success() {
            return Err(Fatal::NotGitRepo(dir.to_path_buf()));
        }
        let root = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        let git = Git { root, verbose };
        if !raw(&git.root, &["rev-parse", "--verify", "-q", "HEAD"], verbose)?
            .status
            .success()
        {
            return Err(Fatal::NoCommits(git.root));
        }
        Ok(git)
    }

    pub(crate) fn is_shallow(&self) -> Result<bool, Fatal> {
        Ok(self.run(&["rev-parse", "--is-shallow-repository"])?.trim() == "true")
    }

    pub(crate) fn weekly_points(&self, max: usize) -> Result<Vec<WeekPoint>, Fatal> {
        let log = self.run(&[
            "log",
            "--first-parent",
            "--date=format:%G-W%V",
            "--format=%H%x09%cd%x09%cs",
            "HEAD",
        ])?;
        let mut seen_weeks = BTreeSet::new();
        Ok(log
            .lines()
            .filter_map(parse_log_line)
            .filter(|point| seen_weeks.insert(point.week.clone()))
            .take(max)
            .collect())
    }

    pub(crate) fn tree(&self, sha: &str) -> Result<Vec<TreeEntry>, Fatal> {
        let out = self.run(&["ls-tree", "-r", "-l", "-z", sha])?;
        out.split('\0')
            .filter(|record| !record.is_empty())
            .filter_map(|record| parse_tree_record(record, sha).transpose())
            .collect()
    }

    pub(crate) fn read_blobs(&self, oids: &[String]) -> Result<Vec<Vec<u8>>, Fatal> {
        if oids.is_empty() {
            return Ok(Vec::new());
        }
        if self.verbose {
            writeln!(
                io::stderr().lock(),
                "  $ git cat-file --batch ({} objects)",
                oids.len()
            )
            .map_err(Fatal::StderrNotWritable)?;
        }
        let mut child = Command::new("git")
            .args(["cat-file", "--batch"])
            .current_dir(&self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| spawn_err(&error))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| cat_file_failed("its stdin was not piped"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| cat_file_failed("its stdout was not piped"))?;
        let mut input = oids.join("\n");
        input.push('\n');
        let writer = write_without_blocking_reads(stdin, input);
        let mut reader = BufReader::new(stdout);
        let mut blobs = Vec::with_capacity(oids.len());
        for _ in oids {
            blobs.push(read_blob(&mut reader)?);
        }
        writer
            .join()
            .map_err(|_panic_payload| cat_file_failed("the thread feeding its stdin panicked"))?
            .map_err(|error| cat_file_failed(&error.to_string()))?;
        let status = child
            .wait()
            .map_err(|error| cat_file_failed(&error.to_string()))?;
        if !status.success() {
            return Err(cat_file_failed(&format!("it exited with {status}")));
        }
        Ok(blobs)
    }

    fn run(&self, args: &[&str]) -> Result<String, Fatal> {
        let out = raw(&self.root, args, self.verbose)?;
        if !out.status.success() {
            return Err(Fatal::GitFailed {
                args: args.join(" "),
                stderr: String::from_utf8_lossy(&out.stderr).trim().to_string(),
            });
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

fn parse_log_line(line: &str) -> Option<WeekPoint> {
    let mut parts = line.split('\t');
    let (Some(sha), Some(week), Some(date)) = (parts.next(), parts.next(), parts.next()) else {
        return None;
    };
    Some(WeekPoint {
        week: week.into(),
        sha: sha.into(),
        date: date.into(),
    })
}

fn parse_tree_record(record: &str, sha: &str) -> Result<Option<TreeEntry>, Fatal> {
    let Some((meta, path)) = record.split_once('\t') else {
        return Ok(None);
    };
    let mut fields = meta.split_whitespace();
    let (Some(mode), Some(kind), Some(oid), Some(size_field), None) = (
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
        fields.next(),
    ) else {
        return Ok(None);
    };
    if kind != "blob" || mode == SYMLINK_MODE {
        return Ok(None);
    }
    let size = size_field.parse().ok().ok_or_else(|| Fatal::GitFailed {
        args: format!("ls-tree -r -l {sha}"),
        stderr: format!("object {oid} for {path} is missing or corrupt"),
    })?;
    Ok(Some(TreeEntry {
        path: path.into(),
        oid: oid.into(),
        size,
    }))
}

fn read_blob(reader: &mut impl BufRead) -> Result<Vec<u8>, Fatal> {
    let mut header = String::new();
    reader
        .read_line(&mut header)
        .map_err(|error| cat_file_failed(&error.to_string()))?;
    let size: usize = header
        .split_whitespace()
        .nth(2)
        .and_then(|size| size.parse().ok())
        .ok_or_else(|| cat_file_failed(header.trim()))?;
    let mut blob = vec![0; size];
    let mut trailing_newline = [0; 1];
    reader
        .read_exact(&mut blob)
        .and_then(|()| reader.read_exact(&mut trailing_newline))
        .map_err(|error| cat_file_failed(&error.to_string()))?;
    Ok(blob)
}

fn write_without_blocking_reads(
    mut stdin: ChildStdin,
    input: String,
) -> JoinHandle<io::Result<()>> {
    std::thread::spawn(move || stdin.write_all(input.as_bytes()))
}

fn cat_file_failed(why: &str) -> Fatal {
    Fatal::GitFailed {
        args: "cat-file --batch".into(),
        stderr: why.into(),
    }
}

fn raw(dir: &Path, args: &[&str], verbose: bool) -> Result<std::process::Output, Fatal> {
    if verbose {
        writeln!(io::stderr().lock(), "  $ git {}", args.join(" "))
            .map_err(Fatal::StderrNotWritable)?;
    }
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .map_err(|error| spawn_err(&error))
}

fn spawn_err(error: &io::Error) -> Fatal {
    if error.kind() == io::ErrorKind::NotFound {
        Fatal::GitMissing
    } else {
        Fatal::GitFailed {
            args: String::new(),
            stderr: error.to_string(),
        }
    }
}
