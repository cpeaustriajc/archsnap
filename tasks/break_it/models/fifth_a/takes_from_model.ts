import { m } from "./passes_on.model.ts";

export const Climb = m.model("Climb", { n: m.int(0, 3).initial(0) });

export const inc = m.action(Climb, "one up", {
	when: (s) => s.n < 3,
	changes: (s) => ({ n: s.n + 1 }),
});

export const top = m.end(Climb, "At the top", (s) => s.n === 3);

export const low = m.rule(Climb, "The count stays at most one", (s) => s.n <= 1);
