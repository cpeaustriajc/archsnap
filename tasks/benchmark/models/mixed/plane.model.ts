import { m } from "@earnest/model";

export const Plane = m.model("Plane", {
	x: m.int(0, 19).initial(0),
	y: m.int(0, 19).initial(0),
});

export const east = Plane.action("the point moves east", {
	when: (s) => s.x < 19,
	then: (s) => ({ x: s.x + 1 }),
});

export const north = Plane.action("the point moves north", {
	when: (s) => s.y < 19,
	then: (s) => ({ y: s.y + 1 }),
});

export const inside = Plane.rule({
	says: "The point stays inside the plane",
	holds: (s) => s.x + s.y <= 38,
});

export const corner = Plane.end({
	says: "The point is at the far corner",
	when: (s) => s.x === 19 && s.y === 19,
});
