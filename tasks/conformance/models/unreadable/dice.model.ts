import { m } from "@earnest/model";

export const Dice = m.model("Dice", {
	rolls: m.int(0, 2).initial(0),
});

export const roll = m.action(Dice, "the player rolls", {
	when: (s) => s.rolls < 2,

	changes: (s) => ({ rolls: Math.random() < 1 ? s.rolls + 1 : s.rolls }),
});

export const few = m.rule(Dice, "The player rolls at most twice", (s) => s.rolls <= 2);
