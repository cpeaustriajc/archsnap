use std::collections::BTreeSet;

use serde::Serialize;

use crate::scan::Snapshot;

#[derive(Debug, Serialize)]
pub(crate) struct Diff {
    pub(crate) added_modules: Vec<String>,
    pub(crate) removed_modules: Vec<String>,
    pub(crate) grown: Vec<SizeChange>,
    pub(crate) shrunk: Vec<SizeChange>,
    pub(crate) added_edges: Vec<(String, String)>,
    pub(crate) removed_edges: Vec<(String, String)>,
    pub(crate) added_packages: Vec<String>,
    pub(crate) removed_packages: Vec<String>,
    pub(crate) summary: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct SizeChange {
    pub(crate) module: String,
    pub(crate) before: u32,
    pub(crate) after: u32,
}

impl SizeChange {
    fn is_notable(&self) -> bool {
        let delta = self.after.abs_diff(self.before);
        delta >= MIN_LINES && f64::from(delta) >= f64::from(self.before.max(1)) * MIN_RATIO
    }

    fn grew(&self) -> bool {
        self.after > self.before
    }

    fn percent(&self) -> String {
        let change = (f64::from(self.after) - f64::from(self.before))
            / f64::from(self.before.max(1))
            * 100.0;
        format!("{change:+.0}%")
    }
}

const MIN_LINES: u32 = 20;
const MIN_RATIO: f64 = 0.2;
const NO_CHANGES: &str = "No structural changes this week.";

pub(crate) fn diff(previous: Option<&Snapshot>, after: &Snapshot) -> Diff {
    let Some(before) = previous else {
        return first_diff(after);
    };
    let (old_modules, new_modules) = (module_names(before), module_names(after));
    let (old_edges, new_edges) = (edge_pairs(before), edge_pairs(after));
    let (old_packages, new_packages) = (package_names(before), package_names(after));
    let (grown, shrunk) = size_changes(before, after);
    let mut diff = Diff {
        added_modules: new_modules.difference(&old_modules).cloned().collect(),
        removed_modules: old_modules.difference(&new_modules).cloned().collect(),
        grown,
        shrunk,
        added_edges: new_edges.difference(&old_edges).cloned().collect(),
        removed_edges: old_edges.difference(&new_edges).cloned().collect(),
        added_packages: new_packages.difference(&old_packages).cloned().collect(),
        removed_packages: old_packages.difference(&new_packages).cloned().collect(),
        summary: Vec::new(),
    };
    let mut summary = crate::system::diff_lines(&before.system, &after.system);
    summary.extend(change_lines(&diff));
    if summary.is_empty() {
        summary.push(NO_CHANGES.into());
    }
    diff.summary = summary;
    diff
}

fn first_diff(after: &Snapshot) -> Diff {
    Diff {
        added_modules: Vec::new(),
        removed_modules: Vec::new(),
        grown: Vec::new(),
        shrunk: Vec::new(),
        added_edges: Vec::new(),
        removed_edges: Vec::new(),
        added_packages: Vec::new(),
        removed_packages: Vec::new(),
        summary: vec![format!(
            "First snapshot: {} areas, {} files, {} lines of code.",
            after.modules.len(),
            after.stats.files,
            after.stats.lines
        )],
    }
}

fn module_names(snapshot: &Snapshot) -> BTreeSet<String> {
    snapshot.modules.keys().cloned().collect()
}

fn edge_pairs(snapshot: &Snapshot) -> BTreeSet<(String, String)> {
    snapshot
        .edges
        .iter()
        .map(|edge| (edge.from.clone(), edge.to.clone()))
        .collect()
}

fn package_names(snapshot: &Snapshot) -> BTreeSet<String> {
    snapshot.externals.keys().cloned().collect()
}

fn size_changes(before: &Snapshot, after: &Snapshot) -> (Vec<SizeChange>, Vec<SizeChange>) {
    after
        .modules
        .iter()
        .filter_map(|(name, module)| {
            let previous = before.modules.get(name)?;
            Some(SizeChange {
                module: name.clone(),
                before: previous.lines,
                after: module.lines,
            })
        })
        .filter(SizeChange::is_notable)
        .partition(SizeChange::grew)
}

fn change_lines(d: &Diff) -> Vec<String> {
    let mut s = Vec::new();
    if !d.added_modules.is_empty() {
        s.push(format!(
            "{} new: {}.",
            plural(d.added_modules.len(), "area", "areas"),
            d.added_modules.join(", ")
        ));
    }
    if !d.removed_modules.is_empty() {
        s.push(format!(
            "{} removed: {}.",
            plural(d.removed_modules.len(), "area", "areas"),
            d.removed_modules.join(", ")
        ));
    }
    for c in &d.grown {
        s.push(format!(
            "{} grew from {} to {} lines ({}).",
            c.module,
            c.before,
            c.after,
            c.percent()
        ));
    }
    for c in &d.shrunk {
        s.push(format!(
            "{} shrank from {} to {} lines ({}).",
            c.module,
            c.before,
            c.after,
            c.percent()
        ));
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
    s
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}
