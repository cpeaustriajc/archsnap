import { m } from "@earnest/model";

export const Quotient = m.model("Quotient", {
	n: m.int(0, 9),
	q: m.int(0, 9).initial(0),
	by: m.int(1, 3).initial(1),
	done: m.boolean().initial(false),
});

export const divide = m.action(Quotient, "the number is divided", {
	input: { by: m.int(1, 3) },
	when: (s) => !s.done,
	changes: (s, { by }) => ({ q: m.div(s.n, by), by, done: true }),
});

export const take = m.action(Quotient, "the quotient becomes the number", {
	when: (s) => s.done,
	changes: (s) => ({ n: s.q, done: false }),
});

export const floor = m.rule(
	Quotient,
	"The quotient is the number divided by the divisor, without a fraction",
	(s) => !s.done || (s.q * s.by <= s.n && s.n < s.q * s.by + s.by),
);

export const smaller = m.rule(Quotient, "The quotient is always below the number", (s) => !s.done || s.q < s.n);
