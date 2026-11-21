use crate::discovery::{
    IN_EVERY_MODEL_FILE, Language, is_model_file, is_model_name, is_searched, language_of,
};
use crate::reading::{ModelFile, read};
use crate::run::Bounds;
use crate::select::Target;
use earnest_ast::{Draft, RuleDraft};
use earnest_chart::{ChangeKind, RuleChange, RuleText};
use earnest_check::{Limits, Verdict, explore};
use earnest_ir::{Code, Expr, Model, Op, Rule};
use earnest_semantic::lower_with_earlier;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    Added,
    Removed,
    Loosened,
    Tightened,
    Changed,
    Reworded,
}

impl Kind {
    fn for_the_chart(self) -> ChangeKind {
        match self {
            Self::Added => ChangeKind::Added,
            Self::Removed => ChangeKind::Removed,
            Self::Loosened => ChangeKind::Loosened,
            Self::Tightened => ChangeKind::Tightened,
            Self::Changed => ChangeKind::Changed,
            Self::Reworded => ChangeKind::Reworded,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Wording {
    pub(crate) says: String,
    pub(crate) holds: String,
}

impl Wording {
    fn of(rule: &RuleDraft) -> Self {
        Self {
            says: rule.says.clone(),
            holds: rule.text.clone(),
        }
    }

    fn for_the_chart(&self) -> RuleText<'_> {
        RuleText {
            says: &self.says,
            holds: &self.holds,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Difference {
    pub(crate) file: String,
    pub(crate) model: String,
    pub(crate) id: String,
    pub(crate) change: Kind,
    pub(crate) was: Option<Wording>,
    pub(crate) now: Option<Wording>,
}

impl Difference {
    pub(crate) fn for_the_chart(&self) -> RuleChange<'_> {
        RuleChange {
            file: &self.file,
            model: &self.model,
            id: &self.id,
            change: self.change.for_the_chart(),
            was: self.was.as_ref().map(Wording::for_the_chart),
            now: self.now.as_ref().map(Wording::for_the_chart),
        }
    }
}

fn git(folder: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(folder)
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    command
}

pub(crate) fn check_base(base: &str) -> Result<(), String> {
    if base.is_empty() || base.starts_with('-') {
        return Err(format!("`{base}` is not a git ref"));
    }
    let found = git(Path::new("."))
        .args(["rev-parse", "--verify", "--quiet"])
        .arg(format!("{base}^{{commit}}"))
        .stdout(Stdio::null())
        .status()
        .map_err(|error| format!("`--base` needs `git`, which did not start: {error}"))?;
    if found.success() {
        Ok(())
    } else {
        Err(format!(
            "`{base}` is not a commit of a git repository that holds this folder"
        ))
    }
}

fn nearest_folder(path: &Path) -> (&Path, PathBuf) {
    let mut folder = path.parent().unwrap_or_else(|| Path::new(""));
    while !folder.as_os_str().is_empty() && !folder.is_dir() {
        folder = folder.parent().unwrap_or_else(|| Path::new(""));
    }
    let below = path.strip_prefix(folder).unwrap_or(path).to_path_buf();
    if folder.as_os_str().is_empty() {
        (Path::new("."), below)
    } else {
        (folder, below)
    }
}

fn bytes_at(base: &str, file: &str) -> Option<Vec<u8>> {
    let (folder, below) = nearest_folder(Path::new(file));
    let name = below.to_str()?;
    let shown = git(folder)
        .arg("show")
        .arg(format!("{base}:./{name}"))
        .output()
        .ok()?;
    shown.status.success().then_some(shown.stdout)
}

#[derive(Debug, PartialEq, Eq)]
enum AtBase {
    Absent,
    Unreadable,
    Models(Vec<Draft>),
}

fn understood(language: Language, file: &str, bytes: Vec<u8>) -> AtBase {
    let source = match String::from_utf8(bytes) {
        Ok(source) => source,
        Err(refused) => {
            let lossy = String::from_utf8_lossy(refused.as_bytes());
            return if is_model_file(language, file, &lossy) {
                AtBase::Unreadable
            } else {
                AtBase::Absent
            };
        }
    };
    if !is_model_file(language, file, &source) {
        return AtBase::Absent;
    }
    let reading = read(language, file, &source);
    if reading.problems.is_empty() {
        AtBase::Models(reading.drafts)
    } else {
        AtBase::Unreadable
    }
}

fn at_base(base: &str, language: Language, file: &str) -> AtBase {
    bytes_at(base, file).map_or(AtBase::Absent, |bytes| understood(language, file, bytes))
}

fn names_from(output: &std::process::Output) -> Vec<&str> {
    output
        .stdout
        .split(|byte| *byte == 0)
        .filter_map(|name| std::str::from_utf8(name).ok())
        .filter(|name| !name.is_empty())
        .collect()
}

fn models_listed_at(base: &str, folder: &Path) -> Vec<PathBuf> {
    let mut search = git(folder);
    search.args(["grep", "--files-with-matches", "--null", "--text"]);
    search.args(["--fixed-strings", "-e", IN_EVERY_MODEL_FILE]);
    let mut listed: Vec<PathBuf> = search
        .arg(base)
        .output()
        .map(|output| {
            names_from(&output)
                .into_iter()
                .filter_map(|name| name.strip_prefix(base)?.strip_prefix(':'))
                .map(PathBuf::from)
                .collect()
        })
        .unwrap_or_default();
    let mut tree = git(folder);
    tree.args(["ls-tree", "-r", "--name-only", "-z"]);
    if let Ok(output) = tree.arg(base).args(["--", "."]).output() {
        listed.extend(
            names_from(&output)
                .into_iter()
                .filter(|name| is_model_name(name))
                .map(PathBuf::from),
        );
    }
    listed
}

fn models_only_at_base(base: &str, roots: &[PathBuf], present: &BTreeSet<PathBuf>) -> Vec<PathBuf> {
    let here = [PathBuf::from(".")];
    let roots = if roots.is_empty() { &here } else { roots };
    let mut only_at_base = BTreeSet::new();
    for root in roots.iter().filter(|root| root.is_dir()) {
        let shown = if root == Path::new(".") {
            Path::new("")
        } else {
            root.as_path()
        };
        for below in models_listed_at(base, root) {
            let path = shown.join(&below);
            if is_searched(&below) && !present.contains(&path) {
                only_at_base.insert(path);
            }
        }
    }
    only_at_base.into_iter().collect()
}

fn implies(first: &Rule, second: &Rule, id: String) -> Rule {
    Rule {
        id,
        says: String::new(),
        line: second.line,
        text: String::new(),
        holds: Code::new(Expr::Or(
            Box::new(Expr::Apply(Op::Not, vec![first.holds.expr().clone()])),
            Box::new(second.holds.expr().clone()),
        )),
        slots: first.slots.max(second.slots),
    }
}

fn loosens_id(id: &str) -> String {
    format!("{id} loosens")
}

fn tightens_id(id: &str) -> String {
    format!("{id} tightens")
}

fn broken_rules(model: Model, rules: Vec<Rule>, bounds: Bounds) -> Vec<String> {
    if rules.is_empty() {
        return Vec::new();
    }
    let limits = Limits {
        max_states: bounds.max_states,
        time_limit: bounds.time_limit,
        jobs: bounds.jobs,
        helpers: None,
        keep_graph: false,
        only_rule: None,
    };
    explore(&Model { rules, ..model }, &limits)
        .rules
        .into_iter()
        .filter(|rule| rule.verdict == Verdict::Broken)
        .map(|rule| rule.id)
        .collect()
}

fn by_wording(was: &RuleDraft, now: &RuleDraft) -> Option<Kind> {
    if was.text != now.text {
        Some(Kind::Changed)
    } else if was.says != now.says {
        Some(Kind::Reworded)
    } else {
        None
    }
}

fn told_apart(
    old: &Draft,
    new: &Draft,
    pairs: &[(&RuleDraft, &RuleDraft)],
    bounds: Bounds,
) -> Vec<Option<Kind>> {
    let Ok((model, at_base)) = lower_with_earlier(new, old) else {
        return pairs
            .iter()
            .map(|(was, now)| by_wording(was, now))
            .collect();
    };
    let lowered: Vec<Option<(&Rule, &Rule)>> = pairs
        .iter()
        .map(|(was, now)| {
            let position = old.rules.iter().position(|rule| rule.id == was.id)?;
            let before = at_base.get(position)?.as_ref()?;
            let after = model.rules.iter().find(|rule| rule.id == now.id)?;
            Some((before, after))
        })
        .collect();
    let compared: Vec<Rule> = lowered
        .iter()
        .flatten()
        .filter(|(before, after)| before.holds != after.holds)
        .flat_map(|(before, after)| {
            [
                implies(after, before, loosens_id(&after.id)),
                implies(before, after, tightens_id(&after.id)),
            ]
        })
        .collect();
    let broken = broken_rules(model.clone(), compared, bounds);
    pairs
        .iter()
        .zip(&lowered)
        .map(|((was, now), rules)| {
            let Some((before, after)) = rules else {
                return Some(Kind::Changed);
            };
            if broken.contains(&loosens_id(&now.id)) {
                Some(Kind::Loosened)
            } else if broken.contains(&tightens_id(&now.id)) {
                Some(Kind::Tightened)
            } else if before.holds != after.holds {
                Some(Kind::Changed)
            } else {
                by_wording(was, now)
            }
        })
        .collect()
}

fn one_sided(file: &str, model: &str, rule: &RuleDraft, change: Kind) -> Difference {
    let wording = Some(Wording::of(rule));
    let (was, now) = match change {
        Kind::Removed => (wording, None),
        Kind::Added | Kind::Loosened | Kind::Tightened | Kind::Changed | Kind::Reworded => {
            (None, wording)
        }
    };
    Difference {
        file: file.to_owned(),
        model: model.to_owned(),
        id: rule.id.clone(),
        change,
        was,
        now,
    }
}

pub(crate) fn model_changes(old: Option<&Draft>, new: &Draft, bounds: Bounds) -> Vec<Difference> {
    let old_rules: &[RuleDraft] = old.map_or(&[], |draft| &draft.rules);
    let was_of = |id: &str| old_rules.iter().find(|rule| rule.id == id);
    let kept: Vec<(&RuleDraft, &RuleDraft)> = new
        .rules
        .iter()
        .filter_map(|now| Some((was_of(&now.id)?, now)))
        .collect();
    let kinds = old.map_or_else(Vec::new, |draft| told_apart(draft, new, &kept, bounds));
    let mut changes: Vec<Difference> = new
        .rules
        .iter()
        .filter(|now| was_of(&now.id).is_none())
        .map(|now| one_sided(&new.file, &new.name, now, Kind::Added))
        .collect();
    for ((was, now), kind) in kept.iter().zip(kinds) {
        if let Some(change) = kind {
            changes.push(Difference {
                file: new.file.clone(),
                model: new.name.clone(),
                id: now.id.clone(),
                change,
                was: Some(Wording::of(was)),
                now: Some(Wording::of(now)),
            });
        }
    }
    let position = |id: &str| new.rules.iter().position(|rule| rule.id == id);
    changes.sort_by_key(|change| position(&change.id));
    changes.extend(
        old_rules
            .iter()
            .filter(|was| !new.rules.iter().any(|now| now.id == was.id))
            .map(|was| one_sided(&new.file, &new.name, was, Kind::Removed)),
    );
    changes
}

#[derive(Debug, Default)]
pub(crate) struct Compared {
    pub(crate) changes: Vec<Difference>,
    unreadable_at_base: Vec<String>,
}

impl Compared {
    pub(crate) fn warnings(&self, base: &str) -> Vec<String> {
        self.unreadable_at_base
            .iter()
            .map(|file| {
                format!(
                    "warning: {file} is unreadable at {base}, so its rule changes are not listed"
                )
            })
            .collect()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Compare<'a> {
    pub(crate) base: &'a str,
    pub(crate) roots: &'a [PathBuf],
    pub(crate) files: &'a [ModelFile],
    pub(crate) model: Option<&'a str>,
    pub(crate) rule: Option<&'a str>,
    pub(crate) bounds: Bounds,
}

impl Compare<'_> {
    fn selects(&self, draft: &Draft) -> bool {
        self.model.is_none_or(|name| draft.name == name)
            && self
                .rule
                .is_none_or(|id| draft.rules.iter().any(|rule| rule.id == id))
    }
}

pub(crate) fn removed_with_their_model(
    compare: &Compare<'_>,
    old: &[Draft],
    now: &[&str],
) -> Vec<Difference> {
    old.iter()
        .filter(|draft| !now.contains(&draft.name.as_str()) && compare.selects(draft))
        .flat_map(|draft| {
            draft
                .rules
                .iter()
                .map(|rule| one_sided(&draft.file, &draft.name, rule, Kind::Removed))
        })
        .collect()
}

pub(crate) fn rule_changes(compare: &Compare<'_>, targets: &[Target<'_>]) -> Compared {
    let mut compared = Compared::default();
    for file in compare.files {
        let Some(language) = language_of(Path::new(&file.path)) else {
            continue;
        };
        if !file.problems().is_empty() {
            match at_base(compare.base, language, &file.path) {
                AtBase::Models(drafts) => {
                    compared
                        .changes
                        .extend(removed_with_their_model(compare, &drafts, &[]));
                }
                AtBase::Unreadable => compared.unreadable_at_base.push(file.path.clone()),
                AtBase::Absent => {}
            }
            continue;
        }
        let old = match at_base(compare.base, language, &file.path) {
            AtBase::Unreadable => {
                compared.unreadable_at_base.push(file.path.clone());
                continue;
            }
            AtBase::Absent => Vec::new(),
            AtBase::Models(drafts) => drafts,
        };
        for target in targets
            .iter()
            .filter(|target| target.file.path == file.path)
        {
            let new = &target.readable.draft;
            let was = old.iter().find(|draft| draft.name == new.name);
            compared
                .changes
                .extend(model_changes(was, new, compare.bounds));
        }
        let now: Vec<&str> = file
            .models()
            .iter()
            .map(|readable| readable.draft.name.as_str())
            .collect();
        compared
            .changes
            .extend(removed_with_their_model(compare, &old, &now));
    }
    let present: BTreeSet<PathBuf> = compare
        .files
        .iter()
        .map(|file| PathBuf::from(&file.path))
        .collect();
    for path in models_only_at_base(compare.base, compare.roots, &present) {
        let Some(language) = language_of(&path) else {
            continue;
        };
        let shown = path.to_string_lossy();
        match at_base(compare.base, language, &shown) {
            AtBase::Unreadable => compared.unreadable_at_base.push(shown.into_owned()),
            AtBase::Absent => {}
            AtBase::Models(drafts) => {
                compared
                    .changes
                    .extend(removed_with_their_model(compare, &drafts, &[]));
            }
        }
    }
    compared
}

#[cfg(test)]
mod tests {
    use super::{Compare, Difference, Kind, Wording, git, model_changes, rule_changes};
    use crate::discovery::discover;
    use crate::reading::{ModelFile, load};
    use crate::run::Bounds;
    use crate::select::select_with_removed;
    use earnest_ast::Draft;
    use std::fmt::Write;
    use std::fs;
    use std::path::{Path, PathBuf};

    const FILE: &str = "tank.model.ts";
    const REST_OF_THE_TANK: &str = "const TOP = 3;\n\
        export const Tank = m.model(\"Tank\", {\n\
        \tlevel: m.int(0, 3).initial(0),\n\
        \tvalve: m.enum([\"shut\", \"open\"]).optional().initial(null),\n\
        });\n\
        export const fill = m.action(Tank, \"the tank fills\", {\n\
        \twhen: (s) => s.level < TOP,\n\
        \tchanges: (s) => ({ level: s.level + 1 }),\n\
        });\n\
        export const full = m.end(Tank, \"full\", (s) => s.level === TOP);\n";

    fn tank(rules: &[(&str, &str, &str)]) -> Draft {
        tank_with("", rules)
    }

    fn tank_with(constants_and_helpers: &str, rules: &[(&str, &str, &str)]) -> Draft {
        let mut source = format!(
            "import {{ m }} from \"@earnest/model\";\n{constants_and_helpers}{REST_OF_THE_TANK}"
        );
        for (id, says, holds) in rules {
            writeln!(
                source,
                "export const {id} = m.rule(Tank, \"{says}\", {holds});"
            )
            .unwrap();
        }
        let mut reading = earnest_ts::read(FILE, &source);
        assert!(
            reading.problems.is_empty(),
            "the test model is readable: {:?}",
            reading.problems
        );
        reading.drafts.remove(0)
    }

    fn bounds() -> Bounds {
        Bounds {
            max_states: 1000,
            time_limit: None,
            jobs: 1,
        }
    }

    fn kinds(old: &[(&str, &str, &str)], new: &[(&str, &str, &str)]) -> Vec<(String, Kind)> {
        model_changes(Some(&tank(old)), &tank(new), bounds())
            .into_iter()
            .map(|change| (change.id, change.change))
            .collect()
    }

    fn kind(was: &str, now: &str) -> Kind {
        let found = kinds(&[("rule", "same", was)], &[("rule", "same", now)]);
        assert_eq!(found.len(), 1, "one rule changed: {found:?}");
        found[0].1
    }

    #[test]
    fn a_rule_a_reachable_state_now_passes_and_used_to_break_is_loosened() {
        assert_eq!(
            kind("(s) => s.level <= 2", "(s) => s.level <= 3"),
            Kind::Loosened,
            "level 3 passes the new rule and breaks the old one"
        );
    }

    #[test]
    fn a_rule_a_reachable_state_now_breaks_and_used_to_pass_is_tightened() {
        assert_eq!(
            kind("(s) => s.level >= 0", "(s) => s.level >= 1"),
            Kind::Tightened,
            "level 0 breaks the new rule and passes the old one"
        );
    }

    #[test]
    fn a_rule_that_both_loosens_and_tightens_is_loosened() {
        assert_eq!(
            kind("(s) => s.level <= 1", "(s) => s.level >= 1"),
            Kind::Loosened,
            "levels 2 and 3 loosen it, level 0 tightens it"
        );
    }

    #[test]
    fn a_condition_no_reachable_state_tells_apart_is_changed() {
        assert_eq!(
            kind("(s) => s.level <= 3", "(s) => s.level < 4"),
            Kind::Changed,
            "every level from 0 to 3 passes both"
        );
        assert_eq!(
            kind("(s) => s.level !== 9", "(s) => s.level !== 8"),
            Kind::Changed,
            "a state that would tell them apart is not reachable"
        );
    }

    #[test]
    fn an_old_condition_that_cant_be_read_against_the_new_model_is_changed() {
        assert_eq!(
            kind("(s) => s.depth <= 2", "(s) => s.level <= 3"),
            Kind::Changed,
            "the new model has no field `depth`"
        );
        assert_eq!(
            kind("(s) => s.level <= BRIM", "(s) => s.level <= 3"),
            Kind::Changed,
            "the new file has no constant `BRIM`"
        );
    }

    #[test]
    fn an_unreadable_old_rule_does_not_hide_the_change_of_another() {
        let found = kinds(
            &[
                ("lost", "same", "(s) => s.depth <= 2"),
                ("capped", "same", "(s) => s.level <= 2"),
            ],
            &[
                ("lost", "same", "(s) => s.level <= 3"),
                ("capped", "same", "(s) => s.level <= TOP"),
            ],
        );
        assert_eq!(
            found,
            [
                ("lost".to_owned(), Kind::Changed),
                ("capped".to_owned(), Kind::Loosened)
            ],
            "each rule is judged on its own"
        );
    }

    #[test]
    fn a_state_where_a_condition_has_no_value_tells_nothing_apart() {
        assert_eq!(
            kind(
                "(s) => m.some(s.valve) === \"open\" || s.level <= 3",
                "(s) => s.level <= 3 || m.some(s.valve) === \"open\""
            ),
            Kind::Changed,
            "the old condition reads a null valve in every state"
        );
    }

    #[test]
    fn a_state_where_the_new_condition_has_no_value_is_a_change_too() {
        assert_eq!(
            kind(
                "(s) => s.level <= 3 || s.valve === \"open\"",
                "(s) => m.some(s.valve) === \"open\" || s.level <= 3"
            ),
            Kind::Changed,
            "the new condition reads a null valve where the old one holds"
        );
    }

    #[test]
    fn the_old_condition_may_name_the_state_differently() {
        assert_eq!(
            kind("(tank) => tank.level <= 2", "(s) => s.level <= 3"),
            Kind::Loosened,
            "each condition keeps the name it gives the state"
        );
    }

    #[test]
    fn only_a_new_sentence_is_reworded_and_an_unchanged_rule_is_not_listed() {
        let found = kinds(
            &[
                ("named", "The level is small", "(s) => s.level < 4"),
                ("kept", "The level is never six", "(s) => s.level !== 6"),
            ],
            &[
                ("named", "The level is tiny", "(s) => s.level < 4"),
                ("kept", "The level is never six", "(s) => s.level !== 6"),
            ],
        );
        assert_eq!(found, [("named".to_owned(), Kind::Reworded)]);
    }

    #[test]
    fn added_and_removed_rules_carry_the_side_they_have() {
        let changes = model_changes(
            Some(&tank(&[("gone", "never five", "(s) => s.level !== 5")])),
            &tank(&[("fresh", "never seven", "(s) => s.level !== 7")]),
            bounds(),
        );
        assert_eq!(
            changes,
            [
                Difference {
                    file: "tank.model.ts".to_owned(),
                    model: "Tank".to_owned(),
                    id: "fresh".to_owned(),
                    change: Kind::Added,
                    was: None,
                    now: Some(Wording {
                        says: "never seven".to_owned(),
                        holds: "s.level !== 7".to_owned(),
                    }),
                },
                Difference {
                    file: "tank.model.ts".to_owned(),
                    model: "Tank".to_owned(),
                    id: "gone".to_owned(),
                    change: Kind::Removed,
                    was: Some(Wording {
                        says: "never five".to_owned(),
                        holds: "s.level !== 5".to_owned(),
                    }),
                    now: None,
                },
            ],
            "the new model's rules first, then the removed ones"
        );
    }

    #[test]
    fn a_model_the_base_does_not_have_adds_every_rule() {
        let new = tank(&[
            ("a", "one", "(s) => s.level < 4"),
            ("b", "two", "(s) => s.level >= 0"),
        ]);
        let found: Vec<Kind> = model_changes(None, &new, bounds())
            .into_iter()
            .map(|change| change.change)
            .collect();
        assert_eq!(found, [Kind::Added, Kind::Added]);
    }

    #[test]
    fn a_search_that_stops_at_a_limit_still_reports_what_it_saw() {
        let tight = Bounds {
            max_states: 2,
            ..bounds()
        };
        let old = tank(&[("capped", "same", "(s) => s.level <= 2")]);
        let new = tank(&[("capped", "same", "(s) => s.level <= 3")]);
        let found = model_changes(Some(&old), &new, tight);
        assert_eq!(
            found[0].change,
            Kind::Changed,
            "level 3 was not reached within two states"
        );
    }

    #[test]
    fn a_change_serializes_as_the_json_output_documents() {
        let change = Difference {
            file: "tank.model.ts".to_owned(),
            model: "Cart".to_owned(),
            id: "oneAddAtATime".to_owned(),
            change: Kind::Loosened,
            was: Some(Wording {
                says: "one".to_owned(),
                holds: "s.inFlight <= 1".to_owned(),
            }),
            now: None,
        };
        assert_eq!(
            serde_json::to_value(&change).unwrap(),
            serde_json::json!({
                "file": "tank.model.ts",
                "model": "Cart",
                "id": "oneAddAtATime",
                "change": "loosened",
                "was": { "says": "one", "holds": "s.inFlight <= 1" },
                "now": null,
            })
        );
    }

    fn kinds_across(old_top: &str, new_top: &str, holds: &str) -> Vec<Kind> {
        let rules = [("rule", "same", holds)];
        let old = tank_with(old_top, &rules);
        let new = tank_with(new_top, &rules);
        model_changes(Some(&old), &new, bounds())
            .into_iter()
            .map(|change| change.change)
            .collect()
    }

    #[test]
    fn a_rule_with_the_same_text_is_judged_by_the_constants_of_each_file() {
        let capped = "(s) => s.level <= MAX";
        assert_eq!(
            kinds_across("const MAX = 2;\n", "const MAX = 3;\n", capped),
            [Kind::Loosened],
            "level 3 passes with the new MAX and broke with the old one"
        );
        assert_eq!(
            kinds_across("const MAX = 3;\n", "const MAX = 2;\n", capped),
            [Kind::Tightened],
            "level 3 breaks with the new MAX and passed with the old one"
        );
        assert_eq!(
            kinds_across("const MAX = 5;\n", "const MAX = 7;\n", capped),
            [Kind::Changed],
            "the condition means something else, but no level from 0 to 3 tells them apart"
        );
        assert_eq!(
            kinds_across("const MAX = 2;\n", "const MAX = 2;\n", capped),
            [],
            "nothing changed"
        );
    }

    #[test]
    fn a_rule_with_the_same_text_is_judged_by_the_helpers_of_each_file() {
        let helper = |most: u8| format!("const allowed = (level: number) => level <= {most};\n");
        assert_eq!(
            kinds_across(&helper(2), &helper(3), "(s) => allowed(s.level)"),
            [Kind::Loosened],
            "level 3 passes the new helper and broke the old one"
        );
    }

    #[test]
    fn a_new_sentence_does_not_hide_a_rule_loosened_through_its_constant() {
        let old = tank_with(
            "const MAX = 2;\n",
            &[("rule", "old words", "(s) => s.level <= MAX")],
        );
        let new = tank_with(
            "const MAX = 3;\n",
            &[("rule", "new words", "(s) => s.level <= MAX")],
        );
        let found = model_changes(Some(&old), &new, bounds());
        assert_eq!(found.len(), 1, "one change: {found:?}");
        assert_eq!(found[0].change, Kind::Loosened, "not only reworded");
    }

    #[test]
    fn an_edit_of_white_space_alone_is_not_a_change() {
        assert_eq!(
            kinds(
                &[("rule", "same", "(s) => s.level   <=\n\t\t3")],
                &[("rule", "same", "(s) => s.level <= 3")]
            ),
            [],
            "both readers store a condition with one space for each run of white space"
        );
    }

    #[test]
    fn changes_keep_the_order_of_the_new_file_s_rules() {
        let found = kinds(
            &[
                ("second", "same", "(s) => s.level <= 2"),
                ("third", "old words", "(s) => s.level < 4"),
            ],
            &[
                ("first", "same", "(s) => s.level >= 0"),
                ("second", "same", "(s) => s.level <= 3"),
                ("third", "new words", "(s) => s.level < 4"),
            ],
        );
        assert_eq!(
            found,
            [
                ("first".to_owned(), Kind::Added),
                ("second".to_owned(), Kind::Loosened),
                ("third".to_owned(), Kind::Reworded)
            ]
        );
    }

    fn one_rule_model(name: &str, rule: &str) -> String {
        format!(
            "export const {name} = m.model(\"{name}\", {{ on: m.boolean().initial(false) }});\n\
             export const {rule} = m.rule({name}, \"{rule} of {name}\", (s) => !s.on);\n"
        )
    }

    fn model_file(models: &[(&str, &str)]) -> String {
        let mut source = String::from("import { m } from \"@earnest/model\";\n");
        for (name, rule) in models {
            source.push_str(&one_rule_model(name, rule));
        }
        source
    }

    fn put(root: &Path, relative: &str, text: &str) {
        let target = root.join(relative);
        fs::create_dir_all(target.parent().expect("a file has a folder"))
            .expect("a folder can be made");
        fs::write(target, text).expect("a file can be written");
    }

    fn run_git(root: &Path, arguments: &[&str]) {
        let done = git(root)
            .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
            .args(arguments)
            .output()
            .expect("`git` is on the `PATH`");
        assert!(done.status.success(), "git {arguments:?} worked");
    }

    fn repository_with_a_base_commit() -> tempfile::TempDir {
        let folder = tempfile::tempdir().expect("a temporary folder can be made");
        let root = folder.path();
        put(
            root,
            "kept.model.ts",
            &model_file(&[("Tank", "capped"), ("Lamp", "dim")]),
        );
        put(root, "gone.model.ts", &model_file(&[("Bell", "rings")]));
        put(root, "old/name.model.ts", &model_file(&[("Door", "shut")]));
        put(root, "plain.model.ts", &model_file(&[("Vase", "whole")]));
        put(
            root,
            "node_modules/pkg/a.model.ts",
            &model_file(&[("Dep", "inside")]),
        );
        put(root, "notes.ts", "export const one = 1;\n");
        run_git(root, &["init", "--quiet", "."]);
        run_git(root, &["add", "--all", "--force"]);
        run_git(root, &["commit", "--quiet", "--message", "base"]);
        folder
    }

    fn changes_since_the_base(root: &Path, model: Option<&str>) -> Vec<(String, String, Kind)> {
        differences_since_the_base(root, model)
            .into_iter()
            .map(|change| (change.model, change.id, change.change))
            .collect()
    }

    fn differences_since_the_base(root: &Path, model: Option<&str>) -> Vec<Difference> {
        let roots = [root.to_path_buf()];
        let files: Vec<ModelFile> = discover(&roots)
            .expect("the root exists")
            .found
            .into_iter()
            .map(load)
            .collect();
        let selected = select_with_removed(&files, model, None, Some("HEAD"))
            .expect("the selection names a model or a removed one");
        let targets = selected.targets;
        let compare = Compare {
            base: "HEAD",
            roots: &roots,
            files: &files,
            model,
            rule: None,
            bounds: bounds(),
        };
        let compared = rule_changes(&compare, &targets);
        assert!(
            compared.warnings("HEAD").is_empty(),
            "every file is readable at the base: {:?}",
            compared.warnings("HEAD")
        );
        compared.changes
    }

    fn change(model: &str, id: &str, kind: Kind) -> (String, String, Kind) {
        (model.to_owned(), id.to_owned(), kind)
    }

    #[test]
    fn a_comment_or_white_space_alone_is_not_a_change() {
        let found = kinds(
            &[("rule", "same", "(s) => s.level <= 2")],
            &[(
                "rule",
                "same",
                "(s) =>\n  s.level /* at most */ <= /* two */ 2",
            )],
        );
        assert_eq!(found, [], "the meaning is the same, so nothing is listed");
    }

    #[test]
    fn every_change_names_its_file_so_models_of_one_name_are_told_apart() {
        let folder = repository_with_a_base_commit();
        let root = folder.path();
        put(root, "kept.model.ts", &model_file(&[("Tank", "capped")]));
        fs::remove_dir_all(root.join("old")).expect("a folder can be deleted");
        put(root, "new/name.model.ts", &model_file(&[("Door", "shut")]));
        let doors: Vec<(String, Kind)> = differences_since_the_base(root, Some("Door"))
            .into_iter()
            .map(|change| (change.file, change.change))
            .collect();
        let under = |relative: &str| root.join(relative).to_string_lossy().into_owned();
        assert_eq!(
            doors.len(),
            2,
            "the added and the removed Door: {doors:?} under {}",
            under("")
        );
        assert!(
            doors[0].0.ends_with("new/name.model.ts") && doors[0].1 == Kind::Added,
            "the new file: {doors:?}"
        );
        assert!(
            doors[1].0.ends_with("old/name.model.ts") && doors[1].1 == Kind::Removed,
            "the old file: {doors:?}"
        );
    }

    #[test]
    fn rules_that_went_with_their_model_or_their_file_are_removed() {
        let folder = repository_with_a_base_commit();
        let root = folder.path();
        put(root, "kept.model.ts", &model_file(&[("Tank", "capped")]));
        fs::remove_file(root.join("gone.model.ts")).expect("a file can be deleted");
        fs::remove_dir_all(root.join("old")).expect("a folder can be deleted");
        put(root, "new/name.model.ts", &model_file(&[("Door", "shut")]));
        put(root, "plain.model.ts", "export const one = 1;\n");
        fs::remove_dir_all(root.join("node_modules")).expect("a folder can be deleted");
        assert_eq!(
            changes_since_the_base(root, None),
            [
                change("Lamp", "dim", Kind::Removed),
                change("Door", "shut", Kind::Added),
                change("Vase", "whole", Kind::Removed),
                change("Bell", "rings", Kind::Removed),
                change("Door", "shut", Kind::Removed),
            ],
            "the files of the run in path order (a model gone from a file that stays, the \
             renamed file's model as added, and the models of a `.model.ts` file that holds \
             none any more), then the files only the base has in path order: the deleted file \
             and the old name of the renamed one; a folder the search skips is not listed"
        );
        assert_eq!(
            changes_since_the_base(root, Some("Door")),
            [
                change("Door", "shut", Kind::Added),
                change("Door", "shut", Kind::Removed),
            ],
            "`--model` narrows the removed models as it narrows the checked ones"
        );
    }

    #[test]
    fn a_file_that_lost_every_model_lists_their_rules_as_removed() {
        let folder = repository_with_a_base_commit();
        let root = folder.path();
        put(root, "gone.model.ts", &model_file(&[]));
        let listed = changes_since_the_base(root, None);
        assert_eq!(
            listed,
            [change("Bell", "rings", Kind::Removed)],
            "the file still imports the library and has no model left"
        );
    }

    #[test]
    fn nothing_is_removed_when_nothing_changed() {
        let folder = repository_with_a_base_commit();
        let listed = changes_since_the_base(&PathBuf::from(folder.path()), None);
        assert_eq!(listed, [], "the files are as the base has them");
    }

    #[test]
    fn a_file_that_became_unreadable_lists_its_base_rules_as_removed() {
        let folder = repository_with_a_base_commit();
        let root = folder.path();
        put(
            root,
            "kept.model.ts",
            "import { m } from \"@earnest/model\";\nexport const now = Date.now();\n",
        );
        assert_eq!(
            changes_since_the_base(root, None),
            [
                change("Tank", "capped", Kind::Removed),
                change("Lamp", "dim", Kind::Removed),
            ],
            "the rules the run can no longer read are gone from it, and the file is listed as \
             unreadable beside them"
        );
        assert_eq!(
            changes_since_the_base(root, Some("Lamp")),
            [change("Lamp", "dim", Kind::Removed)],
            "`--model` narrows them as it does the others"
        );
    }

    #[test]
    fn a_model_that_was_deleted_is_listed_when_it_is_named() {
        let folder = repository_with_a_base_commit();
        let root = folder.path();
        fs::remove_file(root.join("gone.model.ts")).expect("a file can be deleted");
        assert_eq!(
            changes_since_the_base(root, Some("Bell")),
            [change("Bell", "rings", Kind::Removed)],
            "no model is named `Bell` now, and the base had one"
        );
    }

    #[test]
    fn a_file_unreadable_at_the_base_and_now_is_warned_of() {
        let folder = tempfile::tempdir().expect("a temporary folder can be made");
        let root = folder.path();
        let broken = "import { m } from \"@earnest/model\";\nexport const now = Date.now();\n";
        put(root, "a.model.ts", broken);
        run_git(root, &["init", "--quiet", "."]);
        run_git(root, &["add", "--all"]);
        run_git(root, &["commit", "--quiet", "--message", "base"]);
        let roots = [root.to_path_buf()];
        let files: Vec<ModelFile> = discover(&roots)
            .expect("the root exists")
            .found
            .into_iter()
            .map(load)
            .collect();
        let compare = Compare {
            base: "HEAD",
            roots: &roots,
            files: &files,
            model: None,
            rule: None,
            bounds: bounds(),
        };
        let compared = rule_changes(&compare, &[]);
        assert_eq!(compared.changes, [], "nothing can be told of either side");
        assert_eq!(
            compared.warnings("HEAD").len(),
            1,
            "the file is named as unreadable at the base: {:?}",
            compared.warnings("HEAD")
        );
    }
}
