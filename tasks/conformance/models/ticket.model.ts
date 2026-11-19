import { m } from "@earnest/model";

const Buyer = m.oneOf(["Kim", "Lee"]);
const Seat = m.record({ row: m.int(1, 2), aisle: m.boolean() });

export const Ticket = m.model("Ticket", {
	seat: Seat.initial({ row: 1, aisle: false }),
	owner: Buyer.optional().initial(null),
});

export const move = Ticket.action("the seat moves back a row", {
	when: (s) => s.seat.row < 2,
	then: (s) => ({ seat: { ...s.seat, row: s.seat.row + 1 } }),
});

export const swap = Ticket.action("the seat swaps sides", {
	then: (s) => ({ seat: { ...s.seat, aisle: !s.seat.aisle } }),
});

export const sell = Ticket.action("a buyer takes the ticket", {
	input: { buyer: Buyer },
	when: (s) => s.owner === null,
	then: (_, { buyer }) => ({ owner: buyer }),
});

export const refund = Ticket.action("the buyer returns the ticket", {
	when: (s) => s.owner !== null,
	then: () => ({ owner: null }),
});

export const seated = Ticket.rule({
	says: "A sold ticket has a seat in a real row",
	holds: (s) => s.owner === null || (s.seat.row >= 1 && s.seat.row <= 2),
});

export type State = m.Infer<typeof Ticket>;
