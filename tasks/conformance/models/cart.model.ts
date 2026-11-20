import { m } from "@earnest/model";

export const Cart = m.model("Cart", {
	surface: m.enum(["results", "shortlist"]),
	cta: m.enum(["Add", "Adding", "InCart"]).initial("Add"),
	focus: m.enum(["Add", "InCart", "Lost"]).initial("Add"),
});

export const press = Cart.action("the buyer presses Add", {
	when: (s) => s.cta === "Add",
	then: () => ({ cta: "Adding" }),
});

export const land = Cart.action("the add lands", {
	when: (s) => s.cta === "Adding",
	then: (s) => ({ cta: "InCart", focus: s.surface === "results" ? "InCart" : "Lost" }),
});

export const focusLands = Cart.rule({
	says: "After an add lands, focus is on In cart",
	holds: (s) => s.cta !== "InCart" || s.focus === "InCart",
});

export const inCart = Cart.end({
	says: "The item is in the cart",
	when: (s) => s.cta === "InCart",
});

export const view = Cart.view({
	title: "Add to cart focus",
	state: (s) => `${s.cta}, focus on ${s.focus}`,
	group: (s) => s.surface,
	step: { press: "press Add" },
	colors: { broken: "#c62828" },
});

export type State = m.Infer<typeof Cart>;
