import { m } from "@earnest/model";

const Digit = m.int(0, 999);

export const Slow = m.model("Slow", { n: m.int(0, 3).initial(0) });

export const inc = m.action(Slow, "one up", {
	when: (s) => s.n < 3,
	changes: (s) => ({ n: s.n + 1 }),
});

export const top = m.end(Slow, "The count is three", (s) => s.n === 3);

export const sums = m.rule(Slow, "No three digits sum below zero", (s) =>
	m.forall(Digit, (a) => m.forall(Digit, (b) => m.forall(Digit, (c) => a + b + c + s.n >= 0))),
);
