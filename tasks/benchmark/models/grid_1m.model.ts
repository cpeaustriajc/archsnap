import { m } from "@earnest/model";

export const Grid1m = m.model("Grid1m", {
	a: m.int(0, 9).initial(0),
	b: m.int(0, 9).initial(0),
	c: m.int(0, 9).initial(0),
	d: m.int(0, 9).initial(0),
	e: m.int(0, 9).initial(0),
	f: m.int(0, 9).initial(0),
});

export const turn_a = Grid1m.action("wheel a turns", {
	when: (s) => s.a < 9,
	then: (s) => ({ a: s.a + 1 }),
});

export const turn_b = Grid1m.action("wheel b turns", {
	when: (s) => s.b < 9,
	then: (s) => ({ b: s.b + 1 }),
});

export const turn_c = Grid1m.action("wheel c turns", {
	when: (s) => s.c < 9,
	then: (s) => ({ c: s.c + 1 }),
});

export const turn_d = Grid1m.action("wheel d turns", {
	when: (s) => s.d < 9,
	then: (s) => ({ d: s.d + 1 }),
});

export const turn_e = Grid1m.action("wheel e turns", {
	when: (s) => s.e < 9,
	then: (s) => ({ e: s.e + 1 }),
});

export const turn_f = Grid1m.action("wheel f turns", {
	when: (s) => s.f < 9,
	then: (s) => ({ f: s.f + 1 }),
});

export const inside = Grid1m.rule({
	says: "Every wheel stays on its dial",
	holds: (s) => s.a <= 9 && s.b <= 9 && s.c <= 9 && s.d <= 9 && s.e <= 9 && s.f <= 9,
});

export const stopped = Grid1m.end({
	says: "Every wheel is at its last notch",
	when: (s) => s.a === 9 && s.b === 9 && s.c === 9 && s.d === 9 && s.e === 9 && s.f === 9,
});

export type State = m.Infer<typeof Grid1m>;
