import { m } from "@earnest/model";

const Guest = m.enum(["Ann", "Bob"]);

export const Party = m.model("Party", {
	inside: m.set(Guest),
});

export const arrive = Party.action("a guest arrives", {
	input: { guest: Guest },
	when: (s, { guest }) => !s.inside.has(guest) && (guest === "Ann" || s.inside.has("Ann")),
	then: (s, { guest }) => ({ inside: s.inside.add(guest) }),
});

export const leave = Party.action("a guest leaves", {
	input: { guest: Guest },
	when: (s, { guest }) => s.inside.has(guest),
	then: (s, { guest }) => ({ inside: s.inside.delete(guest) }),
});

export const algebra = Party.rule({
	says: "Union, intersection, difference and subset agree with membership",
	holds: (s) =>
		s.inside.union(m.setOf("Ann")).has("Ann") &&
		s.inside.intersect(m.setOf("Ann")).size <= 1 &&
		s.inside.subtract(s.inside).isEmpty &&
		s.inside.isSubset(m.setOf("Ann", "Bob")),
});

export const members = Party.rule({
	says: "Every, some and filter see exactly the guests inside",
	holds: (s) =>
		s.inside.every((guest) => s.inside.has(guest)) &&
		(s.inside.isEmpty || s.inside.some((guest) => guest === "Ann" || guest === "Bob")) &&
		s.inside.filter((guest) => guest === "Ann").size <= 1,
});

export const counted = Party.rule({
	says: "The size counts the guests inside",
	holds: (s) =>
		m.forall(Guest, (guest) => !s.inside.has(guest) || s.inside.size >= 1) &&
		(m.exists(Guest, (guest) => !s.inside.has(guest)) || s.inside.size === 2) &&
		m.forall(s.inside, (guest) => guest === "Ann" || guest === "Bob"),
});

export const alone = Party.rule({
	says: "Ann and Bob are never inside together",
	holds: (s) => !(s.inside.has("Ann") && s.inside.has("Bob")),
});

export type State = m.Infer<typeof Party>;
