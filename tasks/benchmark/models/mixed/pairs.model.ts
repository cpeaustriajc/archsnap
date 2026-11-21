import { m } from "@earnest/model";

export const Pairs = m.model("Pairs", {
	a: m.int(0, 14).initial(0),
	b: m.int(0, 14).initial(0),
});

export const raiseB = m.action(Pairs, "the upper number goes up", {
	when: (s) => s.b < 14,
	changes: (s) => ({ b: s.b + 1 }),
});

export const raiseA = m.action(Pairs, "the lower number catches up by one", {
	when: (s) => s.a < s.b,
	changes: (s) => ({ a: s.a + 1 }),
});

export const ordered = m.rule(Pairs, "The lower number never passes the upper", (s) => s.a <= s.b);

export const level = m.end(Pairs, "Both numbers are at the top", (s) => s.a === 14 && s.b === 14);
