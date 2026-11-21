import { m } from "@earnest/model";

const Item = m.record({ qty: m.int(1, 2) });

export const Basket = m.model("Basket", {
	chosen: Item.optional().initial({ qty: 1 }),
	total: m.int(0, 2).initial(0),
});

export const clear = m.action(Basket, "the buyer clears the choice", {
	when: (s) => s.chosen !== null,
	changes: () => ({ chosen: null }),
});

export const price = m.action(Basket, "the till prices the choice", {
	when: (s) => s.total === 0,
	changes: (s) => ({ total: m.some(s.chosen).qty }),
});

export const capped = m.rule(Basket, "The total is never above two", (s) => s.total <= 2);

export const settled = m.end(Basket, "Nothing is chosen", (s) => s.chosen === null);

export type State = m.Infer<typeof Basket>;
