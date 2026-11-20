import { m } from "@earnest/model";

const Digit = m.int(0, 999);

export const Slow = m.model("Slow", { n: m.int(0, 3).initial(0) });

export const inc = Slow.action("one up", {
	when: (s) => s.n < 3,
	then: (s) => ({ n: s.n + 1 }),
});

export const top = Slow.end({ says: "The count is three", when: (s) => s.n === 3 });

export const sums = Slow.rule({
	says: "No three digits sum below zero",
	holds: (s) =>
		m.forall(Digit, (a) => m.forall(Digit, (b) => m.forall(Digit, (c) => a + b + c + s.n >= 0))),
});
