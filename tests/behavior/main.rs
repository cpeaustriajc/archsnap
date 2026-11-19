use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use assert_cmd::Command as Bin;
use predicates::prelude::*;
use serde_json::Value;
use tempfile::TempDir;

const WEEK_37: &str = "2026-09-08T12:00:00+00:00";
const WEEK_38: &str = "2026-09-15T12:00:00+00:00";

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

struct Repo {
    dir: TempDir,
}

impl Repo {
    fn new() -> TestResult<Self> {
        let repo = Repo {
            dir: TempDir::new()?,
        };
        repo.git(&["init", "-q", "-b", "main"], WEEK_37)?;
        Ok(repo)
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, rel: &str, contents: &str) -> TestResult<&Self> {
        let p = self.path().join(rel);
        fs::create_dir_all(p.parent().ok_or("path has no parent folder")?)?;
        fs::write(p, contents)?;
        Ok(self)
    }

    fn commit(&self, msg: &str, date: &str) -> TestResult<&Self> {
        self.git(&["add", "-A"], date)?;
        self.git(&["commit", "-q", "-m", msg], date)?;
        Ok(self)
    }

    fn git(&self, args: &[&str], date: &str) -> TestResult {
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .current_dir(self.path())
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date)
            .status()?;
        assert!(status.success(), "git {args:?} failed");
        Ok(())
    }

    fn out(&self) -> PathBuf {
        self.path().join(".earnest")
    }

    fn snapshot(&self, week: &str) -> TestResult<Value> {
        let p = self.out().join("snapshots").join(format!("{week}.json"));
        Ok(serde_json::from_str(&fs::read_to_string(&p)?)?)
    }
}

fn earnest(dir: &Path) -> Bin {
    let mut cmd = Bin::new(env!("CARGO_BIN_EXE_earnest"));
    cmd.current_dir(dir).env("NO_COLOR", "1").arg("run");
    cmd
}

fn two_week_repo() -> TestResult<Repo> {
    let repo = Repo::new()?;
    repo.write(
        "src/a/index.ts",
        "import { b } from '../b/util.js';\nexport const a = b;\n",
    )?
    .write("src/b/util.ts", "export const b = 1;\n")?
    .write("node_modules/pkg/index.js", "module.exports = 1;\n")?
    .write("src/types.d.ts", "declare const x: number;\n")?
    .commit("week 37", WEEK_37)?;
    repo.write(
        "src/c/main.tsx",
        "import React from 'react';\nimport { a } from '../a';\nexport const c = () => a;\n",
    )?
    .commit("week 38", WEEK_38)?;
    Ok(repo)
}

#[test]
fn builds_weekly_snapshots_and_report() -> TestResult {
    let repo = two_week_repo()?;

    earnest(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("[1/2]"))
        .stderr(predicate::str::contains("[2/2]"));

    let w37 = repo.snapshot("2026-W37")?;
    let w38 = repo.snapshot("2026-W38")?;
    assert!(w37["modules"].get("src/a").is_some());
    assert!(w37["modules"].get("src/c").is_none());
    assert!(w38["modules"].get("src/c").is_some());
    assert!(
        w38["modules"].get("node_modules/pkg").is_none(),
        "node_modules must be ignored"
    );
    assert_eq!(
        w38["stats"]["files"], 3,
        ".d.ts and node_modules are not counted"
    );

    let edges = w38["edges"].as_array().unwrap();
    let has = |from: &str, to: &str| edges.iter().any(|e| e["from"] == from && e["to"] == to);
    assert!(
        has("src/a", "src/b"),
        ".js specifier resolves to the .ts file"
    );
    assert!(
        has("src/c", "src/a"),
        "directory import resolves to index.ts"
    );
    assert_eq!(w38["externals"]["react"], 1);

    let html = fs::read_to_string(repo.out().join("index.html")).unwrap();
    assert!(html.contains("2026-W37") && html.contains("2026-W38"));
    assert!(html.contains("src/c"));
    Ok(())
}

#[test]
fn second_run_reuses_cached_snapshots() -> TestResult {
    let repo = two_week_repo()?;
    earnest(repo.path()).assert().success();
    earnest(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("cached"));
    Ok(())
}

#[test]
fn not_a_git_repo_fails_with_exit_2() {
    let dir = TempDir::new().unwrap();
    earnest(dir.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("earnest(not_git_repo)"))
        .stderr(predicate::str::contains("--repo"));
}

#[test]
fn repo_without_commits_fails_with_exit_2() -> TestResult {
    let repo = Repo::new()?;
    earnest(repo.path())
        .assert()
        .code(2)
        .stderr(predicate::str::contains("earnest(no_commits)"));
    Ok(())
}

#[test]
fn unwritable_output_fails_with_exit_3_before_scanning() -> TestResult {
    let repo = two_week_repo()?;
    let blocker = repo.path().join("not-a-dir");
    fs::write(&blocker, "file in the way").unwrap();
    earnest(repo.path())
        .arg("--out")
        .arg(&blocker)
        .assert()
        .code(3)
        .stderr(predicate::str::contains("earnest(output_not_writable)"))
        .stderr(predicate::str::contains("[1/").not());
    Ok(())
}

#[test]
fn syntax_error_skips_file_and_points_at_it() -> TestResult {
    let repo = two_week_repo()?;
    repo.write("src/b/broken.ts", "export function f( {\n  return 1\n")?
        .commit("broken", WEEK_38)?;

    earnest(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("earnest(parse_skipped)"))
        .stderr(predicate::str::contains("src/b/broken.ts:"));

    let w38 = repo.snapshot("2026-W38")?;
    assert_eq!(w38["stats"]["skipped"], 1);
    assert_eq!(w38["stats"]["files"], 3, "the broken file is not counted");
    Ok(())
}

#[test]
fn strict_turns_warnings_into_exit_1_but_still_writes_report() -> TestResult {
    let repo = two_week_repo()?;
    repo.write("src/b/broken.ts", "export function f( {\n")?
        .commit("broken", WEEK_38)?;

    earnest(repo.path()).arg("--strict").assert().code(1);
    assert!(repo.out().join("index.html").exists());
    Ok(())
}

#[test]
fn unresolved_relative_import_warns_with_specifier() -> TestResult {
    let repo = two_week_repo()?;
    repo.write(
        "src/c/extra.ts",
        "import { x } from './missing';\nexport const y = x;\n",
    )?
    .commit("unresolved", WEEK_38)?;

    earnest(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("earnest(unresolved_import)"))
        .stderr(predicate::str::contains("./missing"));

    assert_eq!(repo.snapshot("2026-W38")?["stats"]["unresolved"], 1);
    Ok(())
}

#[test]
fn shallow_clone_degrades_to_current_snapshot_only() -> TestResult {
    let origin = two_week_repo()?;
    let parent = TempDir::new().unwrap();
    let url = format!("file://{}", origin.path().display());
    let status = Command::new("git")
        .args(["clone", "-q", "--depth", "1", &url, "clone"])
        .current_dir(parent.path())
        .status()
        .unwrap();
    assert!(status.success());
    let clone = parent.path().join("clone");

    earnest(&clone)
        .assert()
        .success()
        .stderr(predicate::str::contains("earnest(shallow_clone)"))
        .stderr(predicate::str::contains("fetch-depth: 0"));

    let snaps: Vec<_> = fs::read_dir(clone.join(".earnest/snapshots"))
        .unwrap()
        .collect();
    assert_eq!(snaps.len(), 1);
    Ok(())
}

#[test]
fn corrupt_cached_snapshot_is_rebuilt() -> TestResult {
    let repo = two_week_repo()?;
    earnest(repo.path()).assert().success();
    let p = repo.out().join("snapshots/2026-W37.json");
    fs::write(&p, "{ not json").unwrap();

    earnest(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("earnest(snapshot_corrupt)"));
    assert!(repo.snapshot("2026-W37")?["modules"].get("src/a").is_some());
    Ok(())
}

#[test]
fn jsx_in_plain_js_files_parses() -> TestResult {
    let repo = two_week_repo()?;
    repo.write(
        "src/ui/button.js",
        "import { a } from '../a';\nexport const B = () => <div>{a}</div>;\n",
    )?
    .commit("jsx in js", WEEK_38)?;

    earnest(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("parse_skipped").not());
    assert!(
        repo.snapshot("2026-W38")?["modules"]
            .get("src/ui")
            .is_some()
    );
    Ok(())
}

#[test]
fn imports_of_committed_but_unscanned_files_are_not_unresolved() -> TestResult {
    let repo = two_week_repo()?;
    let big = format!("export const x = 1;\n{}", "// padding\n".repeat(60_000));
    repo.write("src/big/huge.ts", &big)?
        .write("src/c/types.d.ts", "export type T = number;\n")?
        .write("src/c/Widget.vue", "<template><div/></template>\n")?
        .write(
            "src/c/uses.ts",
            "import type { T } from './types';\nimport { x } from '../big/huge';\nimport W from './Widget.vue';\nexport const u: T = x;\n",
        )?
        .commit("unscanned targets", WEEK_38)?;

    earnest(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("unresolved_import").not());
    assert_eq!(repo.snapshot("2026-W38")?["stats"]["unresolved"], 0);
    Ok(())
}

#[test]
fn symlinks_are_not_parsed_as_source() -> TestResult {
    let repo = two_week_repo()?;
    std::os::unix::fs::symlink("../a/index.ts", repo.path().join("src/b/link.ts")).unwrap();
    repo.commit("symlink", WEEK_38)?;

    earnest(repo.path())
        .assert()
        .success()
        .stderr(predicate::str::contains("parse_skipped").not());
    Ok(())
}

#[test]
fn missing_git_object_fails_with_exit_3() -> TestResult {
    let repo = two_week_repo()?;
    let out = Command::new("git")
        .args(["rev-parse", "HEAD:src/b/util.ts"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    let oid = String::from_utf8(out.stdout).unwrap().trim().to_string();
    fs::remove_file(
        repo.path()
            .join(".git/objects")
            .join(&oid[..2])
            .join(&oid[2..]),
    )
    .unwrap();

    earnest(repo.path())
        .assert()
        .code(3)
        .stderr(predicate::str::contains("earnest(git_failed)"));
    Ok(())
}

#[test]
fn zero_depth_and_zero_weeks_are_rejected() -> TestResult {
    let repo = two_week_repo()?;
    earnest(repo.path()).args(["--depth", "0"]).assert().code(2);
    earnest(repo.path()).args(["--weeks", "0"]).assert().code(2);
    Ok(())
}

#[test]
fn first_week_does_not_mark_everything_new() -> TestResult {
    let repo = two_week_repo()?;
    earnest(repo.path()).assert().success();
    let html = fs::read_to_string(repo.out().join("index.html")).unwrap();
    let data = html
        .split("id=\"earnest-data\" type=\"application/json\">")
        .nth(1)
        .unwrap()
        .split("</script>")
        .next()
        .unwrap();
    let report: Value = serde_json::from_str(data).unwrap();
    let oldest = report["weeks"].as_array().unwrap().last().unwrap();
    assert_eq!(oldest["diff"]["added_modules"].as_array().unwrap().len(), 0);
    assert_eq!(oldest["diff"]["added_edges"].as_array().unwrap().len(), 0);
    Ok(())
}

fn monorepo() -> TestResult<Repo> {
    let repo = Repo::new()?;
    repo.write("package.json", r#"{ "name": "shop-root", "private": true }"#)?
        .write(
            "apps/api/package.json",
            r#"{ "name": "@shop/api", "dependencies": { "hono": "4", "stripe": "18", "@shop/core": "workspace:*" } }"#,
        )?
        .write(
            "apps/api/wrangler.toml",
            r#"name = "shop"
main = "src/index.ts"
[assets]
directory = "../web/dist"
[[d1_databases]]
binding = "DB"
database_name = "shopdb"
[triggers]
crons = ["0 6 * * *"]
"#,
        )?
        .write(
            "apps/api/src/index.ts",
            "import { Hono } from 'hono';\nimport { formatPrice, type Money } from '@shop/core';\n// docs: https://docs.ignored.dev/guide\nexport const rates = () => fetch('https://api.rates-provider.io/v1/latest');\nexport const p = (m: Money) => formatPrice(m);\n",
        )?
        .write(
            "apps/web/package.json",
            r#"{ "name": "@shop/web", "dependencies": { "react": "19", "@shop/core": "workspace:*" }, "devDependencies": { "vite": "7" } }"#,
        )?
        .write("apps/web/src/main.tsx", "import { formatPrice } from '@shop/core';\nexport const App = () => <p>{formatPrice(1)}</p>;\n")?
        .write("apps/web/src/components/cart.tsx", "import { App } from '../main';\nexport const Cart = () => <App />;\n")?
        .write("packages/core/package.json", r#"{ "name": "@shop/core", "main": "src/index.ts" }"#)?
        .write(
            "packages/core/src/index.ts",
            "export type Money = number;\nexport const formatPrice = (m: Money) => `$${m}`;\n",
        )?
        .commit("shop", WEEK_37)?;
    Ok(repo)
}

fn system(snapshot: &Value) -> TestResult<(Vec<Value>, Vec<Value>)> {
    let s = &snapshot["system"];
    Ok((
        s["nodes"]
            .as_array()
            .ok_or("system.nodes is not an array")?
            .clone(),
        s["edges"]
            .as_array()
            .ok_or("system.edges is not an array")?
            .clone(),
    ))
}

fn node_id(nodes: &[Value], label_part: &str) -> TestResult<String> {
    for n in nodes {
        if n["label"]
            .as_str()
            .ok_or("node label is not a string")?
            .contains(label_part)
        {
            return Ok(n["id"]
                .as_str()
                .ok_or("node id is not a string")?
                .to_string());
        }
    }
    Err(format!("no node labelled like {label_part}: {nodes:?}").into())
}

#[test]
fn system_map_reads_packages_wrangler_sdks_and_urls() -> TestResult {
    let repo = monorepo()?;
    earnest(repo.path()).assert().success();
    let (nodes, edges) = system(&repo.snapshot("2026-W37")?)?;
    let has_edge = |from: &str, to: &str| edges.iter().any(|e| e["from"] == from && e["to"] == to);

    let worker = node_id(&nodes, "@shop/api")?;
    let web = node_id(&nodes, "@shop/web")?;
    let core = node_id(&nodes, "@shop/core")?;
    let d1 = node_id(&nodes, "D1 shopdb")?;
    let cron = node_id(&nodes, "Daily at 06:00 UTC")?;
    let stripe = node_id(&nodes, "Stripe")?;
    let rates = node_id(&nodes, "rates-provider.io")?;

    assert!(has_edge(&worker, &web), "the Worker serves the web assets");
    assert!(has_edge(&worker, &d1));
    assert!(has_edge(&cron, &worker));
    assert!(has_edge(&worker, &stripe));
    assert!(has_edge(&worker, &rates));
    assert!(has_edge(&web, &core), "workspace dependency is shared code");
    assert!(
        !nodes
            .iter()
            .any(|n| n["label"].as_str().unwrap().contains("docs.ignored.dev")),
        "URLs in comments are not services"
    );
    Ok(())
}

#[test]
fn areas_follow_workspace_packages() -> TestResult {
    let repo = monorepo()?;
    earnest(repo.path()).assert().success();
    let s = repo.snapshot("2026-W37")?;
    for area in [
        "apps/api",
        "apps/web",
        "apps/web/src/components",
        "packages/core",
    ] {
        assert!(
            s["modules"].get(area).is_some(),
            "missing area {area}: {:?}",
            s["modules"]
        );
    }
    Ok(())
}

#[test]
fn workspace_imports_resolve_and_record_what_crosses_the_boundary() -> TestResult {
    let repo = monorepo()?;
    earnest(repo.path()).assert().success();
    let s = repo.snapshot("2026-W37")?;
    assert!(
        s["externals"].get("@shop/core").is_none(),
        "workspace packages are not externals"
    );
    let edge = s["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["from"] == "apps/api" && e["to"] == "packages/core")
        .expect("apps/api -> packages/core edge");
    let link = &edge["links"][0];
    assert_eq!(link["from"], "apps/api/src/index.ts");
    assert_eq!(link["to"], "packages/core/src/index.ts");
    let names: Vec<&str> = link["names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n.as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"formatPrice") && names.contains(&"type Money"),
        "type-only imports are marked: {names:?}"
    );
    Ok(())
}

#[test]
fn weekly_summary_reports_system_changes() -> TestResult {
    let repo = monorepo()?;
    repo.write(
        "apps/api/wrangler.toml",
        r#"name = "shop"
main = "src/index.ts"
[assets]
directory = "../web/dist"
[[d1_databases]]
binding = "DB"
database_name = "shopdb"
[[queues.producers]]
binding = "EMAILS"
queue = "emails"
[triggers]
crons = ["0 6 * * *"]
"#,
    )?
    .commit("add queue", WEEK_38)?;
    earnest(repo.path()).assert().success();
    let html = fs::read_to_string(repo.out().join("index.html")).unwrap();
    assert!(
        html.contains("Added queue emails"),
        "summary should mention the new queue"
    );
    Ok(())
}

fn routed_repo() -> TestResult<Repo> {
    let repo = monorepo()?;
    repo.write(
        "apps/api/src/app.ts",
        "import { Hono } from 'hono';\nimport { rateRoutes } from './routes/rates';\nexport const app = new Hono();\napp.route('/api/rates', rateRoutes);\n",
    )?
    .write(
        "apps/api/src/routes/rates.ts",
        "import { Hono } from 'hono';\nimport { latestRate } from '../rates/latest';\nexport const rateRoutes = new Hono()\n  .get('/', (c) => c.json(latestRate()))\n  .post('/:id/refresh', (c) => c.json({}));\n",
    )?
    .write(
        "apps/api/src/rates/latest.ts",
        "export const latestRate = () => fetch('https://api.rates-provider.io/v1/latest');\n",
    )?
    .write(
        "apps/api/src/index.ts",
        "import { app } from './app';\nimport { latestRate } from './rates/latest';\nexport default {\n  fetch: app.fetch,\n  async scheduled() { await latestRate(); },\n};\n",
    )?
    .write(
        "apps/web/src/main.tsx",
        "export const load = (id: string) => fetch(`/api/rates/${id}/refresh`, { method: 'POST' });\nexport const App = () => <p>{String(load)}</p>;\n",
    )?
    .commit("routes", WEEK_38)?;
    Ok(repo)
}

#[test]
fn api_routes_are_read_with_their_mount_prefix() -> TestResult {
    let repo = routed_repo()?;
    earnest(repo.path()).assert().success();
    let s = repo.snapshot("2026-W38")?;
    let routes: Vec<String> = s["system"]["routes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            format!(
                "{} {} {}",
                r["method"].as_str().unwrap(),
                r["path"].as_str().unwrap(),
                r["file"].as_str().unwrap()
            )
        })
        .collect();
    assert!(
        routes.contains(&"GET /api/rates apps/api/src/routes/rates.ts".to_string()),
        "{routes:?}"
    );
    assert!(
        routes.contains(&"POST /api/rates/:id/refresh apps/api/src/routes/rates.ts".to_string()),
        "{routes:?}"
    );
    Ok(())
}

#[test]
fn client_calls_are_matched_to_api_routes() -> TestResult {
    let repo = routed_repo()?;
    earnest(repo.path()).assert().success();
    let s = repo.snapshot("2026-W38")?;
    let calls = s["system"]["route_calls"].as_array().unwrap();
    assert!(
        calls.iter().any(|c| c["file"] == "apps/web/src/main.tsx"
            && c["method"] == "POST"
            && c["path"] == "/api/rates/:id/refresh"),
        "{calls:?}"
    );
    Ok(())
}

#[test]
fn worker_scheduled_handler_is_found() -> TestResult {
    let repo = routed_repo()?;
    earnest(repo.path()).assert().success();
    let s = repo.snapshot("2026-W38")?;
    let handlers = s["system"]["handlers"].as_array().unwrap();
    assert!(
        handlers
            .iter()
            .any(|h| h["kind"] == "scheduled" && h["file"] == "apps/api/src/index.ts"),
        "{handlers:?}"
    );
    Ok(())
}

fn outside_calls_repo() -> TestResult<Repo> {
    let repo = Repo::new()?;
    repo.write(
        "package.json",
        r#"{ "name": "site", "dependencies": { "react": "19" } }"#,
    )?
    .write(
        "src/calls.ts",
        [
            "export const a = () => fetch('https://api.example.com/v1/items');",
            "export const b = () => fetch('http://169.254.169.254/latest/meta-data');",
            "export const c = () => fetch('http://10.0.0.5/health');",
            "export const d = () => fetch('http://192.168.1.20:8080/x');",
            "export const e = () => fetch('http://172.20.0.3/x');",
            "export const f = () => fetch('http://127.0.0.1:3000/x');",
            "export const g = () => fetch('https://hooks.slack.com/services/T0');",
            "export const h = () => fetch('https://1.1.1.1/dns-query');",
            "export const i = () => fetch('https://api.buffer.com/1/profiles.json');",
            "",
        ]
        .join("\n")
        .as_str(),
    )?
    .commit("calls", WEEK_37)?;
    Ok(repo)
}

fn external_labels(snapshot: &Value) -> TestResult<Vec<String>> {
    let (nodes, _) = system(snapshot)?;
    Ok(nodes
        .iter()
        .filter(|n| n["kind"] == "external")
        .map(|n| {
            n["label"]
                .as_str()
                .map(str::to_string)
                .ok_or("node label is not a string")
        })
        .collect::<Result<_, _>>()?)
}

#[test]
fn reserved_domains_and_private_ips_are_not_outside_services() -> TestResult {
    let repo = outside_calls_repo()?;
    earnest(repo.path()).assert().success();
    let labels = external_labels(&repo.snapshot("2026-W37")?)?;
    for gone in [
        "example.com",
        "169.254.169.254",
        "10.0.0.5",
        "192.168.1.20",
        "172.20.0.3",
        "127.0.0.1",
    ] {
        assert!(
            !labels.iter().any(|l| l.contains(gone)),
            "{gone} is not a service: {labels:?}"
        );
    }
    for kept in ["slack.com", "1.1.1.1", "buffer.com"] {
        assert!(
            labels.iter().any(|l| l.contains(kept)),
            "{kept} is a service: {labels:?}"
        );
    }
    Ok(())
}

#[test]
fn snapshots_cached_by_an_older_schema_are_rebuilt() -> TestResult {
    let repo = outside_calls_repo()?;
    earnest(repo.path()).assert().success();
    let p = repo.out().join("snapshots").join("2026-W37.json");
    let mut stale = repo.snapshot("2026-W37")?;
    stale["schema"] = Value::from(stale["schema"].as_u64().unwrap() - 1);
    stale["system"]["nodes"] = Value::Array(vec![]);
    fs::write(&p, serde_json::to_string(&stale).unwrap()).unwrap();
    earnest(repo.path()).assert().success();
    assert!(
        !external_labels(&repo.snapshot("2026-W37")?)?.is_empty(),
        "the stale snapshot should be rescanned"
    );
    Ok(())
}

fn embedded_brands(repo: &Repo) -> TestResult<Value> {
    let html = fs::read_to_string(repo.out().join("index.html"))?;
    let json = html
        .split("id=\"earnest-brands\" type=\"application/json\">")
        .nth(1)
        .ok_or("no brands block in the report")?
        .split("</script>")
        .next()
        .ok_or("split always yields a first part")?;
    Ok(serde_json::from_str(json)?)
}

#[test]
fn outside_services_named_like_a_brand_get_its_logo_and_unused_logos_stay_out() -> TestResult {
    let repo = outside_calls_repo()?;
    earnest(repo.path()).assert().success();
    let icons = &embedded_brands(&repo)?["icons"];
    assert!(
        icons.get("buffer").is_some(),
        "buffer.com should carry the Buffer logo"
    );
    assert!(
        icons.get("stripe").is_some(),
        "the hand-picked logos are always there"
    );
    assert!(
        icons.get("zoom").is_none(),
        "logos no service uses are left out"
    );
    Ok(())
}
