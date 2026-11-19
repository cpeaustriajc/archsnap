import { m } from "@earnest/model";

export const Grid = m.model("Grid", {
	x: m.int(0, 29).initial(0),
	y: m.int(0, 29).initial(0),
	z: m.int(0, 29).initial(0),
});

export const east = Grid.action("the point moves east", {
	when: (s) => s.x < 29,
	then: (s) => ({ x: s.x + 1 }),
});

export const north = Grid.action("the point moves north", {
	when: (s) => s.y < 29,
	then: (s) => ({ y: s.y + 1 }),
});

export const up = Grid.action("the point moves up", {
	when: (s) => s.z < 29,
	then: (s) => ({ z: s.z + 1 }),
});

export const inside = Grid.rule({
	says: "The point stays inside the grid",
	holds: (s) => s.x + s.y + s.z <= 87,
});

export const corner = Grid.end({
	says: "The point is at the far corner",
	when: (s) => s.x === 29 && s.y === 29 && s.z === 29,
});

export type State = m.Infer<typeof Grid>;
