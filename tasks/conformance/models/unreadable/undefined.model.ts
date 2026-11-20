import { m } from "@earnest/model";

export const Bill = m.model("Bill", {
	total: m.int(0, 3).initial(0),
});

export const charge = Bill.action("the till adds the tax", {
	when: (s) => s.total < 3,
	then: (s) => ({ total: s.total + surcharge }),
});

export const low = Bill.rule({
	says: "The bill is never above three",
	holds: (s) => s.total <= 3,
});
