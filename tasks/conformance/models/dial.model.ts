import { m } from "@earnest/model";

export const Dial = m.model("Dial", {
	x: m.int(0, 5).initial(0),
	y: m.int(0, 5).initial(0),
});

export const east = Dial.action("the dial turns east", {
	when: (s) => s.x < 5,
	then: (s) => ({ x: s.x + 1 }),
});

export const north = Dial.action("the dial turns north", {
	when: (s) => s.y < 5,
	then: (s) => ({ y: s.y + 1 }),
});

export const near =Dial.rule({
	says: "The dial never reaches the far corner",
	holds: (s) => !(s.x === 5 && s.y === 5),
});

export const far = Dial.end({
	says: "The dial is at the far corner",
	when: (s) => s.x === 5 && s.y === 5,
});

export const view = Dial.view({
	title: "Dial",
	state: (s) => `x${s.x} y${s.y}`,
});

export type State = m.Infer<typeof Dial>;
