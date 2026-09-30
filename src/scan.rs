use std::collections::{BTreeMap, BTreeSet, HashMap};

use oxc_allocator::Allocator;
use oxc_parser::Parser;
use oxc_span::SourceType;
use serde::{Deserialize, Serialize};

use crate::diag::{Fatal, Source, Warning};
use crate::git::{Git, WeekPoint};

pub const SCHEMA: u32 = 1;
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");
const ASSET_EXTENSIONS: &[&str] = &[
    "css", "scss", "sass", "less", "svg", "png", "jpg", "jpeg", "gif", "webp", "avif", "ico", "json",
    "woff", "woff2", "ttf", "md", "mdx", "html", "txt", "wasm",
];
const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];
const IGNORED_DIRS: &[&str] =
    &["node_modules", "dist", "build", "out", "coverage", "vendor", ".next", ".archsnap"];
const MAX_FILE_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema: u32,
    #[serde(default)]
    pub tool: String,
    pub week: String,
    pub sha: String,
    pub date: String,
    pub depth: usize,
    pub modules: BTreeMap<String, Module>,
    pub edges: Vec<Edge>,
    pub externals: BTreeMap<String, u32>,
    pub stats: Stats,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Module {
    pub files: u32,
    pub lines: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub count: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub files: u32,
    pub lines: u32,
    pub skipped: u32,
    pub ignored: u32,
    pub unresolved: u32,
}

pub fn scan(git: &Git, point: &WeekPoint, depth: usize, warnings: &mut Vec<Warning>) -> Result<Snapshot, Fatal> {
    let mut stats = Stats::default();
    let tree = git.tree(&point.sha)?;
    let committed: BTreeSet<&str> = tree.iter().map(|e| e.path.as_str()).collect();
    let mut candidates = Vec::new();
    for e in &tree {
        if !is_source(&e.path) {
            continue;
        }
        if e.size > MAX_FILE_BYTES || e.path.contains(".min.") {
            stats.ignored += 1;
            continue;
        }
        candidates.push(e);
    }
    let oids: Vec<String> = candidates.iter().map(|e| e.oid.clone()).collect();
    let blobs = git.read_blobs(&oids)?;
    let scanned: BTreeSet<&str> = candidates.iter().map(|e| e.path.as_str()).collect();

    let mut modules: BTreeMap<String, Module> = BTreeMap::new();
    let mut edges: HashMap<(String, String), u32> = HashMap::new();
    let mut externals: BTreeMap<String, u32> = BTreeMap::new();

    for (entry, blob) in candidates.iter().zip(blobs) {
        let Ok(text) = String::from_utf8(blob) else {
            stats.ignored += 1;
            continue;
        };
        let imports = match parse_imports(&entry.path, &text) {
            Ok(i) => i,
            Err(SyntaxError { message, labels }) => {
                stats.skipped += 1;
                warnings.push(
                    Warning::new(
                        "parse_skipped",
                        format!("skipped {}: {message}", entry.path),
                        format!(
                            "this file is left out of the {} snapshot; fix the syntax error and re-run",
                            point.week
                        ),
                    )
                    .in_file(&entry.path, Source { text, labels }),
                );
                continue;
            }
        };
        let from = module_of(&entry.path, depth);
        let m = modules.entry(from.clone()).or_default();
        m.files += 1;
        m.lines += text.lines().count() as u32;
        stats.files += 1;

        for imp in imports {
            match classify(&entry.path, &imp.spec, &committed) {
                Target::File(target) if scanned.contains(target.as_str()) => {
                    let to = module_of(&target, depth);
                    if to != from {
                        *edges.entry((from.clone(), to)).or_default() += 1;
                    }
                }
                Target::File(_) | Target::Asset => {}
                Target::Package(name) => *externals.entry(name).or_default() += 1,
                Target::Unresolved => {
                    stats.unresolved += 1;
                    warnings.push(
                        Warning::new(
                            "unresolved_import",
                            format!("can't find `{}` imported from {}", imp.spec, entry.path),
                            "no committed file matches this path; it is shown as unresolved in the report",
                        )
                        .in_file(
                            &entry.path,
                            Source {
                                text: text.clone(),
                                labels: vec![(imp.start, imp.end, Some("imported here".into()))],
                            },
                        ),
                    );
                }
            }
        }
    }

    stats.lines = modules.values().map(|m| m.lines).sum();
    let mut edges: Vec<Edge> =
        edges.into_iter().map(|((from, to), count)| Edge { from, to, count }).collect();
    edges.sort();
    Ok(Snapshot {
        schema: SCHEMA,
        tool: TOOL_VERSION.to_string(),
        week: point.week.clone(),
        sha: point.sha.clone(),
        date: point.date.clone(),
        depth,
        modules,
        edges,
        externals,
        stats,
    })
}

fn is_source(path: &str) -> bool {
    if path.split('/').any(|seg| IGNORED_DIRS.contains(&seg)) {
        return false;
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.ends_with(".d.ts") || name.ends_with(".d.mts") || name.ends_with(".d.cts") {
        return false;
    }
    name.rsplit_once('.').is_some_and(|(_, ext)| EXTENSIONS.contains(&ext))
}

pub fn module_of(path: &str, depth: usize) -> String {
    let dirs: Vec<&str> = path.split('/').collect();
    let dirs = &dirs[..dirs.len() - 1];
    if dirs.is_empty() {
        return "(root)".into();
    }
    dirs[..dirs.len().min(depth)].join("/")
}

struct Import {
    spec: String,
    start: u32,
    end: u32,
}

type Labels = Vec<(u32, u32, Option<String>)>;

struct SyntaxError {
    message: String,
    labels: Labels,
}

fn parse_imports(path: &str, text: &str) -> Result<Vec<Import>, SyntaxError> {
    let alloc = Allocator::default();
    let mut source_type = SourceType::from_path(path).unwrap_or_default();
    if path.ends_with(".js") || path.ends_with(".mjs") || path.ends_with(".cjs") {
        source_type = source_type.with_jsx(true);
    }
    let ret = Parser::new(&alloc, text, source_type).parse();
    if let Some(first) = ret.diagnostics.errors().next() {
        let message = first.message.to_string();
        let labels = first
            .labels
            .iter()
            .map(|l| {
                let label = l.label().filter(|t| !t.is_empty()).unwrap_or(&message).to_string();
                let last = text.trim_end().len() as u32;
                if l.offset() >= last && last > 0 {
                    // Spans at EOF sit past the last line and render as an empty box.
                    return (last - 1, last, Some("file ends here".to_string()));
                }
                (l.offset(), l.offset() + l.len(), Some(label))
            })
            .collect();
        return Err(SyntaxError { message, labels });
    }
    let mut out = Vec::new();
    for (spec, uses) in ret.module_record.requested_modules.iter() {
        for u in uses.iter() {
            out.push(Import { spec: spec.as_str().to_string(), start: u.span.start, end: u.span.end });
        }
    }
    for d in ret.module_record.dynamic_imports.iter() {
        let raw = &text[d.module_request.start as usize..d.module_request.end as usize];
        let quoted = raw.len() >= 2
            && (raw.starts_with('\'') || raw.starts_with('"') || (raw.starts_with('`') && !raw.contains("${")))
            && raw.ends_with(&raw[..1]);
        if quoted {
            out.push(Import { spec: raw[1..raw.len() - 1].to_string(), start: d.module_request.start, end: d.module_request.end });
        }
    }
    out.sort_by_key(|i| i.start);
    Ok(out)
}

enum Target {
    File(String),
    Asset,
    Package(String),
    Unresolved,
}

fn classify(from: &str, spec: &str, files: &BTreeSet<&str>) -> Target {
    let spec = spec.split(['?', '#']).next().unwrap_or(spec);
    let path = spec;
    let is_asset = path.rsplit_once('.').is_some_and(|(_, ext)| ASSET_EXTENSIONS.contains(&ext));
    // A bare name like `normalize.css` is a package, not a file path.
    if is_asset && path.contains('/') {
        return Target::Asset;
    }
    let relative = spec.starts_with("./") || spec.starts_with("../") || spec == "." || spec == "..";
    let alias = spec.strip_prefix("@/").or_else(|| spec.strip_prefix("~/"));
    if relative {
        let dir = from.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        return match normalize(&format!("{dir}/{spec}")).and_then(|p| resolve(&p, files)) {
            Some(f) => Target::File(f),
            None => Target::Unresolved,
        };
    }
    if let Some(rest) = alias {
        let mut dir = from.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        loop {
            let base = if dir.is_empty() { String::new() } else { format!("{dir}/") };
            let hit = resolve(&format!("{base}src/{rest}"), files).or_else(|| resolve(&format!("{base}{rest}"), files));
            if let Some(f) = hit {
                return Target::File(f);
            }
            if dir.is_empty() {
                return Target::Unresolved;
            }
            dir = dir.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        }
    }
    if spec.starts_with('/') {
        return Target::Unresolved;
    }
    let mut parts = spec.split('/');
    let first = parts.next().unwrap_or(spec);
    let name = match (first.starts_with('@'), parts.next()) {
        (true, Some(second)) => format!("{first}/{second}"),
        _ => first.to_string(),
    };
    Target::Package(name)
}

fn normalize(path: &str) -> Option<String> {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            s => out.push(s),
        }
    }
    Some(out.join("/"))
}

fn resolve(base: &str, files: &BTreeSet<&str>) -> Option<String> {
    if files.contains(base) {
        return Some(base.to_string());
    }
    let (stem, ext) = base.rsplit_once('.').unwrap_or((base, ""));
    let swapped: &[&str] = match ext {
        "js" => &["ts", "tsx", "d.ts"],
        "jsx" => &["tsx"],
        "mjs" => &["mts"],
        "cjs" => &["cts"],
        _ => &[],
    };
    let candidates = swapped
        .iter()
        .map(|e| format!("{stem}.{e}"))
        .chain(EXTENSIONS.iter().map(|e| format!("{base}.{e}")))
        .chain(EXTENSIONS.iter().map(|e| format!("{base}/index.{e}")))
        .chain([format!("{base}.d.ts"), format!("{base}/index.d.ts")]);
    candidates.into_iter().find(|c| files.contains(c.as_str()))
}
