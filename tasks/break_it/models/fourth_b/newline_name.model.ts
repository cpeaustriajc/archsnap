import { m } from "@earnest/model";

export const Split = m.model("First line\nSecond line", { n: m.int(0, 2).initial(0) });

export const inc = Split.action("one up", {
	when: (s) => s.n < 2,
	then: (s) => ({ n: s.n + 1 }),
});

export const top = Split.end({ says: "The count is two", when: (s) => s.n === 2 });

export const low = Split.rule({ says: "The count stays at most one", holds: (s) => s.n <= 1 });
