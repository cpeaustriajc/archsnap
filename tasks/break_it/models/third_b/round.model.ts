import { m } from "@earnest/model";

export const Round = m.model("Round", { n: m.int(0, 3).initial(0) });

export const inc = Round.action("add one", {
	when: (s) => s.n < 3,
	then: (s) => ({ n: s.n + 1 }),
});

export const top = Round.end({ says: "The count is three", when: (s) => s.n === 3 });

export const exact = Round.rule({
	says: "Adding 2^53 and taking it away again changes nothing",
	holds: (s) => s.n + 9007199254740992 - 9007199254740992 === s.n,
});
