import { m } from "@earnest/model";

export const Porch = m.model("Porch", {
	lit: m.boolean().initial(false),
});

export const flip = m.action(Porch, "someone flips the porch switch", {
	changes: (s) => ({ lit: !s.lit }),
});

export const porchKnown = m.rule(Porch, "The porch light is on or off", (s) => s.lit === true || s.lit === false);

export const Hall = m.model("Hall", {
	level: m.int(0, 2).initial(0),
});

export const turn = m.action(Hall, "someone turns the hall dimmer one notch", {
	changes: (s) => ({ level: m.rem(s.level + 1, 3) }),
});

export const hallKnown = m.rule(Hall, "The hall dimmer has three notches", (s) => s.level < 3);
