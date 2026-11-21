import { m } from "@earnest/model";

const Light = m.enum(["Red", "Green", "Amber"]);

export const Traffic = m.model("Traffic", {
	light: Light.initial("Red"),
	cars: m.int(0, 9).initial(0),
});

export const change = m.action(Traffic, "the light changes", {
	changes: (s) => ({
		light: s.light === "Red" ? "Green" : s.light === "Green" ? "Amber" : "Red",
	}),
});

export const pass = m.action(Traffic, "a car passes", {
	when: (s) => s.cars < 9,
	changes: (s) => ({ cars: s.cars + 1 }),
});

export const counted = m.rule(Traffic, "At most nine cars have passed", (s) => s.cars <= 9);
