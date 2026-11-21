import { m } from "@earnest/model";

export const Tank = m.model("Tank", {
	level: m.int(0, 2).initial(0),
});

export const fill = m.action(Tank, "the tank fills", {
	when: (s) => s.level < 2,
	changes: (s) => ({ level: s.level + 1 }),
});

export const capped = m.rule(Tank, "The level is at most two", (s) => s.level <= 2);

export const full = m.end(Tank, "The tank is full", (s) => s.level === 2);
