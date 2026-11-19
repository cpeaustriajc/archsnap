use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::diag::{Fatal, Warning};
use crate::diff::Diff;
use crate::scan::Snapshot;
use crate::system::site_of;

const TEMPLATE: &str = include_str!("report.html");
/// Logos from simple-icons (CC0), fetched by hand; see `source` inside the file.
/// report.html names BRANDS slugs directly; `BRANDS_MORE` is matched to outside services by domain.
const BRANDS: &str = include_str!("brands.json");
const BRANDS_MORE: &str = include_str!("brands-more.json");

#[derive(Serialize)]
pub(crate) struct Report<'a> {
    pub(crate) repo: String,
    pub(crate) shallow: bool,
    /// Newest first: report.html reads weeks[0] as the latest.
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
    // `<` escaped so file names like `</script>` can't end the data block early.
    let escaped_json = json.replace('<', "\\u003c");
    // Title first: substituting data first would let repo contents inject a title marker.
    Ok(TEMPLATE
        .replace("__EARNEST_TITLE__", &html_escape(&report.repo))
        .replace(
            "__EARNEST_BRANDS__",
            &brands(report)?.replace('<', "\\u003c"),
        )
        .replace("__EARNEST_DATA__", &escaped_json))
}

/// The hand-picked logos, plus a logo for each outside service whose domain names a simple-icons slug
/// (api.buffer.com -> buffer), under `domains` keyed by the node label.
fn brands(report: &Report) -> Result<String, Fatal> {
    let mut out: Brands = serde_json::from_str(BRANDS)
        .map_err(|error| Fatal::internal("parsing brands.json", &error))?;
    let more: Brands = serde_json::from_str(BRANDS_MORE)
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
    let site = site_of(label);
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
