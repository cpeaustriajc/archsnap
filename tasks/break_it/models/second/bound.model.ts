import { m } from "@earnest/model";

const Guest = m.enum(["ann", "bob"]);
const Item = m.record({ qty: m.int(0, 2), who: Guest.optional() });

export const Bound = m.model("Bound", {
	chosen: Item.optional().initial(null),
});

export const pick = Bound.action("a guest picks an item", {
	input: { g: Guest },
	when: (s) => s.chosen === null,
	then: (_, { g }) => {
		const fresh = { qty: 0, who: g };
		return { chosen: { ...fresh, qty: 1 } };
	},
});

export const one = Bound.rule({
	says: "A chosen item is one item with a name on it",
	holds: (s) => s.chosen === null || (s.chosen.qty === 1 && s.chosen.who !== null),
});

export const picked = Bound.end({
	says: "An item is chosen",
	when: (s) => s.chosen !== null,
});
