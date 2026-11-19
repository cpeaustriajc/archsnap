import { m } from "@earnest/model";

export const Tank = m.model("Tank", {
	level: m.int(0, 2).initial(0),
});

export const fill = Tank.action("the tank fills", {
	when: (s) => s.level < 2,
	then: (s) => ({ level: s.level + 1 }),
});

export const capped = Tank.rule({
	says: "The level is at most two",
	holds: (s) => s.level <= 2,
});

export const full = Tank.end({
	says: "The tank is full",
	when: (s) => s.level === 2,
});
