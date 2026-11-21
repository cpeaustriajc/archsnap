import { m } from "@earnest/model";

const Press = m.enum(["keyboard", "virtual", "mouse"]);

export const Cart = m.model("Cart", {
  cta: m.enum(["Add", "Adding", "InCart"]).initial("Add"),
  pressBy: Press.optional().initial(null),
  seen: m.set(Press),
  count: m.int(0, 3).initial(0),
});

export const press = m.action(Cart, "the buyer presses Add", {
  changes: (s, { by }) => ({
    cta: "Adding",
    pressBy: by,
    seen: s.seen.add(by),
    count: s.count + 1,
  }),
  when: (s, { by }) => s.cta === "Add" && !s.seen.has(by),
  input: { by: Press },
});

export const counted = m.rule(Cart, "The count is never negative", (s) => s.count >= 0);

export const added = m.end(Cart, "The item is in the cart", (s) => s.cta === "InCart");

export const view = m.view(Cart, { title: "Cart", step: { press: "press Add" } });
