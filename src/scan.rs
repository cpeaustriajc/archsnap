use std::collections::{BTreeMap, BTreeSet, HashMap};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, BindingPattern, CallExpression, Expression, JSXAttribute, NewExpression, ObjectProperty, PropertyKey,
    StringLiteral, TemplateElement, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::SourceType;
use oxc_syntax::module_record::{ExportImportName, ImportImportName};
use serde::{Deserialize, Serialize};

use crate::diag::{Fatal, Source, Warning};
use crate::git::{Git, TreeEntry, WeekPoint};
use crate::system::{self, Package, System};

pub const SCHEMA: u32 = 2;
pub const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");
const ASSET_EXTENSIONS: &[&str] = &[
    "css", "scss", "sass", "less", "svg", "png", "jpg", "jpeg", "gif", "webp", "avif", "ico", "json",
    "woff", "woff2", "ttf", "md", "mdx", "html", "txt", "wasm",
];
const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];
const IGNORED_DIRS: &[&str] =
    &["node_modules", "dist", "build", "out", "coverage", "vendor", ".next", ".archsnap"];
const MAX_FILE_BYTES: u64 = 512 * 1024;
const MAX_LINKS_PER_EDGE: usize = 200;

type FileLinks = BTreeMap<(String, String), BTreeSet<String>>;

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
    #[serde(default)]
    pub system: System,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Module {
    pub files: u32,
    pub lines: u32,
    #[serde(default)]
    pub package: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub count: u32,
    #[serde(default)]
    pub links: Vec<Link>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Link {
    pub from: String,
    pub to: String,
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub files: u32,
    pub lines: u32,
    pub skipped: u32,
    pub ignored: u32,
    pub unresolved: u32,
}

enum Role {
    Source,
    Package,
    Wrangler,
    Swift,
    XcodeGen,
}

fn role(e: &TreeEntry) -> Option<Role> {
    if e.path.split('/').any(|seg| IGNORED_DIRS.contains(&seg)) {
        return None;
    }
    let name = e.path.rsplit('/').next().unwrap_or(&e.path);
    match name {
        "package.json" => return Some(Role::Package),
        "wrangler.toml" | "wrangler.json" | "wrangler.jsonc" => return Some(Role::Wrangler),
        "project.yml" | "project.yaml" => return Some(Role::XcodeGen),
        _ => {}
    }
    if name.ends_with(".swift") {
        return Some(Role::Swift);
    }
    let dts = name.ends_with(".d.ts") || name.ends_with(".d.mts") || name.ends_with(".d.cts");
    let code = name.rsplit_once('.').is_some_and(|(_, ext)| EXTENSIONS.contains(&ext));
    (code && !dts).then_some(Role::Source)
}

pub fn scan(git: &Git, point: &WeekPoint, depth: usize, warnings: &mut Vec<Warning>) -> Result<Snapshot, Fatal> {
    let mut stats = Stats::default();
    let tree = git.tree(&point.sha)?;
    let committed: BTreeSet<&str> = tree.iter().map(|e| e.path.as_str()).collect();

    let mut wanted: Vec<(&TreeEntry, Role)> = Vec::new();
    for e in &tree {
        let Some(r) = role(e) else { continue };
        if e.size > MAX_FILE_BYTES || e.path.contains(".min.") {
            if matches!(r, Role::Source) {
                stats.ignored += 1;
            }
            continue;
        }
        wanted.push((e, r));
    }
    let oids: Vec<String> = wanted.iter().map(|(e, _)| e.oid.clone()).collect();
    let blobs = git.read_blobs(&oids)?;

    let mut sources: Vec<(&str, String)> = Vec::new();
    let mut packages: Vec<Package> = Vec::new();
    let mut wranglers = Vec::new();
    let mut urls: Vec<(String, String)> = Vec::new();
    let mut xcodegen: Vec<(String, String)> = Vec::new();
    for ((entry, r), blob) in wanted.iter().zip(blobs) {
        let Ok(text) = String::from_utf8(blob) else {
            if matches!(r, Role::Source) {
                stats.ignored += 1;
            }
            continue;
        };
        match r {
            Role::Source => sources.push((entry.path.as_str(), text)),
            Role::Package => packages.extend(system::parse_package(&entry.path, &text, warnings)),
            Role::Wrangler => wranglers.push((entry.path.clone(), text)),
            Role::Swift => urls.extend(swift_urls(&text).into_iter().map(|h| (entry.path.clone(), h))),
            Role::XcodeGen => {
                let name = text.lines().find_map(|l| l.strip_prefix("name:")).map(|n| n.trim().trim_matches('"').to_string());
                let dir = entry.path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
                if let Some(name) = name.filter(|n| !n.is_empty()) {
                    xcodegen.push((dir, name));
                }
            }
        }
    }
    let scanned: BTreeSet<&str> = sources.iter().map(|(p, _)| *p).collect();

    let mut modules: BTreeMap<String, Module> = BTreeMap::new();
    let mut edges: HashMap<(String, String), (u32, FileLinks)> = HashMap::new();
    let mut externals: BTreeMap<String, u32> = BTreeMap::new();

    for (path, text) in &sources {
        let parsed = match parse(path, text) {
            Ok(p) => p,
            Err(SyntaxError { message, labels }) => {
                stats.skipped += 1;
                warnings.push(
                    Warning::new(
                        "parse_skipped",
                        format!("skipped {path}: {message}"),
                        format!(
                            "this file is left out of the {} snapshot; fix the syntax error and re-run",
                            point.week
                        ),
                    )
                    .in_file(path, Source { text: text.clone(), labels }),
                );
                continue;
            }
        };
        let from = area_of(path, depth, &packages);
        let m = modules.entry(from.clone()).or_default();
        m.files += 1;
        m.lines += text.lines().count() as u32;
        m.package = system::package_of(&packages, path).map(|p| p.dir.clone()).unwrap_or_default();
        stats.files += 1;
        urls.extend(parsed.hosts.into_iter().map(|h| (path.to_string(), h)));

        for imp in parsed.imports {
            match classify(path, &imp.spec, &committed, &packages) {
                Target::File(target) if scanned.contains(target.as_str()) => {
                    let to = area_of(&target, depth, &packages);
                    if to != from {
                        let (count, links) = edges.entry((from.clone(), to)).or_default();
                        *count += 1;
                        links.entry((path.to_string(), target)).or_default().extend(imp.names);
                    }
                }
                Target::File(_) | Target::Asset => {}
                Target::Package(name) => *externals.entry(name).or_default() += 1,
                Target::Unresolved => {
                    stats.unresolved += 1;
                    warnings.push(
                        Warning::new(
                            "unresolved_import",
                            format!("can't find `{}` imported from {path}", imp.spec),
                            "no committed file matches this path; it is shown as unresolved in the report",
                        )
                        .in_file(
                            path,
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

    let mut xcode: BTreeSet<(String, String)> = tree
        .iter()
        .filter_map(|e| {
            let (before, _) = e.path.split_once(".xcodeproj/")?;
            let (dir, name) = before.rsplit_once('/').unwrap_or(("", before));
            Some((dir.to_string(), name.to_string()))
        })
        .collect();
    xcode.extend(xcodegen);
    let system = system::detect(
        system::Inputs { packages: &packages, wranglers, xcode: xcode.into_iter().collect(), urls },
        warnings,
    );

    stats.lines = modules.values().map(|m| m.lines).sum();
    let mut edges: Vec<Edge> = edges
        .into_iter()
        .map(|((from, to), (count, links))| Edge {
            from,
            to,
            count,
            links: links
                .into_iter()
                .take(MAX_LINKS_PER_EDGE)
                .map(|((from, to), names)| Link { from, to, names: names.into_iter().collect() })
                .collect(),
        })
        .collect();
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
        system,
    })
}

/// `depth` folder levels below the file's package (skipping a leading `src/`).
pub fn area_of(path: &str, depth: usize, packages: &[Package]) -> String {
    let root = system::package_of(packages, path).map(|p| p.dir.as_str()).unwrap_or("");
    let rest = if root.is_empty() { path } else { &path[root.len() + 1..] };
    let mut dirs: Vec<&str> = rest.split('/').collect();
    dirs.pop();
    let had_src = dirs.first() == Some(&"src");
    if had_src {
        dirs.remove(0);
    }
    let mut prefix: Vec<&str> = if root.is_empty() { vec![] } else { vec![root] };
    if had_src && (!dirs.is_empty() || root.is_empty()) {
        prefix.push("src");
    }
    prefix.extend(dirs.iter().take(depth));
    if prefix.is_empty() { "(root)".into() } else { prefix.join("/") }
}

struct Import {
    spec: String,
    start: u32,
    end: u32,
    names: Vec<String>,
}

struct Parsed {
    imports: Vec<Import>,
    hosts: Vec<String>,
}

type Labels = Vec<(u32, u32, Option<String>)>;

struct SyntaxError {
    message: String,
    labels: Labels,
}

/// A URL counts when it feeds a network call directly, or sits in a file that makes one.
/// Files that only hold URLs as data (links, citations) make no calls, so they add nothing.
#[derive(Default)]
struct NetworkUrls {
    direct: Vec<String>,
    anywhere: Vec<String>,
    calls_network: bool,
    in_link: u32,
}

impl NetworkUrls {
    fn hosts(mut self) -> Vec<String> {
        if self.calls_network {
            self.direct.append(&mut self.anywhere);
        }
        self.direct.sort();
        self.direct.dedup();
        self.direct
    }
}

const CALLERS: &[&str] = &["fetch", "$fetch", "ofetch", "ky", "got", "axios", "request", "superagent"];
const CLIENT_OBJECTS: &[&str] = &["axios", "ky", "got", "http", "https", "superagent", "request"];
const URL_CONSTRUCTORS: &[&str] = &["URL", "Request", "WebSocket", "EventSource"];
const URL_KEYS: &[&str] = &["baseURL", "baseUrl", "prefixUrl", "endpoint", "apiUrl", "apiURL", "apiBase"];
const LINK_KEYS: &[&str] = &[
    "href", "link", "links", "website", "homepage", "image", "images", "src", "logo", "icon", "avatar", "docs",
    "docsUrl", "learnMore", "learnMoreUrl", "imageUrl", "thumbnail", "og", "openGraph", "twitter", "canonical",
];

fn url_in(e: &Expression) -> Option<String> {
    match e {
        Expression::StringLiteral(s) => system::host_of(s.value.as_str()),
        Expression::TemplateLiteral(t) => t.quasis.first().and_then(|q| system::host_of(q.value.raw.as_str())),
        _ => None,
    }
}

fn first_arg_url(args: &[Argument]) -> Option<String> {
    args.first().and_then(|a| a.as_expression()).and_then(url_in)
}

fn is_network_call(callee: &Expression) -> bool {
    match callee {
        Expression::Identifier(i) => {
            let n = i.name.as_str();
            CALLERS.contains(&n) || n.to_ascii_lowercase().contains("fetch")
        }
        Expression::StaticMemberExpression(m) => {
            matches!(&m.object, Expression::Identifier(o) if CLIENT_OBJECTS.contains(&o.name.as_str()))
                || m.property.name.as_str().to_ascii_lowercase().contains("fetch")
        }
        _ => false,
    }
}

fn names_an_endpoint(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["api", "endpoint", "base"].iter().any(|k| n.contains(k))
}

impl<'a> Visit<'a> for NetworkUrls {
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if is_network_call(&it.callee) {
            self.calls_network = true;
            self.direct.extend(first_arg_url(&it.arguments));
        }
        walk::walk_call_expression(self, it);
    }
    fn visit_new_expression(&mut self, it: &NewExpression<'a>) {
        if matches!(&it.callee, Expression::Identifier(i) if URL_CONSTRUCTORS.contains(&i.name.as_str())) {
            self.direct.extend(first_arg_url(&it.arguments));
        }
        walk::walk_new_expression(self, it);
    }
    fn visit_variable_declarator(&mut self, it: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(b) = &it.id
            && names_an_endpoint(b.name.as_str())
            && let Some(init) = &it.init
        {
            self.direct.extend(url_in(init));
        }
        walk::walk_variable_declarator(self, it);
    }
    fn visit_object_property(&mut self, it: &ObjectProperty<'a>) {
        let key = match &it.key {
            PropertyKey::StaticIdentifier(k) => Some(k.name.as_str()),
            PropertyKey::StringLiteral(k) => Some(k.value.as_str()),
            _ => None,
        };
        if key.is_some_and(|k| URL_KEYS.contains(&k)) {
            self.direct.extend(url_in(&it.value));
        }
        let link = key.is_some_and(|k| LINK_KEYS.contains(&k));
        self.in_link += link as u32;
        walk::walk_object_property(self, it);
        self.in_link -= link as u32;
    }
    fn visit_jsx_attribute(&mut self, it: &JSXAttribute<'a>) {
        self.in_link += 1;
        walk::walk_jsx_attribute(self, it);
        self.in_link -= 1;
    }
    fn visit_string_literal(&mut self, it: &StringLiteral<'a>) {
        if self.in_link == 0 {
            self.anywhere.extend(system::host_of(it.value.as_str()));
        }
    }
    fn visit_template_element(&mut self, it: &TemplateElement<'a>) {
        if self.in_link == 0 {
            self.anywhere.extend(system::host_of(it.value.raw.as_str()));
        }
    }
}

fn parse(path: &str, text: &str) -> Result<Parsed, SyntaxError> {
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

    let record = &ret.module_record;
    let mut names: HashMap<&str, BTreeSet<String>> = HashMap::new();
    for e in record.import_entries.iter() {
        let n = match &e.import_name {
            ImportImportName::Name(n) => n.name.as_str().to_string(),
            ImportImportName::NamespaceObject => "* (everything)".to_string(),
            ImportImportName::Default(_) => "default".to_string(),
        };
        names.entry(e.module_request.name.as_str()).or_default().insert(n);
    }
    for e in record.indirect_export_entries.iter().chain(record.star_export_entries.iter()) {
        let Some(req) = &e.module_request else { continue };
        let n = match &e.import_name {
            ExportImportName::Name(n) => n.name.as_str().to_string(),
            ExportImportName::All | ExportImportName::AllButDefault => "* (re-exported)".to_string(),
            _ => continue,
        };
        names.entry(req.name.as_str()).or_default().insert(n);
    }

    let mut imports = Vec::new();
    for (spec, uses) in record.requested_modules.iter() {
        let spec = spec.as_str();
        let found: Vec<String> = names.get(spec).map(|s| s.iter().cloned().collect()).unwrap_or_default();
        for u in uses.iter() {
            imports.push(Import { spec: spec.to_string(), start: u.span.start, end: u.span.end, names: found.clone() });
        }
    }
    for d in record.dynamic_imports.iter() {
        let raw = &text[d.module_request.start as usize..d.module_request.end as usize];
        let quoted = raw.len() >= 2
            && (raw.starts_with('\'') || raw.starts_with('"') || (raw.starts_with('`') && !raw.contains("${")))
            && raw.ends_with(&raw[..1]);
        if quoted {
            imports.push(Import {
                spec: raw[1..raw.len() - 1].to_string(),
                start: d.module_request.start,
                end: d.module_request.end,
                names: vec!["import() (lazy)".into()],
            });
        }
    }
    imports.sort_by_key(|i| i.start);

    let mut urls = NetworkUrls::default();
    urls.visit_program(&ret.program);
    Ok(Parsed { imports, hosts: urls.hosts() })
}

/// Same rule as TS: a URL counts in a file that makes requests, or when its constant names an endpoint.
fn swift_urls(text: &str) -> Vec<String> {
    let requests = text.contains("URLSession") || text.contains("URLRequest");
    let mut hosts = Vec::new();
    for line in text.lines() {
        let Some(i) = line.find("\"http") else { continue };
        let named = line
            .split(['=', ':'])
            .next()
            .and_then(|lhs| lhs.split_whitespace().last())
            .is_some_and(names_an_endpoint);
        if requests || named {
            hosts.extend(system::host_of(&line[i + 1..]));
        }
    }
    hosts
}

enum Target {
    File(String),
    Asset,
    Package(String),
    Unresolved,
}

fn classify(from: &str, spec: &str, files: &BTreeSet<&str>, packages: &[Package]) -> Target {
    let spec = spec.split(['?', '#']).next().unwrap_or(spec);
    let is_asset = spec.rsplit_once('.').is_some_and(|(_, ext)| ASSET_EXTENSIONS.contains(&ext));
    // A bare name like `normalize.css` is a package, not a file path.
    if is_asset && spec.contains('/') {
        return Target::Asset;
    }
    let relative = spec.starts_with("./") || spec.starts_with("../") || spec == "." || spec == "..";
    if relative {
        let dir = from.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        return match join(dir, spec).and_then(|p| resolve(&p, files)) {
            Some(f) => Target::File(f),
            None => Target::Unresolved,
        };
    }
    if let Some(rest) = spec.strip_prefix("@/").or_else(|| spec.strip_prefix("~/")) {
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
    if let Some(pkg) = packages.iter().find(|p| p.name == name) {
        let sub = spec[name.len()..].trim_start_matches('/');
        let base = |p: &str| if pkg.dir.is_empty() { p.to_string() } else { format!("{}/{p}", pkg.dir) };
        let mut tries: Vec<String> = Vec::new();
        if sub.is_empty() {
            if let Some(main) = &pkg.main {
                tries.push(base(main.trim_start_matches("./")));
            }
            tries.extend([base("src/index"), base("index")]);
        } else {
            tries.extend([base(sub), base(&format!("src/{sub}"))]);
        }
        // A workspace package that only ships built files has nothing to link; it is still not external.
        return tries.iter().find_map(|t| resolve(t, files)).map(Target::File).unwrap_or(Target::Asset);
    }
    Target::Package(name)
}

pub fn join(dir: &str, rel: &str) -> Option<String> {
    let joined = format!("{dir}/{rel}");
    let mut out: Vec<&str> = Vec::new();
    for seg in joined.split('/') {
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
