import { m } from "@earnest/model";

const Letter = m.enum(["Bill", "Card"]);

export const Flag = m.model("Flag", {
	isFull: m.boolean().initial(false),
	inbox: m.queue(Letter, { capacity: 1 }),
});

export const deliver = Flag.action("the postman delivers a bill", {
	when: (s) => !s.inbox.isFull,
	then: (s) => ({ inbox: s.inbox.send("Bill"), isFull: true }),
});

export const agree = Flag.rule({
	says: "The flag says whether the inbox is full",
	holds: (s) => s.isFull === s.inbox.isFull,
});

export const full = Flag.end({
	says: "The inbox is full",
	when: (s) => s.inbox.isFull,
});
