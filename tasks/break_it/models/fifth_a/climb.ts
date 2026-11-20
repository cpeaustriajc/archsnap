import { m } from "@earnest/model";

export const Climb = m.model("Climb", { n: m.int(0, 3).initial(0) });

export const inc = Climb.action("one up", {
	when: (s) => s.n < 3,
	then: (s) => ({ n: s.n + 1 }),
});

export const top = Climb.end({ says: "At the top", when: (s) => s.n === 3 });

export const low = Climb.rule({ says: "The count stays at most one", holds: (s) => s.n <= 1 });
