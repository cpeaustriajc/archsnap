import { m } from "@earnest/model";

export const Meter = m.model("Meter", {
	level: m.int(-1, 1).initial(1),
});

export const drain = m.action(Meter, "the meter drains", {
	when: (s) => s.level >= 0,
	changes: (s) => ({ level: s.level - 1 }),
});

export const refill = m.action(Meter, "the meter refills on an even level", {
	when: (s) => m.rem(s.level, 2) === 0,
	changes: () => ({ level: 1 }),
});

export const floor = m.rule(Meter, "The level is never below minus one", (s) => s.level + 1 >= 0);

export const empty = m.end(Meter, "The meter is below zero", (s) => s.level < 0);

export type State = m.Infer<typeof Meter>;
