import { m } from "@earnest/model";

const Size = m.int(1, 2);

export const Pile = m.model("Pile", {
	height: m.int(0, 3).initial(0),
	waiting: m.set(Size),
});

export const stack = Pile.action("the mover stacks what is waiting", {
	when: (s) => s.height < 2,
	then: (s) => {
		for (const size of s.waiting) return { height: s.height + size };
		return { height: s.height };
	},
});

export const low = Pile.rule({
	says: "The pile is never above three",
	holds: (s) => s.height <= 3,
});
