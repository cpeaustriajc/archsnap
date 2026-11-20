import { m } from "@earnest/model";

const HALF = 4503599627370496;

export const Huge = m.model("Huge", {
	n: m.int(0, 2).initial(0),
});

export const grow = Huge.action("the count goes up", {
	when: (s) => s.n < 2,
	then: (s) => ({ n: s.n + 1 }),
});

export const apart = Huge.rule({
	says: "One more than a large number is another number",
	holds: (s) => s.n * HALF * 2 + 1 !== s.n * HALF * 2,
});

export const top = Huge.end({
	says: "The count is at the top",
	when: (s) => s.n === 2,
});
