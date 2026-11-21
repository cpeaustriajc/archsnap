import { m } from "@earnest/model";

const Digit = m.int(1, 3);

export const Sweep = m.model("Sweep", {
	n: m.int(0, 1).initial(0),
});

export const inc = m.action(Sweep, "one up", {
	when: (s) => s.n < 1,
	changes: (s) => ({ n: s.n + 1 }),
});

export const top = m.end(Sweep, "At the top", (s) => s.n === 1);

export const wide = m.rule(Sweep, "Six shares out widely", (s) => m.forall(Digit, (x) => m.div(6, 3 - x) > 3 + s.n));

export const some = m.rule(Sweep, "Six shares out somewhere", (s) =>
	m.exists(Digit, (x) => m.div(6, 3 - x) >= 3 - s.n),
);
