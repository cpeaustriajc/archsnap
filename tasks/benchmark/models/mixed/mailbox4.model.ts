import { m } from "@earnest/model";

const Letter = m.enum(["Bill", "Card"]);

export const Mailbox = m.model("Mailbox", {
	inbox: m.queue(Letter, { capacity: 4 }),
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

export const counted = Mailbox.rule({
	says: "The inbox holds at most four letters",
	holds: (s) => s.inbox.size <= 4,
});
