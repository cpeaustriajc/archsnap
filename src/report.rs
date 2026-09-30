use serde::Serialize;

use crate::diag::Warning;
use crate::diff::Diff;
use crate::scan::Snapshot;
use crate::system::site_of;

const TEMPLATE: &str = include_str!("report.html");
/// Logos from simple-icons (CC0), fetched by hand; see `source` inside the file.
/// report.html names BRANDS slugs directly; BRANDS_MORE is matched to outside services by domain.
const BRANDS: &str = include_str!("brands.json");
const BRANDS_MORE: &str = include_str!("brands-more.json");

#[derive(Serialize)]
pub struct Report<'a> {
    pub repo: String,
    pub shallow: bool,
    /// Newest first: report.html reads weeks[0] as the latest.
    pub weeks: Vec<WeekView>,
    pub warnings: &'a [Warning],
}

#[derive(Serialize)]
pub struct WeekView {
    pub snapshot: Snapshot,
    pub diff: Diff,
}

pub fn render(report: &Report) -> String {
    let json = serde_json::to_string(report).expect("report serializes");
    // `<` escaped so file names like `</script>` can't end the data block early.
    let json = json.replace('<', "\\u003c");
    // Title first: substituting data first would let repo contents inject a title marker.
    TEMPLATE
        .replace("__ARCHSNAP_TITLE__", &html_escape(&report.repo))
        .replace("__ARCHSNAP_BRANDS__", &brands(report).replace('<', "\\u003c"))
        .replace("__ARCHSNAP_DATA__", &json)
}

/// The hand-picked logos, plus a logo for each outside service whose domain names a simple-icons slug
/// (api.buffer.com -> buffer), under `domains` keyed by the node label.
fn brands(report: &Report) -> String {
    let mut out: serde_json::Value = serde_json::from_str(BRANDS).expect("brands.json parses");
    let more: serde_json::Value = serde_json::from_str(BRANDS_MORE).expect("brands-more.json parses");
    let mut domains = serde_json::Map::new();
    for n in report.weeks.iter().flat_map(|w| &w.snapshot.system.nodes).filter(|n| n.kind == "external") {
        let site = site_of(&n.label);
        let slug: String = site.split('.').next().unwrap_or("").chars().filter(|c| c.is_ascii_alphanumeric()).collect();
        if slug.is_empty() || !n.label.contains('.') {
            continue;
        }
        if let Some(icon) = more["icons"].get(&slug) {
            out["icons"][&slug] = icon.clone();
        } else if out["icons"].get(&slug).is_none() {
            continue;
        }
        domains.insert(n.label.clone(), slug.into());
    }
    out["domains"] = domains.into();
    out.to_string()
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
