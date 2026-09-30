use std::collections::BTreeSet;

use serde::Serialize;

use crate::scan::Snapshot;

#[derive(Debug, Serialize)]
pub struct Diff {
    pub added_modules: Vec<String>,
    pub removed_modules: Vec<String>,
    pub grown: Vec<SizeChange>,
    pub shrunk: Vec<SizeChange>,
    pub added_edges: Vec<(String, String)>,
    pub removed_edges: Vec<(String, String)>,
    pub added_packages: Vec<String>,
    pub removed_packages: Vec<String>,
    pub summary: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SizeChange {
    pub module: String,
    pub before: u32,
    pub after: u32,
}

const MIN_LINES: u32 = 20;
const MIN_RATIO: f64 = 0.2;

pub fn diff(before: Option<&Snapshot>, after: &Snapshot) -> Diff {
    let empty = BTreeSet::new();
    let keys = |s: Option<&Snapshot>| s.map(|s| s.modules.keys().cloned().collect()).unwrap_or_else(|| empty.clone());
    let (old, new): (BTreeSet<String>, BTreeSet<String>) = (keys(before), keys(Some(after)));

    let mut grown = Vec::new();
    let mut shrunk = Vec::new();
    if let Some(b) = before {
        for (name, m) in &after.modules {
            let Some(prev) = b.modules.get(name) else { continue };
            let delta = m.lines.abs_diff(prev.lines);
            if delta < MIN_LINES || (delta as f64) < prev.lines.max(1) as f64 * MIN_RATIO {
                continue;
            }
            let c = SizeChange { module: name.clone(), before: prev.lines, after: m.lines };
            if m.lines > prev.lines { grown.push(c) } else { shrunk.push(c) }
        }
    }

    let edges = |s: Option<&Snapshot>| -> BTreeSet<(String, String)> {
        s.map(|s| s.edges.iter().map(|e| (e.from.clone(), e.to.clone())).collect()).unwrap_or_default()
    };
    let (old_e, new_e) = (edges(before), edges(Some(after)));
    let pkgs = |s: Option<&Snapshot>| -> BTreeSet<String> {
        s.map(|s| s.externals.keys().cloned().collect()).unwrap_or_default()
    };
    let (old_p, new_p) = (pkgs(before), pkgs(Some(after)));

    let mut d = Diff {
        added_modules: if before.is_none() { Vec::new() } else { new.difference(&old).cloned().collect() },
        removed_modules: old.difference(&new).cloned().collect(),
        grown,
        shrunk,
        added_edges: if before.is_none() { Vec::new() } else { new_e.difference(&old_e).cloned().collect() },
        removed_edges: old_e.difference(&new_e).cloned().collect(),
        added_packages: if before.is_none() { Vec::new() } else { new_p.difference(&old_p).cloned().collect() },
        removed_packages: old_p.difference(&new_p).cloned().collect(),
        summary: Vec::new(),
    };
    d.summary = summarize(&d, before.is_none(), after);
    d
}

fn summarize(d: &Diff, first: bool, after: &Snapshot) -> Vec<String> {
    let mut s = Vec::new();
    if first {
        s.push(format!(
            "First snapshot: {} areas, {} files, {} lines of code.",
            after.modules.len(),
            after.stats.files,
            after.stats.lines
        ));
        return s;
    }
    if !d.added_modules.is_empty() {
        s.push(format!("{} new: {}.", plural(d.added_modules.len(), "area", "areas"), d.added_modules.join(", ")));
    }
    if !d.removed_modules.is_empty() {
        s.push(format!("{} removed: {}.", plural(d.removed_modules.len(), "area", "areas"), d.removed_modules.join(", ")));
    }
    for c in &d.grown {
        s.push(format!("{} grew from {} to {} lines ({}).", c.module, c.before, c.after, pct(c)));
    }
    for c in &d.shrunk {
        s.push(format!("{} shrank from {} to {} lines ({}).", c.module, c.before, c.after, pct(c)));
    }
    for (from, to) in &d.added_edges {
        s.push(format!("{from} now depends on {to}."));
    }
    for (from, to) in &d.removed_edges {
        s.push(format!("{from} no longer depends on {to}."));
    }
    if !d.added_packages.is_empty() {
        s.push(format!("Started using {}.", d.added_packages.join(", ")));
    }
    if !d.removed_packages.is_empty() {
        s.push(format!("Stopped using {}.", d.removed_packages.join(", ")));
    }
    if s.is_empty() {
        s.push("No structural changes this week.".into());
    }
    s
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

fn pct(c: &SizeChange) -> String {
    let p = (c.after as f64 - c.before as f64) / c.before.max(1) as f64 * 100.0;
    format!("{p:+.0}%")
}
