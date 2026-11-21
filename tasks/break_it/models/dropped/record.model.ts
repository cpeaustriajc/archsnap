import { m } from "@earnest/model";

export const Climb = m.model("Climb", {
	n: m.int(0, 3).initial(0),
});

export const inc = m.action(Climb, "the count goes up", {
	when: (s) => s.n < 3,
	changes: (s) => ({ n: s.n + 1 }),
});

export const top = m.end(Climb, "The count is at the top", (s) => s.n === 3);

export const rules = {
	low: m.rule(Climb, "The count stays at most one", (s: { n: number }) => s.n <= 1),
};
