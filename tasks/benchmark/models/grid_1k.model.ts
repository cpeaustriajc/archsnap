import { m } from "@earnest/model";

export const Grid1k = m.model("Grid1k", {
	a: m.int(0, 9).initial(0),
	b: m.int(0, 9).initial(0),
	c: m.int(0, 9).initial(0),
});

export const turn_a = m.action(Grid1k, "wheel a turns", {
	when: (s) => s.a < 9,
	changes: (s) => ({ a: s.a + 1 }),
});

export const turn_b = m.action(Grid1k, "wheel b turns", {
	when: (s) => s.b < 9,
	changes: (s) => ({ b: s.b + 1 }),
});

export const turn_c = m.action(Grid1k, "wheel c turns", {
	when: (s) => s.c < 9,
	changes: (s) => ({ c: s.c + 1 }),
});

export const inside = m.rule(Grid1k, "Every wheel stays on its dial", (s) => s.a <= 9 && s.b <= 9 && s.c <= 9);

export const stopped = m.end(Grid1k, "Every wheel is at its last notch", (s) => s.a === 9 && s.b === 9 && s.c === 9);

export type State = m.Infer<typeof Grid1k>;
