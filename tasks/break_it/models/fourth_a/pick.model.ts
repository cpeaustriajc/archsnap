import { m } from "@earnest/model";

export const Pick = m.model("Pick", {
	pick: m.int(0, 3).optional().initial(null),
});

export const choose = m.action(Pick, "the visitor picks a number", {
	input: { v: m.int(0, 3) },
	when: (s) => s.pick === null,
	changes: (_, { v }) => ({ pick: v }),
});

export const chosen = m.end(Pick, "A number is picked", (s) => s.pick !== null);

export const small = m.rule(Pick, "The pick is at most three", (s) => m.some(s.pick) <= 3);
