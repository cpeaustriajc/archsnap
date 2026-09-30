use serde::Serialize;

use crate::diag::Warning;
use crate::diff::Diff;
use crate::scan::Snapshot;

const TEMPLATE: &str = include_str!("report.html");

#[derive(Serialize)]
pub struct Report<'a> {
    pub repo: String,
    pub shallow: bool,
    /// Newest first: report.html reads weeks[0] as the latest.
    pub weeks: Vec<WeekView<'a>>,
    pub warnings: &'a [Warning],
}

#[derive(Serialize)]
pub struct WeekView<'a> {
    pub snapshot: &'a Snapshot,
    pub diff: Diff,
}

pub fn render(report: &Report) -> String {
    let json = serde_json::to_string(report).expect("report serializes");
    // `<` escaped so file names like `</script>` can't end the data block early.
    let json = json.replace('<', "\\u003c");
    // Title first: substituting data first would let repo contents inject a title marker.
    TEMPLATE.replace("__ARCHSNAP_TITLE__", &html_escape(&report.repo)).replace("__ARCHSNAP_DATA__", &json)
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
