import { m } from "@earnest/model";

const MAX = 3;
const allowed = (level: number) => level <= 3;

export const Tank = m.model("Tank", {
	level: m.int(0, 3).initial(0),
});

export const fill = m.action(Tank, "the tank fills", {
	when: (s) => s.level < 3,
	changes: (s) => ({ level: s.level + 1 }),
});

export const full = m.end(Tank, "The tank is full", (s) => s.level === 3);

export const capped = m.rule(Tank, "The level is at most the maximum", (s) => s.level <= MAX);

export const checked = m.rule(Tank, "The level is allowed", (s) => allowed(s.level));
