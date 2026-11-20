import { m } from "@earnest/model";

export const Oven = m.model("Oven", {
	door: m.enum(["Closed", "Open"]).initial("Closed"),
	heat: m.enum(["Off", "On"]).initial("Off"),
});

export const open = Oven.action("the cook opens the door", {
	when: (s) => s.door === "Closed",
	then: () => ({ door: "Open" }),
});

export const close = Oven.action("the cook closes the door", {
	when: (s) => s.door === "Open",
	then: () => ({ door: "Closed" }),
});

export const start = Oven.action("the cook starts the heat", {
	when: (s) => s.heat === "Off" && s.door === "Closed",
	then: () => ({ heat: "On" }),
});

export const stop = Oven.action("the cook stops the heat", {
	when: (s) => s.heat === "On",
	then: () => ({ heat: "Off" }),
});

export const safe = Oven.rule({
	says: "The oven never heats with the door open",
	holds: (s) => !(s.door === "Open" && s.heat === "On"),
});

export const closed = Oven.rule({
	says: "The door is closed whenever the heat is off",
	holds: (s) => s.door === "Closed" || s.heat === "On",
});

export type State = m.Infer<typeof Oven>;
