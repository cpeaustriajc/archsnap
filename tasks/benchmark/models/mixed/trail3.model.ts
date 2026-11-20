import { m } from "@earnest/model";

const Stop = m.enum(["North", "South", "East"]);

export const Trail = m.model("Trail", {
	visited: m.seq(Stop, { max: 3 }),
});

export const visit = Trail.action("the walker visits a stop", {
	input: { stop: Stop },
	when: (s) => s.visited.size < 3,
	then: (s, { stop }) => ({ visited: s.visited.push(stop) }),
});

export const undo = Trail.action("the walker turns back", {
	when: (s) => s.visited.size > 0,
	then: (s) => ({ visited: s.visited.pop() }),
});

export const counted = Trail.rule({
	says: "The walker has visited at most three stops",
	holds: (s) => s.visited.size <= 3,
});
