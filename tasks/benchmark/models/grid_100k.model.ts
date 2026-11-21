import { m } from "@earnest/model";

export const Grid100k = m.model("Grid100k", {
	a: m.int(0, 9).initial(0),
	b: m.int(0, 9).initial(0),
	c: m.int(0, 9).initial(0),
	d: m.int(0, 9).initial(0),
	e: m.int(0, 9).initial(0),
});

export const turn_a = m.action(Grid100k, "wheel a turns", {
	when: (s) => s.a < 9,
	changes: (s) => ({ a: s.a + 1 }),
});

export const turn_b = m.action(Grid100k, "wheel b turns", {
	when: (s) => s.b < 9,
	changes: (s) => ({ b: s.b + 1 }),
});

export const turn_c = m.action(Grid100k, "wheel c turns", {
	when: (s) => s.c < 9,
	changes: (s) => ({ c: s.c + 1 }),
});

export const turn_d = m.action(Grid100k, "wheel d turns", {
	when: (s) => s.d < 9,
	changes: (s) => ({ d: s.d + 1 }),
});

export const turn_e = m.action(Grid100k, "wheel e turns", {
	when: (s) => s.e < 9,
	changes: (s) => ({ e: s.e + 1 }),
});

export const inside = m.rule(
	Grid100k,
	"Every wheel stays on its dial",
	(s) => s.a <= 9 && s.b <= 9 && s.c <= 9 && s.d <= 9 && s.e <= 9,
);

export const stopped = m.end(
	Grid100k,
	"Every wheel is at its last notch",
	(s) => s.a === 9 && s.b === 9 && s.c === 9 && s.d === 9 && s.e === 9,
);

export type State = m.Infer<typeof Grid100k>;
