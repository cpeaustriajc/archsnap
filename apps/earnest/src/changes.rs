use crate::reading::read;
use crate::run::Bounds;
use crate::select::Target;
use earnest_ast::{Draft, RuleDraft};
use earnest_chart::{ChangeKind, RuleChange, RuleText};
use earnest_check::{Limits, Verdict, explore};
use earnest_ir::{Expr, Model, Op, Rule};
use earnest_semantic::lower_with_earlier;
use serde::Serialize;
use std::path::Path;
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
    pub(crate) model: String,
    pub(crate) id: String,
    pub(crate) change: Kind,
    pub(crate) was: Option<Wording>,
    pub(crate) now: Option<Wording>,
}

impl Difference {
    pub(crate) fn for_the_chart(&self) -> RuleChange<'_> {
        RuleChange {
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

fn source_at(base: &str, file: &str) -> Option<String> {
    let path = Path::new(file);
    let folder = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path.file_name()?.to_str()?;
    let shown = git(folder)
        .arg("show")
        .arg(format!("{base}:./{name}"))
        .output()
        .ok()?;
    if !shown.status.success() {
        return None;
    }
    String::from_utf8(shown.stdout).ok()
}

fn implies(first: &Rule, second: &Rule, id: String) -> Rule {
    Rule {
        id,
        says: String::new(),
        line: second.line,
        text: String::new(),
        holds: Expr::Or(
            Box::new(Expr::Apply(Op::Not, vec![first.holds.clone()])),
            Box::new(second.holds.clone()),
        ),
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

fn one_sided(model: &str, rule: &RuleDraft, change: Kind) -> Difference {
    let wording = Some(Wording::of(rule));
    let (was, now) = match change {
        Kind::Removed => (wording, None),
        Kind::Added | Kind::Loosened | Kind::Tightened | Kind::Changed | Kind::Reworded => {
            (None, wording)
        }
    };
    Difference {
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
        .map(|now| one_sided(&new.name, now, Kind::Added))
        .collect();
    for ((was, now), kind) in kept.iter().zip(kinds) {
        if let Some(change) = kind {
            changes.push(Difference {
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
            .map(|was| one_sided(&new.name, was, Kind::Removed)),
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

pub(crate) fn rule_changes(base: &str, targets: &[Target<'_>], bounds: Bounds) -> Compared {
    let mut compared = Compared::default();
    let mut file_at_base: Option<(&str, Vec<Draft>)> = None;
    for target in targets {
        let file = target.file;
        if file_at_base
            .as_ref()
            .is_none_or(|(path, _)| *path != file.path)
        {
            let reading = source_at(base, &file.path)
                .map(|source| read(file.language, &file.path, &source))
                .unwrap_or_default();
            if !reading.problems.is_empty() {
                compared.unreadable_at_base.push(file.path.clone());
            }
            let drafts = if reading.problems.is_empty() {
                reading.drafts
            } else {
                Vec::new()
            };
            file_at_base = Some((&file.path, drafts));
        }
        if compared.unreadable_at_base.last() == Some(&file.path) {
            continue;
        }
        let new = &target.readable.draft;
        let old = file_at_base
            .as_ref()
            .and_then(|(_, drafts)| drafts.iter().find(|draft| draft.name == new.name));
        compared.changes.extend(model_changes(old, new, bounds));
    }
    compared
}

#[cfg(test)]
mod tests {
    use super::{Difference, Kind, Wording, model_changes};
    use crate::run::Bounds;
    use earnest_ast::Draft;
    use std::fmt::Write;

    const FILE: &str = "tank.model.ts";
    const REST_OF_THE_TANK: &str = "const TOP = 3;\n\
        export const Tank = m.model(\"Tank\", {\n\
        \tlevel: m.int(0, 3).initial(0),\n\
        \tvalve: m.enum([\"shut\", \"open\"]).optional().initial(null),\n\
        });\n\
        export const fill = Tank.action(\"the tank fills\", {\n\
        \twhen: (s) => s.level < TOP,\n\
        \tthen: (s) => ({ level: s.level + 1 }),\n\
        });\n\
        export const full = Tank.end({ says: \"full\", when: (s) => s.level === TOP });\n";

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
                "export const {id} = Tank.rule({{ says: \"{says}\", holds: {holds} }});"
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
                "(s) => s.valve! === \"open\" || s.level <= 3",
                "(s) => s.level <= 3 || s.valve! === \"open\""
            ),
            Kind::Changed,
            "the old condition reads a null valve in every state"
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
}
