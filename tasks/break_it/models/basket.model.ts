import { m } from "@earnest/model";

const Item = m.record({ qty: m.int(0, 2) });

export const Basket = m.model("Basket", {
	chosen: Item.optional().initial({ qty: 0 }),
	n: m.int(0, 2).initial(0),
});

export const inc = m.action(Basket, "the count goes up", {
	when: (s) => s.n < 2,
	changes: (s) => ({ n: s.n + 1 }),
});

export const clear = m.action(Basket, "the buyer empties the basket", {
	when: (s) => s.n === 2 && s.chosen !== null,
	changes: () => ({ chosen: null }),
});

export const few = m.rule(Basket, "The basket holds at most one", (s) => m.some(s.chosen).qty <= 1);

export const early = m.rule(Basket, "A late basket holds nothing", (s) => s.n <= 1 || m.some(s.chosen).qty === 0);

export const emptied = m.end(Basket, "The basket is empty", (s) => s.chosen === null);
