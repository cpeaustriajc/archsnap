import { m } from "@earnest/model";

export const Tally = m.model("Tally", {
	count: m.int(0, 3).initial(0),
});

export const add = m.action(Tally, "the clerk adds one", {
	when: (s) => s.count <= 3,
	changes: (s) => ({ count: s.count + 1 }),
});

export const bounded = m.rule(Tally, "The count is never below zero", (s) => s.count >= 0);

export const full = m.end(Tally, "The tally is full", (s) => s.count >= 3);

export type State = m.Infer<typeof Tally>;
