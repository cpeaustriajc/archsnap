import { m } from "@earnest/model";

const limit = 5;

export const Shadow = m.model("Shadow", {
	n: m.int(0, 9).initial(0),
});

export const inc = m.action(Shadow, "the count goes up", {
	input: { limit: m.int(0, 2) },
	when: (s, input) => s.n < limit && input.limit >= 0,
	changes: (s) => ({ n: s.n + 1 }),
});

export const low = m.rule(Shadow, "The count stays at most two", (s) => s.n <= 2);

export const high = m.end(Shadow, "The count is two or more", (s) => s.n >= 2);
