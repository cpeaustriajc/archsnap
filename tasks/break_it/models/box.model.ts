import { m } from "@earnest/model";

const Item = m.record({ qty: m.int(0, 2), hot: m.boolean() });

export const Box = m.model("Box", {
	chosen: Item.optional().initial(null),
});

export const pick = m.action(Box, "the buyer picks an item", {
	when: (s) => s.chosen === null,
	changes: () => ({ chosen: { qty: 0, hot: false } }),
});

export const more = m.action(Box, "the buyer takes one more", {
	when: (s) => s.chosen !== null && s.chosen.qty < 2,
	changes: (s) => ({ chosen: { ...m.some(s.chosen), qty: m.some(s.chosen).qty + 1 } }),
});

export const heat = m.action(Box, "the item is heated", {
	when: (s) => s.chosen !== null && !s.chosen.hot,
	changes: (s) => ({ chosen: { ...m.some(s.chosen), hot: true } }),
});

export const cold = m.rule(
	Box,
	"Two items are never hot",
	(s) => s.chosen === null || !(s.chosen.hot && s.chosen.qty === 2),
);

export const served = m.end(
	Box,
	"Two hot items are served",
	(s) => s.chosen !== null && s.chosen.hot && s.chosen.qty === 2,
);
