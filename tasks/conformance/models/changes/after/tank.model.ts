import { m } from "@earnest/model";

export const Tank = m.model("Tank", {
	level: m.int(0, 3).initial(0),
});

export const fill = m.action(Tank, "the pump adds a unit", {
	when: (s) => s.level < 3,
	changes: (s) => ({ level: s.level + 1 }),
});

export const capped = m.rule(Tank, "The level is at most three", (s) => s.level <= 3);

export const floor = m.rule(Tank, "The level is at least one", (s) => s.level >= 1);

export const steady = m.rule(Tank, "The level stays within the tank", (s) => s.level < 4);

export const named = m.rule(Tank, "The level is tiny", (s) => s.level < 4);

export const kept = m.rule(Tank, "The level is never six", (s) => s.level !== 6);

export const fresh = m.rule(Tank, "The level is never seven", (s) => s.level !== 7);

export const full = m.end(Tank, "The tank is full", (s) => s.level === 3);

export type State = m.Infer<typeof Tank>;
