import { m } from "@earnest/model";

const MAX = 3;
const allowed = (level: number) => level <= 3;

export const Tank = m.model("Tank", {
	level: m.int(0, 3).initial(0),
});

export const fill = Tank.action("the tank fills", {
	when: (s) => s.level < 3,
	then: (s) => ({ level: s.level + 1 }),
});

export const full = Tank.end({
	says: "The tank is full",
	when: (s) => s.level === 3,
});

export const capped = Tank.rule({
	says: "The level is at most the maximum",
	holds: (s) => s.level <= MAX,
});

export const checked = Tank.rule({
	says: "The level is allowed",
	holds: (s) => allowed(s.level),
});
