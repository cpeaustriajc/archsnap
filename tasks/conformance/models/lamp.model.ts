import { m } from "@earnest/model";

export const Lamp = m.model("Lamp", {
	power: m.boolean(),
	level: m.int(0, 2).initial(0),
});

export const toggle = m.action(Lamp, "someone flips the switch", {
	changes: (s) => ({ power: !s.power }),
});

export const brighten = m.action(Lamp, "someone turns the dial up", {
	when: (s) => s.power && s.level < 2,
	changes: (s) => ({ level: s.level + 1 }),
});

export const dim = m.rule(Lamp, "The dial is never at full while the lamp is off", (s) => s.power || s.level < 2);

export type State = m.Infer<typeof Lamp>;
