import { m } from "@earnest/model";

const Who = m.enum(["ann", "bob"]);
const Line = m.record({ qty: m.int(1, 2), tag: m.record({ size: m.int(0, 1) }).optional() });

export const Keyed = m.model("Keyed", {
	byWho: m.map(Who, Line),
});

export const tag = m.action(Keyed, "Ann's line gets a tag", {
	when: (s) => s.byWho.isEmpty,
	changes: () => ({ byWho: m.mapOf(["ann", { qty: 1, tag: { size: 1 } }]) }),
});

export const empty = m.rule(Keyed, "Nobody has a line", (s) => s.byWho.isEmpty);

export const stop = m.end(Keyed, "Any state may stop", () => true);
