use std::collections::{BTreeMap, BTreeSet, HashMap};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    Argument, BindingPattern, CallExpression, Expression, JSXAttribute, NewExpression,
    ObjectProperty, ObjectPropertyKind, PropertyKey, StaticMemberExpression, StringLiteral,
    TemplateElement, VariableDeclarator,
};
use oxc_ast_visit::{Visit, walk};
use oxc_parser::Parser;
use oxc_span::{LabeledSpan, SourceType};
use oxc_syntax::module_record::{
    ExportEntry, ExportExportName, ExportImportName, ImportEntry, ImportImportName, ModuleRecord,
};
use serde::{Deserialize, Serialize};

use crate::diag::{Fatal, Source, Warning};
use crate::git::{Git, TreeEntry, WeekPoint};
use crate::system::{self, CodeFacts, Package, System};

pub(crate) const SCHEMA: u32 = 3;
pub(crate) const TOOL_VERSION: &str = env!("CARGO_PKG_VERSION");
const ASSET_EXTENSIONS: &[&str] = &[
    "css", "scss", "sass", "less", "svg", "png", "jpg", "jpeg", "gif", "webp", "avif", "ico",
    "json", "woff", "woff2", "ttf", "md", "mdx", "html", "txt", "wasm",
];
const EXTENSIONS: &[&str] = &["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"];
const PLAIN_JS_EXTENSIONS: &[&str] = &["js", "mjs", "cjs"];
const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "out",
    "coverage",
    "vendor",
    ".next",
    ".archsnap",
];
const MAX_FILE_BYTES: u64 = 512 * 1024;
const MAX_LINKS_PER_EDGE: usize = 200;

type FileLinks = BTreeMap<(String, String), BTreeSet<String>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Snapshot {
    pub(crate) schema: u32,
    #[serde(default)]
    pub(crate) tool: String,
    pub(crate) week: String,
    pub(crate) sha: String,
    pub(crate) date: String,
    pub(crate) depth: usize,
    pub(crate) modules: BTreeMap<String, Module>,
    pub(crate) edges: Vec<Edge>,
    pub(crate) externals: BTreeMap<String, u32>,
    pub(crate) stats: Stats,
    #[serde(default)]
    pub(crate) system: System,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Module {
    pub(crate) files: u32,
    pub(crate) lines: u32,
    #[serde(default)]
    pub(crate) package: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Edge {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) count: u32,
    #[serde(default)]
    pub(crate) links: Vec<Link>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Link {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) names: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct Stats {
    pub(crate) files: u32,
    pub(crate) lines: u32,
    pub(crate) skipped: u32,
    pub(crate) ignored: u32,
    pub(crate) unresolved: u32,
}

enum Role {
    Source,
    Package,
    Wrangler,
    Swift,
    XcodeGen,
}

fn role(entry: &TreeEntry) -> Option<Role> {
    if entry
        .path
        .split('/')
        .any(|segment| IGNORED_DIRS.contains(&segment))
    {
        return None;
    }
    match file_name(&entry.path) {
        "package.json" => Some(Role::Package),
        "wrangler.toml" | "wrangler.json" | "wrangler.jsonc" => Some(Role::Wrangler),
        "project.yml" | "project.yaml" => Some(Role::XcodeGen),
        name => role_by_extension(name),
    }
}

fn role_by_extension(name: &str) -> Option<Role> {
    let ext = extension(name);
    if ext == Some("swift") {
        return Some(Role::Swift);
    }
    let declaration =
        name.ends_with(".d.ts") || name.ends_with(".d.mts") || name.ends_with(".d.cts");
    let code = ext.is_some_and(|code_ext| EXTENSIONS.contains(&code_ext));
    (code && !declaration).then_some(Role::Source)
}

fn file_name(path: &str) -> &str {
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

fn parent_dir(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn extension(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(_, ext)| ext)
}

fn under(dir: &str, path: &str) -> String {
    if dir.is_empty() {
        path.to_string()
    } else {
        format!("{dir}/{path}")
    }
}

fn saturating_count(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

struct Selection<'t> {
    wanted: Vec<(&'t TreeEntry, Role)>,
    ignored_sources: Vec<&'t str>,
}

fn is_oversized_or_minified(entry: &TreeEntry) -> bool {
    entry.size > MAX_FILE_BYTES || entry.path.contains(".min.")
}

fn select_entries(tree: &[TreeEntry]) -> Selection<'_> {
    let (dropped, wanted): (Vec<_>, Vec<_>) = tree
        .iter()
        .filter_map(|entry| role(entry).map(|found| (entry, found)))
        .partition(|(entry, _)| is_oversized_or_minified(entry));
    let ignored_sources = dropped
        .into_iter()
        .filter(|(_, dropped_role)| matches!(dropped_role, Role::Source))
        .map(|(entry, _)| entry.path.as_str())
        .collect();
    Selection {
        wanted,
        ignored_sources,
    }
}

#[derive(Default)]
struct Inputs<'t> {
    sources: Vec<(&'t str, String)>,
    packages: Vec<Package>,
    wranglers: Vec<(String, String)>,
    xcodegen: Vec<(String, String)>,
    urls: Vec<(String, String)>,
    code: Vec<(String, CodeFacts)>,
    ignored_sources: Vec<&'t str>,
}

fn decode_blobs<'t>(
    selection: Selection<'t>,
    blobs: Vec<Vec<u8>>,
    warnings: &mut Vec<Warning>,
) -> Inputs<'t> {
    let mut inputs = Inputs {
        ignored_sources: selection.ignored_sources,
        ..Inputs::default()
    };
    for ((entry, entry_role), blob) in selection.wanted.into_iter().zip(blobs) {
        match String::from_utf8(blob) {
            Ok(text) => add_input(&mut inputs, entry, &entry_role, text, warnings),
            Err(_) if matches!(entry_role, Role::Source) => {
                inputs.ignored_sources.push(entry.path.as_str());
            }
            Err(_) => {}
        }
    }
    inputs
}

fn add_input<'t>(
    inputs: &mut Inputs<'t>,
    entry: &'t TreeEntry,
    entry_role: &Role,
    text: String,
    warnings: &mut Vec<Warning>,
) {
    let path = entry.path.as_str();
    match entry_role {
        Role::Source => inputs.sources.push((path, text)),
        Role::Package => inputs
            .packages
            .extend(system::parse_package(path, &text, warnings)),
        Role::Wrangler => inputs.wranglers.push((path.to_string(), text)),
        Role::Swift => add_swift(inputs, path, &text),
        Role::XcodeGen => inputs.xcodegen.extend(xcodegen_project(path, &text)),
    }
}

fn add_swift(inputs: &mut Inputs<'_>, path: &str, text: &str) {
    inputs.urls.extend(
        swift_urls(text)
            .into_iter()
            .map(|host| (path.to_string(), host)),
    );
    let paths = swift_api_paths(text);
    if !paths.is_empty() {
        inputs.code.push((
            path.to_string(),
            CodeFacts {
                paths,
                ..CodeFacts::default()
            },
        ));
    }
}

fn xcodegen_project(path: &str, text: &str) -> Option<(String, String)> {
    let name = text
        .lines()
        .find_map(|line| line.strip_prefix("name:"))?
        .trim()
        .trim_matches('"');
    if name.is_empty() {
        return None;
    }
    Some((parent_dir(path).to_string(), name.to_string()))
}

fn xcode_project(entry: &TreeEntry) -> Option<(String, String)> {
    let (before, _) = entry.path.split_once(".xcodeproj/")?;
    let (dir, name) = before.rsplit_once('/').unwrap_or(("", before));
    Some((dir.to_string(), name.to_string()))
}

struct Resolver<'r> {
    committed: &'r BTreeSet<&'r str>,
    scanned: &'r BTreeSet<&'r str>,
    packages: &'r [Package],
    depth: usize,
}

struct FileLink {
    file: String,
    target: String,
    names: Vec<String>,
}

#[derive(Default)]
struct Collected<'s> {
    area_files: BTreeMap<String, Vec<(&'s str, usize)>>,
    edges: BTreeMap<(String, String), Vec<FileLink>>,
    externals: BTreeMap<String, Vec<&'s str>>,
    skipped: Vec<&'s str>,
    unresolved: Vec<&'s str>,
    urls: Vec<(String, String)>,
    code: Vec<(String, CodeFacts)>,
}

pub(crate) fn scan(
    git: &Git,
    point: &WeekPoint,
    depth: usize,
    warnings: &mut Vec<Warning>,
) -> Result<Snapshot, Fatal> {
    let tree = git.tree(&point.sha)?;
    let selection = select_entries(&tree);
    let oids: Vec<String> = selection
        .wanted
        .iter()
        .map(|(entry, _)| entry.oid.clone())
        .collect();
    let blobs = git.read_blobs(&oids)?;
    let Inputs {
        sources,
        packages,
        wranglers,
        xcodegen,
        urls,
        code,
        ignored_sources,
    } = decode_blobs(selection, blobs, warnings);

    let committed: BTreeSet<&str> = tree.iter().map(|entry| entry.path.as_str()).collect();
    let scanned: BTreeSet<&str> = sources.iter().map(|(path, _)| *path).collect();
    let resolver = Resolver {
        committed: &committed,
        scanned: &scanned,
        packages: &packages,
        depth,
    };
    let mut collected = Collected {
        urls,
        code,
        ..Collected::default()
    };
    for (path, text) in &sources {
        match parse(path, text) {
            Ok(parsed) => add_file(&mut collected, &resolver, path, text, parsed, warnings),
            Err(error) => {
                collected.skipped.push(path);
                warnings.push(skipped_warning(path, text, error, &point.week));
            }
        }
    }

    let modules = modules(&collected.area_files, &packages);
    let mut xcode: BTreeSet<(String, String)> = tree.iter().filter_map(xcode_project).collect();
    xcode.extend(xcodegen);
    let areas: Vec<(String, String)> = modules
        .iter()
        .map(|(area, module)| (area.clone(), module.package.clone()))
        .collect();
    let area_of_file = |file: &str| area_of(file, depth, &packages);
    let system = system::detect(
        system::Inputs {
            packages: &packages,
            wranglers,
            xcode: xcode.into_iter().collect(),
            urls: collected.urls,
            areas,
            area_of: &area_of_file,
            code: collected.code,
        },
        warnings,
    );

    let stats = Stats {
        files: saturating_count(collected.area_files.values().map(Vec::len).sum()),
        lines: saturating_count(
            collected
                .area_files
                .values()
                .flatten()
                .map(|(_, lines)| lines)
                .sum(),
        ),
        skipped: saturating_count(collected.skipped.len()),
        ignored: saturating_count(ignored_sources.len()),
        unresolved: saturating_count(collected.unresolved.len()),
    };
    Ok(Snapshot {
        schema: SCHEMA,
        tool: TOOL_VERSION.to_string(),
        week: point.week.clone(),
        sha: point.sha.clone(),
        date: point.date.clone(),
        depth,
        modules,
        edges: collected.edges.into_iter().map(edge).collect(),
        externals: collected
            .externals
            .into_iter()
            .map(|(name, importers)| (name, saturating_count(importers.len())))
            .collect(),
        stats,
        system,
    })
}

fn add_file<'s>(
    collected: &mut Collected<'s>,
    resolver: &Resolver<'_>,
    path: &'s str,
    text: &'s str,
    parsed: Parsed,
    warnings: &mut Vec<Warning>,
) {
    let from = area_of(path, resolver.depth, resolver.packages);
    collected
        .area_files
        .entry(from.clone())
        .or_default()
        .push((path, text.lines().count()));
    collected.urls.extend(
        parsed
            .hosts
            .into_iter()
            .map(|host| (path.to_string(), host)),
    );
    collected
        .code
        .push((path.to_string(), without_redundant_paths(parsed.http)));

    for import in parsed.imports {
        match classify(path, &import.spec, resolver.committed, resolver.packages) {
            Target::File(target) => {
                add_link(collected, resolver, &from, path, target, import.names);
            }
            Target::Asset => {}
            Target::Package(name) => collected.externals.entry(name).or_default().push(path),
            Target::Unresolved => {
                collected.unresolved.push(path);
                warnings.push(unresolved_warning(path, text, &import));
            }
        }
    }
}

fn add_link(
    collected: &mut Collected<'_>,
    resolver: &Resolver<'_>,
    from: &str,
    file: &str,
    target: String,
    names: Vec<String>,
) {
    if !resolver.scanned.contains(target.as_str()) {
        return;
    }
    let to = area_of(&target, resolver.depth, resolver.packages);
    if to == from {
        return;
    }
    collected
        .edges
        .entry((from.to_string(), to))
        .or_default()
        .push(FileLink {
            file: file.to_string(),
            target,
            names,
        });
}

fn without_redundant_paths(mut http: CodeFacts) -> CodeFacts {
    // A literal inside fetch("/api/x", { method }) is also seen on its own, without the method.
    let with_method: BTreeSet<String> = http
        .paths
        .iter()
        .filter(|(_, method)| method.is_some())
        .map(|(path, _)| path.clone())
        .collect();
    let defined: BTreeSet<String> = http
        .routes
        .iter()
        .map(|(_, path, _)| path.clone())
        .chain(http.mounts.iter().map(|(_, path, _)| path.clone()))
        .collect();
    http.paths.retain(|(path, method)| {
        (method.is_some() || !with_method.contains(path)) && !defined.contains(path)
    });
    http
}

fn skipped_warning(path: &str, text: &str, error: SyntaxError, week: &str) -> Warning {
    let SyntaxError { message, labels } = error;
    Warning::new(
        "parse_skipped",
        format!("skipped {path}: {message}"),
        format!("this file is left out of the {week} snapshot; fix the syntax error and re-run"),
    )
    .in_file(
        path,
        Source {
            text: text.to_string(),
            labels,
        },
    )
}

fn unresolved_warning(path: &str, text: &str, import: &Import) -> Warning {
    Warning::new(
        "unresolved_import",
        format!("can't find `{}` imported from {path}", import.spec),
        "no committed file matches this path; it is shown as unresolved in the report",
    )
    .in_file(
        path,
        Source {
            text: text.to_string(),
            labels: vec![(import.start, import.end, Some("imported here".into()))],
        },
    )
}

fn modules(
    area_files: &BTreeMap<String, Vec<(&str, usize)>>,
    packages: &[Package],
) -> BTreeMap<String, Module> {
    area_files
        .iter()
        .map(|(area, files)| (area.clone(), module(files, packages)))
        .collect()
}

fn module(files: &[(&str, usize)], packages: &[Package]) -> Module {
    let package = files
        .last()
        .and_then(|(path, _)| system::package_of(packages, path))
        .map(|found| found.dir.clone())
        .unwrap_or_default();
    Module {
        files: saturating_count(files.len()),
        lines: saturating_count(files.iter().map(|(_, lines)| lines).sum()),
        package,
    }
}

fn edge(((from, to), file_links): ((String, String), Vec<FileLink>)) -> Edge {
    let count = saturating_count(file_links.len());
    let mut by_file: FileLinks = BTreeMap::new();
    for link in file_links {
        by_file
            .entry((link.file, link.target))
            .or_default()
            .extend(link.names);
    }
    let links = by_file
        .into_iter()
        .take(MAX_LINKS_PER_EDGE)
        .map(|((file, target), names)| Link {
            from: file,
            to: target,
            names: names.into_iter().collect(),
        })
        .collect();
    Edge {
        from,
        to,
        count,
        links,
    }
}

/// `depth` folder levels below the file's package (skipping a leading `src/`).
pub(crate) fn area_of(path: &str, depth: usize, packages: &[Package]) -> String {
    let root = system::package_of(packages, path).map_or("", |package| package.dir.as_str());
    let inside_root = path
        .strip_prefix(root)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(path);
    let dirs: Vec<&str> = inside_root
        .rsplit_once('/')
        .map_or_else(Vec::new, |(dir, _)| dir.split('/').collect());
    let (had_src, below_src) = match dirs.as_slice() {
        ["src", below @ ..] => (true, below),
        all => (false, all),
    };
    let mut prefix: Vec<&str> = Vec::new();
    if !root.is_empty() {
        prefix.push(root);
    }
    if had_src && (!below_src.is_empty() || root.is_empty()) {
        prefix.push("src");
    }
    prefix.extend(below_src.iter().take(depth));
    if prefix.is_empty() {
        "(root)".into()
    } else {
        prefix.join("/")
    }
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
    http: CodeFacts,
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
    inside_link: bool,
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

const CALLERS: &[&str] = &[
    "fetch",
    "$fetch",
    "ofetch",
    "ky",
    "got",
    "axios",
    "request",
    "superagent",
];
const CLIENT_OBJECTS: &[&str] = &[
    "axios",
    "ky",
    "got",
    "http",
    "https",
    "superagent",
    "request",
];
const URL_CONSTRUCTORS: &[&str] = &["URL", "Request", "WebSocket", "EventSource"];
const URL_KEYS: &[&str] = &[
    "baseURL",
    "baseUrl",
    "prefixUrl",
    "endpoint",
    "apiUrl",
    "apiURL",
    "apiBase",
];
const LINK_KEYS: &[&str] = &[
    "href",
    "link",
    "links",
    "website",
    "homepage",
    "image",
    "images",
    "src",
    "logo",
    "icon",
    "avatar",
    "docs",
    "docsUrl",
    "learnMore",
    "learnMoreUrl",
    "imageUrl",
    "thumbnail",
    "og",
    "openGraph",
    "twitter",
    "canonical",
];

fn argument_at<'e, 'a>(
    arguments: &'e [Argument<'a>],
    position: usize,
) -> Option<&'e Expression<'a>> {
    arguments.get(position).and_then(Argument::as_expression)
}

fn url_in(expression: &Expression) -> Option<String> {
    if let Expression::StringLiteral(literal) = expression {
        system::host_of(literal.value.as_str())
    } else if let Expression::TemplateLiteral(template) = expression {
        template
            .quasis
            .first()
            .and_then(|quasi| system::host_of(quasi.value.raw.as_str()))
    } else {
        None
    }
}

fn first_arg_url(arguments: &[Argument]) -> Option<String> {
    argument_at(arguments, 0).and_then(url_in)
}

fn mentions_fetch(name: &str) -> bool {
    name.to_ascii_lowercase().contains("fetch")
}

fn is_network_call(callee: &Expression) -> bool {
    if let Expression::Identifier(identifier) = callee {
        let name = identifier.name.as_str();
        CALLERS.contains(&name) || mentions_fetch(name)
    } else if let Expression::StaticMemberExpression(member) = callee {
        matches!(&member.object, Expression::Identifier(object) if CLIENT_OBJECTS.contains(&object.name.as_str()))
            || mentions_fetch(member.property.name.as_str())
    } else {
        false
    }
}

fn names_an_endpoint(name: &str) -> bool {
    let lowercase = name.to_ascii_lowercase();
    ["api", "endpoint", "base"]
        .iter()
        .any(|word| lowercase.contains(word))
}

fn key_name<'k>(key: &'k PropertyKey<'_>) -> Option<&'k str> {
    if let PropertyKey::StaticIdentifier(identifier) = key {
        Some(identifier.name.as_str())
    } else if let PropertyKey::StringLiteral(literal) = key {
        Some(literal.value.as_str())
    } else {
        None
    }
}

impl<'a> Visit<'a> for NetworkUrls {
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        if is_network_call(&call.callee) {
            self.calls_network = true;
            self.direct.extend(first_arg_url(&call.arguments));
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_new_expression(&mut self, construction: &NewExpression<'a>) {
        if matches!(&construction.callee, Expression::Identifier(class) if URL_CONSTRUCTORS.contains(&class.name.as_str()))
        {
            self.direct.extend(first_arg_url(&construction.arguments));
        }
        walk::walk_new_expression(self, construction);
    }
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        if let BindingPattern::BindingIdentifier(binding) = &declarator.id
            && names_an_endpoint(binding.name.as_str())
            && let Some(init) = &declarator.init
        {
            self.direct.extend(url_in(init));
        }
        walk::walk_variable_declarator(self, declarator);
    }
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        let key = key_name(&property.key);
        if key.is_some_and(|name| URL_KEYS.contains(&name)) {
            self.direct.extend(url_in(&property.value));
        }
        let outer = self.inside_link;
        self.inside_link = outer || key.is_some_and(|name| LINK_KEYS.contains(&name));
        walk::walk_object_property(self, property);
        self.inside_link = outer;
    }
    fn visit_jsx_attribute(&mut self, attribute: &JSXAttribute<'a>) {
        let outer = self.inside_link;
        self.inside_link = true;
        walk::walk_jsx_attribute(self, attribute);
        self.inside_link = outer;
    }
    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        if !self.inside_link {
            self.anywhere
                .extend(system::host_of(literal.value.as_str()));
        }
    }
    fn visit_template_element(&mut self, element: &TemplateElement<'a>) {
        if !self.inside_link {
            self.anywhere
                .extend(system::host_of(element.value.raw.as_str()));
        }
    }
}

const HTTP_METHODS: &[&str] = &["get", "post", "put", "patch", "delete", "all"];
const WORKER_HANDLERS: &[&str] = &["scheduled", "queue"];

#[derive(Default)]
struct HttpFacts {
    facts: CodeFacts,
    current_var: Option<String>,
}

fn path_pattern(expression: &Expression) -> Option<String> {
    if let Expression::StringLiteral(literal) = expression {
        Some(literal.value.as_str().to_string())
    } else if let Expression::TemplateLiteral(template) = expression {
        Some(
            template
                .quasis
                .iter()
                .map(|quasi| quasi.value.raw.as_str())
                .collect::<Vec<_>>()
                .join("*"),
        )
    } else {
        None
    }
}

/// The variable a route chain hangs off: `x.get()` -> x, `new Hono().get().post()` -> the declared name.
fn router_root(expression: &Expression, declared: Option<&str>) -> Option<String> {
    if let Expression::Identifier(identifier) = expression {
        Some(identifier.name.as_str().to_string())
    } else if let Expression::CallExpression(call) = expression
        && let Expression::StaticMemberExpression(member) = &call.callee
    {
        router_root(&member.object, declared)
    } else if matches!(expression, Expression::NewExpression(_)) {
        declared.map(str::to_string)
    } else {
        None
    }
}

fn method_option(arguments: &[Argument]) -> Option<String> {
    let Some(Expression::ObjectExpression(options)) = argument_at(arguments, 1) else {
        return None;
    };
    options.properties.iter().find_map(method_value)
}

fn method_value(kind: &ObjectPropertyKind) -> Option<String> {
    match kind {
        ObjectPropertyKind::ObjectProperty(property) => {
            if let PropertyKey::StaticIdentifier(key) = &property.key
                && key.name.as_str() == "method"
                && let Expression::StringLiteral(value) = &property.value
            {
                Some(value.value.as_str().to_ascii_uppercase())
            } else {
                None
            }
        }
        ObjectPropertyKind::SpreadProperty(_) => None,
    }
}

fn record_route(
    facts: &mut CodeFacts,
    member: &StaticMemberExpression,
    path: &str,
    arguments: &[Argument],
    declared: Option<&str>,
) {
    let property = member.property.name.as_str();
    if HTTP_METHODS.contains(&property) {
        let root = router_root(&member.object, declared);
        facts
            .routes
            .push((property.to_ascii_uppercase(), path.to_string(), root));
    } else if property == "route"
        && let Some(Expression::Identifier(child)) = argument_at(arguments, 1)
    {
        let parent = router_root(&member.object, declared);
        facts
            .mounts
            .push((parent, path.to_string(), child.name.as_str().to_string()));
    }
}

fn is_function(property: &ObjectProperty) -> bool {
    property.method
        || matches!(
            property.value,
            Expression::FunctionExpression(_) | Expression::ArrowFunctionExpression(_)
        )
}

fn looks_like_api_path(path: &str) -> bool {
    path.starts_with("/api/") || path.starts_with("api/") || path == "/api"
}

impl<'a> Visit<'a> for HttpFacts {
    fn visit_variable_declarator(&mut self, declarator: &VariableDeclarator<'a>) {
        let saved = self.current_var.take();
        if let BindingPattern::BindingIdentifier(binding) = &declarator.id {
            self.current_var = Some(binding.name.as_str().to_string());
        }
        walk::walk_variable_declarator(self, declarator);
        self.current_var = saved;
    }
    fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
        let first = argument_at(&call.arguments, 0).and_then(path_pattern);
        if let Expression::StaticMemberExpression(member) = &call.callee
            && let Some(path) = first.as_deref().filter(|path| path.starts_with('/'))
        {
            let declared = self.current_var.as_deref();
            record_route(&mut self.facts, member, path, &call.arguments, declared);
        }
        if is_network_call(&call.callee)
            && let Some(path) = first
        {
            let method = method_option(&call.arguments).unwrap_or_else(|| "GET".into());
            self.facts.paths.push((path, Some(method)));
        }
        walk::walk_call_expression(self, call);
    }
    fn visit_object_property(&mut self, property: &ObjectProperty<'a>) {
        if let PropertyKey::StaticIdentifier(key) = &property.key
            && WORKER_HANDLERS.contains(&key.name.as_str())
            && is_function(property)
        {
            self.facts.handlers.push(key.name.as_str().to_string());
        }
        walk::walk_object_property(self, property);
    }
    fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
        if looks_like_api_path(literal.value.as_str()) {
            self.facts
                .paths
                .push((literal.value.as_str().to_string(), None));
        }
    }
}

fn source_type(path: &str) -> SourceType {
    let detected = SourceType::from_path(path).unwrap_or_default();
    if extension(path).is_some_and(|ext| PLAIN_JS_EXTENSIONS.contains(&ext)) {
        detected.with_jsx(true)
    } else {
        detected
    }
}

fn error_label(labeled: &LabeledSpan, message: &str, text: &str) -> (u32, u32, Option<String>) {
    let span = labeled.span();
    if let Ok(last) = u32::try_from(text.trim_end().len())
        && let Some(before_last) = last.checked_sub(1)
        && span.start >= last
    {
        // Spans at EOF sit past the last line and render as an empty box.
        return (before_last, last, Some("file ends here".to_string()));
    }
    let label = labeled
        .label()
        .filter(|label_text| !label_text.is_empty())
        .unwrap_or(message);
    (span.start, span.end, Some(label.to_string()))
}

fn parse(path: &str, text: &str) -> Result<Parsed, SyntaxError> {
    let allocator = Allocator::default();
    let ret = Parser::new(&allocator, text, source_type(path)).parse();
    if let Some(first) = ret.diagnostics.errors().next() {
        let message = first.message.to_string();
        let labels = first
            .labels
            .iter()
            .map(|labeled| error_label(labeled, &message, text))
            .collect();
        return Err(SyntaxError { message, labels });
    }

    let record = &ret.module_record;
    let names = imported_names(record);
    let mut imports = static_imports(record, &names);
    imports.extend(dynamic_imports(record, text));
    imports.sort_by_key(|import| import.start);

    let mut urls = NetworkUrls::default();
    urls.visit_program(&ret.program);
    let mut http = HttpFacts::default();
    http.visit_program(&ret.program);
    http.facts.exports.extend(
        record
            .local_export_entries
            .iter()
            .filter_map(local_export_name),
    );
    Ok(Parsed {
        imports,
        hosts: urls.hosts(),
        http: http.facts,
    })
}

fn import_label(entry: &ImportEntry) -> String {
    let name = match &entry.import_name {
        ImportImportName::Name(imported) => imported.name.as_str().to_string(),
        ImportImportName::NamespaceObject => "* (everything)".to_string(),
        ImportImportName::Default(_) => "default".to_string(),
    };
    // report.html hides "type "-prefixed names from the plain-language views.
    if entry.is_type {
        format!("type {name}")
    } else {
        name
    }
}

fn re_export_label(name: &ExportImportName) -> Option<String> {
    match name {
        ExportImportName::Name(exported) => Some(exported.name.as_str().to_string()),
        ExportImportName::All | ExportImportName::AllButDefault => {
            Some("* (re-exported)".to_string())
        }
        ExportImportName::Null => None,
    }
}

fn local_export_name(entry: &ExportEntry) -> Option<String> {
    match &entry.export_name {
        ExportExportName::Name(exported) => Some(exported.name.as_str().to_string()),
        ExportExportName::Default(_) | ExportExportName::Null => None,
    }
}

fn imported_names<'r>(record: &'r ModuleRecord) -> HashMap<&'r str, BTreeSet<String>> {
    let mut names: HashMap<&str, BTreeSet<String>> = HashMap::new();
    for entry in &record.import_entries {
        names
            .entry(entry.module_request.name.as_str())
            .or_default()
            .insert(import_label(entry));
    }
    for entry in record
        .indirect_export_entries
        .iter()
        .chain(&record.star_export_entries)
    {
        if let Some(request) = &entry.module_request
            && let Some(name) = re_export_label(&entry.import_name)
        {
            names.entry(request.name.as_str()).or_default().insert(name);
        }
    }
    names
}

fn static_imports(record: &ModuleRecord, names: &HashMap<&str, BTreeSet<String>>) -> Vec<Import> {
    record
        .requested_modules
        .iter()
        .flat_map(|(spec, uses)| {
            let found: Vec<String> = names
                .get(spec.as_str())
                .map(|set| set.iter().cloned().collect())
                .unwrap_or_default();
            uses.iter().map(move |used| Import {
                spec: spec.as_str().to_string(),
                start: used.span.start,
                end: used.span.end,
                names: found.clone(),
            })
        })
        .collect()
}

fn dynamic_imports(record: &ModuleRecord, text: &str) -> Vec<Import> {
    record
        .dynamic_imports
        .iter()
        .filter_map(|dynamic| {
            let request = dynamic.module_request;
            let spec = literal_spec(request.source_text(text))?;
            Some(Import {
                spec: spec.to_string(),
                start: request.start,
                end: request.end,
                names: vec!["import() (lazy)".into()],
            })
        })
        .collect()
}

fn literal_spec(raw: &str) -> Option<&str> {
    let mut chars = raw.chars();
    let quote = chars
        .next()
        .filter(|first| ['\'', '"', '`'].contains(first))?;
    let inside = chars.as_str().strip_suffix(quote)?;
    let interpolated = quote == '`' && raw.contains("${");
    (!interpolated).then_some(inside)
}

fn assigns_endpoint(line: &str) -> bool {
    line.split(['=', ':'])
        .next()
        .and_then(|lhs| lhs.split_whitespace().last())
        .is_some_and(names_an_endpoint)
}

/// Same rule as TS: a URL counts in a file that makes requests, or when its constant names an endpoint.
fn swift_urls(text: &str) -> Vec<String> {
    let requests = text.contains("URLSession") || text.contains("URLRequest");
    text.lines()
        .filter_map(|line| {
            let url = line
                .split('"')
                .skip(1)
                .find(|part| part.starts_with("http"))?;
            if requests || assigns_endpoint(line) {
                system::host_of(url)
            } else {
                None
            }
        })
        .collect()
}

fn after_quote(text: &str) -> Option<&str> {
    text.split_once('"').map(|(_, after)| after)
}

/// `"api/instruments/\(id)/prices"` -> `api/instruments/*/prices`.
fn swift_api_paths(text: &str) -> Vec<(String, Option<String>)> {
    std::iter::successors(after_quote(text), |rest| after_quote(rest))
        .filter_map(swift_api_literal)
        .map(|pattern| (pattern, None))
        .collect()
}

fn swift_api_literal(after_quote: &str) -> Option<String> {
    if !(after_quote.starts_with("api/") || after_quote.starts_with("/api/")) {
        return None;
    }
    let (literal, _) = after_quote.split_once(['"', '\n'])?;
    Some(star_interpolations(literal))
}

fn star_interpolations(literal: &str) -> String {
    let mut pattern = String::new();
    let mut chars = literal.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'(') {
            skip_parenthesized(&mut chars);
            pattern.push('*');
        } else {
            pattern.push(c);
        }
    }
    pattern
}

fn skip_parenthesized(chars: &mut impl Iterator<Item = char>) {
    let mut open: usize = 0;
    for c in chars {
        open = match c {
            '(' => open.saturating_add(1),
            ')' => match open.checked_sub(1) {
                Some(0) | None => return,
                Some(still_open) => still_open,
            },
            _ => open,
        };
    }
}

enum Target {
    File(String),
    Asset,
    Package(String),
    Unresolved,
}

fn is_relative(path: &str) -> bool {
    path.starts_with("./") || path.starts_with("../") || path == "." || path == ".."
}

fn classify(from: &str, spec: &str, files: &BTreeSet<&str>, packages: &[Package]) -> Target {
    let path = spec.split_once(['?', '#']).map_or(spec, |(path, _)| path);
    let is_asset = extension(path).is_some_and(|ext| ASSET_EXTENSIONS.contains(&ext));
    // A bare name like `normalize.css` is a package, not a file path.
    if is_asset && path.contains('/') {
        return Target::Asset;
    }
    if is_relative(path) {
        return join(parent_dir(from), path)
            .and_then(|joined| resolve(&joined, files))
            .map_or(Target::Unresolved, Target::File);
    }
    if let Some(rest) = path.strip_prefix("@/").or_else(|| path.strip_prefix("~/")) {
        return resolve_alias(from, rest, files).map_or(Target::Unresolved, Target::File);
    }
    if path.starts_with('/') {
        return Target::Unresolved;
    }
    let (name, subpath) = package_name(path);
    match packages.iter().find(|package| package.name == name) {
        Some(package) => resolve_workspace(package, subpath, files),
        None => Target::Package(name),
    }
}

fn ancestors(dir: &str) -> impl Iterator<Item = &str> {
    std::iter::successors(Some(dir), |current| {
        (!current.is_empty()).then(|| parent_dir(current))
    })
}

fn resolve_alias(from: &str, rest: &str, files: &BTreeSet<&str>) -> Option<String> {
    let under_src = format!("src/{rest}");
    ancestors(parent_dir(from)).find_map(|dir| {
        resolve(&under(dir, &under_src), files).or_else(|| resolve(&under(dir, rest), files))
    })
}

fn package_name(path: &str) -> (String, &str) {
    let Some((first, after_first)) = path.split_once('/') else {
        return (path.to_string(), "");
    };
    if !first.starts_with('@') {
        return (first.to_string(), after_first.trim_start_matches('/'));
    }
    let (second, subpath) = after_first.split_once('/').unwrap_or((after_first, ""));
    (format!("{first}/{second}"), subpath.trim_start_matches('/'))
}

fn resolve_workspace(package: &Package, subpath: &str, files: &BTreeSet<&str>) -> Target {
    let candidates: Vec<String> = if subpath.is_empty() {
        package
            .main
            .iter()
            .map(|main| main.trim_start_matches("./"))
            .chain(["src/index", "index"])
            .map(|candidate| under(&package.dir, candidate))
            .collect()
    } else {
        [subpath.to_string(), format!("src/{subpath}")]
            .iter()
            .map(|candidate| under(&package.dir, candidate))
            .collect()
    };
    // A workspace package that only ships built files has nothing to link; it is still not external.
    candidates
        .iter()
        .find_map(|candidate| resolve(candidate, files))
        .map_or(Target::Asset, Target::File)
}

pub(crate) fn join(dir: &str, rel: &str) -> Option<String> {
    let joined = format!("{dir}/{rel}");
    let mut out: Vec<&str> = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                out.pop()?;
            }
            name => out.push(name),
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
        .map(|swapped_ext| format!("{stem}.{swapped_ext}"))
        .chain(
            EXTENSIONS
                .iter()
                .map(|code_ext| format!("{base}.{code_ext}")),
        )
        .chain(
            EXTENSIONS
                .iter()
                .map(|code_ext| format!("{base}/index.{code_ext}")),
        )
        .chain([format!("{base}.d.ts"), format!("{base}/index.d.ts")]);
    candidates
        .into_iter()
        .find(|candidate| files.contains(candidate.as_str()))
}
