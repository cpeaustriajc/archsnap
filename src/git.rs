use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::diag::Fatal;

pub struct Git {
    pub root: PathBuf,
    verbose: bool,
}

#[derive(Debug, Clone)]
pub struct WeekPoint {
    pub week: String,
    pub sha: String,
    pub date: String,
}

pub struct TreeEntry {
    pub path: String,
    pub oid: String,
    pub size: u64,
}

impl Git {
    pub fn open(dir: &Path, verbose: bool) -> Result<Git, Fatal> {
        let out = raw(dir, &["rev-parse", "--show-toplevel"], verbose)?;
        if !out.status.success() {
            return Err(Fatal::NotGitRepo(dir.to_path_buf()));
        }
        let root = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        let git = Git { root, verbose };
        if !raw(&git.root, &["rev-parse", "--verify", "-q", "HEAD"], verbose)?.status.success() {
            return Err(Fatal::NoCommits(git.root));
        }
        Ok(git)
    }

    pub fn is_shallow(&self) -> Result<bool, Fatal> {
        Ok(self.run(&["rev-parse", "--is-shallow-repository"])?.trim() == "true")
    }

    pub fn weekly_points(&self, max: usize) -> Result<Vec<WeekPoint>, Fatal> {
        let log = self.run(&[
            "log",
            "--first-parent",
            "--date=format:%G-W%V",
            "--format=%H%x09%cd%x09%cs",
            "HEAD",
        ])?;
        let mut points: Vec<WeekPoint> = Vec::new();
        for line in log.lines() {
            let mut parts = line.split('\t');
            let (Some(sha), Some(week), Some(date)) = (parts.next(), parts.next(), parts.next()) else {
                continue;
            };
            if points.iter().any(|p| p.week == week) {
                continue;
            }
            points.push(WeekPoint { week: week.into(), sha: sha.into(), date: date.into() });
            if points.len() == max {
                break;
            }
        }
        Ok(points)
    }

    pub fn tree(&self, sha: &str) -> Result<Vec<TreeEntry>, Fatal> {
        let out = self.run(&["ls-tree", "-r", "-l", "-z", sha])?;
        let mut entries = Vec::new();
        for rec in out.split('\0').filter(|r| !r.is_empty()) {
            let Some((meta, path)) = rec.split_once('\t') else { continue };
            let f: Vec<&str> = meta.split_whitespace().collect();
            // Mode 120000 blobs are symlinks: their content is the link target, not code.
            if f.len() == 4 && f[1] == "blob" && f[0] != "120000" {
                let size = f[3].parse().map_err(|_| Fatal::GitFailed {
                    args: format!("ls-tree -r -l {sha}"),
                    stderr: format!("object {} for {path} is missing or corrupt", f[2]),
                })?;
                entries.push(TreeEntry { path: path.into(), oid: f[2].into(), size });
            }
        }
        Ok(entries)
    }

    pub fn read_blobs(&self, oids: &[String]) -> Result<Vec<Vec<u8>>, Fatal> {
        if oids.is_empty() {
            return Ok(Vec::new());
        }
        if self.verbose {
            eprintln!("  $ git cat-file --batch ({} objects)", oids.len());
        }
        let mut child = Command::new("git")
            .args(["cat-file", "--batch"])
            .current_dir(&self.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(spawn_err)?;
        let mut stdin = child.stdin.take().expect("piped stdin");
        let input: String = oids.iter().map(|o| format!("{o}\n")).collect();
        // Writing on another thread: a full stdout pipe would otherwise deadlock the write.
        let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()));
        let mut reader = BufReader::new(child.stdout.take().expect("piped stdout"));
        let mut blobs = Vec::with_capacity(oids.len());
        let failed = |why: &str| Fatal::GitFailed { args: "cat-file --batch".into(), stderr: why.into() };
        for _ in oids {
            let mut header = String::new();
            reader.read_line(&mut header).map_err(|e| failed(&e.to_string()))?;
            let size: usize = header
                .split_whitespace()
                .nth(2)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| failed(header.trim()))?;
            let mut buf = vec![0; size + 1];
            reader.read_exact(&mut buf).map_err(|e| failed(&e.to_string()))?;
            buf.pop();
            blobs.push(buf);
        }
        let _ = writer.join();
        let _ = child.wait();
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

fn raw(dir: &Path, args: &[&str], verbose: bool) -> Result<std::process::Output, Fatal> {
    if verbose {
        eprintln!("  $ git {}", args.join(" "));
    }
    Command::new("git").args(args).current_dir(dir).output().map_err(spawn_err)
}

fn spawn_err(e: std::io::Error) -> Fatal {
    if e.kind() == std::io::ErrorKind::NotFound {
        Fatal::GitMissing
    } else {
        Fatal::GitFailed { args: String::new(), stderr: e.to_string() }
    }
}
