import { m } from "@earnest/model";

export const Opt = m.model("Opt", { o: m.int(0, 2).optional().initial(null) });

export const set = m.action(Opt, "a number is set", {
	input: { to: m.int(0, 2) },
	when: (s) => s.o === null,
	changes: (_, { to }) => ({ o: to }),
});

export const done = m.end(Opt, "A number is set", (s) => s.o !== null);

export const below = m.rule(Opt, "The number is below three", (s) => (s.o as number) < 3);
