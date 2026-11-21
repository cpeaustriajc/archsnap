import { m } from "@earnest/model";

const MOST = 3;
const MOST = 1;

export const Climb = m.model("Climb", { n: m.int(0, 3).initial(0) });

export const inc = m.action(Climb, "one up", {
	when: (s) => s.n < 3,
	changes: (s) => ({ n: s.n + 1 }),
});

export const top = m.end(Climb, "The count is three", (s) => s.n === 3);

export const low = m.rule(Climb, "The count stays at most the most", (s) => s.n <= MOST);
