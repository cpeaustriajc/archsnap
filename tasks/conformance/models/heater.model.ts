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

export const warm = Heater.action("the element warms the room", {
	when: (s) => s.heating && s.temp < MAX,
	then: (s) => ({ temp: raised(s.temp) }),
});

export const cut = Heater.action("the thermostat cuts the power", {
	when: (s) => s.heating && isHot(s.temp),
	then: () => ({ heating: false }),
});

export const cool = Heater.action("the room cools", {
	when: (s) => !s.heating && s.temp > 0,
	then: (s) => ({ temp: s.temp - STEP }),
});

export const resume = Heater.action("the thermostat restores the power", {
	when: (s) => !s.heating && s.temp === 0,
	then: () => ({ heating: true }),
});

export const capped = Heater.rule({
	says: "The temperature never passes the maximum",
	holds: (s) => s.temp <= MAX,
});

export type State = m.Infer<typeof Heater>;
