import { m } from "@earnest/model";

const Size = m.oneOf([1, 5, 9]);
const Buyer = m.oneOf(["Kim", "Lee"]);

export const Pick = m.model("Pick", {
	size: Size,
	buyer: Buyer.optional().initial(null),
});

export const resize = m.action(Pick, "the buyer picks another size", {
	input: { to: Size },
	when: (s, { to }) => s.size !== to,
	changes: (_, { to }) => ({ size: to }),
});

export const sell = m.action(Pick, "a buyer takes the pick", {
	input: { who: Buyer },
	when: (s) => s.buyer === null,
	changes: (_, { who }) => ({ buyer: who }),
});

export const refund = m.action(Pick, "the buyer returns the pick", {
	when: (s) => s.buyer !== null,
	changes: () => ({ buyer: null }),
});

export const fits = m.rule(Pick, "A sold pick is never the largest size", (s) => s.buyer === null || s.size <= 5);

export const listed = m.rule(
	Pick,
	"A sold pick has a buyer on the list",
	(s) => s.buyer === null || s.buyer === "Kim" || s.buyer === "Lee",
);

export type State = m.Infer<typeof Pick>;
