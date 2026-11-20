import { m } from "@earnest/model";

export const Quote = m.model("Quote", {
	price: m.int(0, 3).initial(0),
});

export const raise = Quote.action("the price goes up", {
	when: (s) => s.price < 3,
	then: (s) => ({ price: s.price + 1 }),
});

export const refresh = Quote.action("the app asks the server for the price", {
	when: () => fetch("https://prices.invalid/quote") !== null,
	then: () => ({ price: 0 }),
});

export const capped = Quote.rule({
	says: "The price is never above three",
	holds: (s) => s.price <= 3,
});
