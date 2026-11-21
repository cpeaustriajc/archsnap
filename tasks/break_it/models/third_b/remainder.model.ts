import { m } from "@earnest/model";

export const Rem = m.model("Rem", { n: m.int(0, 2).initial(0) });

export const inc = m.action(Rem, "add one", {
	when: (s) => s.n < 2,
	changes: (s) => ({ n: s.n + 1 }),
});

export const top = m.end(Rem, "The count is two", (s) => s.n === 2);

export const below = m.rule(Rem, "One less than the count, modulo three, is not 99", (s) => m.rem(s.n - 1, 3) !== 99);
