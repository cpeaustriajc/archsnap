import { m } from "@earnest/model";

export const Round = m.model("Round", { n: m.int(0, 3).initial(0) });

export const inc = m.action(Round, "add one", {
	when: (s) => s.n < 3,
	changes: (s) => ({ n: s.n + 1 }),
});

export const top = m.end(Round, "The count is three", (s) => s.n === 3);

export const exact = m.rule(
	Round,
	"Adding 2^53 and taking it away again changes nothing",
	(s) => s.n + 9007199254740992 - 9007199254740992 === s.n,
);
