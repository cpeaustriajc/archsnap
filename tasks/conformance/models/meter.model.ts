import { m } from "@earnest/model";

export const Meter = m.model("Meter", {
	level: m.int(-1, 1).initial(1),
});

export const drain = Meter.action("the meter drains", {
	when: (s) => s.level >= 0,
	then: (s) => ({ level: s.level - 1 }),
});

export const refill = Meter.action("the meter refills on an even level", {
	when: (s) => m.rem(s.level, 2) === 0,
	then: () => ({ level: 1 }),
});

export const floor = Meter.rule({
	says: "The level is never below minus one",
	holds: (s) => s.level + 1 >= 0,
});

export const empty = Meter.end({
	says: "The meter is below zero",
	when: (s) => s.level < 0,
});

export type State = m.Infer<typeof Meter>;
