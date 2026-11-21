import { m } from "@earnest/model";

export const Score = m.model("Score", {
	points: m.int(0, 3).initial(0),
});

export const gain = m.action(Score, "the player gains a point", {
	when: (s) => s.points < 3,
	changes: (s) => {
		const base = s.points;
		let next = base + 1;
		return { points: next };
	},
});

export const low = m.rule(Score, "The score is never above three", (s) => s.points <= 3);
