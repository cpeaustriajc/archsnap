import { m } from "@earnest/model";

export const Dice = m.model("Dice", {
	rolls: m.int(0, 2).initial(0),
});

export const roll = Dice.action("the player rolls", {
	when: (s) => s.rolls < 2,

	then: (s) => ({ rolls: Math.random() < 1 ? s.rolls + 1 : s.rolls }),
});

export const few = Dice.rule({
	says: "The player rolls at most twice",
	holds: (s) => s.rolls <= 2,
});
