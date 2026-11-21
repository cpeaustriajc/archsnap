import { m } from "@earnest/model";

const Size = m.int(1, 2);

export const Pile = m.model("Pile", {
	height: m.int(0, 3).initial(0),
	waiting: m.set(Size),
});

export const stack = m.action(Pile, "the mover stacks what is waiting", {
	when: (s) => s.height < 2,
	changes: (s) => {
		for (const size of s.waiting) return { height: s.height + size };
		return { height: s.height };
	},
});

export const low = m.rule(Pile, "The pile is never above three", (s) => s.height <= 3);
