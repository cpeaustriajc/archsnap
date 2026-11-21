import { m } from "@earnest/model";

const Item = m.enum(["Pen", "Ink", "Cap"]);

export const Shelf = m.model("Shelf", {
	stock: m.map(Item, m.int(0, 2)),
});

export const put = m.action(Shelf, "the clerk sets the stock of an item", {
	input: { item: Item, quantity: m.int(0, 2) },
	changes: (s, { item, quantity }) => ({ stock: s.stock.set(item, quantity) }),
});

export const drop = m.action(Shelf, "the clerk removes an item", {
	input: { item: Item },
	when: (s, { item }) => s.stock.has(item),
	changes: (s, { item }) => ({ stock: s.stock.delete(item) }),
});

export const counted = m.rule(Shelf, "The shelf has at most three entries", (s) => s.stock.size <= 3);
