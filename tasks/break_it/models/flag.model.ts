import { m } from "@earnest/model";

const Letter = m.enum(["Bill", "Card"]);

export const Flag = m.model("Flag", {
	isFull: m.boolean().initial(false),
	inbox: m.queue(Letter, { capacity: 1 }),
});

export const deliver = m.action(Flag, "the postman delivers a bill", {
	when: (s) => !s.inbox.isFull,
	changes: (s) => ({ inbox: s.inbox.send("Bill"), isFull: true }),
});

export const agree = m.rule(Flag, "The flag says whether the inbox is full", (s) => s.isFull === s.inbox.isFull);

export const full = m.end(Flag, "The inbox is full", (s) => s.inbox.isFull);
