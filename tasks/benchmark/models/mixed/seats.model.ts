import { m } from "@earnest/model";

const Buyer = m.oneOf(["Kim", "Lee"]);
const Seat = m.record({ row: m.int(1, 3), aisle: m.boolean() });

export const Seats = m.model("Seats", {
	seat: Seat,
	owner: Buyer.optional(),
});

export const sell = m.action(Seats, "a buyer takes the seat", {
	input: { buyer: Buyer },
	when: (s) => s.owner === null,
	changes: (_, { buyer }) => ({ owner: buyer }),
});

export const refund = m.action(Seats, "the buyer returns the seat", {
	when: (s) => s.owner !== null,
	changes: () => ({ owner: null }),
});

export const seated = m.rule(
	Seats,
	"A sold seat is in a real row",
	(s) => s.owner === null || (s.seat.row >= 1 && s.seat.row <= 3),
);
