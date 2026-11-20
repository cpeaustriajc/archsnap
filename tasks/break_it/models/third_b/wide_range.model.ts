import { m } from "@earnest/model";

export const Wide = m.model("Wide", {
	n: m.int(9007199254740990, 9007199254740996),
});

export const inc = Wide.action("add one", {
	when: (s) => s.n < 9007199254740996,
	then: (s) => ({ n: s.n + 1 }),
});

export const top = Wide.end({ says: "The count is at the top", when: (s) => s.n === 9007199254740996 });

export const skips = Wide.rule({
	says: "The count never is 2^53 + 3",
	holds: (s) => s.n !== 9007199254740995,
});
