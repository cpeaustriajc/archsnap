import { m } from "@earnest/model";

const Buyer = m.oneOf(["Kim", "Lee"]);
const Seat = m.record({ row: m.int(1, 3), aisle: m.boolean() });

export const Seats = m.model("Seats", {
	seat: Seat,
	owner: Buyer.optional(),
});

export const sell = Seats.action("a buyer takes the seat", {
	input: { buyer: Buyer },
	when: (s) => s.owner === null,
	then: (_, { buyer }) => ({ owner: buyer }),
});

export const refund = Seats.action("the buyer returns the seat", {
	when: (s) => s.owner !== null,
	then: () => ({ owner: null }),
});

export const seated = Seats.rule({
	says: "A sold seat is in a real row",
	holds: (s) => s.owner === null || (s.seat.row >= 1 && s.seat.row <= 3),
});
