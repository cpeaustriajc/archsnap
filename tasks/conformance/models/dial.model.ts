import { m } from "@earnest/model";

export const Dial = m.model("Dial", {
	x: m.int(0, 5).initial(0),
	y: m.int(0, 5).initial(0),
});

export const east = m.action(Dial, "the dial turns east", {
	when: (s) => s.x < 5,
	changes: (s) => ({ x: s.x + 1 }),
});

export const north = m.action(Dial, "the dial turns north", {
	when: (s) => s.y < 5,
	changes: (s) => ({ y: s.y + 1 }),
});

export const near =m.rule(Dial, "The dial never reaches the far corner", (s) => !(s.x === 5 && s.y === 5));

export const far = m.end(Dial, "The dial is at the far corner", (s) => s.x === 5 && s.y === 5);

export const view = m.view(Dial, {
	title: "Dial",
	state: (s) => `x${s.x} y${s.y}`,
});

export type State = m.Infer<typeof Dial>;
