import { m } from "@earnest/model";

const Press = m.enum(["keyboard", "virtual", "mouse"]);

export const Cart = m.model("Cart", {
  cta: m.enum(["Add", "Adding", "InCart"]).initial("Add"),
  pressBy: Press.optional().initial(null),
  seen: m.set(Press),
  count: m.int(0, 3).initial(0),
});

export const press = Cart.action("the buyer presses Add", {
  then: () => ({ cta: "Nope" }),
});
