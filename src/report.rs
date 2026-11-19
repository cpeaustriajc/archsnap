use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diag::{Fatal, Warning};
use crate::diff::Diff;
use crate::scan::Snapshot;
use crate::system::registrable_domain;

const TEMPLATE: &str = include_str!("report.html");
const BRANDS_NAMED_IN_TEMPLATE: &str = include_str!("brands.json");
const BRANDS_MATCHED_BY_DOMAIN: &str = include_str!("brands-more.json");

#[derive(Serialize)]
pub(crate) struct Report<'a> {
    pub(crate) repo: String,
    pub(crate) shallow: bool,
    pub(crate) weeks: Vec<WeekView>,
    pub(crate) warnings: &'a [Warning],
}

#[derive(Serialize)]
pub(crate) struct WeekView {
    pub(crate) snapshot: Snapshot,
    pub(crate) diff: Diff,
}

#[derive(Deserialize, Serialize)]
struct Brands {
    #[serde(default)]
    domains: BTreeMap<String, String>,
    icons: serde_json::Map<String, serde_json::Value>,
    source: String,
}

pub(crate) fn render(report: &Report) -> Result<String, Fatal> {
    let json = serde_json::to_string(report)
        .map_err(|error| Fatal::internal("encoding the report data", &error))?;
    let titled_before_repo_content =
        TEMPLATE.replace("__EARNEST_TITLE__", &html_escape(&report.repo));
    Ok(titled_before_repo_content
        .replace(
            "__EARNEST_BRANDS__",
            &escape_script_end(&brands_with_outside_service_logos(report)?),
        )
        .replace("__EARNEST_DATA__", &escape_script_end(&json)))
}

fn escape_script_end(json: &str) -> String {
    json.replace('<', "\\u003c")
}

fn brands_with_outside_service_logos(report: &Report) -> Result<String, Fatal> {
    let mut out: Brands = serde_json::from_str(BRANDS_NAMED_IN_TEMPLATE)
        .map_err(|error| Fatal::internal("parsing brands.json", &error))?;
    let more: Brands = serde_json::from_str(BRANDS_MATCHED_BY_DOMAIN)
        .map_err(|error| Fatal::internal("parsing brands-more.json", &error))?;
    for node in report
        .weeks
        .iter()
        .flat_map(|week| &week.snapshot.system.nodes)
        .filter(|node| node.kind == "external")
    {
        let Some(slug) = brand_slug(&node.label) else {
            continue;
        };
        let has_icon = match more.icons.get(&slug) {
            Some(icon) => {
                out.icons.insert(slug.clone(), icon.clone());
                true
            }
            None => out.icons.contains_key(&slug),
        };
        if has_icon {
            out.domains.insert(node.label.clone(), slug);
        }
    }
    serde_json::to_string(&out).map_err(|error| Fatal::internal("encoding the brands", &error))
}

fn brand_slug(label: &str) -> Option<String> {
    let site = registrable_domain(label);
    let first_label = site
        .split_once('.')
        .map_or(site.as_str(), |(first, _)| first);
    let slug: String = first_label
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect();
    (!slug.is_empty() && label.contains('.')).then_some(slug)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
