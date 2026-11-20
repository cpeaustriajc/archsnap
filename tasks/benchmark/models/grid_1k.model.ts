import { m } from "@earnest/model";

export const Grid1k = m.model("Grid1k", {
	a: m.int(0, 9).initial(0),
	b: m.int(0, 9).initial(0),
	c: m.int(0, 9).initial(0),
});

export const turn_a = Grid1k.action("wheel a turns", {
	when: (s) => s.a < 9,
	then: (s) => ({ a: s.a + 1 }),
});

export const turn_b = Grid1k.action("wheel b turns", {
	when: (s) => s.b < 9,
	then: (s) => ({ b: s.b + 1 }),
});

export const turn_c = Grid1k.action("wheel c turns", {
	when: (s) => s.c < 9,
	then: (s) => ({ c: s.c + 1 }),
});

export const inside = Grid1k.rule({
	says: "Every wheel stays on its dial",
	holds: (s) => s.a <= 9 && s.b <= 9 && s.c <= 9,
});

export const stopped = Grid1k.end({
	says: "Every wheel is at its last notch",
	when: (s) => s.a === 9 && s.b === 9 && s.c === 9,
});

export type State = m.Infer<typeof Grid1k>;
