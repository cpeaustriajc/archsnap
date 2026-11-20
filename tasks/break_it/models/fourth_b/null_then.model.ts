import { m } from "@earnest/model";

export const Bump = m.model("Bump", {
	o: m.int(0, 3).optional().initial(null),
	n: m.int(0, 3).initial(0),
});

export const bump = Bump.action("the count takes the number plus one", {
	when: (s) => s.n < 3,
	then: (s) => ({ n: m.some(s.o) + 1 }),
});

export const settled = Bump.end({ says: "The count is at least one", when: (s) => s.n >= 1 });

export const small = Bump.rule({ says: "The count is at most three", holds: (s) => s.n <= 3 });
