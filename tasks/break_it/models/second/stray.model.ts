import { m } from "@earnest/model";

export const Stray = m.model("Stray", {
	n: m.int(0, 3).initial(0),
});

export const inc = m.action(Stray, "the count goes up at the café", {
	when: (s) => s.n < 3,
	changes: (s) => ({ n: s.n + 1 }),
});

export const low = m.rule(Stray, "The count stays at most one", (s) => s.n <= 1);

export const top = m.end(Stray, "The count is three", (s) => s.n === 3);
