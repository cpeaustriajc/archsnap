import { m } from "@earnest/model";

export const Oven = m.model("Oven", {
	door: m.enum(["Closed", "Open"]).initial("Closed"),
	heat: m.enum(["Off", "On"]).initial("Off"),
});

export const open = m.action(Oven, "the cook opens the door", {
	when: (s) => s.door === "Closed",
	changes: () => ({ door: "Open" }),
});

export const close = m.action(Oven, "the cook closes the door", {
	when: (s) => s.door === "Open",
	changes: () => ({ door: "Closed" }),
});

export const start = m.action(Oven, "the cook starts the heat", {
	when: (s) => s.heat === "Off" && s.door === "Closed",
	changes: () => ({ heat: "On" }),
});

export const stop = m.action(Oven, "the cook stops the heat", {
	when: (s) => s.heat === "On",
	changes: () => ({ heat: "Off" }),
});

export const safe = m.rule(
	Oven,
	"The oven never heats with the door open",
	(s) => !(s.door === "Open" && s.heat === "On"),
);

export const closed = m.rule(
	Oven,
	"The door is closed whenever the heat is off",
	(s) => s.door === "Closed" || s.heat === "On",
);

export type State = m.Infer<typeof Oven>;
