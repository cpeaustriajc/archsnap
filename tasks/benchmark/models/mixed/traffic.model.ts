import { m } from "@earnest/model";

const Light = m.enum(["Red", "Green", "Amber"]);

export const Traffic = m.model("Traffic", {
	light: Light.initial("Red"),
	cars: m.int(0, 9).initial(0),
});

export const change = Traffic.action("the light changes", {
	then: (s) => ({
		light: s.light === "Red" ? "Green" : s.light === "Green" ? "Amber" : "Red",
	}),
});

export const pass = Traffic.action("a car passes", {
	when: (s) => s.cars < 9,
	then: (s) => ({ cars: s.cars + 1 }),
});

export const counted = Traffic.rule({
	says: "At most nine cars have passed",
	holds: (s) => s.cars <= 9,
});
