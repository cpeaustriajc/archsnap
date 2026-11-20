import { m } from "@earnest/model";

export const Opt = m.model("Opt", { o: m.int(0, 2).optional().initial(null) });

export const set = Opt.action("the number is set", {
	input: { v: m.int(0, 2) },
	when: (s) => s.o === null,
	then: (_, { v }) => ({ o: v }),
});

export const done = Opt.end({ says: "The number is set", when: (s) => s.o !== null });

export const below = Opt.rule({ says: "The number is below three", holds: (s) => m.some(s.o) < 3 });

export const sum = Opt.rule({ says: "The number plus one is positive", holds: (s) => m.some(s.o) + 1 > 0 });

export const remainder = Opt.rule({
	says: "The remainder by five is not negative",
	holds: (s) => m.rem(m.some(s.o), 5) >= 0,
});
