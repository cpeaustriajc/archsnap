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

export const open = Shelf.action("the shelf is opened", {
	when: (s) => s.n === 0,
	then: () => ({ n: 1 }),
});

export const shown = Shelf.rule({
	says: "The shelf is open",
	holds: (s) => s.n === 1,
});

export const appleFirst = Shelf.rule({
	says: "Apple is the first fruit walked",
	holds: (s) => m.exists(s.fruit, (x) => x === "Apple" || m.div(6, s.n) > 1),
});

export const pearFirst = Shelf.rule({
	says: "Pear is the first fruit walked",
	holds: (s) => m.exists(s.fruit, (x) => x === "Pear" || m.div(6, s.n) > 1),
});

export const upperFirst = Shelf.rule({
	says: "The capital B is the first name walked",
	holds: (s) => m.exists(s.names, (x) => x === "B" || m.div(6, s.n) > 1),
});

export const lowerFirst = Shelf.rule({
	says: "The small b is the first name walked",
	holds: (s) => m.exists(s.names, (x) => x === "b" || m.div(6, s.n) > 1),
});

export const nullFirst = Shelf.rule({
	says: "The absent name is walked first",
	holds: (s) => m.exists(s.maybe, (x) => x === null || m.div(6, s.n) > 1),
});

export const aFirstOfSome = Shelf.rule({
	says: "The small a is walked before the small b",
	holds: (s) =>
		m.forall(s.maybe, (x) => x === null || x === "a" || m.div(6, s.n) > 1),
});

export const done = Shelf.end({
	says: "The shelf is open",
	when: (s) => s.n === 1,
});
