import { m } from "@earnest/model";

export const Counter = m.model("Counter", {
	n: m.int(0, 99).initial(0),
});

export const increment = Counter.action("the counter goes up by one", {
	when: (s) => s.n < 99,
	then: (s) => ({ n: s.n + 1 }),
});

export const bounded = Counter.rule({
	says: "The count is never above ninety-nine",
	holds: (s) => s.n <= 99,
});

export const full = Counter.end({
	says: "The counter is full",
	when: (s) => s.n === 99,
});
