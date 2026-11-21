import { m } from "@earnest/model";

const Stop = m.enum(["North", "South"]);

export const Trail = m.model("Trail", {
	visited: m.seq(Stop, { max: 2 }),
});

export const visit = m.action(Trail, "the walker reaches a stop", {
	input: { stop: Stop },
	when: (s) => s.visited.size < 2,
	changes: (s, { stop }) => ({ visited: s.visited.push(stop) }),
});

export const back = m.action(Trail, "the walker forgets the newest stop", {
	when: (s) => !s.visited.isEmpty,
	changes: (s) => ({ visited: s.visited.pop() }),
});

export const skip = m.action(Trail, "the walker forgets the oldest stop", {
	when: (s) => !s.visited.isEmpty,
	changes: (s) => ({ visited: s.visited.shift() }),
});

export const ends = m.rule(
	Trail,
	"The first and last stops are the ones at each end",
	(s) =>
		(s.visited.isEmpty || s.visited.first === s.visited.get(0)) &&
		(s.visited.size < 2 || s.visited.last === s.visited.get(1)),
);

export const kept = m.rule(
	Trail,
	"Includes, every, some and filter see exactly the stops visited",
	(s) =>
		s.visited.every((stop) => s.visited.includes(stop)) &&
		(!s.visited.includes("North") || s.visited.some((stop) => stop === "North")) &&
		s.visited.filter((stop) => stop === "North").size <= s.visited.size &&
		m.seqOf("South", "North").last === "North",
);

export const ordered = m.rule(
	Trail,
	"North is never visited right after South",
	(s) => !(s.visited.size === 2 && s.visited.first === "South" && s.visited.last === "North"),
);

export type State = m.Infer<typeof Trail>;
