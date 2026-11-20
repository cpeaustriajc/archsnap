import { m } from "@earnest/model";

const Size = m.oneOf([1, 5, 9]);
const Buyer = m.oneOf(["Kim", "Lee"]);

export const Pick = m.model("Pick", {
	size: Size,
	buyer: Buyer.optional().initial(null),
});

export const resize = Pick.action("the buyer picks another size", {
	input: { to: Size },
	when: (s, { to }) => s.size !== to,
	then: (_, { to }) => ({ size: to }),
});

export const sell = Pick.action("a buyer takes the pick", {
	input: { who: Buyer },
	when: (s) => s.buyer === null,
	then: (_, { who }) => ({ buyer: who }),
});

export const refund = Pick.action("the buyer returns the pick", {
	when: (s) => s.buyer !== null,
	then: () => ({ buyer: null }),
});

export const fits = Pick.rule({
	says: "A sold pick is never the largest size",
	holds: (s) => s.buyer === null || s.size <= 5,
});

export const listed = Pick.rule({
	says: "A sold pick has a buyer on the list",
	holds: (s) => s.buyer === null || s.buyer === "Kim" || s.buyer === "Lee",
});

export type State = m.Infer<typeof Pick>;
