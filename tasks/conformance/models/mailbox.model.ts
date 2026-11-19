import { m } from "@earnest/model";

const Letter = m.enum(["Bill", "Card"]);

export const Mailbox = m.model("Mailbox", {
	inbox: m.queue(Letter, { capacity: 2 }),
});

export const deliver = Mailbox.action("the postman delivers a letter", {
	input: { letter: Letter },
	when: (s) => !s.inbox.isFull,
	then: (s, { letter }) => ({ inbox: s.inbox.send(letter) }),
});

export const read = Mailbox.action("the owner reads the oldest letter", {
	when: (s) => !s.inbox.isEmpty,
	then: (s) => ({ inbox: s.inbox.receive() }),
});

export const empty = Mailbox.action("the owner empties a full inbox", {
	when: (s) => s.inbox.isFull,
	then: (s) => ({ inbox: s.inbox.clear() }),
});

export const bills = Mailbox.rule({
	says: "A full inbox holds a card",
	holds: (s) => s.inbox.size < 2 || s.inbox.some((letter) => letter === "Card"),
});

export const fronted = Mailbox.rule({
	says: "An inbox with letters has a head and includes each of them",
	holds: (s) =>
		(s.inbox.isEmpty || s.inbox.head !== null) &&
		s.inbox.every((letter) => s.inbox.includes(letter)),
});

export type State = m.Infer<typeof Mailbox>;
