import { m } from "@earnest/model";

const Tone = m.enum(["Low", "Bright", "Mid"]);
const Spot = m.record({ zebra: m.int(0, 2), apple: m.int(0, 2), tone: Tone });
const Small = m.int(0, 3);

export const Shapes = m.model("Shapes", {
	n: m.int(0, 1).initial(0),
	nums: m.set(Small).initial(m.setOf(3, 1, 2)),
	flags: m.set(m.boolean()).initial(m.setOf(true, false)),
	spots: m
		.set(Spot)
		.initial(
			m.setOf(
				{ zebra: 0, apple: 2, tone: "Low" },
				{ zebra: 2, apple: 1, tone: "Mid" },
				{ zebra: 1, apple: 1, tone: "Bright" },
				{ zebra: 1, apple: 1, tone: "Low" },
			),
		),
	groups: m.set(m.set(Small)).initial(m.setOf(m.setOf(2), m.setOf(1, 3), m.setOf(1, 2), m.setOf<number>())),
	lines: m.set(m.seq(Tone, { max: 2 })).initial(m.setOf(m.seqOf("Mid"), m.seqOf("Low", "Mid"), m.seqOf("Low"), m.seqOf<string>())),
	maps: m
		.set(m.map(Tone, Small))
		.initial(
			m.setOf(
				m.mapOf(["Mid", 1]),
				m.mapOf(["Bright", 2]),
				m.mapOf(["Bright", 1], ["Mid", 1]),
				m.mapOf(["Bright", 1]),
			),
		),
	grid: m.map(Spot, Tone).initial(
		m.mapOf(
			[{ zebra: 2, apple: 0, tone: "Low" }, "Low"],
			[{ zebra: 0, apple: 2, tone: "Low" }, "Mid"],
			[{ zebra: 0, apple: 2, tone: "Bright" }, "Bright"],
		),
	),
});

export const open = m.action(Shapes, "the shapes are opened", {
	when: (s) => s.n === 0,
	changes: () => ({ n: 1 }),
});

export const shown = m.rule(Shapes, "The shapes are open", (s) => s.n === 1);

export const numsAscend = m.rule(Shapes, "The least number is walked first", (s) =>
	m.exists(s.nums, (x) => x === 1 || m.div(6, s.n) > 1),
);

export const numsDoNotDescend = m.rule(Shapes, "The greatest number is walked first", (s) =>
	m.exists(s.nums, (x) => x === 3 || m.div(6, s.n) > 1),
);

export const falseFirst = m.rule(Shapes, "False is walked first", (s) =>
	m.exists(s.flags, (x) => !x || m.div(6, s.n) > 1),
);

export const spotsByNames = m.rule(
	Shapes,
	"The record with the least apple, then zebra, then tone is walked first",
	(s) => m.exists(s.spots, (x) => (x.apple === 1 && x.zebra === 1 && x.tone === "Bright") || m.div(6, s.n) > 1),
);

export const spotsNotInDeclaredOrder = m.rule(Shapes, "The record with the least zebra is walked first", (s) =>
	m.exists(s.spots, (x) => x.zebra === 0 || m.div(6, s.n) > 1),
);

export const emptyGroupFirst = m.rule(Shapes, "The empty group is walked first", (s) =>
	m.exists(s.groups, (x) => x.isEmpty || m.div(6, s.n) > 1),
);

export const prefixFirst = m.rule(Shapes, "A group is walked before every group that starts with it", (s) =>
	m.forall(s.groups, (x) => x.isEmpty || x.has(1) || x.has(2) || m.div(6, s.n) > 1),
);

export const shortLineFirst = m.rule(Shapes, "The empty line is walked first", (s) =>
	m.exists(s.lines, (x) => x.isEmpty || m.div(6, s.n) > 1),
);

export const lowLineBeforeMid = m.rule(
	Shapes,
	"The line of Low alone is walked before the line of Mid alone",
	(s) =>
		m.exists(s.lines, (x) => x.first === "Mid" || m.div(6, s.n) > 1) &&
		m.exists(s.lines, (x) => x.first === "Low" || m.div(6, s.n) > 1),
);

export const brightMapFirst = m.rule(Shapes, "The map of Bright to 1 alone is walked first", (s) =>
	m.exists(s.maps, (x) => x.size === 1 && x.get("Bright") === 1 || m.div(6, s.n) > 1),
);

export const done = m.end(Shapes, "The shapes are open", (s) => s.n === 1);
