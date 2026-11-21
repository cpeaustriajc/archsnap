import { m } from "@earnest/model";

export const Grid = m.model("Grid", {
	x: m.int(0, 29).initial(0),
	y: m.int(0, 29).initial(0),
	z: m.int(0, 29).initial(0),
});

export const east = m.action(Grid, "the point moves east", {
	when: (s) => s.x < 29,
	changes: (s) => ({ x: s.x + 1 }),
});

export const north = m.action(Grid, "the point moves north", {
	when: (s) => s.y < 29,
	changes: (s) => ({ y: s.y + 1 }),
});

export const up = m.action(Grid, "the point moves up", {
	when: (s) => s.z < 29,
	changes: (s) => ({ z: s.z + 1 }),
});

export const inside = m.rule(Grid, "The point stays inside the grid", (s) => s.x + s.y + s.z <= 87);

export const corner = m.end(Grid, "The point is at the far corner", (s) => s.x === 29 && s.y === 29 && s.z === 29);

export type State = m.Infer<typeof Grid>;
