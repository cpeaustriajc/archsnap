import { m } from "@earnest/model";

export const Counter = m.model("Counter", {
	count: m.int(0, 3).initial(0),
});

export const increment = Counter.action("the counter goes up by one", {
	when: (s) => s.count < 3,
	then: (s) => ({ count: s.count + 1 }),
});

export const bounded = Counter.rule({
	says: "The count is never above three",
	holds: (s) => s.count <= 3,
});

export const even = Counter.rule({
	says: "Twice the count is even",
	holds: (s) => m.rem(s.count * 2, 2) === 0,
});

export const full = Counter.end({
	says: "The counter is full",
	when: (s) => s.count === 3,
});

export type State = m.Infer<typeof Counter>;
