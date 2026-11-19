import { m } from "@earnest/model";

export const Stamp = m.model("Stamp", {
	parity: m.int(0, 1).initial(0),
});

export const mark = Stamp.action("the app stamps the time", {
	then: () => ({ parity: Date.now() % 2 }),
});

export const binary = Stamp.rule({
	says: "The parity is zero or one",
	holds: (s) => s.parity <= 1,
});
