import { m } from "@earnest/model";

export const Prod = m.model("Prod", { n: m.int(0, 3).initial(0) });

export const inc = Prod.action("one up", { when: (s) => s.n < 3, then: (s) => ({ n: s.n + 1 }) });

export const top = Prod.end({ says: "At the top", when: (s) => s.n === 3 });

export const square = Prod.rule({
	says: "The square of fifty thousand is positive",
	holds: (s) => (50000 + s.n) * (50000 + s.n) > 0,
});
