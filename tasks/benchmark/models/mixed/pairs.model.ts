import { m } from "@earnest/model";

export const Pairs = m.model("Pairs", {
	a: m.int(0, 14).initial(0),
	b: m.int(0, 14).initial(0),
});

export const raiseB = Pairs.action("the upper number goes up", {
	when: (s) => s.b < 14,
	then: (s) => ({ b: s.b + 1 }),
});

export const raiseA = Pairs.action("the lower number catches up by one", {
	when: (s) => s.a < s.b,
	then: (s) => ({ a: s.a + 1 }),
});

export const ordered = Pairs.rule({
	says: "The lower number never passes the upper",
	holds: (s) => s.a <= s.b,
});

export const level = Pairs.end({
	says: "Both numbers are at the top",
	when: (s) => s.a === 14 && s.b === 14,
});
