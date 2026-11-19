import { m } from "@earnest/model";

export const Tally = m.model("Tally", {
	count: m.int(0, 3).initial(0),
});

export const add = Tally.action("the clerk adds one", {
	when: (s) => s.count <= 3,
	then: (s) => ({ count: s.count + 1 }),
});

export const bounded = Tally.rule({
	says: "The count is never below zero",
	holds: (s) => s.count >= 0,
});

export const full = Tally.end({
	says: "The tally is full",
	when: (s) => s.count >= 3,
});

export type State = m.Infer<typeof Tally>;
