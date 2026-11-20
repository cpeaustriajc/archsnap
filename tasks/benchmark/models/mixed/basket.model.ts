import { m } from "@earnest/model";

const Item = m.enum(["Pen", "Ink", "Cap", "Pad"]);

export const Basket = m.model("Basket", {
	items: m.set(Item),
});

export const add = Basket.action("an item goes in", {
	input: { item: Item },
	when: (s, { item }) => !s.items.has(item),
	then: (s, { item }) => ({ items: s.items.add(item) }),
});

export const remove = Basket.action("an item comes out", {
	input: { item: Item },
	when: (s, { item }) => s.items.has(item),
	then: (s, { item }) => ({ items: s.items.delete(item) }),
});

export const counted = Basket.rule({
	says: "The basket holds at most four items",
	holds: (s) => s.items.size <= 4,
});
