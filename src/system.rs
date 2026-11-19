use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::diag::{Source, Warning};
use crate::weekday::index_in_sunday_first_week;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub(crate) struct System {
    pub(crate) nodes: Vec<SysNode>,
    pub(crate) edges: Vec<SysEdge>,
    #[serde(default)]
    pub(crate) routes: Vec<Route>,
    #[serde(default)]
    pub(crate) route_calls: Vec<RouteCall>,
    #[serde(default)]
    pub(crate) handlers: Vec<WorkerHandler>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct Route {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) file: String,
    pub(crate) service: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct RouteCall {
    pub(crate) from: String,
    pub(crate) file: String,
    pub(crate) method: String,
    pub(crate) path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct WorkerHandler {
    pub(crate) kind: String,
    pub(crate) file: String,
    pub(crate) service: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RouteDefinition {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) router_variable: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RouterMount {
    pub(crate) parent_router: Option<String>,
    pub(crate) prefix: String,
    pub(crate) child_router: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RequestedPath {
    pub(crate) pattern_with_stars: String,
    pub(crate) method: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CodeFacts {
    pub(crate) routes: Vec<RouteDefinition>,
    pub(crate) mounts: Vec<RouterMount>,
    pub(crate) paths: Vec<RequestedPath>,
    pub(crate) handlers: Vec<String>,
    pub(crate) exports: Vec<String>,
}

fn segments(path: &str) -> Vec<&str> {
    path.split(['?', '#'])
        .next()
        .unwrap_or("")
        .split('/')
        .filter(|s| !s.is_empty())
        .collect()
}

fn is_param(seg: &str) -> bool {
    seg == "*" || seg.starts_with(':') || seg.starts_with('[') || seg.starts_with('{')
}

fn path_matches(client: &str, server: &str) -> bool {
    let (c, s) = (segments(client), segments(server));
    c.len() == s.len()
        && !c.is_empty()
        && c.iter()
            .zip(&s)
            .all(|(a, b)| a == b || is_param(a) || is_param(b))
}

fn join_route(prefix: &str, path: &str) -> String {
    let joined = format!("/{}/{}", prefix.trim_matches('/'), path.trim_matches('/'));
    let parts: Vec<&str> = joined.split('/').filter(|s| !s.is_empty()).collect();
    format!("/{}", parts.join("/"))
}

fn next_file_route(file: &str, package_dir: &str) -> Option<String> {
    let in_package = if package_dir.is_empty() {
        file
    } else {
        file.strip_prefix(&format!("{package_dir}/"))?
    };
    let relative = in_package.strip_prefix("src/").unwrap_or(in_package);
    let (dir, name) = relative.rsplit_once('/')?;
    let raw_segments = match dir
        .strip_prefix("app")
        .filter(|_| name.starts_with("route."))
    {
        Some(app_dir) => app_router_segments(app_dir),
        None => pages_api_segments(dir, name)?,
    };
    let route_segments: Vec<String> = raw_segments.into_iter().map(route_param).collect();
    Some(format!("/{}", route_segments.join("/")))
}

fn app_router_segments(app_dir: &str) -> Vec<String> {
    app_dir
        .split('/')
        .filter(|s| !s.is_empty())
        .filter(|s| !(s.starts_with('(') && s.ends_with(')')))
        .map(str::to_string)
        .collect()
}

fn pages_api_segments(dir: &str, name: &str) -> Option<Vec<String>> {
    let api_dir = dir
        .strip_prefix("pages/api")
        .map(|rest| format!("api{rest}"))?;
    let stem = name.split('.').next()?;
    let mut found: Vec<String> = api_dir
        .split('/')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();
    if stem != "index" {
        found.push(stem.to_string());
    }
    Some(found)
}

fn route_param(segment: String) -> String {
    match segment
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
    {
        Some(param) => format!(":{}", param.trim_start_matches("...")),
        None => segment,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct SysNode {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) label: String,
    pub(crate) detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SysEdge {
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) label: String,
}

#[derive(Debug, Clone)]
pub(crate) struct Package {
    pub(crate) name: String,
    pub(crate) dir: String,
    pub(crate) main: Option<String>,
    pub(crate) deps: BTreeSet<String>,
}

pub(crate) fn parse_package(
    path: &str,
    text: &str,
    warnings: &mut Vec<Warning>,
) -> Option<Package> {
    let json = match parse_json(text) {
        Ok(value) => value,
        Err(failure) => {
            warnings.push(config_warning(path, text, &failure));
            return None;
        }
    };
    let dir = path
        .rsplit_once('/')
        .map(|(d, _)| d.to_string())
        .unwrap_or_default();
    let mut deps = BTreeSet::new();
    for key in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(obj) = json.get(key).and_then(|v| v.as_object()) {
            deps.extend(obj.keys().cloned());
        }
    }
    Some(Package {
        name: json
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(&dir)
            .to_string(),
        dir,
        main: json
            .get("main")
            .or_else(|| json.get("module"))
            .and_then(|v| v.as_str())
            .map(str::to_string),
        deps,
    })
}

pub(crate) fn owning_package<'a>(packages: &'a [Package], path: &str) -> Option<&'a Package> {
    packages
        .iter()
        .filter(|p| p.dir.is_empty() || path.starts_with(&format!("{}/", p.dir)))
        .max_by_key(|p| p.dir.len())
}

fn package_of_dir<'a>(packages: &'a [Package], dir: &str) -> Option<&'a Package> {
    owning_package(packages, &format!("{dir}/x"))
}

pub(crate) struct WranglerFile {
    pub(crate) path: String,
    pub(crate) text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct XcodeProject {
    pub(crate) dir: String,
    pub(crate) name: String,
}

pub(crate) struct OutsideUrl {
    pub(crate) file: String,
    pub(crate) host: String,
}

pub(crate) struct CodeArea {
    pub(crate) area: String,
    pub(crate) package_dir: String,
}

pub(crate) struct Inputs<'a> {
    pub(crate) packages: &'a [Package],
    pub(crate) wranglers: Vec<WranglerFile>,
    pub(crate) xcode: Vec<XcodeProject>,
    pub(crate) urls: Vec<OutsideUrl>,
    pub(crate) areas: Vec<CodeArea>,
    pub(crate) area_of: &'a dyn Fn(&str) -> String,
    pub(crate) code: Vec<(String, CodeFacts)>,
}

const GENERIC_FOLDERS: &[&str] = &[
    "lib",
    "libs",
    "utils",
    "util",
    "helpers",
    "helper",
    "common",
    "shared",
    "components",
    "hooks",
    "types",
    "styles",
    "test",
    "tests",
    "__tests__",
    "spec",
    "e2e",
    "playwright",
    "cypress",
    "storybook",
    "stories",
    "scripts",
    "config",
    "constants",
    "ui",
    "assets",
    "public",
    "src",
    "middleware",
    "routes",
    "pages",
    "app",
    "server",
    "client",
    "core",
    "db",
    "store",
    "models",
    "schemas",
    "env",
    "fixtures",
    "mocks",
    "generated",
    "icons",
];

fn feature_name(area: &str, package_dir: &str) -> Option<String> {
    if area == package_dir || area.is_empty() {
        return None;
    }
    let last = area.rsplit('/').next()?;
    if GENERIC_FOLDERS.contains(&last.to_ascii_lowercase().as_str())
        || last.starts_with(['(', '[', '_', '.'])
    {
        return None;
    }
    let words = last.replace(['-', '_'], " ");
    if words.len() <= 3 {
        return Some(words.to_ascii_uppercase());
    }
    let mut chars = words.chars();
    chars
        .next()
        .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
}

const SERVER_FRAMEWORKS: &[(&str, &str)] = &[
    ("hono", "Hono"),
    ("express", "Express"),
    ("fastify", "Fastify"),
    ("@nestjs/core", "NestJS"),
    ("koa", "Koa"),
    ("elysia", "Elysia"),
    ("@hapi/hapi", "hapi"),
    ("@trpc/server", "tRPC"),
    ("graphql-yoga", "GraphQL Yoga"),
    ("@apollo/server", "Apollo Server"),
];
const WEB_FRAMEWORKS: &[(&str, &str)] = &[
    ("next", "Next.js"),
    ("nuxt", "Nuxt"),
    ("@remix-run/react", "Remix"),
    ("astro", "Astro"),
    ("@sveltejs/kit", "SvelteKit"),
    ("react", "React"),
    ("vue", "Vue"),
    ("svelte", "Svelte"),
    ("solid-js", "Solid"),
    ("@angular/core", "Angular"),
    ("vite", "Vite"),
];
const MOBILE_FRAMEWORKS: &[(&str, &str)] = &[("react-native", "React Native"), ("expo", "Expo")];
const META_FRAMEWORKS: &[&str] = &["Next.js", "Nuxt", "Remix", "Astro", "SvelteKit"];
const CONFIG_PACKAGE_SUFFIXES: &[&str] = &[
    "tsconfig",
    "eslint-config",
    "prettier-config",
    "tailwind-config",
    "typescript-config",
    "biome-config",
];

type SdkLabel = &'static str;
type ApiDomain = &'static str;
type PackagePrefix = &'static str;
type NodeKind = &'static str;

const SDK_DOMAINS: &[(SdkLabel, ApiDomain)] = &[
    ("Stripe", "stripe.com"),
    ("Resend", "resend.com"),
    ("PlanetScale", "planetscale.com"),
    ("OpenAI", "openai.com"),
    ("Anthropic", "anthropic.com"),
    ("Sentry", "sentry.io"),
    ("Upstash Redis", "upstash.io"),
    ("PostHog", "posthog.com"),
    ("Twilio", "twilio.com"),
    ("SendGrid", "sendgrid.com"),
    ("Clerk", "clerk.com"),
    ("Supabase", "supabase.co"),
    ("Algolia", "algolia.net"),
    ("Firebase", "firebaseio.com"),
    ("Auth0", "auth0.com"),
];
const SHARED_HOSTING_DOMAINS: &[&str] = &[
    "appspot.com",
    "herokuapp.com",
    "vercel.app",
    "netlify.app",
    "workers.dev",
    "pages.dev",
    "github.io",
    "fly.dev",
    "onrender.com",
    "azurewebsites.net",
    "cloudfront.net",
    "amazonaws.com",
    "web.app",
    "firebaseapp.com",
    "run.app",
];
const PLACEHOLDERS: &[&str] = &[
    "a.com",
    "b.com",
    "c.com",
    "acme.com",
    "acme.co",
    "foo.com",
    "bar.com",
    "test.com",
    "domain.com",
    "mydomain.com",
    "yourdomain.com",
    "yourwebsite.com",
    "your-server.com",
    "newdomain.com",
    "website.com",
    "mysite.com",
    "company.com",
];

const SDKS: &[(PackagePrefix, NodeKind, SdkLabel)] = &[
    ("pg", "datastore", "PostgreSQL"),
    ("postgres", "datastore", "PostgreSQL"),
    ("@neondatabase/serverless", "datastore", "Neon PostgreSQL"),
    ("mysql2", "datastore", "MySQL"),
    ("better-sqlite3", "datastore", "SQLite"),
    ("@libsql/client", "datastore", "libSQL / Turso"),
    ("mongodb", "datastore", "MongoDB"),
    ("mongoose", "datastore", "MongoDB"),
    ("redis", "datastore", "Redis"),
    ("ioredis", "datastore", "Redis"),
    ("@upstash/redis", "datastore", "Upstash Redis"),
    ("@planetscale/database", "datastore", "PlanetScale"),
    ("@supabase/supabase-js", "external", "Supabase"),
    ("firebase", "external", "Firebase"),
    ("firebase-admin", "external", "Firebase"),
    ("@aws-sdk/client-s3", "datastore", "AWS S3"),
    ("@aws-sdk/client-dynamodb", "datastore", "AWS DynamoDB"),
    ("@aws-sdk/client-sqs", "queue", "AWS SQS"),
    ("kafkajs", "queue", "Kafka"),
    ("amqplib", "queue", "RabbitMQ"),
    ("bullmq", "queue", "BullMQ (Redis)"),
    ("stripe", "external", "Stripe"),
    ("@stripe/stripe-js", "external", "Stripe"),
    ("@sentry/", "external", "Sentry"),
    ("posthog-js", "external", "PostHog"),
    ("posthog-node", "external", "PostHog"),
    ("openai", "external", "OpenAI"),
    ("@anthropic-ai/sdk", "external", "Anthropic"),
    ("resend", "external", "Resend"),
    ("@sendgrid/mail", "external", "SendGrid"),
    ("twilio", "external", "Twilio"),
    ("@clerk/", "external", "Clerk"),
    ("@auth0/", "external", "Auth0"),
    ("algoliasearch", "external", "Algolia"),
];

const IGNORED_HOSTS: &[&str] = &[
    "localhost",
    "127.0.0.1",
    "0.0.0.0",
    "example.com",
    "example.org",
    "example.net",
    "www.w3.org",
    "w3.org",
    "schema.org",
    "json-schema.org",
    "www.example.com",
];

struct StorageBinding {
    pointer: &'static str,
    name_fields: &'static [&'static str],
    id_prefix: &'static str,
    label_prefix: &'static str,
    detail: &'static str,
}

const STORAGE_BINDINGS: &[StorageBinding] = &[
    StorageBinding {
        pointer: "/d1_databases",
        name_fields: &["database_name", "binding"],
        id_prefix: "d1",
        label_prefix: "D1",
        detail: "Cloudflare D1 database",
    },
    StorageBinding {
        pointer: "/kv_namespaces",
        name_fields: &["binding"],
        id_prefix: "kv",
        label_prefix: "KV",
        detail: "Cloudflare KV namespace",
    },
    StorageBinding {
        pointer: "/r2_buckets",
        name_fields: &["bucket_name", "binding"],
        id_prefix: "r2",
        label_prefix: "R2",
        detail: "Cloudflare R2 bucket",
    },
    StorageBinding {
        pointer: "/durable_objects/bindings",
        name_fields: &["class_name", "name"],
        id_prefix: "do",
        label_prefix: "Durable Object",
        detail: "Cloudflare Durable Object",
    },
];

const HTTP_METHODS: &[&str] = &["GET", "POST", "PUT", "PATCH", "DELETE"];
const TEST_MARKERS: &[&str] = &[
    "tests/",
    "test/",
    "__tests__/",
    "e2e/",
    ".test.",
    ".spec.",
    "tests.swift",
    "test.swift",
];
const MOUNT_PASSES: usize = 4;

pub(crate) fn host_of(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest
        .split(['/', '?', '#', ':', '`', '"', '\'', ' '])
        .next()?
        .to_ascii_lowercase();
    let valid = host.contains('.')
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    let reserved = [".example", ".local", ".localhost", ".test", ".invalid"]
        .iter()
        .any(|t| host.ends_with(t))
        || ["example.com", "example.org", "example.net"]
            .contains(&registrable_domain(&host).as_str())
        || is_non_public_ipv4(&host);
    if !valid
        || reserved
        || IGNORED_HOSTS.contains(&host.as_str())
        || PLACEHOLDERS.contains(&registrable_domain(&host).as_str())
    {
        return None;
    }
    Some(host)
}

fn is_non_public_ipv4(host: &str) -> bool {
    let Ok(ip) = host.parse::<std::net::Ipv4Addr>() else {
        return false;
    };
    ip.is_loopback() || ip.is_private() || ip.is_link_local() || ip.is_unspecified()
}

fn has_extension(path: &str, extension: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, found)| found == extension)
}

#[derive(Default)]
struct Builder {
    nodes: BTreeMap<String, SysNode>,
    edges: BTreeSet<SysEdge>,
}

impl Builder {
    fn node(&mut self, id: &str, kind: &str, label: &str, detail: &str) -> String {
        self.nodes.entry(id.to_string()).or_insert_with(|| SysNode {
            id: id.to_string(),
            kind: kind.to_string(),
            label: label.to_string(),
            detail: detail.to_string(),
            parent: None,
        });
        id.to_string()
    }

    fn edge(&mut self, from: &str, to: &str, label: &str) {
        if from != to {
            self.edges.insert(SysEdge {
                from: from.into(),
                to: to.into(),
                label: label.into(),
            });
        }
    }

    fn has(&self, id: &str) -> bool {
        self.nodes.contains_key(id)
    }

    fn kind_of(&self, id: &str) -> &str {
        self.nodes.get(id).map_or("", |n| n.kind.as_str())
    }
}

fn package_id(dir: &str) -> String {
    format!("pkg:{}", if dir.is_empty() { "." } else { dir })
}

fn pkg_id(package: &Package) -> String {
    package_id(&package.dir)
}

fn xcode_id(dir: &str, name: &str) -> String {
    format!("xcode:{dir}/{name}")
}

fn location(dir: &str) -> &str {
    if dir.is_empty() { "repo root" } else { dir }
}

fn found<'a>(deps: &BTreeSet<String>, table: &'a [(&str, &str)]) -> Vec<&'a str> {
    table
        .iter()
        .filter(|(dep, _)| deps.contains(*dep))
        .map(|(_, label)| *label)
        .collect()
}

#[derive(Default)]
struct Workers {
    by_name: BTreeMap<String, String>,
    by_domain: BTreeMap<String, String>,
    mains: Vec<(String, String)>,
}

impl Workers {
    fn serving(&self, host: &str) -> Option<String> {
        host.strip_suffix(".workers.dev")
            .and_then(|h| h.split('.').next())
            .and_then(|worker| self.by_name.get(worker))
            .or_else(|| self.by_domain.get(host))
            .cloned()
    }
}

pub(crate) fn detect(inputs: Inputs, warnings: &mut Vec<Warning>) -> System {
    let Inputs {
        packages,
        wranglers,
        xcode,
        urls,
        areas,
        area_of,
        code,
    } = inputs;
    let mut graph = Builder::default();
    add_package_nodes(&mut graph, packages);
    add_package_edges(&mut graph, packages);
    let workers = add_workers(&mut graph, packages, &wranglers, warnings);
    add_xcode_apps(&mut graph, &xcode);
    let features = add_features(&mut graph, &areas);
    let url_sources = UrlSources {
        packages,
        xcode: &xcode,
        workers: &workers,
        features: &features,
        area_of,
    };
    add_url_calls(&mut graph, &url_sources, &urls);
    add_browser(&mut graph);
    let (routes, route_calls, handlers) = http(&graph, packages, &xcode, &code, &workers.mains);
    System {
        nodes: graph.nodes.into_values().collect(),
        edges: graph.edges.into_iter().collect(),
        routes,
        route_calls,
        handlers,
    }
}

fn add_package_nodes(graph: &mut Builder, packages: &[Package]) {
    let monorepo = packages.len() > 1;
    for package in packages {
        if let Some((kind, stack)) = classify(package, monorepo) {
            let detail = package_detail(&stack, &package.dir);
            graph.node(&pkg_id(package), kind, &package.name, &detail);
        }
    }
}

fn package_detail(stack: &[&str], dir: &str) -> String {
    let place = location(dir);
    if stack.is_empty() {
        place.to_string()
    } else {
        format!("{}, {place}", stack.join(" + "))
    }
}

fn classify(package: &Package, monorepo: bool) -> Option<(&'static str, Vec<&'static str>)> {
    let server = found(&package.deps, SERVER_FRAMEWORKS);
    let web = web_stack(found(&package.deps, WEB_FRAMEWORKS));
    let mobile = found(&package.deps, MOBILE_FRAMEWORKS);
    let workspace_root = monorepo && package.dir.is_empty() && server.is_empty() && web.is_empty();
    let config_only = CONFIG_PACKAGE_SUFFIXES
        .iter()
        .any(|suffix| package.name.ends_with(suffix));
    if workspace_root || config_only {
        return None;
    }
    let shared =
        monorepo && (package.dir.starts_with("packages/") || package.dir.starts_with("libs/"));
    let app_folder = package.dir.starts_with("apps/") || package.dir.starts_with("services/");
    let classified = if !mobile.is_empty() {
        ("mobile", mobile)
    } else if shared {
        ("library", vec![])
    } else if !server.is_empty() {
        ("service", server)
    } else if !web.is_empty() {
        ("web", web)
    } else if app_folder || !monorepo {
        ("service", vec![])
    } else {
        ("library", vec![])
    };
    Some(classified)
}

fn web_stack(web: Vec<&'static str>) -> Vec<&'static str> {
    match web.iter().find(|w| META_FRAMEWORKS.contains(w)) {
        Some(meta) => vec![*meta],
        None => web,
    }
}

fn add_package_edges(graph: &mut Builder, packages: &[Package]) {
    for package in packages {
        let id = pkg_id(package);
        if !graph.has(&id) {
            continue;
        }
        for dep in &package.deps {
            link_dependency(graph, packages, &id, dep);
        }
    }
}

fn link_dependency(graph: &mut Builder, packages: &[Package], id: &str, dep: &str) {
    if let Some(target) = packages
        .iter()
        .find(|other| other.name == dep)
        .map(pkg_id)
        .filter(|target| graph.has(target))
    {
        graph.edge(id, &target, "uses code from");
    }
    for (_, kind, label) in SDKS.iter().filter(|(prefix, ..)| sdk_matches(prefix, dep)) {
        let target = graph.node(
            &format!("sdk:{label}"),
            kind,
            label,
            "found in package.json",
        );
        let verb = if *kind == "datastore" {
            "reads and writes"
        } else {
            "calls"
        };
        graph.edge(id, &target, verb);
    }
}

fn sdk_matches(prefix: &str, dep: &str) -> bool {
    if prefix.ends_with('/') {
        dep.starts_with(prefix)
    } else {
        dep == prefix
    }
}

fn add_workers(
    graph: &mut Builder,
    packages: &[Package],
    wranglers: &[WranglerFile],
    warnings: &mut Vec<Warning>,
) -> Workers {
    let mut workers = Workers::default();
    for WranglerFile { path, text } in wranglers {
        if let Some(config) = parse_wrangler(path, text, warnings) {
            add_worker(graph, packages, &mut workers, path, &config);
        }
    }
    workers
}

fn add_worker(
    graph: &mut Builder,
    packages: &[Package],
    workers: &mut Workers,
    path: &str,
    config: &Value,
) {
    let dir = path.rsplit_once('/').map_or("", |(d, _)| d);
    let owner = package_of_dir(packages, dir).map_or_else(|| format!("pkg:{dir}"), pkg_id);
    let name = config
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(dir)
        .to_string();
    let package_node = graph.nodes.get(&owner);
    let detail = package_node.map_or_else(
        || format!("Cloudflare Worker \"{name}\""),
        |n| format!("Cloudflare Worker \"{name}\", {}", n.detail),
    );
    let label = package_node.map_or_else(|| name.clone(), |n| n.label.clone());
    graph.nodes.insert(
        owner.clone(),
        SysNode {
            id: owner.clone(),
            kind: "service".into(),
            label,
            detail,
            parent: None,
        },
    );
    workers.by_name.insert(name.clone(), owner.clone());
    if let Some(main) = config
        .get("main")
        .and_then(Value::as_str)
        .and_then(|m| crate::scan::join(dir, m))
    {
        workers.mains.push((owner.clone(), main));
    }
    let routes = array_at(config, "/routes")
        .iter()
        .chain(config.get("route"));
    for host in routes.filter_map(route_host) {
        workers.by_domain.insert(host.to_string(), owner.clone());
    }
    add_storage(graph, &owner, config);
    add_queues(graph, &owner, config);
    add_crons(graph, &owner, &name, config);
    add_static_assets(graph, packages, &owner, dir, config);
    add_service_calls(graph, &workers.by_name, &owner, config);
}

fn array_at<'a>(config: &'a Value, pointer: &str) -> &'a [Value] {
    config
        .pointer(pointer)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn binding_names<'a>(
    config: &'a Value,
    pointer: &str,
    name_fields: &'static [&'static str],
) -> impl Iterator<Item = &'a str> + use<'a> {
    array_at(config, pointer).iter().filter_map(move |binding| {
        name_fields
            .iter()
            .find_map(|field| binding.get(field).and_then(Value::as_str))
    })
}

fn route_host(route: &Value) -> Option<&str> {
    let pattern = route
        .as_str()
        .or_else(|| route.get("pattern").and_then(Value::as_str))?;
    let host = pattern
        .split('/')
        .next()?
        .trim_start_matches("*.")
        .trim_start_matches('*');
    (!host.is_empty()).then_some(host)
}

fn add_storage(graph: &mut Builder, owner: &str, config: &Value) {
    for binding in STORAGE_BINDINGS {
        for name in binding_names(config, binding.pointer, binding.name_fields) {
            let store = graph.node(
                &format!("{}:{name}", binding.id_prefix),
                "datastore",
                &format!("{} {name}", binding.label_prefix),
                binding.detail,
            );
            graph.edge(owner, &store, "reads and writes");
        }
    }
}

fn add_queues(graph: &mut Builder, owner: &str, config: &Value) {
    for name in binding_names(config, "/queues/producers", &["queue"]) {
        let queue = queue_node(graph, name);
        graph.edge(owner, &queue, "sends to");
    }
    for name in binding_names(config, "/queues/consumers", &["queue"]) {
        let queue = queue_node(graph, name);
        graph.edge(&queue, owner, "delivers to");
    }
}

fn queue_node(graph: &mut Builder, name: &str) -> String {
    graph.node(
        &format!("queue:{name}"),
        "queue",
        &format!("queue {name}"),
        "Cloudflare Queue",
    )
}

fn add_crons(graph: &mut Builder, owner: &str, worker: &str, config: &Value) {
    let crons = array_at(config, "/triggers/crons")
        .iter()
        .filter_map(Value::as_str);
    for cron in crons {
        let schedule = graph.node(
            &format!("cron:{worker}:{cron}"),
            "schedule",
            &describe_cron(cron),
            &format!("cron {cron}"),
        );
        graph.edge(&schedule, owner, "runs");
    }
}

fn add_static_assets(
    graph: &mut Builder,
    packages: &[Package],
    owner: &str,
    dir: &str,
    config: &Value,
) {
    let Some(web_id) = served_package(packages, dir, config) else {
        return;
    };
    if web_id != owner && graph.has(&web_id) {
        graph.edge(owner, &web_id, "serves");
        graph.edge(&web_id, owner, "calls (same origin)");
    }
}

fn served_package(packages: &[Package], dir: &str, config: &Value) -> Option<String> {
    let assets = config
        .pointer("/assets/directory")
        .and_then(Value::as_str)?;
    let target = crate::scan::join(dir, assets)?;
    package_of_dir(packages, &target)
        .filter(|p| !p.dir.is_empty() || target.is_empty())
        .map(pkg_id)
}

fn add_service_calls(
    graph: &mut Builder,
    workers: &BTreeMap<String, String>,
    owner: &str,
    config: &Value,
) {
    for service in binding_names(config, "/services", &["service"]) {
        let target = workers.get(service).cloned().unwrap_or_else(|| {
            graph.node(
                &format!("worker:{service}"),
                "service",
                service,
                "Cloudflare Worker (not in this repo)",
            )
        });
        graph.edge(owner, &target, "calls");
    }
}

fn add_xcode_apps(graph: &mut Builder, xcode: &[XcodeProject]) {
    for XcodeProject { dir, name } in xcode {
        graph.node(
            &xcode_id(dir, name),
            "mobile",
            name,
            &format!("Apple app (Xcode), {}", location(dir)),
        );
    }
}

fn add_features(graph: &mut Builder, areas: &[CodeArea]) -> BTreeMap<String, String> {
    let mut features = BTreeMap::new();
    for CodeArea { area, package_dir } in areas {
        let parent = package_id(package_dir);
        let hosts_features = matches!(graph.kind_of(&parent), "service" | "web");
        if !hosts_features {
            continue;
        }
        let Some(label) = feature_name(area, package_dir) else {
            continue;
        };
        let id = format!("feat:{area}");
        let node = SysNode {
            id: id.clone(),
            kind: "feature".into(),
            label,
            detail: area.clone(),
            parent: Some(parent),
        };
        graph.nodes.insert(id.clone(), node);
        features.insert(area.clone(), id);
    }
    features
}

struct UrlSources<'a> {
    packages: &'a [Package],
    xcode: &'a [XcodeProject],
    workers: &'a Workers,
    features: &'a BTreeMap<String, String>,
    area_of: &'a dyn Fn(&str) -> String,
}

fn add_url_calls(graph: &mut Builder, sources: &UrlSources, urls: &[OutsideUrl]) {
    let mut mirrors: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for OutsideUrl { file, host } in urls {
        let Some(owner) = url_owner(graph, sources, file) else {
            continue;
        };
        let target = url_target(graph, sources.workers, &mut mirrors, host);
        graph.edge(&owner, &target, "calls");
        if let Some(feature) = sources.features.get(&(sources.area_of)(file)) {
            graph.edge(feature, &target, "calls");
        }
    }
    for (key, hosts) in mirrors {
        if let Some(node) = graph.nodes.get_mut(&format!("host:{key}")) {
            node.detail = hosts.into_iter().collect::<Vec<_>>().join(", ");
        }
    }
}

fn url_owner(graph: &Builder, sources: &UrlSources, file: &str) -> Option<String> {
    if !has_extension(file, "swift") {
        return package_owner(graph, sources.packages, file);
    }
    sources
        .xcode
        .iter()
        .filter(|project| project.dir.is_empty() || file.starts_with(&format!("{}/", project.dir)))
        .max_by_key(|project| project.dir.len())
        .or(sources.xcode.first())
        .map(|project| xcode_id(&project.dir, &project.name))
}

fn package_owner(graph: &Builder, packages: &[Package], file: &str) -> Option<String> {
    owning_package(packages, file)
        .map(pkg_id)
        .filter(|id| graph.has(id))
}

fn url_target(
    graph: &mut Builder,
    workers: &Workers,
    mirrors: &mut BTreeMap<String, BTreeSet<String>>,
    host: &str,
) -> String {
    if let Some(worker) = workers.serving(host) {
        return worker;
    }
    let key = service_key(host);
    let sdk = SDK_DOMAINS
        .iter()
        .find(|(_, domain)| *domain == key)
        .map(|(label, _)| format!("sdk:{label}"));
    if let Some(id) = sdk.filter(|id| graph.has(id)) {
        return id;
    }
    mirrors
        .entry(key.clone())
        .or_default()
        .insert(host.to_string());
    graph.node(&format!("host:{key}"), "external", &key, "")
}

fn add_browser(graph: &mut Builder) {
    let webs: Vec<String> = graph
        .nodes
        .values()
        .filter(|n| n.kind == "web")
        .map(|n| n.id.clone())
        .collect();
    if webs.is_empty() {
        return;
    }
    let browser = graph.node(
        "client:browser",
        "client",
        "Browser",
        "people using the web app",
    );
    for web in webs {
        graph.edge(&browser, &web, "opens");
    }
}

struct OwnedCode<'a> {
    file: &'a str,
    facts: &'a CodeFacts,
    owner: String,
}

impl OwnedCode<'_> {
    fn route(&self, method: &str, path: String) -> Route {
        Route {
            method: method.to_string(),
            path,
            file: self.file.to_string(),
            service: self.owner.clone(),
        }
    }
}

type RouterPrefixes = BTreeMap<(String, String), String>;

fn http(
    graph: &Builder,
    packages: &[Package],
    xcode: &[XcodeProject],
    code: &[(String, CodeFacts)],
    mains: &[(String, String)],
) -> (Vec<Route>, Vec<RouteCall>, Vec<WorkerHandler>) {
    let owned: Vec<OwnedCode> = code
        .iter()
        .filter_map(|(file, facts)| {
            Some(OwnedCode {
                file,
                facts,
                owner: code_owner(graph, packages, xcode, file)?,
            })
        })
        .collect();
    let prefixes = mount_prefixes(&owned);
    let routes = server_routes(graph, packages, &owned, &prefixes);
    let calls = route_calls(&owned, &routes);
    (routes, calls, worker_handlers(code, mains))
}

fn code_owner(
    graph: &Builder,
    packages: &[Package],
    xcode: &[XcodeProject],
    file: &str,
) -> Option<String> {
    if has_extension(file, "swift") {
        return xcode
            .first()
            .map(|project| xcode_id(&project.dir, &project.name));
    }
    package_owner(graph, packages, file)
}

fn mount_prefixes(code: &[OwnedCode]) -> RouterPrefixes {
    let mut prefixes = RouterPrefixes::new();
    for _ in 0..MOUNT_PASSES {
        let mounts = code
            .iter()
            .flat_map(|unit| unit.facts.mounts.iter().map(|mount| (&unit.owner, mount)));
        for (owner, mount) in mounts {
            let base = router_prefix(&prefixes, owner, mount.parent_router.as_deref());
            prefixes.insert(
                (owner.clone(), mount.child_router.clone()),
                join_route(&base, &mount.prefix),
            );
        }
    }
    prefixes
}

fn router_prefix(prefixes: &RouterPrefixes, owner: &str, router: Option<&str>) -> String {
    router
        .and_then(|r| prefixes.get(&(owner.to_string(), r.to_string())))
        .cloned()
        .unwrap_or_default()
}

fn server_routes(
    graph: &Builder,
    packages: &[Package],
    code: &[OwnedCode],
    prefixes: &RouterPrefixes,
) -> Vec<Route> {
    let mut routes: Vec<Route> = Vec::new();
    for unit in code {
        let kind = graph.kind_of(&unit.owner);
        if kind == "service" {
            routes.extend(router_routes(unit, prefixes));
        }
        if kind == "service" || kind == "web" {
            routes.extend(file_routes(unit, packages));
        }
    }
    routes.sort_by(|left, right| (&left.path, &left.method).cmp(&(&right.path, &right.method)));
    routes.dedup_by(|later, earlier| {
        later.path == earlier.path
            && later.method == earlier.method
            && later.service == earlier.service
    });
    routes
}

fn router_routes(unit: &OwnedCode, prefixes: &RouterPrefixes) -> Vec<Route> {
    unit.facts
        .routes
        .iter()
        .map(|route| {
            let base = router_prefix(prefixes, &unit.owner, route.router_variable.as_deref());
            unit.route(&route.method, join_route(&base, &route.path))
        })
        .collect()
}

fn file_routes(unit: &OwnedCode, packages: &[Package]) -> Vec<Route> {
    let package_dir = owning_package(packages, unit.file).map_or("", |p| p.dir.as_str());
    let Some(path) = next_file_route(unit.file, package_dir) else {
        return Vec::new();
    };
    let pages_api = unit.file.contains("/pages/api/") || unit.file.starts_with("pages/api/");
    let exported = unit
        .facts
        .exports
        .iter()
        .map(String::as_str)
        .filter(|export| HTTP_METHODS.contains(export));
    pages_api
        .then_some("ANY")
        .into_iter()
        .chain(exported)
        .map(|method| unit.route(method, path.clone()))
        .collect()
}

fn is_test_file(file: &str) -> bool {
    let lower = file.to_ascii_lowercase();
    TEST_MARKERS.iter().any(|marker| lower.contains(marker))
}

fn route_calls(code: &[OwnedCode], routes: &[Route]) -> Vec<RouteCall> {
    let mut calls: Vec<RouteCall> = Vec::new();
    let found_calls = code
        .iter()
        .filter(|unit| !is_test_file(unit.file))
        .flat_map(|unit| calls_from(unit, routes));
    for call in found_calls {
        if !calls.contains(&call) {
            calls.push(call);
        }
    }
    calls
}

fn calls_from<'a>(unit: &'a OwnedCode, routes: &'a [Route]) -> impl Iterator<Item = RouteCall> {
    unit.facts.paths.iter().filter_map(move |requested| {
        let route = routes
            .iter()
            .filter(|r| r.file != unit.file && path_matches(&requested.pattern_with_stars, &r.path))
            .find(|r| {
                requested
                    .method
                    .as_ref()
                    .is_none_or(|m| *m == r.method || r.method == "ANY")
            })?;
        Some(RouteCall {
            from: unit.owner.clone(),
            file: unit.file.to_string(),
            method: route.method.clone(),
            path: route.path.clone(),
        })
    })
}

fn worker_handlers(code: &[(String, CodeFacts)], mains: &[(String, String)]) -> Vec<WorkerHandler> {
    let mut handlers = Vec::new();
    for (owner, main) in mains {
        let Some((file, facts)) = entry_point(code, main) else {
            continue;
        };
        handlers.extend(facts.handlers.iter().map(|kind| WorkerHandler {
            kind: kind.clone(),
            file: file.clone(),
            service: owner.clone(),
        }));
    }
    handlers
}

fn entry_point<'a>(code: &'a [(String, CodeFacts)], main: &str) -> Option<&'a (String, CodeFacts)> {
    let stem = main.rsplit_once('.').map_or(main, |(s, _)| s);
    code.iter().find(|(file, _)| {
        file == main
            || file
                .rsplit_once('.')
                .is_some_and(|(file_stem, _)| file_stem == stem)
    })
}

const WEEKDAYS: [(&str, &str); 7] = [
    ("Sun", "Sunday"),
    ("Mon", "Monday"),
    ("Tue", "Tuesday"),
    ("Wed", "Wednesday"),
    ("Thu", "Thursday"),
    ("Fri", "Friday"),
    ("Sat", "Saturday"),
];

fn describe_cron(expression: &str) -> String {
    describe_schedule(expression).unwrap_or_else(|| format!("Schedule {expression}"))
}

fn describe_schedule(expression: &str) -> Option<String> {
    let fields: Vec<&str> = expression.split_whitespace().collect();
    let &[minute, hour, day_of_month, month, weekday] = fields.as_slice() else {
        return None;
    };
    if month != "*" {
        return None;
    }
    match (minute, hour, day_of_month, weekday) {
        ("*", "*", "*", "*") => Some("Every minute".to_string()),
        (_, "*", "*", "*") => every_minutes_or_hourly(minute),
        (_, _, "*", "*") => Some(format!("Daily at {}", clock_time(minute, hour)?)),
        (_, _, "*", _) => Some(format!(
            "Weekly on {} at {}",
            weekday_name(weekday)?,
            clock_time(minute, hour)?
        )),
        (_, _, _, "*") if cron_number(day_of_month).is_some() => Some(format!(
            "Monthly on day {day_of_month} at {}",
            clock_time(minute, hour)?
        )),
        _ => None,
    }
}

fn cron_number(field: &str) -> Option<u32> {
    field.parse().ok()
}

fn every_minutes_or_hourly(minute: &str) -> Option<String> {
    if let Some(step) = minute.strip_prefix("*/") {
        return Some(format!("Every {step} minutes"));
    }
    cron_number(minute).map(|at| format!("Hourly at :{at:02}"))
}

fn clock_time(minute: &str, hour: &str) -> Option<String> {
    let minutes = cron_number(minute)?;
    let hours = cron_number(hour)?;
    Some(format!("{hours:02}:{minutes:02} UTC"))
}

fn weekday_name(field: &str) -> Option<&'static str> {
    if let Some(number) = cron_number(field) {
        let index = usize::try_from(index_in_sunday_first_week(number)).ok()?;
        return WEEKDAYS.get(index).map(|(_, name)| *name);
    }
    WEEKDAYS
        .iter()
        .find(|(abbreviation, _)| abbreviation.eq_ignore_ascii_case(field))
        .map(|(_, name)| *name)
}

pub(crate) fn registrable_domain(host: &str) -> String {
    if host.parse::<std::net::Ipv4Addr>().is_ok() {
        return host.to_string();
    }
    let labels: Vec<&str> = host.rsplit('.').collect();
    match labels.as_slice() {
        [top, second, third, ..] if is_country_second_level(top, second) => {
            format!("{third}.{second}.{top}")
        }
        [top, second, _, ..] => format!("{second}.{top}"),
        _ => host.to_string(),
    }
}

fn is_country_second_level(top: &str, second: &str) -> bool {
    top.len() == 2 && ["co", "com", "org", "net", "gov", "edu", "ac"].contains(&second)
}

fn service_key(host: &str) -> String {
    let site = registrable_domain(host);
    if SHARED_HOSTING_DOMAINS.contains(&site.as_str()) {
        numbered_mirror_key(host)
    } else {
        site
    }
}

fn numbered_mirror_key(host: &str) -> String {
    let (first, rest) = host.split_once('.').unwrap_or((host, ""));
    let without_digits = first.trim_end_matches(|c: char| c.is_ascii_digit());
    let name = if matches!(without_digits, "" | "-") {
        first
    } else {
        without_digits
    };
    if rest.is_empty() {
        name.to_string()
    } else {
        format!("{name}.{rest}")
    }
}

fn parse_wrangler(path: &str, text: &str, warnings: &mut Vec<Warning>) -> Option<Value> {
    let parsed = if has_extension(path, "toml") {
        parse_toml(text)
    } else {
        parse_json(&jsonc_to_json(text))
    };
    match parsed {
        Ok(config) => Some(config),
        Err(failure) => {
            warnings.push(config_warning(path, text, &failure));
            None
        }
    }
}

struct ParseFailure {
    position: TextPosition,
    message: String,
}

fn parse_json(text: &str) -> Result<Value, ParseFailure> {
    serde_json::from_str(text).map_err(|e| ParseFailure {
        position: json_error_position(&e),
        message: e.to_string(),
    })
}

fn parse_toml(text: &str) -> Result<Value, ParseFailure> {
    toml::from_str(text).map_err(|e| ParseFailure {
        position: e.span().map_or_else(TextPosition::default, |span| {
            position_before(text, span.start)
        }),
        message: e.message().to_string(),
    })
}

#[derive(Debug, Clone, Copy, Default)]
struct TextPosition {
    line_index: usize,
    char_column: usize,
}

fn json_error_position(error: &serde_json::Error) -> TextPosition {
    TextPosition {
        line_index: error.line().saturating_sub(1),
        char_column: error.column().saturating_sub(1),
    }
}

fn position_before(text: &str, offset: usize) -> TextPosition {
    let (before, _) = text.split_at(text.floor_char_boundary(offset));
    let current_line = before.rsplit_once('\n').map_or(before, |(_, line)| line);
    TextPosition {
        line_index: before.matches('\n').count(),
        char_column: current_line.chars().count(),
    }
}

fn byte_offset(text: &str, position: TextPosition) -> usize {
    let line_start: usize = text
        .split_inclusive('\n')
        .take(position.line_index)
        .map(str::len)
        .sum();
    line_start
        .saturating_add(position.char_column)
        .min(text.len().saturating_sub(1))
}

fn error_label(offset: usize, message: &str) -> Option<(u32, u32, Option<String>)> {
    let start = u32::try_from(offset).ok()?;
    let end = start.checked_add(1)?;
    Some((start, end, Some(message.to_string())))
}

fn config_warning(path: &str, text: &str, failure: &ParseFailure) -> Warning {
    let message = &failure.message;
    let labels = error_label(byte_offset(text, failure.position), message)
        .into_iter()
        .collect();
    Warning::new(
        "config_unreadable",
        format!("can't read {path}: {message}"),
        "the system map leaves out what this file would add; fix it and re-run",
    )
    .in_file(
        path,
        Source {
            text: text.to_string(),
            labels,
        },
    )
}

fn jsonc_to_json(text: &str) -> String {
    remove_trailing_commas(&remove_comments(text))
}

fn remove_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('"', _) => copy_string(&mut chars, &mut out),
            ('/', Some('/')) => skip_line_comment(&mut chars, &mut out),
            ('/', Some('*')) => skip_block_comment(&mut chars),
            _ => out.push(c),
        }
    }
    out
}

fn copy_string(chars: &mut impl Iterator<Item = char>, out: &mut String) {
    out.push('"');
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '\\' {
            out.extend(chars.next());
        } else if c == '"' {
            return;
        }
    }
}

fn skip_line_comment(chars: &mut impl Iterator<Item = char>, out: &mut String) {
    if chars.any(|c| c == '\n') {
        out.push('\n');
    }
}

fn skip_block_comment(chars: &mut impl Iterator<Item = char>) {
    chars.next();
    let mut previous = ' ';
    for c in chars {
        if previous == '*' && c == '/' {
            return;
        }
        previous = c;
    }
}

fn remove_trailing_commas(text: &str) -> String {
    text.char_indices()
        .filter(|&(at, c)| !(c == ',' && closes_after_comma(text.split_at(at).1)))
        .map(|(_, c)| c)
        .collect()
}

fn closes_after_comma(from_comma: &str) -> bool {
    from_comma
        .chars()
        .skip(1)
        .find(|c| !c.is_whitespace())
        .is_some_and(|c| c == '}' || c == ']')
}

pub(crate) fn diff_lines(before: &System, after: &System) -> Vec<String> {
    let old: BTreeSet<&str> = before.nodes.iter().map(|n| n.id.as_str()).collect();
    let new: BTreeSet<&str> = after.nodes.iter().map(|n| n.id.as_str()).collect();
    let old_edges: BTreeSet<&SysEdge> = before.edges.iter().collect();
    let new_edges: BTreeSet<&SysEdge> = after.edges.iter().collect();
    let added_nodes = after
        .nodes
        .iter()
        .filter(|n| !old.contains(n.id.as_str()))
        .map(|n| format!("Added {}.", describe_node(n)));
    let removed_nodes = before
        .nodes
        .iter()
        .filter(|n| !new.contains(n.id.as_str()))
        .map(|n| format!("Removed {}.", describe_node(n)));
    let added_edges = after
        .edges
        .iter()
        .filter(|e| !old_edges.contains(e) && connects(&old, e))
        .map(|e| {
            let (from, to) = (node_label(after, &e.from), node_label(after, &e.to));
            format!("{from} now {} {to}.", e.label)
        });
    let removed_edges = before
        .edges
        .iter()
        .filter(|e| !new_edges.contains(e) && connects(&new, e))
        .map(|e| {
            let (from, to) = (node_label(before, &e.from), node_label(before, &e.to));
            format!("{from} no longer {} {to}.", e.label)
        });
    added_nodes
        .chain(removed_nodes)
        .chain(added_edges)
        .chain(removed_edges)
        .collect()
}

fn connects(ids: &BTreeSet<&str>, edge: &SysEdge) -> bool {
    ids.contains(edge.from.as_str()) && ids.contains(edge.to.as_str())
}

fn node_label(system: &System, id: &str) -> String {
    system
        .nodes
        .iter()
        .find(|n| n.id == id)
        .map_or_else(|| id.to_string(), |n| n.label.clone())
}

fn describe_node(node: &SysNode) -> String {
    match node.kind.as_str() {
        "datastore" | "queue" => node.label.clone(),
        "schedule" => format!("schedule ({})", node.label.to_lowercase()),
        "external" => format!("outside service {}", node.label),
        "library" => format!("shared package {}", node.label),
        "feature" => format!("feature {} ({})", node.label, node.detail),
        "mobile" => format!("app {}", node.label),
        "web" => format!("web app {}", node.label),
        _ => format!("service {}", node.label),
    }
}
