import { m } from "@earnest/model";

export const Lamp = m.model("Lamp", {
	power: m.boolean(),
	level: m.int(0, 2).initial(0),
});

export const toggle = Lamp.action("someone flips the switch", {
	then: (s) => ({ power: !s.power }),
});

export const brighten = Lamp.action("someone turns the dial up", {
	when: (s) => s.power && s.level < 2,
	then: (s) => ({ level: s.level + 1 }),
});

export const dim = Lamp.rule({
	says: "The dial is never at full while the lamp is off",
	holds: (s) => s.power || s.level < 2,
});

export type State = m.Infer<typeof Lamp>;
