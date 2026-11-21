import { m } from "@earnest/model";

export const Counter = m.model("Counter", {
	count: m.int(0, 3).initial(0),
});

export const increment = m.action(Counter, "the counter goes up by one", {
	when: (s) => s.count < 3,
	changes: (s) => ({ count: s.count + 1 }),
});

export const bounded = m.rule(Counter, "The count is never above three", (s) => s.count <= 3);

export const even = m.rule(Counter, "Twice the count is even", (s) => m.rem(s.count * 2, 2) === 0);

export const full = m.end(Counter, "The counter is full", (s) => s.count === 3);

export type State = m.Infer<typeof Counter>;
