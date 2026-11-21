import { m } from "@earnest/model";

export const Bump = m.model("Bump", {
	o: m.int(0, 3).optional().initial(null),
	n: m.int(0, 3).initial(0),
});

export const bump = m.action(Bump, "the count takes the number plus one", {
	when: (s) => s.n < 3,
	changes: (s) => ({ n: m.some(s.o) + 1 }),
});

export const settled = m.end(Bump, "The count is at least one", (s) => s.n >= 1);

export const small = m.rule(Bump, "The count is at most three", (s) => s.n <= 3);
