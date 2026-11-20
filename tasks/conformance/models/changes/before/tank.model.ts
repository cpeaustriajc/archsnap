import { m } from "@earnest/model";

export const Tank = m.model("Tank", {
	level: m.int(0, 3).initial(0),
});

export const fill = Tank.action("the pump adds a unit", {
	when: (s) => s.level < 3,
	then: (s) => ({ level: s.level + 1 }),
});

export const capped = Tank.rule({
	says: "The level is at most two",
	holds: (s) => s.level <= 2,
});

export const floor = Tank.rule({
	says: "The level is at least zero",
	holds: (s) => s.level >= 0,
});

export const steady = Tank.rule({
	says: "The level stays within the tank",
	holds: (s) => s.level <= 3,
});

export const named = Tank.rule({
	says: "The level is small",
	holds: (s) => s.level < 4,
});

export const gone = Tank.rule({
	says: "The level is never five",
	holds: (s) => s.level !== 5,
});

export const kept = Tank.rule({
	says: "The level is never six",
	holds: (s) => s.level !== 6,
});

export const full = Tank.end({
	says: "The tank is full",
	when: (s) => s.level === 3,
});

export type State = m.Infer<typeof Tank>;
