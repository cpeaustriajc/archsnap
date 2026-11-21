import { m } from "@earnest/model";

const STEP = 1;
const MAX = 3;
const LIMIT = 2;

const isHot = (temp: number) => temp >= LIMIT;

const raised = (temp: number) => {
	const next = temp + STEP;
	if (next > MAX) return MAX;
	return next;
};

export const Heater = m.model("Heater", {
	temp: m.int(0, 3).initial(0),
	heating: m.boolean().initial(true),
});

export const warm = m.action(Heater, "the element warms the room", {
	when: (s) => s.heating && s.temp < MAX,
	changes: (s) => ({ temp: raised(s.temp) }),
});

export const cut = m.action(Heater, "the thermostat cuts the power", {
	when: (s) => s.heating && isHot(s.temp),
	changes: () => ({ heating: false }),
});

export const cool = m.action(Heater, "the room cools", {
	when: (s) => !s.heating && s.temp > 0,
	changes: (s) => ({ temp: s.temp - STEP }),
});

export const resume = m.action(Heater, "the thermostat restores the power", {
	when: (s) => !s.heating && s.temp === 0,
	changes: () => ({ heating: true }),
});

export const capped = m.rule(Heater, "The temperature never passes the maximum", (s) => s.temp <= MAX);

export type State = m.Infer<typeof Heater>;
