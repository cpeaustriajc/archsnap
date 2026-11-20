import { m } from "@earnest/model";

export const Opt = m.model("Opt", { o: m.int(0, 2).optional().initial(null) });

export const set = Opt.action("a number is set", {
	input: { to: m.int(0, 2) },
	when: (s) => s.o === null,
	then: (_, { to }) => ({ o: to }),
});

export const done = Opt.end({ says: "A number is set", when: (s) => s.o !== null });

export const below = Opt.rule({
	says: "The number is below three",
	holds: (s) => (s.o as number) < 3,
});
