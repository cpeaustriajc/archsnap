import { m } from "@earnest/model";

const Item = m.record({ qty: m.int(0, 2), hot: m.boolean() });

export const Box = m.model("Box", {
	chosen: Item.optional().initial(null),
});

export const pick = Box.action("the buyer picks an item", {
	when: (s) => s.chosen === null,
	then: () => ({ chosen: { qty: 0, hot: false } }),
});

export const more = Box.action("the buyer takes one more", {
	when: (s) => s.chosen !== null && s.chosen.qty < 2,
	then: (s) => ({ chosen: { ...s.chosen!, qty: s.chosen!.qty + 1 } }),
});

export const heat = Box.action("the item is heated", {
	when: (s) => s.chosen !== null && !s.chosen.hot,
	then: (s) => ({ chosen: { ...s.chosen!, hot: true } }),
});

export const cold = Box.rule({
	says: "Two items are never hot",
	holds: (s) => s.chosen === null || !(s.chosen.hot && s.chosen.qty === 2),
});

export const served = Box.end({
	says: "Two hot items are served",
	when: (s) => s.chosen !== null && s.chosen.hot && s.chosen.qty === 2,
});
