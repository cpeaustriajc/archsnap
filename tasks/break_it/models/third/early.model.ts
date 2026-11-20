import { m } from "@earnest/model";

const LIMIT = 5;

export const Early = m.model("Early", {
	n: m.int(0, 3).initial(0),
});

export const inc = Early.action("the count goes up", {
	when: (s) => s.n < 3,
	then: (s) => ({ n: s.n + 1 }),
});

export const low = Early.rule({
	says: "The count stays at most the limit",
	holds: (s) => {
		const most = LIMIT;
		const LIMIT = 1;
		return s.n <= most + LIMIT - LIMIT;
	},
});

export const top = Early.end({
	says: "The count is at the top",
	when: (s) => s.n === 3,
});
