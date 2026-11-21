import { m } from "@earnest/model";

export { m };

export const Lamp = m.model("Lamp", { on: m.boolean().initial(false) });

export const turn = m.action(Lamp, "the lamp turns on", {
	when: (s) => !s.on,
	changes: () => ({ on: true }),
});

export const lit = m.end(Lamp, "The lamp is lit", (s) => s.on);
