import { m } from "@earnest/model";

export const Tank = m.model("Tank", {
	level: m.int(0, 3).initial(0),
});

export const fill = Tank.action("the pump adds a unit", {
	when: (s) => s.level < 3,
	then: (s) => ({ level: s.level + 1 }),
});

export const capped = Tank.rule({
	says: "The level is at most three",
	holds: (s) => s.level <= 3,
});

export const floor = Tank.rule({
	says: "The level is at least one",
	holds: (s) => s.level >= 1,
});

export const steady = Tank.rule({
	says: "The level stays within the tank",
	holds: (s) => s.level < 4,
});

export const named = Tank.rule({
	says: "The level is tiny",
	holds: (s) => s.level < 4,
});

export const kept = Tank.rule({
	says: "The level is never six",
	holds: (s) => s.level !== 6,
});

export const fresh = Tank.rule({
	says: "The level is never seven",
	holds: (s) => s.level !== 7,
});

export const full = Tank.end({
	says: "The tank is full",
	when: (s) => s.level === 3,
});

export type State = m.Infer<typeof Tank>;
