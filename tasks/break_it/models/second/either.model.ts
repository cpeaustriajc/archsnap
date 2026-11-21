import { m } from "@earnest/model";

const Guest = m.enum(["ann", "bob"]);
const Item = m.record({ qty: m.int(0, 2), who: Guest.optional() });

export const Either = m.model("Either", {
	chosen: Item.optional().initial(null),
});

export const pick = m.action(Either, "a guest picks an item", {
	input: { g: Guest },
	when: (s) => s.chosen === null,
	changes: (_, { g }) => ({ chosen: g === "ann" ? { qty: 0, who: g } : { qty: 1, who: null } }),
});

export const named = m.rule(
	Either,
	"A chosen item has a name on it",
	(s) => s.chosen === null || s.chosen.who !== null,
);

export const picked = m.end(Either, "An item is chosen", (s) => s.chosen !== null);
