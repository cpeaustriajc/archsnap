import { m } from "@earnest/model";

export const Prod = m.model("Prod", { n: m.int(0, 3).initial(0) });

export const inc = m.action(Prod, "one up", { when: (s) => s.n < 3, changes: (s) => ({ n: s.n + 1 }) });

export const top = m.end(Prod, "At the top", (s) => s.n === 3);

export const square = m.rule(
	Prod,
	"The square of fifty thousand is positive",
	(s) => (50000 + s.n) * (50000 + s.n) > 0,
);
