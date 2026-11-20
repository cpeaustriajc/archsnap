import { m } from "@earnest/model";

const Item = m.record({ qty: m.int(0, 2) });

export const Basket = m.model("Basket", {
	chosen: Item.optional().initial({ qty: 0 }),
	n: m.int(0, 2).initial(0),
});

export const inc = Basket.action("the count goes up", {
	when: (s) => s.n < 2,
	then: (s) => ({ n: s.n + 1 }),
});

export const clear = Basket.action("the buyer empties the basket", {
	when: (s) => s.n === 2 && s.chosen !== null,
	then: () => ({ chosen: null }),
});

export const few = Basket.rule({
	says: "The basket holds at most one",
	holds: (s) => m.some(s.chosen).qty <= 1,
});

export const early = Basket.rule({
	says: "A late basket holds nothing",
	holds: (s) => s.n <= 1 || m.some(s.chosen).qty === 0,
});

export const emptied = Basket.end({
	says: "The basket is empty",
	when: (s) => s.chosen === null,
});
