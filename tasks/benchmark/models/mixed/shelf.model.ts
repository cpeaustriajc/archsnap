import { m } from "@earnest/model";

const Item = m.enum(["Pen", "Ink", "Cap"]);

export const Shelf = m.model("Shelf", {
	stock: m.map(Item, m.int(0, 2)),
});

export const put = Shelf.action("the clerk sets the stock of an item", {
	input: { item: Item, quantity: m.int(0, 2) },
	then: (s, { item, quantity }) => ({ stock: s.stock.set(item, quantity) }),
});

export const drop = Shelf.action("the clerk removes an item", {
	input: { item: Item },
	when: (s, { item }) => s.stock.has(item),
	then: (s, { item }) => ({ stock: s.stock.delete(item) }),
});

export const counted = Shelf.rule({
	says: "The shelf has at most three entries",
	holds: (s) => s.stock.size <= 3,
});
