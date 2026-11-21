const { m } = await import("@ear" + "nest/model");

interface Count {
	readonly n: number;
}

export const Climb = m.model("Climb", { n: m.int(0, 3).initial(0) });

export const inc = m.action(Climb, "one up", {
	when: (s: Count) => s.n < 3,
	changes: (s: Count) => ({ n: s.n + 1 }),
});

export const top = m.end(Climb, "The count is three", (s: Count) => s.n === 3);

export const low = m.rule(Climb, "The count stays at most one", (s: Count) => s.n <= 1);
