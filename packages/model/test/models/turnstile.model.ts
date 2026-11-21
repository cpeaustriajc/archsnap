import { m } from "@earnest/model";

export const Turnstile = m.model("Turnstile", {
  gate: m.enum(["locked", "open"]).initial("locked"),
  paid: m.boolean().initial(false),
  visitors: m.int(0, 3).initial(0),
});

export const pay = m.action(Turnstile, "the visitor pays", {
  when: (s) => !s.paid,
  changes: () => ({ paid: true }),
});

export const push = m.action(Turnstile, "the visitor pushes", {
  when: (s) => s.paid && s.visitors < 3,
  changes: (s) => ({ gate: "open", visitors: s.visitors + 1 }),
});

export const staysLocked = m.rule(
  Turnstile,
  "The gate stays locked until the visitor pays",
  (s) => s.gate === "locked" || s.paid,
);

export const full = m.end(Turnstile, "Three visitors went through", (s) => s.visitors === 3);

export type State = m.Infer<typeof Turnstile>;
