import { m } from "@earnest/model";

export const Split = m.model("Split", {
	n: m.int(0, 3).initial(3),
	q: m.int(0, 3).initial(0),
});

export const cut = Split.action("the number is cut", {
	input: { by: m.int(0, 2) },
	then: (s, { by }) => ({ q: m.div(s.n, by) }),
});

export const Below = m.model("Below", {
	n: m.int(-1, 1).initial(-1),
	q: m.int(-1, 1).initial(0),
});

export const slice = Below.action("the number is cut", {
	then: (s) => ({ q: m.div(s.n, 1) }),
});
