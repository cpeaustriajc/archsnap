import { m } from "@earnest/model";

export const Stray = m.model("Stray", {
	n: m.int(0, 3).initial(0),
});

export const inc = Stray.action("the count goes up at the café", {
	when: (s) => s.n < 3,
	then: (s) => ({ n: s.n + 1 }),
});

export const low = Stray.rule({
	says: "The count stays at most one",
	holds: (s) => s.n <= 1,
});

export const top = Stray.end({
	says: "The count is three",
	when: (s) => s.n === 3,
});
