import { m } from "@earnest/model";

const Press = m.enum(["keyboard", "virtual", "mouse"]);

export const AddToCartFocus = m.model("AddToCartFocus", {
  surface: m.enum(["results", "shortlist"]),
  cta: m.enum(["Add", "Adding", "InCart"]).initial("Add"),
  focus: m.enum(["Add", "InCart", "Heart", "Away", "Lost"]).initial("Add"),
  pressBy: Press.optional().initial(null),
  keyboardAdd: m.boolean().initial(false),
});

export const press = m.action(AddToCartFocus, "the buyer presses Add", {
  input: { by: Press },
  when: (s, { by }) => s.cta === "Add" && (by === "mouse" || s.focus === "Add"),
  changes: (_, { by }) => ({ cta: "Adding", pressBy: by, keyboardAdd: by === "keyboard" }),
});

export const focusLandsOnInCart = m.rule(
  AddToCartFocus,
  "After a keyboard add lands, focus is on In cart",
  (s) => s.pressBy !== "keyboard" || s.cta !== "InCart" || s.focus === "InCart",
);

export const inCart = m.end(AddToCartFocus, "The item is in the cart", (s) => s.cta === "InCart");

export const view = m.view(AddToCartFocus, {
  title: "Add to cart focus",
  state: (s) => `${s.cta}, focus on ${s.focus}`,
  group: (s) => s.surface,
  step: { press: "press Add" },
  colors: { broken: "#c62828" },
});

export type State = m.Infer<typeof AddToCartFocus>;
