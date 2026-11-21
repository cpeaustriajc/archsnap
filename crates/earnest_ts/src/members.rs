use crate::Reader;
use crate::bodies::body_span;
use crate::terms::bare;
use earnest_ast::{
    ActionBody, ActionDraft, COLORED_PARTS, Draft, EndDraft, LabelDraft, Quoting, RuleDraft, Shape,
    Term, TermKind, ViewDraft, condition_text, is_color,
};
use oxc_ast::ast::{
    Argument, ArrowFunctionExpression, BindingPattern, CallExpression, Expression, ObjectPattern,
    ObjectPropertyKind,
};
use oxc_span::GetSpan;

#[derive(Clone, Copy)]
pub(crate) enum MemberKind {
    Action,
    Rule,
    End,
    View,
}

pub(crate) struct Member<'p, 'a> {
    pub(crate) model: usize,
    pub(crate) kind: MemberKind,
    pub(crate) id: String,
    pub(crate) start: u32,
    pub(crate) call: &'p CallExpression<'a>,
}

impl MemberKind {
    pub(crate) fn named(part: &str) -> Option<Self> {
        match part {
            "action" => Some(Self::Action),
            "rule" => Some(Self::Rule),
            "end" => Some(Self::End),
            "view" => Some(Self::View),
            _ => None,
        }
    }
}

const CHANGES_NOT_THEN: &str = "an object with a `then` is a promise to JavaScript, and an action \
    that was exported would hang an `async` function: write `changes`";
const CONDITION_KEYS: [&str; 3] = ["says", "holds", "when"];

const SENTENCE: usize = 1;
const FUNCTION: usize = 2;
const ACTION_OBJECT: usize = 2;
const VIEW_OBJECT: usize = 1;

type Entries<'p, 'a> = Vec<(String, u32, &'p Expression<'a>)>;

#[derive(Default)]
struct ActionParameters {
    state: Option<String>,
    whole_input: Option<String>,
    picked_inputs: Vec<(String, String)>,
}

struct StateFunction {
    state_name: Option<String>,
    term: Term,
    text: String,
}

fn pick<'p, 'a>(entries: &Entries<'p, 'a>, key: &str) -> Option<&'p Expression<'a>> {
    entries
        .iter()
        .find(|(name, _, _)| name == key)
        .map(|(_, _, value)| *value)
}

fn has_condition_key(wrapped: &Expression<'_>) -> bool {
    let Expression::ObjectExpression(object) = bare(wrapped) else {
        return false;
    };
    object.properties.iter().any(|property| {
        matches!(
            property,
            ObjectPropertyKind::ObjectProperty(written)
                if written
                    .key
                    .static_name()
                    .is_some_and(|name| CONDITION_KEYS.contains(&name.as_ref()))
        )
    })
}

impl Reader<'_> {
    pub(crate) fn member_of(&mut self, draft: &mut Draft, member: Member<'_, '_>) {
        let Member {
            kind,
            id,
            start,
            call,
            ..
        } = member;
        match kind {
            MemberKind::Action => {
                if let Some(action) = self.action(id, start, call) {
                    draft.actions.push(action);
                }
            }
            MemberKind::Rule => {
                if let Some(rule) = self.rule(id, start, call) {
                    draft.rules.push(rule);
                }
            }
            MemberKind::End => {
                if let Some(end) = self.end(id, start, call) {
                    draft.ends.push(end);
                }
            }
            MemberKind::View => draft.view = self.view(call),
        }
    }

    pub(crate) fn sentence(&mut self, wrapped: &Expression<'_>) -> String {
        let expr = bare(wrapped);
        if let TermKind::Text(text) = self.term(expr).kind {
            text
        } else {
            let before = self.quoted(expr.span());
            self.refuse(expr.span().start, &before, "text in quotes is needed here");
            String::new()
        }
    }

    fn parts<'p, 'a>(
        &mut self,
        call: &'p CallExpression<'a>,
        index: usize,
        allowed: &[&str],
        renamed: &[(&str, &str)],
    ) -> Option<Entries<'p, 'a>> {
        let expected = index + 1;
        let found = call.arguments.get(index).and_then(Argument::as_expression);
        let (Some(object), true) = (found, call.arguments.len() == expected) else {
            self.refuse(
                call.span.start,
                &self.quoted(call.callee.span()),
                &format!(
                    "its parts are needed here, as an object with {}",
                    allowed.join(", ")
                ),
            );
            return None;
        };
        let entries = self.entries(object);
        for (name, start, _) in &entries {
            if let Some((_, message)) = renamed.iter().find(|(key, _)| key == name) {
                self.refuse(*start, name, message);
            } else if !allowed.contains(&name.as_str()) {
                self.refuse(
                    *start,
                    name,
                    &format!(
                        "it is not a part of this; the parts are {}",
                        allowed.join(", ")
                    ),
                );
            }
        }
        Some(entries)
    }

    fn needed<'p, 'a>(
        &mut self,
        entries: &Entries<'p, 'a>,
        key: &str,
        call: &CallExpression<'_>,
    ) -> Option<&'p Expression<'a>> {
        let found = pick(entries, key);
        if found.is_none() {
            self.refuse(
                call.span.start,
                &self.quoted(call.callee.span()),
                &format!("it needs a `{key}`"),
            );
        }
        found
    }

    fn action(&mut self, id: String, start: u32, call: &CallExpression<'_>) -> Option<ActionDraft> {
        let entries = self.parts(
            call,
            ACTION_OBJECT,
            &["input", "when", "changes"],
            &[("then", CHANGES_NOT_THEN)],
        )?;
        let label = call
            .arguments
            .get(SENTENCE)
            .and_then(Argument::as_expression)
            .map(|sentence| self.sentence(sentence))?;
        let inputs: Vec<(String, Shape)> = match pick(&entries, "input") {
            Some(declared) => self.fields(declared),
            None => Vec::new(),
        };
        let when = pick(&entries, "when").and_then(|written| self.arrow(written));
        let changes = if pick(&entries, "then").is_some() {
            pick(&entries, "changes")
        } else {
            self.needed(&entries, "changes", call)
        };
        let changing = changes.and_then(|written| self.arrow(written));
        let guard = when.map(|arrow| self.action_body(arrow));
        let change = changing.map(|arrow| self.action_body(arrow))?;
        Some(ActionDraft {
            id,
            label,
            place: self.place(start),
            inputs,
            guard,
            change,
        })
    }

    fn action_parameters(&mut self, arrow: &ArrowFunctionExpression<'_>) -> ActionParameters {
        let mut found = ActionParameters::default();
        for (index, parameter) in arrow.params.items.iter().enumerate() {
            match (index, &parameter.pattern) {
                (0, BindingPattern::BindingIdentifier(name)) => {
                    found.state = Some(name.name.to_string());
                }
                (1, BindingPattern::BindingIdentifier(name)) => {
                    found.whole_input = Some(name.name.to_string());
                }
                (1, BindingPattern::ObjectPattern(pattern)) => {
                    found.picked_inputs = self.picked_inputs(pattern);
                }
                (_, other) => {
                    let written = self.quoted(other.span());
                    self.refuse(
                        other.span().start,
                        &written,
                        "an action's function takes the state by one name, then its input",
                    );
                }
            }
        }
        if let Some(rest) = &arrow.params.rest {
            self.refuse(
                rest.span.start,
                "...",
                "a rest parameter is outside the library: take each value as a parameter of its own",
            );
        }
        found
    }

    fn picked_inputs(&mut self, pattern: &ObjectPattern<'_>) -> Vec<(String, String)> {
        let mut picked = Vec::with_capacity(pattern.properties.len());
        for property in &pattern.properties {
            let key = property.key.static_name();
            match (key, &property.value) {
                (Some(input), BindingPattern::BindingIdentifier(local)) if !property.computed => {
                    picked.push((local.name.to_string(), input.into_owned()));
                }
                _ => {
                    let written = self.quoted(property.span);
                    self.refuse(
                        property.span.start,
                        &written,
                        "an input is picked by its name: `{ name }`",
                    );
                }
            }
        }
        if let Some(rest) = &pattern.rest {
            self.refuse(rest.span.start, "...", "each input is picked by its name");
        }
        picked
    }

    fn action_body(&mut self, arrow: &ArrowFunctionExpression<'_>) -> ActionBody {
        let parameters = self.action_parameters(arrow);
        let mark = self.locals.len();
        for own in [&parameters.state, &parameters.whole_input]
            .into_iter()
            .flatten()
        {
            self.locals.push((own.clone(), own.clone()));
        }
        self.locals.extend(parameters.picked_inputs.iter().cloned());
        let term = self.body(arrow);
        self.locals.truncate(mark);
        ActionBody {
            state_name: parameters.state,
            input_name: parameters.whole_input,
            taken: parameters
                .picked_inputs
                .into_iter()
                .map(|(_, input)| input)
                .collect(),
            term,
        }
    }

    fn state_function(&mut self, wrapped: &Expression<'_>) -> Option<StateFunction> {
        let arrow = self.arrow(wrapped)?;
        let (parameters, term) = self.function(arrow);
        if parameters.len() > 1 {
            self.refuse(
                arrow.params.span.start,
                &self.quoted(arrow.params.span),
                "this function takes only the state",
            );
        }
        Some(StateFunction {
            state_name: parameters.into_iter().next(),
            term,
            text: condition_text(self.text(body_span(arrow)), Quoting::TypeScript),
        })
    }

    fn sentence_and_function(
        &mut self,
        call: &CallExpression<'_>,
    ) -> (Option<String>, Option<StateFunction>) {
        let sentence = call
            .arguments
            .get(SENTENCE)
            .and_then(Argument::as_expression);
        let function = call
            .arguments
            .get(FUNCTION)
            .and_then(Argument::as_expression);
        let keyed = [sentence, function]
            .into_iter()
            .flatten()
            .find(|written| has_condition_key(written));
        if let Some(object) = keyed {
            let written = self.quoted(object.span());
            let callee = self.quoted(call.callee.span());
            let model = call
                .arguments
                .first()
                .map_or_else(|| "Model".to_owned(), |first| self.quoted(first.span()));
            self.refuse(
                object.span().start,
                &written,
                &format!(
                    "the sentence and the function are arguments, not keys: write \
                     `{callee}({model}, \"sentence\", (s) => condition)`"
                ),
            );
            return (None, None);
        }
        let (Some(sentence), Some(function), 3) = (sentence, function, call.arguments.len()) else {
            let callee = self.quoted(call.callee.span());
            self.refuse(
                call.span.start,
                &callee,
                &format!(
                    "its model, its sentence and its condition are needed here, as in \
                     `{callee}(Model, \"sentence\", (s) => ...)`"
                ),
            );
            return (None, None);
        };
        (Some(self.sentence(sentence)), self.state_function(function))
    }

    fn rule(&mut self, id: String, start: u32, call: &CallExpression<'_>) -> Option<RuleDraft> {
        let (says, holds) = self.sentence_and_function(call);
        let condition = holds?;
        Some(RuleDraft {
            id,
            says: says?,
            place: self.place(start),
            state_name: condition.state_name,
            holds: condition.term,
            text: condition.text,
        })
    }

    fn end(&mut self, id: String, start: u32, call: &CallExpression<'_>) -> Option<EndDraft> {
        let (says, when) = self.sentence_and_function(call);
        let condition = when?;
        Some(EndDraft {
            id,
            says: says?,
            place: self.place(start),
            state_name: condition.state_name,
            when: condition.term,
        })
    }

    fn view(&mut self, call: &CallExpression<'_>) -> ViewDraft {
        let allowed = ["title", "state", "group", "step", "colors"];
        let Some(entries) = self.parts(call, VIEW_OBJECT, &allowed, &[]) else {
            return ViewDraft::default();
        };
        ViewDraft {
            title: pick(&entries, "title").map(|title| self.sentence(title)),
            state: pick(&entries, "state").and_then(|label| self.label(label)),
            group: pick(&entries, "group").and_then(|label| self.label(label)),
            steps: pick(&entries, "step").map_or_else(Vec::new, |steps| self.texts(steps)),
            colors: pick(&entries, "colors").map_or_else(Vec::new, |colors| self.colors(colors)),
        }
    }

    fn label(&mut self, wrapped: &Expression<'_>) -> Option<LabelDraft> {
        let function = self.state_function(wrapped)?;
        Some(LabelDraft {
            state_name: function.state_name,
            term: function.term,
        })
    }

    fn colors(&mut self, wrapped: &Expression<'_>) -> Vec<(String, String)> {
        let mut found = Vec::new();
        for (name, start, value) in self.entries(wrapped) {
            let color = self.sentence(value);
            if !COLORED_PARTS.contains(&name.as_str()) {
                self.refuse(
                    start,
                    &name,
                    "`colors` colors `broken`, `start` and `end`, and no other part",
                );
            } else if !is_color(&color) {
                let written = self.quoted(value.span());
                self.refuse(
                    value.span().start,
                    &written,
                    "a color is `#` and 3, 4, 6 or 8 hexadecimal digits, or a color name of \
                     letters such as `red`; the chart would leave anything else out",
                );
            }
            found.push((name, color));
        }
        found
    }

    fn texts(&mut self, wrapped: &Expression<'_>) -> Vec<(String, String)> {
        let mut found = Vec::new();
        for (name, start, value) in self.entries(wrapped) {
            self.step_places.push((name.clone(), start));
            found.push((name, self.sentence(value)));
        }
        found
    }
}

#[cfg(test)]
mod tests {
    use crate::read;
    use crate::show::{show, show_shape};
    use earnest_ast::{ActionDraft, Draft};
    use earnest_diagnostics::Place;

    const HEAD: &str = "import { m } from \"@earnest/model\";\n\
        const Press = m.enum([\"key\", \"mouse\"]);\n\
        export const M = m.model(\"M\", { n: m.int(0, 3), last: Press.optional() });\n";

    const NO_NAMES: [&str; 0] = [];

    fn draft(rest: &str) -> Draft {
        let reading = read("m.model.ts", &format!("{HEAD}{rest}"));
        assert_eq!(reading.problems, [], "no problem in: {rest}");
        reading.drafts.into_iter().next().expect("one model")
    }

    fn action(parts: &str) -> ActionDraft {
        let rest = format!("export const act = m.action(M, \"the user acts\", {parts});\n");
        draft(&rest).actions.into_iter().next().expect("one action")
    }

    fn features(rest: &str) -> Vec<String> {
        read("m.model.ts", &format!("{HEAD}{rest}"))
            .problems
            .into_iter()
            .map(|problem| problem.feature)
            .collect()
    }

    #[test]
    fn an_action_with_only_its_changes() {
        let found = action("{ changes: (s) => ({ n: s.n + 1 }) }");
        assert_eq!(found.id, "act", "the exported name");
        assert_eq!(found.label, "the user acts", "the sentence");
        assert_eq!(found.place, Place { line: 4, column: 1 }, "the `export`");
        assert_eq!(found.inputs, [], "no input");
        assert_eq!(found.guard, None, "no guard");
        assert_eq!(
            found.change.state_name.as_deref(),
            Some("s"),
            "the state's name"
        );
        assert_eq!(found.change.input_name, None, "no input name");
        assert_eq!(found.change.taken, NO_NAMES, "no input is taken");
        assert_eq!(show(&found.change.term), "{n: (+ s.n 1)}", "the change");
    }

    #[test]
    fn an_action_that_picks_its_input() {
        let found = action(
            "{ input: { by: Press }, when: (s, { by }) => s.n < 3 && by === \"key\", \
             changes: (_, { by }) => ({ last: by }) }",
        );
        let inputs: Vec<(&str, String)> = found
            .inputs
            .iter()
            .map(|(name, shape)| (name.as_str(), show_shape(shape)))
            .collect();
        assert_eq!(
            inputs,
            [("by", "enum(\"key\", \"mouse\")".to_owned())],
            "the input"
        );
        let guard = found.guard.expect("a guard");
        assert_eq!(
            guard.state_name.as_deref(),
            Some("s"),
            "the state in `when`"
        );
        assert_eq!(guard.input_name, None, "the input is read by its own name");
        assert_eq!(guard.taken, ["by"], "`when` takes the input");
        assert_eq!(
            show(&guard.term),
            "(&& (< s.n 3) (=== by \"key\"))",
            "the guard"
        );
        assert_eq!(
            found.change.state_name.as_deref(),
            Some("_"),
            "the state in `changes`"
        );
        assert_eq!(found.change.taken, ["by"], "`changes` takes the input");
        assert_eq!(show(&found.change.term), "{last: by}", "the change");
    }

    #[test]
    fn an_action_that_takes_its_whole_input() {
        let found = action(
            "{ input: { by: Press }, when: (s, input) => input.by === \"key\", \
             changes: (state, given) => ({ last: given.by, n: state.n }) }",
        );
        let guard = found.guard.expect("a guard");
        assert_eq!(
            guard.state_name.as_deref(),
            Some("s"),
            "the state in `when`"
        );
        assert_eq!(
            guard.input_name.as_deref(),
            Some("input"),
            "the input in `when`"
        );
        assert_eq!(guard.taken, NO_NAMES, "no input is picked out");
        assert_eq!(show(&guard.term), "(=== input.by \"key\")", "the guard");
        assert_eq!(
            found.change.state_name.as_deref(),
            Some("state"),
            "the state in `changes`"
        );
        assert_eq!(
            found.change.input_name.as_deref(),
            Some("given"),
            "the input in `changes`"
        );
        assert_eq!(
            show(&found.change.term),
            "{last: given.by, n: state.n}",
            "`changes` reads through its own names"
        );
    }

    #[test]
    fn a_picked_input_may_be_renamed() {
        let found =
            action("{ input: { by: Press }, changes: (s, { by: who }) => ({ last: who }) }");
        assert_eq!(found.change.taken, ["by"], "the input that is taken");
        assert_eq!(
            show(&found.change.term),
            "{last: by}",
            "the input's own name reaches the tree"
        );
    }

    #[test]
    fn an_action_without_parameters_has_no_state_name() {
        let found = action("{ changes: () => ({ n: 0 }) }");
        assert_eq!(found.change.state_name, None, "nothing is read");
    }

    #[test]
    fn a_block_in_changes_folds_like_a_helper() {
        let found = action(
            "{ changes: (s) => { const next = s.n + 1; if (next > 3) return { n: 0 }; \
             return { n: next }; } }",
        );
        assert_eq!(
            show(&found.change.term),
            "(let next = (+ s.n 1) in (? (> next 3) {n: 0} {n: next}))",
            "a binding and an early return"
        );
    }

    #[test]
    fn a_rule_keeps_its_source_text() {
        let found = draft(
            "export const low = m.rule(M, \"The count is low\", (s) =>\n\t\ts.n <= 2 &&\n\t\ts.n >= 0,\n);\n",
        );
        let rule = found.rules.first().expect("one rule");
        assert_eq!(rule.id, "low", "the exported name");
        assert_eq!(rule.says, "The count is low", "the sentence");
        assert_eq!(rule.place, Place { line: 4, column: 1 }, "the `export`");
        assert_eq!(rule.state_name.as_deref(), Some("s"), "the state's name");
        assert_eq!(
            show(&rule.holds),
            "(&& (<= s.n 2) (>= s.n 0))",
            "the condition"
        );
        assert_eq!(
            rule.text, "s.n <= 2 && s.n >= 0",
            "as written, with each run of white space as one space"
        );
    }

    #[test]
    fn an_end() {
        let found = draft("export const full = m.end(M, \"Full\", (s) => s.n === 3);\n");
        let end = found.ends.first().expect("one end");
        assert_eq!(end.id, "full", "the exported name");
        assert_eq!(end.says, "Full", "the sentence");
        assert_eq!(end.state_name.as_deref(), Some("s"), "the state's name");
        assert_eq!(show(&end.when), "(=== s.n 3)", "the condition");
    }

    #[test]
    fn a_view_with_every_part() {
        let found = draft(
            "export const act = m.action(M, \"act\", { changes: () => ({ n: 0 }) });\n\
             export const view = m.view(M, { title: \"Counting\", \
             state: (s) => `at ${s.n}, by ${s.last}!`, group: (s) => s.last, \
             step: { act: \"go\" }, colors: { broken: \"#c62828\", start: \"#fff\" } });\n",
        );
        let view = found.view;
        assert_eq!(view.title.as_deref(), Some("Counting"), "the title");
        let state = view.state.expect("a state label");
        assert_eq!(state.state_name.as_deref(), Some("s"), "the state's name");
        assert_eq!(
            show(&state.term),
            "`at ${s.n}, by ${s.last}!`",
            "a template"
        );
        assert_eq!(
            view.group.map(|label| show(&label.term)).as_deref(),
            Some("s.last"),
            "group"
        );
        assert_eq!(
            view.steps,
            [("act".to_owned(), "go".to_owned())],
            "arrow labels"
        );
        assert_eq!(
            view.colors,
            [
                ("broken".to_owned(), "#c62828".to_owned()),
                ("start".to_owned(), "#fff".to_owned())
            ],
            "colors"
        );
    }

    #[test]
    fn a_file_without_a_view_has_an_empty_one() {
        let found = draft("");
        assert_eq!(
            found.view,
            earnest_ast::ViewDraft::default(),
            "nothing declared"
        );
    }

    #[test]
    fn a_member_that_is_written_wrongly_is_named() {
        assert_eq!(
            features("export const a = m.action(M, \"a\", { when: (s) => s.n < 1 });\n"),
            ["`m.action`"],
            "no changes"
        );
        assert_eq!(
            features(
                "export const a = m.action(M, \"a\", { changes: (s) => ({ n: 1 }), until: 3 });\n"
            ),
            ["`until`"],
            "an unknown part"
        );
        assert_eq!(
            features("export const a = m.action(M, \"a\", { changes: ({ n }) => ({ n }) });\n"),
            ["`{ n }`"],
            "a state taken apart"
        );
        assert_eq!(
            features("export const r = m.rule(M, \"r\", (s, t) => s.n < t.n);\n"),
            ["`(s, t)`"],
            "a rule with two parameters"
        );
        assert_eq!(
            features("export const r = m.rule(M, 3, (s) => s.n < 1);\n"),
            ["`3`"],
            "a sentence that is not text"
        );
        assert_eq!(
            features("export const r = m.rule(M, \"r\", true);\n"),
            ["`true`"],
            "a condition that is not a function"
        );
        assert_eq!(
            features("export const r = m.rule();\n"),
            ["`m.rule`"],
            "no parts"
        );
    }
}
