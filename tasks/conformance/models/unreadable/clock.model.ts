import { m } from "@earnest/model";

export const Stamp = m.model("Stamp", {
	parity: m.int(0, 1).initial(0),
});

export const mark = m.action(Stamp, "the app stamps the time", {
	changes: () => ({ parity: m.rem(Date.now(), 2) }),
});

export const binary = m.rule(Stamp, "The parity is zero or one", (s) => s.parity <= 1);
