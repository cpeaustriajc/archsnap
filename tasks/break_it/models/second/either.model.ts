import { m } from "@earnest/model";

const Guest = m.enum(["ann", "bob"]);
const Item = m.record({ qty: m.int(0, 2), who: Guest.optional() });

export const Either = m.model("Either", {
	chosen: Item.optional().initial(null),
});

export const pick = Either.action("a guest picks an item", {
	input: { g: Guest },
	when: (s) => s.chosen === null,
	then: (_, { g }) => ({ chosen: g === "ann" ? { qty: 0, who: g } : { qty: 1, who: null } }),
});

export const named = Either.rule({
	says: "A chosen item has a name on it",
	holds: (s) => s.chosen === null || s.chosen.who !== null,
});

export const picked = Either.end({
	says: "An item is chosen",
	when: (s) => s.chosen !== null,
});
