import { m } from "@earnest/model";

export const Score = m.model("Score", {
	points: m.int(0, 3).initial(0),
});

export const gain = Score.action("the player gains a point", {
	when: (s) => s.points < 3,
	then: (s) => {
		const base = s.points;
		let next = base + 1;
		return { points: next };
	},
});

export const low = Score.rule({
	says: "The score is never above three",
	holds: (s) => s.points <= 3,
});
