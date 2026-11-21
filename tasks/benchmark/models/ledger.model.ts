import { m } from "@earnest/model";

const Letter = m.enum(["Bill", "Card", "Note"]);
const Tally = m.record({ a: m.int(0, 9), b: m.int(0, 9) });

export const Ledger = m.model("Ledger", {
	inbox: m.queue(Letter, { capacity: 5 }),
	tally: Tally.initial({ a: 0, b: 0 }),
});

export const deliver = m.action(Ledger, "a letter arrives", {
	input: { letter: Letter },
	when: (s) => !s.inbox.isFull,
	changes: (s, { letter }) => ({ inbox: s.inbox.send(letter) }),
});

export const read = m.action(Ledger, "the oldest letter is read", {
	when: (s) => !s.inbox.isEmpty,
	changes: (s) => ({ inbox: s.inbox.receive() }),
});

export const bumpA = m.action(Ledger, "the first count goes up", {
	when: (s) => s.tally.a < 9,
	changes: (s) => ({ tally: { ...s.tally, a: s.tally.a + 1 } }),
});

export const bumpB = m.action(Ledger, "the second count goes up", {
	when: (s) => s.tally.b < 9,
	changes: (s) => ({ tally: { ...s.tally, b: s.tally.b + 1 } }),
});

export const consistent = m.rule(
	Ledger,
	"The inbox is within its capacity and the counts are on their dials",
	(s) => s.inbox.size <= 5 && s.inbox.every((letter) => s.inbox.includes(letter)) && s.tally.a <= 9 && s.tally.b <= 9,
);

export type State = m.Infer<typeof Ledger>;
