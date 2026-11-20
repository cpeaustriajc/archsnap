import { m } from "@earnest/model";

const Digit = m.int(1, 3);

export const Sweep = m.model("Sweep", {
	n: m.int(0, 1).initial(0),
});

export const inc = Sweep.action("one up", {
	when: (s) => s.n < 1,
	then: (s) => ({ n: s.n + 1 }),
});

export const top = Sweep.end({ says: "At the top", when: (s) => s.n === 1 });

export const wide = Sweep.rule({
	says: "Six shares out widely",
	holds: (s) => m.forall(Digit, (x) => m.div(6, 3 - x) > 3 + s.n),
});

export const some = Sweep.rule({
	says: "Six shares out somewhere",
	holds: (s) => m.exists(Digit, (x) => m.div(6, 3 - x) >= 3 - s.n),
});
