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

export const Lamp = m.model("Lamp", {
	on: m.boolean().initial(false),
});

export const flip = Lamp.action("the lamp turns on", {
	when: (s) => !s.on,
	then: () => ({ on: true }),
});

export const dim = Lamp.rule({
	says: "The lamp is never on",
	holds: (s) => !s.on,
});

export const lit = Lamp.end({
	says: "The lamp is lit",
	when: (s) => s.on,
});
