import { m } from "@earnest/model";

const Wide = m.int(0, 99);

export const Slow = m.model("Slow", {
	a: m.int(0, 9).initial(0),
	b: m.int(0, 9).initial(0),
	c: m.int(0, 9).initial(0),
});

export const first = Slow.action("the first wheel turns", {
	when: (s) => s.a < 9,
	then: (s) => ({ a: s.a + 1 }),
});

export const second = Slow.action("the second wheel turns", {
	when: (s) => s.b < 9,
	then: (s) => ({ b: s.b + 1 }),
});

export const third = Slow.action("the third wheel turns", {
	when: (s) => s.c < 9,
	then: (s) => ({ c: s.c + 1 }),
});

export const heavy = Slow.rule({
	says: "No sum of three wide numbers and the wheels is below zero",
	holds: (s) =>
		m.forall(Wide, (x) =>
			m.forall(Wide, (y) => m.forall(Wide, (z) => x + y + z + s.a + s.b + s.c >= 0)),
		),
});

export const stopped = Slow.end({
	says: "Every wheel is at its last notch",
	when: (s) => s.a === 9 && s.b === 9 && s.c === 9,
});

export type State = m.Infer<typeof Slow>;
