use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::diag::{Source, Warning};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct System {
    pub nodes: Vec<SysNode>,
    pub edges: Vec<SysEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SysNode {
    pub id: String,
    /// One of: client, web, mobile, service, library, datastore, queue, schedule, external.
    pub kind: String,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SysEdge {
    pub from: String,
    pub to: String,
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct Package {
    pub name: String,
    pub dir: String,
    pub main: Option<String>,
    pub deps: BTreeSet<String>,
}

pub fn parse_package(path: &str, text: &str, warnings: &mut Vec<Warning>) -> Option<Package> {
    let json: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(e) => {
            warnings.push(config_warning(path, text, e.line(), e.column(), &e.to_string()));
            return None;
        }
    };
    let dir = path.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
    let mut deps = BTreeSet::new();
    for key in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(obj) = json.get(key).and_then(|v| v.as_object()) {
            deps.extend(obj.keys().cloned());
        }
    }
    Some(Package {
        name: json.get("name").and_then(|v| v.as_str()).unwrap_or(&dir).to_string(),
        dir,
        main: json.get("main").or_else(|| json.get("module")).and_then(|v| v.as_str()).map(str::to_string),
        deps,
    })
}

/// The package whose folder holds `path`; the root package has dir "".
pub fn package_of<'a>(packages: &'a [Package], path: &str) -> Option<&'a Package> {
    packages
        .iter()
        .filter(|p| p.dir.is_empty() || path.starts_with(&format!("{}/", p.dir)))
        .max_by_key(|p| p.dir.len())
}

pub struct Inputs<'a> {
    pub packages: &'a [Package],
    /// (path, text) of each wrangler.toml / wrangler.json / wrangler.jsonc.
    pub wranglers: Vec<(String, String)>,
    /// (dir, name) of each Xcode project.
    pub xcode: Vec<(String, String)>,
    /// (file, host) of each outside URL found in string literals.
    pub urls: Vec<(String, String)>,
}

const SERVER_FRAMEWORKS: &[(&str, &str)] = &[
    ("hono", "Hono"), ("express", "Express"), ("fastify", "Fastify"), ("@nestjs/core", "NestJS"),
    ("koa", "Koa"), ("elysia", "Elysia"), ("@hapi/hapi", "hapi"), ("@trpc/server", "tRPC"),
    ("graphql-yoga", "GraphQL Yoga"), ("@apollo/server", "Apollo Server"),
];
const WEB_FRAMEWORKS: &[(&str, &str)] = &[
    ("next", "Next.js"), ("nuxt", "Nuxt"), ("@remix-run/react", "Remix"), ("astro", "Astro"),
    ("@sveltejs/kit", "SvelteKit"), ("react", "React"), ("vue", "Vue"), ("svelte", "Svelte"),
    ("solid-js", "Solid"), ("@angular/core", "Angular"), ("vite", "Vite"),
];
const MOBILE_FRAMEWORKS: &[(&str, &str)] = &[("react-native", "React Native"), ("expo", "Expo")];

/// SDK label -> the domain its API lives on, so URL hosts merge into the SDK's box.
const SDK_DOMAINS: &[(&str, &str)] = &[
    ("Stripe", "stripe.com"), ("Resend", "resend.com"), ("PlanetScale", "planetscale.com"), ("OpenAI", "openai.com"),
    ("Anthropic", "anthropic.com"), ("Sentry", "sentry.io"), ("Upstash Redis", "upstash.io"), ("PostHog", "posthog.com"),
    ("Twilio", "twilio.com"), ("SendGrid", "sendgrid.com"), ("Clerk", "clerk.com"), ("Supabase", "supabase.co"),
    ("Algolia", "algolia.net"), ("Firebase", "firebaseio.com"), ("Auth0", "auth0.com"),
];
/// Each subdomain on these hosting platforms is a different service, so they are not grouped by domain.
const PLATFORMS: &[&str] = &[
    "appspot.com", "herokuapp.com", "vercel.app", "netlify.app", "workers.dev", "pages.dev", "github.io", "fly.dev",
    "onrender.com", "azurewebsites.net", "cloudfront.net", "amazonaws.com", "web.app", "firebaseapp.com", "run.app",
];
const PLACEHOLDERS: &[&str] = &[
    "a.com", "b.com", "c.com", "acme.com", "acme.co", "foo.com", "bar.com", "test.com", "domain.com", "mydomain.com",
    "yourdomain.com", "yourwebsite.com", "your-server.com", "newdomain.com", "website.com", "mysite.com", "company.com",
];

/// npm package prefix -> (node kind, label).
const SDKS: &[(&str, &str, &str)] = &[
    ("pg", "datastore", "PostgreSQL"), ("postgres", "datastore", "PostgreSQL"),
    ("@neondatabase/serverless", "datastore", "Neon PostgreSQL"), ("mysql2", "datastore", "MySQL"),
    ("better-sqlite3", "datastore", "SQLite"), ("@libsql/client", "datastore", "libSQL / Turso"),
    ("mongodb", "datastore", "MongoDB"), ("mongoose", "datastore", "MongoDB"),
    ("redis", "datastore", "Redis"), ("ioredis", "datastore", "Redis"), ("@upstash/redis", "datastore", "Upstash Redis"),
    ("@planetscale/database", "datastore", "PlanetScale"), ("@supabase/supabase-js", "external", "Supabase"),
    ("firebase", "external", "Firebase"), ("firebase-admin", "external", "Firebase"),
    ("@aws-sdk/client-s3", "datastore", "AWS S3"), ("@aws-sdk/client-dynamodb", "datastore", "AWS DynamoDB"),
    ("@aws-sdk/client-sqs", "queue", "AWS SQS"), ("kafkajs", "queue", "Kafka"), ("amqplib", "queue", "RabbitMQ"),
    ("bullmq", "queue", "BullMQ (Redis)"), ("stripe", "external", "Stripe"), ("@stripe/stripe-js", "external", "Stripe"),
    ("@sentry/", "external", "Sentry"), ("posthog-js", "external", "PostHog"), ("posthog-node", "external", "PostHog"),
    ("openai", "external", "OpenAI"), ("@anthropic-ai/sdk", "external", "Anthropic"), ("resend", "external", "Resend"),
    ("@sendgrid/mail", "external", "SendGrid"), ("twilio", "external", "Twilio"), ("@clerk/", "external", "Clerk"),
    ("@auth0/", "external", "Auth0"), ("algoliasearch", "external", "Algolia"),
];

const IGNORED_HOSTS: &[&str] = &[
    "localhost", "127.0.0.1", "0.0.0.0", "example.com", "example.org", "example.net", "www.w3.org",
    "w3.org", "schema.org", "json-schema.org", "www.example.com",
];

pub fn host_of(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#', ':', '`', '"', '\'', ' ']).next()?.to_ascii_lowercase();
    let valid = host.contains('.') && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    let reserved = [".example", ".local", ".localhost", ".test", ".invalid"].iter().any(|t| host.ends_with(t));
    if !valid || reserved || IGNORED_HOSTS.contains(&host.as_str()) || PLACEHOLDERS.contains(&site_of(&host).as_str()) {
        return None;
    }
    Some(host)
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
        });
        id.to_string()
    }

    fn edge(&mut self, from: &str, to: &str, label: &str) {
        if from != to {
            self.edges.insert(SysEdge { from: from.into(), to: to.into(), label: label.into() });
        }
    }
}

fn pkg_id(p: &Package) -> String {
    format!("pkg:{}", if p.dir.is_empty() { "." } else { &p.dir })
}

fn found<'a>(deps: &BTreeSet<String>, table: &'a [(&str, &str)]) -> Vec<&'a str> {
    table.iter().filter(|(dep, _)| deps.contains(*dep)).map(|(_, label)| *label).collect()
}

pub fn detect(inputs: Inputs, warnings: &mut Vec<Warning>) -> System {
    let mut b = Builder::default();
    let pkgs = inputs.packages;
    let names: BTreeSet<&str> = pkgs.iter().map(|p| p.name.as_str()).collect();
    let monorepo = pkgs.len() > 1;

    for p in pkgs {
        let server = found(&p.deps, SERVER_FRAMEWORKS);
        let web = found(&p.deps, WEB_FRAMEWORKS);
        let mobile = found(&p.deps, MOBILE_FRAMEWORKS);
        let is_workspace_root = monorepo && p.dir.is_empty() && server.is_empty() && web.is_empty();
        let config_only = ["tsconfig", "eslint-config", "prettier-config", "tailwind-config", "typescript-config", "biome-config"]
            .iter()
            .any(|c| p.name.ends_with(c));
        if is_workspace_root || config_only {
            continue;
        }
        let shared = monorepo && (p.dir.starts_with("packages/") || p.dir.starts_with("libs/"));
        let web = match web.iter().position(|w| ["Next.js", "Nuxt", "Remix", "Astro", "SvelteKit"].contains(w)) {
            Some(i) => vec![web[i]],
            None => web,
        };
        let (kind, stack) = if !mobile.is_empty() {
            ("mobile", mobile)
        } else if shared {
            ("library", vec![])
        } else if !server.is_empty() {
            ("service", server)
        } else if !web.is_empty() {
            ("web", web)
        } else if p.dir.starts_with("apps/") || p.dir.starts_with("services/") || !monorepo {
            ("service", vec![])
        } else {
            ("library", vec![])
        };
        let where_ = if p.dir.is_empty() { "repo root".to_string() } else { p.dir.clone() };
        let detail = if stack.is_empty() { where_ } else { format!("{}, {where_}", stack.join(" + ")) };
        b.node(&pkg_id(p), kind, &p.name, &detail);
    }

    for p in pkgs {
        let id = pkg_id(p);
        if !b.nodes.contains_key(&id) {
            continue;
        }
        for dep in &p.deps {
            if names.contains(dep.as_str())
                && let Some(target) = pkgs.iter().find(|q| &q.name == dep)
                && b.nodes.contains_key(&pkg_id(target))
            {
                b.edge(&id, &pkg_id(target), "uses code from");
            }
            for (prefix, kind, label) in SDKS {
                let hit = if prefix.ends_with('/') { dep.starts_with(prefix) } else { dep == prefix };
                if hit {
                    let target = b.node(&format!("sdk:{label}"), kind, label, "found in package.json");
                    b.edge(&id, &target, if *kind == "datastore" { "reads and writes" } else { "calls" });
                }
            }
        }
    }

    let mut workers: BTreeMap<String, String> = BTreeMap::new();
    let mut domains: BTreeMap<String, String> = BTreeMap::new();
    for (path, text) in &inputs.wranglers {
        let dir = path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        let Some(cfg) = parse_wrangler(path, text, warnings) else { continue };
        let owner = package_of(pkgs, &format!("{dir}/x")).map(pkg_id).unwrap_or_else(|| format!("pkg:{dir}"));
        let name = cfg.get("name").and_then(|v| v.as_str()).unwrap_or(dir).to_string();
        let detail = match b.nodes.get(&owner) {
            Some(n) => format!("Cloudflare Worker \"{name}\", {}", n.detail),
            None => format!("Cloudflare Worker \"{name}\""),
        };
        let label = b.nodes.get(&owner).map(|n| n.label.clone()).unwrap_or_else(|| name.clone());
        b.nodes.insert(owner.clone(), SysNode { id: owner.clone(), kind: "service".into(), label, detail });
        workers.insert(name.clone(), owner.clone());
        let routes = cfg.get("routes").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        for r in routes.iter().chain(cfg.get("route")) {
            let pattern = r.as_str().or_else(|| r.get("pattern").and_then(|p| p.as_str())).unwrap_or("");
            let host = pattern.split('/').next().unwrap_or("").trim_start_matches("*.").trim_start_matches('*');
            if !host.is_empty() {
                domains.insert(host.to_string(), owner.clone());
            }
        }

        let list = |key: &str| cfg.get(key).and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let field = |v: &serde_json::Value, k: &str| v.get(k).and_then(|x| x.as_str()).map(str::to_string);
        for db in list("d1_databases") {
            if let Some(n) = field(&db, "database_name").or_else(|| field(&db, "binding")) {
                let t = b.node(&format!("d1:{n}"), "datastore", &format!("D1 {n}"), "Cloudflare D1 database");
                b.edge(&owner, &t, "reads and writes");
            }
        }
        for kv in list("kv_namespaces") {
            if let Some(n) = field(&kv, "binding") {
                let t = b.node(&format!("kv:{n}"), "datastore", &format!("KV {n}"), "Cloudflare KV namespace");
                b.edge(&owner, &t, "reads and writes");
            }
        }
        for r2 in list("r2_buckets") {
            if let Some(n) = field(&r2, "bucket_name").or_else(|| field(&r2, "binding")) {
                let t = b.node(&format!("r2:{n}"), "datastore", &format!("R2 {n}"), "Cloudflare R2 bucket");
                b.edge(&owner, &t, "reads and writes");
            }
        }
        if let Some(dobj) = cfg.get("durable_objects").and_then(|d| d.get("bindings")).and_then(|v| v.as_array()) {
            for d in dobj {
                if let Some(n) = field(d, "class_name").or_else(|| field(d, "name")) {
                    let t = b.node(&format!("do:{n}"), "datastore", &format!("Durable Object {n}"), "Cloudflare Durable Object");
                    b.edge(&owner, &t, "reads and writes");
                }
            }
        }
        if let Some(q) = cfg.get("queues") {
            let queues = |k: &str| q.get(k).and_then(|v| v.as_array()).cloned().unwrap_or_default();
            for p in queues("producers") {
                if let Some(n) = field(&p, "queue") {
                    let t = b.node(&format!("queue:{n}"), "queue", &format!("queue {n}"), "Cloudflare Queue");
                    b.edge(&owner, &t, "sends to");
                }
            }
            for c in queues("consumers") {
                if let Some(n) = field(&c, "queue") {
                    let t = b.node(&format!("queue:{n}"), "queue", &format!("queue {n}"), "Cloudflare Queue");
                    b.edge(&t, &owner, "delivers to");
                }
            }
        }
        if let Some(crons) = cfg.get("triggers").and_then(|t| t.get("crons")).and_then(|v| v.as_array()) {
            for c in crons.iter().filter_map(|c| c.as_str()) {
                let t = b.node(&format!("cron:{name}:{c}"), "schedule", &describe_cron(c), &format!("cron {c}"));
                b.edge(&t, &owner, "runs");
            }
        }
        if let Some(assets) = cfg.get("assets").and_then(|a| a.get("directory")).and_then(|v| v.as_str())
            && let Some(target) = crate::scan::join(dir, assets)
            && let Some(web) = package_of(pkgs, &format!("{target}/x")).filter(|p| !p.dir.is_empty() || target.is_empty())
        {
            let web_id = pkg_id(web);
            if web_id != owner && b.nodes.contains_key(&web_id) {
                b.edge(&owner, &web_id, "serves");
                b.edge(&web_id, &owner, "calls (same origin)");
            }
        }
        for s in list("services") {
            if let Some(n) = field(&s, "service") {
                let t = workers.get(&n).cloned().unwrap_or_else(|| b.node(&format!("worker:{n}"), "service", &n, "Cloudflare Worker (not in this repo)"));
                b.edge(&owner, &t, "calls");
            }
        }
    }

    for (dir, name) in &inputs.xcode {
        b.node(&format!("xcode:{dir}/{name}"), "mobile", name, &format!("Apple app (Xcode), {}", if dir.is_empty() { "repo root" } else { dir }));
    }

    let mut mirrors: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (file, host) in &inputs.urls {
        let owner = if file.ends_with(".swift") {
            inputs
                .xcode
                .iter()
                .filter(|(d, _)| d.is_empty() || file.starts_with(&format!("{d}/")))
                .max_by_key(|(d, _)| d.len())
                .or(inputs.xcode.first())
                .map(|(d, n)| format!("xcode:{d}/{n}"))
        } else {
            package_of(pkgs, file).map(pkg_id).filter(|id| b.nodes.contains_key(id))
        };
        let Some(owner) = owner else { continue };
        let worker = host
            .strip_suffix(".workers.dev")
            .and_then(|h| h.split('.').next())
            .and_then(|w| workers.get(w).cloned())
            .or_else(|| domains.get(host.as_str()).cloned());
        let target = match worker {
            Some(w) => w,
            None => {
                let key = service_key(host);
                let sdk = SDK_DOMAINS.iter().find(|(_, d)| *d == key).map(|(label, _)| format!("sdk:{label}"));
                match sdk.filter(|id| b.nodes.contains_key(id)) {
                    Some(id) => id,
                    None => {
                        mirrors.entry(key.clone()).or_default().insert(host.clone());
                        b.node(&format!("host:{key}"), "external", &key, "")
                    }
                }
            }
        };
        b.edge(&owner, &target, "calls");
    }
    for (key, hosts) in mirrors {
        if let Some(n) = b.nodes.get_mut(&format!("host:{key}")) {
            n.detail = hosts.into_iter().collect::<Vec<_>>().join(", ");
        }
    }

    if b.nodes.values().any(|n| n.kind == "web") {
        let browser = b.node("client:browser", "client", "Browser", "people using the web app");
        let webs: Vec<String> = b.nodes.values().filter(|n| n.kind == "web").map(|n| n.id.clone()).collect();
        for w in webs {
            b.edge(&browser, &w, "opens");
        }
    }

    System { nodes: b.nodes.into_values().collect(), edges: b.edges.into_iter().collect() }
}

/// Cloudflare crons run in UTC. Uncommon patterns fall back to the raw expression.
fn describe_cron(c: &str) -> String {
    let f: Vec<&str> = c.split_whitespace().collect();
    if f.len() != 5 {
        return format!("Schedule {c}");
    }
    let num = |s: &str| s.parse::<u32>().ok();
    let at = |m: u32, h: u32| format!("{h:02}:{m:02} UTC");
    const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
    let day = |s: &str| {
        num(s).and_then(|d| DAYS.get(d as usize % 7).map(|d| d.to_string()))
            .or_else(|| DAYS.iter().find(|d| d[..3].eq_ignore_ascii_case(s)).map(|d| d.to_string()))
    };
    match (f[0], f[1], f[2], f[3], f[4]) {
        ("*", "*", "*", "*", "*") => "Every minute".into(),
        (m, "*", "*", "*", "*") if m.starts_with("*/") => format!("Every {} minutes", &m[2..]),
        (m, "*", "*", "*", "*") if num(m).is_some() => format!("Hourly at :{:02}", num(m).unwrap()),
        (m, h, "*", "*", "*") if num(m).is_some() && num(h).is_some() => format!("Daily at {}", at(num(m).unwrap(), num(h).unwrap())),
        (m, h, "*", "*", d) if num(m).is_some() && num(h).is_some() && day(d).is_some() => {
            format!("Weekly on {} at {}", day(d).unwrap(), at(num(m).unwrap(), num(h).unwrap()))
        }
        (m, h, dom, "*", "*") if num(m).is_some() && num(h).is_some() && num(dom).is_some() => {
            format!("Monthly on day {dom} at {}", at(num(m).unwrap(), num(h).unwrap()))
        }
        _ => format!("Schedule {c}"),
    }
}

/// The registrable domain: api.stripe.com -> stripe.com, www.bdo.com.ph -> bdo.com.ph.
fn site_of(host: &str) -> String {
    let parts: Vec<&str> = host.split('.').collect();
    let n = parts.len();
    if n <= 2 {
        return host.to_string();
    }
    let country_second_level =
        parts[n - 1].len() == 2 && ["co", "com", "org", "net", "gov", "edu", "ac"].contains(&parts[n - 2]);
    parts[n - if country_second_level { 3 } else { 2 }..].join(".")
}

fn service_key(host: &str) -> String {
    let site = site_of(host);
    if PLATFORMS.contains(&site.as_str()) { mirror_key(host) } else { site }
}

/// query1.finance.yahoo.com and query2.finance.yahoo.com are one service.
fn mirror_key(host: &str) -> String {
    let (first, rest) = host.split_once('.').unwrap_or((host, ""));
    let trimmed = first.trim_end_matches(|c: char| c.is_ascii_digit());
    let first = if trimmed.is_empty() || trimmed.ends_with('-') && trimmed.len() < 2 { first } else { trimmed };
    if rest.is_empty() { first.to_string() } else { format!("{first}.{rest}") }
}

fn parse_wrangler(path: &str, text: &str, warnings: &mut Vec<Warning>) -> Option<serde_json::Value> {
    if path.ends_with(".toml") {
        return match toml::from_str::<serde_json::Value>(text) {
            Ok(v) => Some(v),
            Err(e) => {
                let (line, col) = e.span().map(|s| line_col(text, s.start)).unwrap_or((1, 1));
                warnings.push(config_warning(path, text, line, col, e.message()));
                None
            }
        };
    }
    match serde_json::from_str(&strip_json_comments(text)) {
        Ok(v) => Some(v),
        Err(e) => {
            warnings.push(config_warning(path, text, e.line(), e.column(), &e.to_string()));
            None
        }
    }
}

fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset.min(text.len())];
    let line = before.matches('\n').count() + 1;
    let col = before.rsplit('\n').next().map(|l| l.chars().count() + 1).unwrap_or(1);
    (line, col)
}

fn config_warning(path: &str, text: &str, line: usize, col: usize, msg: &str) -> Warning {
    let offset: usize = text.split_inclusive('\n').take(line.saturating_sub(1)).map(str::len).sum::<usize>() + col.saturating_sub(1);
    let offset = offset.min(text.len().saturating_sub(1)) as u32;
    Warning::new(
        "config_unreadable",
        format!("can't read {path}: {msg}"),
        "the system map leaves out what this file would add; fix it and re-run",
    )
    .in_file(path, Source { text: text.to_string(), labels: vec![(offset, offset + 1, Some(msg.to_string()))] })
}

/// Drops `//` and `/* */` comments and trailing commas outside strings (wrangler.jsonc).
fn strip_json_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_str = false;
    while let Some(c) = chars.next() {
        if in_str {
            out.push(c);
            if c == '\\' {
                if let Some(n) = chars.next() {
                    out.push(n);
                }
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match (c, chars.peek()) {
            ('"', _) => {
                in_str = true;
                out.push(c);
            }
            ('/', Some('/')) => {
                for n in chars.by_ref() {
                    if n == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for n in chars.by_ref() {
                    if prev == '*' && n == '/' {
                        break;
                    }
                    prev = n;
                }
            }
            _ => out.push(c),
        }
    }
    let mut cleaned = String::with_capacity(out.len());
    let bytes: Vec<char> = out.chars().collect();
    for (i, c) in bytes.iter().enumerate() {
        if *c == ',' && bytes[i + 1..].iter().find(|n| !n.is_whitespace()).is_some_and(|n| *n == '}' || *n == ']') {
            continue;
        }
        cleaned.push(*c);
    }
    cleaned
}

pub fn diff_lines(before: &System, after: &System) -> Vec<String> {
    let label = |s: &System, id: &str| s.nodes.iter().find(|n| n.id == id).map(|n| n.label.clone()).unwrap_or_else(|| id.to_string());
    let describe = |n: &SysNode| match n.kind.as_str() {
        "datastore" | "queue" => n.label.clone(),
        "schedule" => format!("schedule ({})", n.label.to_lowercase()),
        "external" => format!("outside service {}", n.label),
        "library" => format!("shared package {}", n.label),
        "mobile" => format!("app {}", n.label),
        _ => format!("{} {}", if n.kind == "web" { "web app" } else { "service" }, n.label),
    };
    let old: BTreeSet<&str> = before.nodes.iter().map(|n| n.id.as_str()).collect();
    let new: BTreeSet<&str> = after.nodes.iter().map(|n| n.id.as_str()).collect();
    let mut lines = Vec::new();
    for n in after.nodes.iter().filter(|n| !old.contains(n.id.as_str())) {
        lines.push(format!("Added {}.", describe(n)));
    }
    for n in before.nodes.iter().filter(|n| !new.contains(n.id.as_str())) {
        lines.push(format!("Removed {}.", describe(n)));
    }
    let old_e: BTreeSet<_> = before.edges.iter().collect();
    let new_e: BTreeSet<_> = after.edges.iter().collect();
    for e in after.edges.iter().filter(|e| !old_e.contains(e)) {
        if old.contains(e.from.as_str()) && old.contains(e.to.as_str()) {
            lines.push(format!("{} now {} {}.", label(after, &e.from), e.label, label(after, &e.to)));
        }
    }
    for e in before.edges.iter().filter(|e| !new_e.contains(e)) {
        if new.contains(e.from.as_str()) && new.contains(e.to.as_str()) {
            lines.push(format!("{} no longer {} {}.", label(before, &e.from), e.label, label(before, &e.to)));
        }
    }
    lines
}
