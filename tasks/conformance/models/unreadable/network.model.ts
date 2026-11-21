import { m } from "@earnest/model";

export const Quote = m.model("Quote", {
	price: m.int(0, 3).initial(0),
});

export const raise = m.action(Quote, "the price goes up", {
	when: (s) => s.price < 3,
	changes: (s) => ({ price: s.price + 1 }),
});

export const refresh = m.action(Quote, "the app asks the server for the price", {
	when: () => fetch("https://prices.invalid/quote") !== null,
	changes: () => ({ price: 0 }),
});

export const capped = m.rule(Quote, "The price is never above three", (s) => s.price <= 3);
