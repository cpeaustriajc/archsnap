import { m } from "@earnest/model";

const Buyer = m.oneOf(["Kim", "Lee"]);
const Seat = m.record({ row: m.int(1, 2), aisle: m.boolean() });

export const Ticket = m.model("Ticket", {
	seat: Seat.initial({ row: 1, aisle: false }),
	owner: Buyer.optional().initial(null),
});

export const move = m.action(Ticket, "the seat moves back a row", {
	when: (s) => s.seat.row < 2,
	changes: (s) => ({ seat: { ...s.seat, row: s.seat.row + 1 } }),
});

export const swap = m.action(Ticket, "the seat swaps sides", {
	changes: (s) => ({ seat: { ...s.seat, aisle: !s.seat.aisle } }),
});

export const sell = m.action(Ticket, "a buyer takes the ticket", {
	input: { buyer: Buyer },
	when: (s) => s.owner === null,
	changes: (_, { buyer }) => ({ owner: buyer }),
});

export const refund = m.action(Ticket, "the buyer returns the ticket", {
	when: (s) => s.owner !== null,
	changes: () => ({ owner: null }),
});

export const seated = m.rule(
	Ticket,
	"A sold ticket has a seat in a real row",
	(s) => s.owner === null || (s.seat.row >= 1 && s.seat.row <= 2),
);

export type State = m.Infer<typeof Ticket>;
