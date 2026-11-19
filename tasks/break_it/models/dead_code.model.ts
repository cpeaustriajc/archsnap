import { m } from "@earnest/model";

export const Climb = m.model("Climb", {
	n: m.int(0, 3).initial(0),
});

export const inc = Climb.action("the count goes up", {
	when: (s) => s.n < 3,
	then: (s) => ({ n: s.n + 1 }),
});

export const top = Climb.end({
	says: "The count is at the top",
	when: (s) => s.n === 3,
});

export const low = Climb.rule({
	says: "The count stays at most three",
	holds: (s) => {
		return s.n <= 3;
		s.n = 4;
	},
});
