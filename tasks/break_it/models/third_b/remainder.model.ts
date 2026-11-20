import { m } from "@earnest/model";

export const Rem = m.model("Rem", { n: m.int(0, 2).initial(0) });

export const inc = Rem.action("add one", {
	when: (s) => s.n < 2,
	then: (s) => ({ n: s.n + 1 }),
});

export const top = Rem.end({ says: "The count is two", when: (s) => s.n === 2 });

export const below = Rem.rule({
	says: "One less than the count, modulo three, is not 99",
	holds: (s) => m.rem(s.n - 1, 3) !== 99,
});
