import { m } from "@earnest/model";

const Guest = m.enum(["Ann", "Bob"]);

export const Party = m.model("Party", {
	inside: m.set(Guest),
});

export const arrive = m.action(Party, "a guest arrives", {
	input: { guest: Guest },
	when: (s, { guest }) => !s.inside.has(guest) && (guest === "Ann" || s.inside.has("Ann")),
	changes: (s, { guest }) => ({ inside: s.inside.add(guest) }),
});

export const leave = m.action(Party, "a guest leaves", {
	input: { guest: Guest },
	when: (s, { guest }) => s.inside.has(guest),
	changes: (s, { guest }) => ({ inside: s.inside.delete(guest) }),
});

export const algebra = m.rule(
	Party,
	"Union, intersection, difference and subset agree with membership",
	(s) =>
		s.inside.union(m.setOf("Ann")).has("Ann") &&
		s.inside.intersect(m.setOf("Ann")).size <= 1 &&
		s.inside.subtract(s.inside).isEmpty &&
		s.inside.isSubset(m.setOf("Ann", "Bob")),
);

export const members = m.rule(
	Party,
	"Every, some and filter see exactly the guests inside",
	(s) =>
		s.inside.every((guest) => s.inside.has(guest)) &&
		(s.inside.isEmpty || s.inside.some((guest) => guest === "Ann" || guest === "Bob")) &&
		s.inside.filter((guest) => guest === "Ann").size <= 1,
);

export const counted = m.rule(
	Party,
	"The size counts the guests inside",
	(s) =>
		m.forall(Guest, (guest) => !s.inside.has(guest) || s.inside.size >= 1) &&
		(m.exists(Guest, (guest) => !s.inside.has(guest)) || s.inside.size === 2) &&
		m.forall(s.inside, (guest) => guest === "Ann" || guest === "Bob"),
);

export const alone = m.rule(
	Party,
	"Ann and Bob are never inside together",
	(s) => !(s.inside.has("Ann") && s.inside.has("Bob")),
);

export type State = m.Infer<typeof Party>;
