import { m } from "@earnest/model";

const Who = m.enum(["ann", "bob"]);
const Line = m.record({ qty: m.int(1, 2), tag: m.record({ size: m.int(0, 1) }).optional() });

export const Keyed = m.model("Keyed", {
	byWho: m.map(Who, Line),
});

export const tag = Keyed.action("Ann's line gets a tag", {
	when: (s) => s.byWho.isEmpty,
	then: () => ({ byWho: m.mapOf(["ann", { qty: 1, tag: { size: 1 } }]) }),
});

export const empty = Keyed.rule({
	says: "Nobody has a line",
	holds: (s) => s.byWho.isEmpty,
});

export const stop = Keyed.end({
	says: "Any state may stop",
	when: () => true,
});
