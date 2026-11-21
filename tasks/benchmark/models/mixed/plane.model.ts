import { m } from "@earnest/model";

export const Plane = m.model("Plane", {
	x: m.int(0, 19).initial(0),
	y: m.int(0, 19).initial(0),
});

export const east = m.action(Plane, "the point moves east", {
	when: (s) => s.x < 19,
	changes: (s) => ({ x: s.x + 1 }),
});

export const north = m.action(Plane, "the point moves north", {
	when: (s) => s.y < 19,
	changes: (s) => ({ y: s.y + 1 }),
});

export const inside = m.rule(Plane, "The point stays inside the plane", (s) => s.x + s.y <= 38);

export const corner = m.end(Plane, "The point is at the far corner", (s) => s.x === 19 && s.y === 19);
