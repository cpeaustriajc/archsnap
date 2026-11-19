import { m } from "@earnest/model";

const Stop = m.enum(["North", "South"]);

export const Trail = m.model("Trail", {
	visited: m.seq(Stop, { max: 2 }),
});

export const visit = Trail.action("the walker reaches a stop", {
	input: { stop: Stop },
	when: (s) => s.visited.size < 2,
	then: (s, { stop }) => ({ visited: s.visited.push(stop) }),
});

export const back = Trail.action("the walker forgets the newest stop", {
	when: (s) => !s.visited.isEmpty,
	then: (s) => ({ visited: s.visited.pop() }),
});

export const skip = Trail.action("the walker forgets the oldest stop", {
	when: (s) => !s.visited.isEmpty,
	then: (s) => ({ visited: s.visited.shift() }),
});

export const ends = Trail.rule({
	says: "The first and last stops are the ones at each end",
	holds: (s) =>
		(s.visited.isEmpty || s.visited.first === s.visited.get(0)) &&
		(s.visited.size < 2 || s.visited.last === s.visited.get(1)),
});

export const kept = Trail.rule({
	says: "Includes, every, some and filter see exactly the stops visited",
	holds: (s) =>
		s.visited.every((stop) => s.visited.includes(stop)) &&
		(!s.visited.includes("North") || s.visited.some((stop) => stop === "North")) &&
		s.visited.filter((stop) => stop === "North").size <= s.visited.size &&
		m.seqOf("South", "North").last === "North",
});

export const ordered = Trail.rule({
	says: "North is never visited right after South",
	holds: (s) => !(s.visited.size === 2 && s.visited.first === "South" && s.visited.last === "North"),
});

export type State = m.Infer<typeof Trail>;
