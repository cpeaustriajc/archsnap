import { m } from "@earnest/model";

const LIMIT = 5;

export const Early = m.model("Early", {
	n: m.int(0, 3).initial(0),
});

export const inc = m.action(Early, "the count goes up", {
	when: (s) => s.n < 3,
	changes: (s) => ({ n: s.n + 1 }),
});

export const low = m.rule(Early, "The count stays at most the limit", (s) => {
	const most = LIMIT;
	const LIMIT = 1;
	return s.n <= most + LIMIT - LIMIT;
});

export const top = m.end(Early, "The count is at the top", (s) => s.n === 3);
