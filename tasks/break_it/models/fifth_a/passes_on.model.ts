import { m } from "@earnest/model";

export { m };

export const Lamp = m.model("Lamp", { on: m.boolean().initial(false) });

export const turn = Lamp.action("the lamp turns on", {
	when: (s) => !s.on,
	then: () => ({ on: true }),
});

export const lit = Lamp.end({ says: "The lamp is lit", when: (s) => s.on });
