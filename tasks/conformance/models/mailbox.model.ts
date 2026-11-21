import { m } from "@earnest/model";

const Letter = m.enum(["Bill", "Card"]);

export const Mailbox = m.model("Mailbox", {
	inbox: m.queue(Letter, { capacity: 2 }),
});

export const deliver = m.action(Mailbox, "the postman delivers a letter", {
	input: { letter: Letter },
	when: (s) => !s.inbox.isFull,
	changes: (s, { letter }) => ({ inbox: s.inbox.send(letter) }),
});

export const read = m.action(Mailbox, "the owner reads the oldest letter", {
	when: (s) => !s.inbox.isEmpty,
	changes: (s) => ({ inbox: s.inbox.receive() }),
});

export const empty = m.action(Mailbox, "the owner empties a full inbox", {
	when: (s) => s.inbox.isFull,
	changes: (s) => ({ inbox: s.inbox.clear() }),
});

export const bills = m.rule(
	Mailbox,
	"A full inbox holds a card",
	(s) => s.inbox.size < 2 || s.inbox.some((letter) => letter === "Card"),
);

export const fronted = m.rule(
	Mailbox,
	"An inbox with letters has a head and includes each of them",
	(s) => (s.inbox.isEmpty || s.inbox.head !== null) && s.inbox.every((letter) => s.inbox.includes(letter)),
);

export type State = m.Infer<typeof Mailbox>;
