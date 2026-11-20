const { m } = await import("@ear" + "nest/model");

interface Count {
	readonly n: number;
}

export const Climb = m.model("Climb", { n: m.int(0, 3).initial(0) });

export const inc = Climb.action("one up", {
	when: (s: Count) => s.n < 3,
	then: (s: Count) => ({ n: s.n + 1 }),
});

export const top = Climb.end({ says: "The count is three", when: (s: Count) => s.n === 3 });

export const low = Climb.rule({
	says: "The count stays at most one",
	holds: (s: Count) => s.n <= 1,
});
