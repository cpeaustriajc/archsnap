import { m } from "@earnest/model";

export const Opt = m.model("Opt", { o: m.int(0, 2).optional().initial(null) });

export const set = m.action(Opt, "the number is set", {
	input: { v: m.int(0, 2) },
	when: (s) => s.o === null,
	changes: (_, { v }) => ({ o: v }),
});

export const done = m.end(Opt, "The number is set", (s) => s.o !== null);

export const below = m.rule(Opt, "The number is below three", (s) => m.some(s.o) < 3);

export const sum = m.rule(Opt, "The number plus one is positive", (s) => m.some(s.o) + 1 > 0);

export const remainder = m.rule(Opt, "The remainder by five is not negative", (s) => m.rem(m.some(s.o), 5) >= 0);
