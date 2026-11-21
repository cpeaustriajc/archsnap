import { m } from "@earnest/model";

export const Counter = m.model("Counter", {
	n: m.int(0, 99).initial(0),
});

export const increment = m.action(Counter, "the counter goes up by one", {
	when: (s) => s.n < 99,
	changes: (s) => ({ n: s.n + 1 }),
});

export const bounded = m.rule(Counter, "The count is never above ninety-nine", (s) => s.n <= 99);

export const full = m.end(Counter, "The counter is full", (s) => s.n === 99);
