import { m } from "@earnest/model";

const Fruit = m.enum(["Pear", "Apple", "Fig"]);
const Name = m.oneOf(["b", "a", "B"]);

export const Shelf = m.model("Shelf", {
	n: m.int(0, 1).initial(0),
	fruit: m.set(Fruit).initial(m.setOf("Pear", "Fig", "Apple")),
	names: m.set(Name).initial(m.setOf("b", "a", "B")),
	stock: m.map(Fruit, m.int(0, 3)).initial(m.mapOf(["Pear", 1], ["Fig", 2], ["Apple", 3])),
	maybe: m.set(Name.optional()).initial(m.setOf("b", null, "a")),
});

export const open = m.action(Shelf, "the shelf is opened", {
	when: (s) => s.n === 0,
	changes: () => ({ n: 1 }),
});

export const shown = m.rule(Shelf, "The shelf is open", (s) => s.n === 1);

export const appleFirst = m.rule(Shelf, "Apple is the first fruit walked", (s) =>
	m.exists(s.fruit, (x) => x === "Apple" || m.div(6, s.n) > 1),
);

export const pearFirst = m.rule(Shelf, "Pear is the first fruit walked", (s) =>
	m.exists(s.fruit, (x) => x === "Pear" || m.div(6, s.n) > 1),
);

export const upperFirst = m.rule(Shelf, "The capital B is the first name walked", (s) =>
	m.exists(s.names, (x) => x === "B" || m.div(6, s.n) > 1),
);

export const lowerFirst = m.rule(Shelf, "The small b is the first name walked", (s) =>
	m.exists(s.names, (x) => x === "b" || m.div(6, s.n) > 1),
);

export const nullFirst = m.rule(Shelf, "The absent name is walked first", (s) =>
	m.exists(s.maybe, (x) => x === null || m.div(6, s.n) > 1),
);

export const aFirstOfSome = m.rule(Shelf, "The small a is walked before the small b", (s) =>
	m.forall(s.maybe, (x) => x === null || x === "a" || m.div(6, s.n) > 1),
);

export const done = m.end(Shelf, "The shelf is open", (s) => s.n === 1);
