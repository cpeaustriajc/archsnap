import { m } from "@earnest/model";

const limit = 5;

export const Shadow = m.model("Shadow", {
	n: m.int(0, 9).initial(0),
});

export const inc = Shadow.action("the count goes up", {
	input: { limit: m.int(0, 2) },
	when: (s, input) => s.n < limit && input.limit >= 0,
	then: (s) => ({ n: s.n + 1 }),
});

export const low = Shadow.rule({
	says: "The count stays at most two",
	holds: (s) => s.n <= 2,
});

export const high = Shadow.end({
	says: "The count is two or more",
	when: (s) => s.n >= 2,
});
