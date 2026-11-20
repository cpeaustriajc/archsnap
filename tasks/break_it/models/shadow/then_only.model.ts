import { m } from "@earnest/model";

const limit = 5;

export const Shadow = m.model("Shadow", {
	n: m.int(0, 9).initial(0),
});

export const inc = Shadow.action("the count goes up", {
	input: { limit: m.int(0, 2) },
	when: (s) => s.n < limit,
	then: (s, { limit }) => ({ n: s.n + 1 + limit - limit }),
});

export const low = Shadow.rule({
	says: "The count stays at most two",
	holds: (s) => s.n <= 2,
});

export const high = Shadow.end({
	says: "The count is two or more",
	when: (s) => s.n >= 2,
});
