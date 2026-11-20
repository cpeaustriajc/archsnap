import { m } from "@earnest/model";

const Letter = m.enum(["Bill", "Card", "Note"]);
const Tally = m.record({ a: m.int(0, 9), b: m.int(0, 9) });

export const Ledger = m.model("Ledger", {
	inbox: m.queue(Letter, { capacity: 5 }),
	tally: Tally.initial({ a: 0, b: 0 }),
});

export const deliver = Ledger.action("a letter arrives", {
	input: { letter: Letter },
	when: (s) => !s.inbox.isFull,
	then: (s, { letter }) => ({ inbox: s.inbox.send(letter) }),
});

export const read = Ledger.action("the oldest letter is read", {
	when: (s) => !s.inbox.isEmpty,
	then: (s) => ({ inbox: s.inbox.receive() }),
});

export const bumpA = Ledger.action("the first count goes up", {
	when: (s) => s.tally.a < 9,
	then: (s) => ({ tally: { ...s.tally, a: s.tally.a + 1 } }),
});

export const bumpB = Ledger.action("the second count goes up", {
	when: (s) => s.tally.b < 9,
	then: (s) => ({ tally: { ...s.tally, b: s.tally.b + 1 } }),
});

export const consistent = Ledger.rule({
	says: "The inbox is within its capacity and the counts are on their dials",
	holds: (s) =>
		s.inbox.size <= 5 &&
		s.inbox.every((letter) => s.inbox.includes(letter)) &&
		s.tally.a <= 9 &&
		s.tally.b <= 9,
});

export type State = m.Infer<typeof Ledger>;
