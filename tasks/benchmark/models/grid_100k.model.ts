import { m } from "@earnest/model";

export const Grid100k = m.model("Grid100k", {
	a: m.int(0, 9).initial(0),
	b: m.int(0, 9).initial(0),
	c: m.int(0, 9).initial(0),
	d: m.int(0, 9).initial(0),
	e: m.int(0, 9).initial(0),
});

export const turn_a = Grid100k.action("wheel a turns", {
	when: (s) => s.a < 9,
	then: (s) => ({ a: s.a + 1 }),
});

export const turn_b = Grid100k.action("wheel b turns", {
	when: (s) => s.b < 9,
	then: (s) => ({ b: s.b + 1 }),
});

export const turn_c = Grid100k.action("wheel c turns", {
	when: (s) => s.c < 9,
	then: (s) => ({ c: s.c + 1 }),
});

export const turn_d = Grid100k.action("wheel d turns", {
	when: (s) => s.d < 9,
	then: (s) => ({ d: s.d + 1 }),
});

export const turn_e = Grid100k.action("wheel e turns", {
	when: (s) => s.e < 9,
	then: (s) => ({ e: s.e + 1 }),
});

export const inside = Grid100k.rule({
	says: "Every wheel stays on its dial",
	holds: (s) => s.a <= 9 && s.b <= 9 && s.c <= 9 && s.d <= 9 && s.e <= 9,
});

export const stopped = Grid100k.end({
	says: "Every wheel is at its last notch",
	when: (s) => s.a === 9 && s.b === 9 && s.c === 9 && s.d === 9 && s.e === 9,
});

export type State = m.Infer<typeof Grid100k>;
