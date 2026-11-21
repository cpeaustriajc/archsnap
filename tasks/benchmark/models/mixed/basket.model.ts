import { m } from "@earnest/model";

const Item = m.enum(["Pen", "Ink", "Cap", "Pad"]);

export const Basket = m.model("Basket", {
	items: m.set(Item),
});

export const add = m.action(Basket, "an item goes in", {
	input: { item: Item },
	when: (s, { item }) => !s.items.has(item),
	changes: (s, { item }) => ({ items: s.items.add(item) }),
});

export const remove = m.action(Basket, "an item comes out", {
	input: { item: Item },
	when: (s, { item }) => s.items.has(item),
	changes: (s, { item }) => ({ items: s.items.delete(item) }),
});

export const counted = m.rule(Basket, "The basket holds at most four items", (s) => s.items.size <= 4);
