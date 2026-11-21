import { m } from "@earnest/model";

export const Wide = m.model("Wide", {
	n: m.int(9007199254740990, 9007199254740996),
});

export const inc = m.action(Wide, "add one", {
	when: (s) => s.n < 9007199254740996,
	changes: (s) => ({ n: s.n + 1 }),
});

export const top = m.end(Wide, "The count is at the top", (s) => s.n === 9007199254740996);

export const skips = m.rule(Wide, "The count never is 2^53 + 3", (s) => s.n !== 9007199254740995);
