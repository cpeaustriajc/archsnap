import { m } from "@earnest/model";

export const Turnstile = m.model("Turnstile", {
	gate: m.enum(["Locked", "Open"]).initial("Locked"),
	paid: m.boolean().initial(false),
});

export const pay = Turnstile.action("the visitor pays", {
	when: (s) => !s.paid && s.gate === "Locked",
	then: () => ({ paid: true }),
});

export const push = Turnstile.action("the visitor pushes", {
	when: (s) => s.gate === "Locked",
	then: () => ({ gate: "Open" }),
});

export const enter = Turnstile.action("the visitor walks through", {
	when: (s) => s.gate === "Open",
	then: () => ({ gate: "Locked", paid: false }),
});

export const locked = Turnstile.rule({
	says: "The gate stays locked until the visitor pays",
	holds: (s) => s.gate === "Locked" || s.paid,
});

export type State = m.Infer<typeof Turnstile>;
