import { m } from "@earnest/model";

const Press = m.enum(["keyboard", "virtual", "mouse"]);

export const Cart = m.model("Cart", {
  cta: m.enum(["Add", "Adding", "InCart"]).initial("Add"),
  pressBy: Press.optional().initial(null),
  seen: m.set(Press),
  count: m.int(0, 3).initial(0),
});

export const press = Cart.action("the buyer presses Add", {
  input: { by: Press },
  when: (s, { by }) => s.cta === "Add" && !s.seen.has(by),
  then: (s, { by }) => ({ cta: "Adding", pressBy: by, seen: s.seen.add(by), count: s.count + 1 }),
});

export const counted = Cart.rule({
  says: "The count is never negative",
  holds: (s) => s.count >= 0,
});
