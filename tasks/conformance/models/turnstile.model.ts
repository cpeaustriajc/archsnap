import { m } from "@earnest/model";

export const Turnstile = m.model("Turnstile", {
	gate: m.enum(["Locked", "Open"]).initial("Locked"),
	paid: m.boolean().initial(false),
});

export const pay = m.action(Turnstile, "the visitor pays", {
	when: (s) => !s.paid && s.gate === "Locked",
	changes: () => ({ paid: true }),
});

export const push = m.action(Turnstile, "the visitor pushes", {
	when: (s) => s.gate === "Locked",
	changes: () => ({ gate: "Open" }),
});

export const enter = m.action(Turnstile, "the visitor walks through", {
	when: (s) => s.gate === "Open",
	changes: () => ({ gate: "Locked", paid: false }),
});

export const locked = m.rule(
	Turnstile,
	"The gate stays locked until the visitor pays",
	(s) => s.gate === "Locked" || s.paid,
);

export type State = m.Infer<typeof Turnstile>;
