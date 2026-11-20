import { m } from "@earnest/model";

export const Quotient = m.model("Quotient", {
	n: m.int(0, 9),
	q: m.int(0, 9).initial(0),
	by: m.int(1, 3).initial(1),
	done: m.boolean().initial(false),
});

export const divide = Quotient.action("the number is divided", {
	input: { by: m.int(1, 3) },
	when: (s) => !s.done,
	then: (s, { by }) => ({ q: m.div(s.n, by), by, done: true }),
});

export const take = Quotient.action("the quotient becomes the number", {
	when: (s) => s.done,
	then: (s) => ({ n: s.q, done: false }),
});

export const floor = Quotient.rule({
	says: "The quotient is the number divided by the divisor, without a fraction",
	holds: (s) => !s.done || (s.q * s.by <= s.n && s.n < s.q * s.by + s.by),
});

export const smaller = Quotient.rule({
	says: "The quotient is always below the number",
	holds: (s) => !s.done || s.q < s.n,
});
