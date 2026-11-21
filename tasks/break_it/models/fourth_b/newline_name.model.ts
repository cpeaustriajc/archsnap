import { m } from "@earnest/model";

export const Split = m.model("First line\nSecond line", { n: m.int(0, 2).initial(0) });

export const inc = m.action(Split, "one up", {
	when: (s) => s.n < 2,
	changes: (s) => ({ n: s.n + 1 }),
});

export const top = m.end(Split, "The count is two", (s) => s.n === 2);

export const low = m.rule(Split, "The count stays at most one", (s) => s.n <= 1);
