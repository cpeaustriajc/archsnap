import { m } from "@earnest/model";

export const Turnstile = m.model("Turnstile", {
  gate: m.enum(["locked", "open"]).initial("locked"),
  paid: m.boolean().initial(false),
  visitors: m.int(0, 3).initial(0),
});

export const pay = Turnstile.action("the visitor pays", {
  when: (s) => !s.paid,
  then: () => ({ paid: true }),
});

export const push = Turnstile.action("the visitor pushes", {
  when: (s) => s.paid && s.visitors < 3,
  then: (s) => ({ gate: "open", visitors: s.visitors + 1 }),
});

export const staysLocked = Turnstile.rule({
  says: "The gate stays locked until the visitor pays",
  holds: (s) => s.gate === "locked" || s.paid,
});

export const full = Turnstile.end({
  says: "Three visitors went through",
  when: (s) => s.visitors === 3,
});

export type State = m.Infer<typeof Turnstile>;
