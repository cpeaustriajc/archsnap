import { m } from "@earnest/model";

const Item = m.record({ qty: m.int(1, 2) });

export const Basket = m.model("Basket", {
	chosen: Item.optional().initial({ qty: 1 }),
	total: m.int(0, 2).initial(0),
});

export const clear = Basket.action("the buyer clears the choice", {
	when: (s) => s.chosen !== null,
	then: () => ({ chosen: null }),
});

export const price = Basket.action("the till prices the choice", {
	when: (s) => s.total === 0,
	then: (s) => ({ total: m.some(s.chosen).qty }),
});

export const capped = Basket.rule({
	says: "The total is never above two",
	holds: (s) => s.total <= 2,
});

export const settled = Basket.end({
	says: "Nothing is chosen",
	when: (s) => s.chosen === null,
});

export type State = m.Infer<typeof Basket>;
