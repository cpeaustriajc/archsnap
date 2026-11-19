import { m } from "@earnest/model";

export const Porch = m.model("Porch", {
	lit: m.boolean().initial(false),
});

export const flip = Porch.action("someone flips the porch switch", {
	then: (s) => ({ lit: !s.lit }),
});

export const porchKnown = Porch.rule({
	says: "The porch light is on or off",
	holds: (s) => s.lit || !s.lit,
});

export const Hall = m.model("Hall", {
	level: m.int(0, 2).initial(0),
});

export const turn = Hall.action("someone turns the hall dimmer one notch", {
	then: (s) => ({ level: (s.level + 1) % 3 }),
});

export const hallKnown = Hall.rule({
	says: "The hall dimmer has three notches",
	holds: (s) => s.level < 3,
});
