import { m } from "@earnest/model";

export const Bill = m.model("Bill", {
	total: m.int(0, 3).initial(0),
});

export const charge = m.action(Bill, "the till adds the tax", {
	when: (s) => s.total < 3,
	changes: (s) => ({ total: s.total + surcharge }),
});

export const low = m.rule(Bill, "The bill is never above three", (s) => s.total <= 3);
