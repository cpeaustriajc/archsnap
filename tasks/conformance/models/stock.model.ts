import { m } from "@earnest/model";

const Item = m.enum(["Pen", "Ink"]);

export const Stock = m.model("Stock", {
	shelf: m.map(Item, m.int(1, 2)),
});

export const stock = m.action(Stock, "the clerk stocks one of an item", {
	input: { item: Item },
	when: (s, { item }) => !s.shelf.has(item),
	changes: (s, { item }) => ({ shelf: s.shelf.set(item, 1) }),
});

export const restock = m.action(Stock, "the clerk adds a second pen", {
	input: { item: Item },
	when: (s, { item }) => item === "Pen" && s.shelf.get(item) === 1,
	changes: (s, { item }) => ({ shelf: s.shelf.set(item, 2) }),
});

export const sell = m.action(Stock, "the clerk sells every one of an item", {
	input: { item: Item },
	when: (s, { item }) => s.shelf.has(item),
	changes: (s, { item }) => ({ shelf: s.shelf.delete(item) }),
});

export const counted = m.rule(
	Stock,
	"The shelf has one entry for each of its keys",
	(s) => s.shelf.size <= 2 && s.shelf.keys.size === s.shelf.size && m.mapOf(["Pen", 1]).get("Pen") === 1,
);

export const known = m.rule(
	Stock,
	"An item is on the shelf or has no count",
	(s) =>
		m.forall(Item, (item) => s.shelf.has(item) || s.shelf.get(item) === null) &&
		(s.shelf.isEmpty || m.exists(Item, (item) => s.shelf.has(item))),
);

export const single = m.rule(Stock, "No item is ever stocked twice over", (s) =>
	m.forall(Item, (item) => s.shelf.get(item) !== 2),
);

export type State = m.Infer<typeof Stock>;
