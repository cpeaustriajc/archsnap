import { m } from "@earnest/model";

export const Pick = m.model("Pick", {
	pick: m.int(0, 3).optional().initial(null),
});

export const choose = Pick.action("the visitor picks a number", {
	input: { v: m.int(0, 3) },
	when: (s) => s.pick === null,
	then: (_, { v }) => ({ pick: v }),
});

export const chosen = Pick.end({ says: "A number is picked", when: (s) => s.pick !== null });

export const small = Pick.rule({
	says: "The pick is at most three",
	holds: (s) => m.some(s.pick) <= 3,
});
