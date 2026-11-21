import { m } from "@earnest/model";

const Stop = m.enum(["North", "South", "East"]);

export const Trail = m.model("Trail", {
	visited: m.seq(Stop, { max: 3 }),
});

export const visit = m.action(Trail, "the walker visits a stop", {
	input: { stop: Stop },
	when: (s) => s.visited.size < 3,
	changes: (s, { stop }) => ({ visited: s.visited.push(stop) }),
});

export const undo = m.action(Trail, "the walker turns back", {
	when: (s) => s.visited.size > 0,
	changes: (s) => ({ visited: s.visited.pop() }),
});

export const counted = m.rule(Trail, "The walker has visited at most three stops", (s) => s.visited.size <= 3);
