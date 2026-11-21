import { m } from "@earnest/model";

const Letter = m.enum(["Bill", "Card"]);

export const Mailbox = m.model("Mailbox", {
	inbox: m.queue(Letter, { capacity: 4 }),
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

export const counted = m.rule(Mailbox, "The inbox holds at most four letters", (s) => s.inbox.size <= 4);
