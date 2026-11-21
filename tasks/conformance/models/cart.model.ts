import { m } from "@earnest/model";

export const Cart = m.model("Cart", {
	surface: m.enum(["results", "shortlist"]),
	cta: m.enum(["Add", "Adding", "InCart"]).initial("Add"),
	focus: m.enum(["Add", "InCart", "Lost"]).initial("Add"),
});

export const press = m.action(Cart, "the buyer presses Add", {
	when: (s) => s.cta === "Add",
	changes: () => ({ cta: "Adding" }),
});

export const land = m.action(Cart, "the add lands", {
	when: (s) => s.cta === "Adding",
	changes: (s) => ({ cta: "InCart", focus: s.surface === "results" ? "InCart" : "Lost" }),
});

export const focusLands = m.rule(
	Cart,
	"After an add lands, focus is on In cart",
	(s) => s.cta !== "InCart" || s.focus === "InCart",
);

export const inCart = m.end(Cart, "The item is in the cart", (s) => s.cta === "InCart");

export const view = m.view(Cart, {
	title: "Add to cart focus",
	state: (s) => `${s.cta}, focus on ${s.focus}`,
	group: (s) => s.surface,
	step: { press: "press Add" },
	colors: { broken: "#c62828" },
});

export type State = m.Infer<typeof Cart>;
