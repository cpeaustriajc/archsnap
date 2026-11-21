import { m } from "@earnest/model";

const limit = 5;

export const Shadow = m.model("Shadow", {
	n: m.int(0, 9).initial(0),
});

export const inc = m.action(Shadow, "the count goes up", {
	input: { limit: m.int(0, 2) },
	when: (s) => s.n < limit,
	changes: (s, { limit }) => ({ n: s.n + 1 + limit - limit }),
});

export const low = m.rule(Shadow, "The count stays at most two", (s) => s.n <= 2);

export const high = m.end(Shadow, "The count is two or more", (s) => s.n >= 2);
