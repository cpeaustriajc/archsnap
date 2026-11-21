import { m } from "@earnest/model";

const HALF = 4503599627370496;

export const Huge = m.model("Huge", {
	n: m.int(0, 2).initial(0),
});

export const grow = m.action(Huge, "the count goes up", {
	when: (s) => s.n < 2,
	changes: (s) => ({ n: s.n + 1 }),
});

export const apart = m.rule(
	Huge,
	"One more than a large number is another number",
	(s) => s.n * HALF * 2 + 1 !== s.n * HALF * 2,
);

export const top = m.end(Huge, "The count is at the top", (s) => s.n === 2);
