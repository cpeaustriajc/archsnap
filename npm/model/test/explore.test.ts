import assert from "node:assert/strict";
import { test } from "node:test";
import { type Exploration, type Finding, explore, m } from "@earnest/model";
import * as cart from "./models/cart.model.ts";
import * as turnstile from "./models/turnstile.model.ts";

const STUCK = "no action can happen here, and the model does not mark it as an end";

const kinds = (found: Exploration): string[] => found.findings.map((finding) => finding.kind);

const only = (found: Exploration): Finding => {
	const [finding, ...others] = found.findings;
	if (finding === undefined || others.length > 0) {
		throw new Error(`expected one finding, got ${found.findings.length}`);
	}
	return finding;
};

const counter = () => {
	const Counter = m.model("Counter", { count: m.int(0, 3).initial(0) });
	const increment = Counter.action("the count goes up", {
		when: (s) => s.count < 3,
		then: (s) => ({ count: s.count + 1 }),
	});
	const full = Counter.end({ says: "The count is full", when: (s) => s.count === 3 });
	return { Counter, increment, full };
};

const hops = () => {
	const Hops = m.model("Hops", { n: m.int(0, 5).initial(0) });
	const one = Hops.action("add one", { when: (s) => s.n < 5, then: (s) => ({ n: s.n + 1 }) });
	const two = Hops.action("add two", { when: (s) => s.n <= 3, then: (s) => ({ n: s.n + 2 }) });
	const belowFour = Hops.rule({ says: "n stays below four", holds: (s) => s.n < 4 });
	const notFive = Hops.rule({ says: "n is never five", holds: (s) => s.n !== 5 });
	const notNegative = Hops.rule({ says: "n is never negative", holds: (s) => s.n >= 0 });
	const atFive = Hops.end({ says: "n is five", when: (s) => s.n === 5 });
	return { Hops, one, two, belowFour, notFive, notNegative, atFive };
};

test("a counter from 0 to 3 has 4 states and passes", () => {
	const { Counter, ...exports } = counter();
	assert.deepEqual(explore(Counter, { exports }), {
		status: "pass",
		states: 4,
		rules: [],
		findings: [],
		reason: null,
	});
});

test("a queue of capacity 2 over 2 messages has 1 + 2 + 4 = 7 contents", () => {
	const Message = m.enum(["ping", "pong"]);
	const Inbox = m.model("Inbox", { queue: m.queue(Message, { capacity: 2 }) });
	Inbox.action("a message arrives", {
		input: { message: Message },
		when: (s) => !s.queue.isFull,
		then: (s, { message }) => ({ queue: s.queue.send(message) }),
	});
	Inbox.action("a message is handled", {
		when: (s) => !s.queue.isEmpty,
		then: (s) => ({ queue: s.queue.receive() }),
	});
	const found = explore(Inbox);
	assert.equal(found.states, 7);
	assert.equal(found.status, "pass");
	assert.deepEqual(found.findings, []);
});

test("two sets with the same members are one state", () => {
	const Letter = m.enum(["a", "b"]);
	const Seen = m.model("Seen", { seen: m.set(Letter) });
	Seen.action("a letter is seen", {
		input: { letter: Letter },
		then: (s, { letter }) => ({ seen: s.seen.add(letter) }),
	});
	assert.equal(explore(Seen).states, 4);
});

test("a model with two start states searches from both", () => {
	const Lamp = m.model("Lamp", { wired: m.boolean(), level: m.int(0, 1).initial(0) });
	const brighten = Lamp.action("the lamp brightens", {
		when: (s) => s.level === 0,
		then: () => ({ level: 1 }),
	});
	const staysDim = Lamp.rule({ says: "The lamp stays dim", holds: (s) => s.level === 0 });
	const bright = Lamp.end({ says: "The lamp is bright", when: (s) => s.level === 1 });
	const found = explore(Lamp, { exports: { brighten, staysDim, bright } });
	assert.equal(found.states, 4);
	assert.equal(found.status, "fail");
	assert.deepEqual(found.rules, [
		{
			id: "staysDim",
			says: "The lamp stays dim",
			verdict: "broken",
			path: {
				start: [
					{ name: "wired", value: false },
					{ name: "level", value: 0 },
				],
				steps: [
					{
						action: "brighten",
						label: "the lamp brightens",
						input: [],
						changes: [{ field: "level", before: 0, after: 1 }],
						state: [
							{ name: "wired", value: false },
							{ name: "level", value: 1 },
						],
					},
				],
			},
		},
	]);
});

test("each broken rule gets the shortest path to a state that breaks it", () => {
	const { Hops, ...exports } = hops();
	const found = explore(Hops, { exports });
	assert.equal(found.states, 6);
	assert.equal(found.status, "fail");
	assert.deepEqual(found.findings, []);
	assert.deepEqual(found.rules, [
		{
			id: "belowFour",
			says: "n stays below four",
			verdict: "broken",
			path: {
				start: [{ name: "n", value: 0 }],
				steps: [
					{
						action: "two",
						label: "add two",
						input: [],
						changes: [{ field: "n", before: 0, after: 2 }],
						state: [{ name: "n", value: 2 }],
					},
					{
						action: "two",
						label: "add two",
						input: [],
						changes: [{ field: "n", before: 2, after: 4 }],
						state: [{ name: "n", value: 4 }],
					},
				],
			},
		},
		{
			id: "notFive",
			says: "n is never five",
			verdict: "broken",
			path: {
				start: [{ name: "n", value: 0 }],
				steps: [
					{
						action: "one",
						label: "add one",
						input: [],
						changes: [{ field: "n", before: 0, after: 1 }],
						state: [{ name: "n", value: 1 }],
					},
					{
						action: "two",
						label: "add two",
						input: [],
						changes: [{ field: "n", before: 1, after: 3 }],
						state: [{ name: "n", value: 3 }],
					},
					{
						action: "two",
						label: "add two",
						input: [],
						changes: [{ field: "n", before: 3, after: 5 }],
						state: [{ name: "n", value: 5 }],
					},
				],
			},
		},
		{ id: "notNegative", says: "n is never negative", verdict: "holds", path: null },
	]);
});

test("without exports the id of an action or a rule is its sentence", () => {
	const { Hops } = hops();
	const found = explore(Hops);
	assert.deepEqual(
		found.rules.map((rule) => rule.id),
		["n stays below four", "n is never five", "n is never negative"],
	);
	assert.deepEqual(
		found.rules[0]?.path?.steps.map((step) => step.action),
		["add two", "add two"],
	);
});

test("of two actions that break a rule at the same depth, the one declared first is reported", () => {
	const Walk = m.model("Walk", { at: m.enum(["middle", "left", "right"]).initial("middle") });
	const goRight = Walk.action("step right", {
		when: (s) => s.at === "middle",
		then: () => ({ at: "right" }),
	});
	const goLeft = Walk.action("step left", {
		when: (s) => s.at === "middle",
		then: () => ({ at: "left" }),
	});
	const staysPut = Walk.rule({ says: "Nobody moves", holds: (s) => s.at === "middle" });
	const found = explore(Walk, { exports: { goRight, goLeft, staysPut } });
	assert.deepEqual(
		found.rules[0]?.path?.steps.map((step) => step.action),
		["goRight"],
	);
});

test("inputs are tried with the first input varying fastest", () => {
	const Bit = m.int(0, 1);
	const Pick = m.model("Pick", {
		x: Bit.initial(0),
		y: Bit.initial(0),
		done: m.boolean().initial(false),
	});
	const choose = Pick.action("two bits are chosen", {
		input: { a: Bit, b: Bit },
		when: (s) => !s.done,
		then: (_, { a, b }) => ({ x: a, y: b, done: true }),
	});
	const even = Pick.rule({ says: "The bits agree", holds: (s) => s.x === s.y });
	const chosen = Pick.end({ says: "The bits are chosen", when: (s) => s.done });
	const found = explore(Pick, { exports: { choose, even, chosen } });
	assert.equal(found.states, 5);
	assert.deepEqual(found.rules[0]?.path?.steps[0]?.input, [
		{ name: "a", value: 1 },
		{ name: "b", value: 0 },
	]);
});

test("a state with no action that no end covers is stuck", () => {
	const Gate = m.model("Gate", { gate: m.enum(["locked", "open"]).initial("locked") });
	const push = Gate.action("the visitor pushes", {
		when: (s) => s.gate === "locked",
		then: () => ({ gate: "open" }),
	});
	const found = explore(Gate, { exports: { push } });
	assert.equal(found.states, 2);
	assert.equal(found.status, "fail");
	assert.deepEqual(found.findings, [
		{
			kind: "stuck",
			message: STUCK,
			path: {
				start: [{ name: "gate", value: "locked" }],
				steps: [
					{
						action: "push",
						label: "the visitor pushes",
						input: [],
						changes: [{ field: "gate", before: "locked", after: "open" }],
						state: [{ name: "gate", value: "open" }],
					},
				],
			},
		},
	]);
});

test("the same state is not stuck when an end covers it", () => {
	const Gate = m.model("Gate", { gate: m.enum(["locked", "open"]).initial("locked") });
	Gate.action("the visitor pushes", {
		when: (s) => s.gate === "locked",
		then: () => ({ gate: "open" }),
	});
	Gate.end({ says: "The gate is open", when: (s) => s.gate === "open" });
	const found = explore(Gate);
	assert.equal(found.states, 2);
	assert.equal(found.status, "pass");
	assert.deepEqual(found.findings, []);
});

test("an end does not stop a state that still has an action", () => {
	const Loop = m.model("Loop", { on: m.boolean().initial(false) });
	Loop.action("the switch flips", { then: (s) => ({ on: !s.on }) });
	Loop.end({ says: "Anywhere is fine", when: () => true });
	const found = explore(Loop);
	assert.equal(found.states, 2);
	assert.equal(found.status, "pass");
});

test("a value outside its declaration is out of range, counted and not expanded", () => {
	const Climb = m.model("Climb", { n: m.int(0, 2).initial(0) });
	const up = Climb.action("n goes up", { then: (s) => ({ n: s.n + 1 }) });
	const small = Climb.rule({ says: "n stays below three", holds: (s) => s.n < 3 });
	const found = explore(Climb, { exports: { up, small } });
	assert.equal(found.states, 4);
	assert.equal(found.status, "fail");
	assert.deepEqual(kinds(found), ["out-of-range"]);
	assert.equal(only(found).message, "the field `n` leaves the values it declares");
	assert.deepEqual(
		only(found).path.steps.map((step) => step.state),
		[[{ name: "n", value: 1 }], [{ name: "n", value: 2 }], [{ name: "n", value: 3 }]],
	);
	assert.deepEqual(found.rules, [
		{ id: "small", says: "n stays below three", verdict: "holds", path: null },
	]);
});

test("sending to a full queue, pushing past max and retrying past the most are out of range", () => {
	const Letter = m.enum(["a"]);
	const Flood = m.model("Flood", { queue: m.queue(Letter, { capacity: 1 }) });
	Flood.action("a letter arrives", { then: (s) => ({ queue: s.queue.send("a") }) });
	const flooded = explore(Flood);
	assert.equal(flooded.states, 3);
	assert.deepEqual(kinds(flooded), ["out-of-range"]);
	assert.deepEqual(only(flooded).path.steps[1]?.state, [{ name: "queue", value: ["a", "a"] }]);

	const Pile = m.model("Pile", { pile: m.seq(Letter, { max: 2 }) });
	Pile.action("a letter is piled on", { then: (s) => ({ pile: s.pile.push("a") }) });
	const piled = explore(Pile);
	assert.equal(piled.states, 4);
	assert.deepEqual(kinds(piled), ["out-of-range"]);

	const Stubborn = m.model("Stubborn", { tries: m.retry({ max: 1 }) });
	Stubborn.action("it tries again", { then: (s) => ({ tries: s.tries.again() }) });
	const tried = explore(Stubborn);
	assert.equal(tried.states, 3);
	assert.deepEqual(kinds(tried), ["out-of-range"]);
	assert.equal(only(tried).message, "the field `tries` leaves the values it declares");
});

interface Person {
	readonly age: number;
}

const assumedPresent = (who: Person | null): who is Person => who !== undefined;

const People = () =>
	m.model("People", {
		who: m
			.record({ age: m.int(0, 1) })
			.optional()
			.initial(null),
	});

test("a rule that reads a field of null is a fault, and the rule is unknown, not broken", () => {
	const Model = People();
	const adult = Model.rule({
		says: "Whoever is there is an adult",
		holds: (s) => assumedPresent(s.who) && s.who.age === 1,
	});
	const empty = Model.end({ says: "Nobody came", when: (s) => s.who === null });
	const found = explore(Model, { exports: { adult, empty } });
	assert.equal(found.states, 1);
	assert.equal(found.status, "fail");
	assert.deepEqual(found.rules, [
		{ id: "adult", says: "Whoever is there is an adult", verdict: "unknown", path: null },
	]);
	assert.deepEqual(found.findings, [
		{
			kind: "fault",
			message: "the rule `adult` reads a value that is null in this state",
			path: { start: [{ name: "who", value: null }], steps: [] },
		},
	]);
});

test("only a rule that could not be evaluated somewhere and was not seen broken is unknown", () => {
	const Model = m.model("Guest", {
		who: m
			.record({ age: m.int(0, 1) })
			.optional()
			.initial({ age: 0 }),
	});
	const grow = Model.action("the guest grows up", {
		when: (s) => s.who !== null && s.who.age === 0,
		then: () => ({ who: { age: 1 } }),
	});
	const leave = Model.action("the guest leaves", {
		when: (s) => s.who !== null && s.who.age === 1,
		then: () => ({ who: null }),
	});
	const known = Model.rule({
		says: "The guest has an age",
		holds: (s) => assumedPresent(s.who) && s.who.age >= 0,
	});
	const young = Model.rule({
		says: "The guest is young",
		holds: (s) => assumedPresent(s.who) && s.who.age === 0,
	});
	const small = Model.rule({
		says: "Nobody is older than one",
		holds: (s) => s.who === null || s.who.age <= 1,
	});
	const gone = Model.end({ says: "The guest left", when: (s) => s.who === null });
	const found = explore(Model, { exports: { grow, leave, known, young, small, gone } });
	assert.equal(found.states, 3);
	assert.deepEqual(
		found.rules.map((rule) => [rule.id, rule.verdict, rule.path?.steps.length ?? null]),
		[
			["known", "unknown", null],
			["young", "broken", 1],
			["small", "holds", null],
		],
	);
	assert.deepEqual(kinds(found), ["fault"]);
	assert.equal(only(found).path.steps.length, 2);
});

test("a when that faults is a fault, and the state is not stuck", () => {
	const Model = People();
	const age = Model.action("a year passes", {
		when: (s) => assumedPresent(s.who) && s.who.age === 0,
		then: () => ({ who: { age: 1 } }),
	});
	const found = explore(Model, { exports: { age } });
	assert.equal(found.states, 1);
	assert.deepEqual(kinds(found), ["fault"]);
	assert.equal(
		only(found).message,
		"the `when` of the action `age` reads a value that is null in this state",
	);
});

test("an end that faults is a fault, and the state is not stuck", () => {
	const Model = People();
	const gone = Model.end({
		says: "Whoever was there is a newborn",
		when: (s) => assumedPresent(s.who) && s.who.age === 0,
	});
	const found = explore(Model, { exports: { gone } });
	assert.equal(found.states, 1);
	assert.equal(found.status, "fail");
	assert.deepEqual(kinds(found), ["fault"]);
	assert.equal(only(found).message, "the end `gone` reads a value that is null in this state");
});

test("an end that faults beside an end that is true leaves the fault and no stuck state", () => {
	const Model = People();
	const gone = Model.end({
		says: "Whoever was there is a newborn",
		when: (s) => assumedPresent(s.who) && s.who.age === 0,
	});
	const empty = Model.end({ says: "Nobody is there", when: (s) => s.who === null });
	const found = explore(Model, { exports: { gone, empty } });
	assert.deepEqual(kinds(found), ["fault"]);
});

test("an end is not asked where an action can happen, so it can't fault there", () => {
	const Model = People();
	const arrive = Model.action("someone arrives", {
		when: (s) => s.who === null,
		then: () => ({ who: { age: 1 } }),
	});
	const gone = Model.end({
		says: "Whoever is there is an adult",
		when: (s) => assumedPresent(s.who) && s.who.age === 1,
	});
	const found = explore(Model, { exports: { arrive, gone } });
	assert.equal(found.states, 2);
	assert.equal(found.status, "pass");
	assert.deepEqual(kinds(found), []);
});

test("a then that faults gives no next state, and the state is not stuck", () => {
	const Model = People();
	const age = Model.action("a year passes", {
		then: (s) => ({ who: assumedPresent(s.who) ? { age: s.who.age + 1 } : null }),
	});
	const found = explore(Model, { exports: { age } });
	assert.equal(found.states, 1);
	assert.deepEqual(kinds(found), ["fault"]);
	assert.equal(
		only(found).message,
		"the `then` of the action `age` reads a value that is null in this state",
	);
});

test("a remainder by zero is a fault", () => {
	const Share = m.model("Share", {
		left: m.int(0, 5).initial(5),
		people: m.int(0, 2).initial(0),
	});
	const share = Share.action("the rest is shared out", {
		then: (s) => ({ left: s.left % s.people }),
	});
	const found = explore(Share, { exports: { share } });
	assert.equal(found.states, 1);
	assert.equal(found.status, "fail");
	assert.deepEqual(kinds(found), ["fault"]);
	assert.equal(
		only(found).message,
		"the `then` of the action `share` takes a remainder with a negative number or a divisor below 1 in this state",
	);
});

test("a remainder of whole numbers that are not negative is not a fault", () => {
	const Clock = m.model("Clock", { hour: m.int(0, 2).initial(0) });
	Clock.action("an hour passes", { then: (s) => ({ hour: (s.hour + 1) % 3 }) });
	const found = explore(Clock);
	assert.equal(found.states, 3);
	assert.equal(found.status, "pass");
});

test("maxStates stops the search, which is then unfinished with unknown rules", () => {
	const { Counter, ...exports } = counter();
	const small = Counter.rule({ says: "The count stays small", holds: (s) => s.count <= 3 });
	const found = explore(Counter, { maxStates: 2, exports: { ...exports, small } });
	assert.deepEqual(found, {
		status: "unfinished",
		states: 2,
		rules: [{ id: "small", says: "The count stays small", verdict: "unknown", path: null }],
		findings: [],
		reason: "state limit of 2",
	});
});

test("maxStates equal to the number of states lets the search finish", () => {
	const { Counter } = counter();
	const found = explore(Counter, { maxStates: 4 });
	assert.equal(found.status, "pass");
	assert.equal(found.states, 4);
	assert.equal(found.reason, null);
});

test("a state counted at the limit has its rules judged, though the search does not go on from it", () => {
	const { Hops, ...exports } = hops();
	const found = explore(Hops, { maxStates: 5, exports });
	assert.equal(found.states, 5);
	assert.equal(found.status, "fail");
	assert.equal(found.reason, "state limit of 5");
	assert.deepEqual(
		found.rules.map((rule) => [rule.id, rule.verdict, rule.path?.steps.length ?? null]),
		[
			["belowFour", "broken", 2],
			["notFive", "unknown", null],
			["notNegative", "unknown", null],
		],
	);
	assert.deepEqual(kinds(found), []);
});

test("a start state counted at the limit has its rules judged", () => {
	const Dial = m.model("Dial", { n: m.int(0, 2) });
	const notOne = Dial.rule({ says: "n is never one", holds: (s) => s.n !== 1 });
	const counted = explore(Dial, { maxStates: 2, exports: { notOne } });
	assert.equal(counted.states, 2);
	assert.equal(counted.status, "fail");
	assert.equal(counted.reason, "state limit of 2");
	assert.deepEqual(counted.rules[0]?.path, { start: [{ name: "n", value: 1 }], steps: [] });
	const leftOut = explore(Dial, { maxStates: 1, exports: { notOne } });
	assert.equal(leftOut.states, 1);
	assert.equal(leftOut.status, "unfinished");
	assert.equal(leftOut.rules[0]?.verdict, "unknown");
});

test("a rule seen broken before the limit is broken, and the model fails", () => {
	const Long = m.model("Long", { n: m.int(0, 9).initial(0) });
	const up = Long.action("n goes up", { when: (s) => s.n < 9, then: (s) => ({ n: s.n + 1 }) });
	const notOne = Long.rule({ says: "n is never one", holds: (s) => s.n !== 1 });
	const notNine = Long.rule({ says: "n is never nine", holds: (s) => s.n !== 9 });
	const found = explore(Long, { maxStates: 3, exports: { up, notOne, notNine } });
	assert.equal(found.states, 3);
	assert.equal(found.status, "fail");
	assert.equal(found.reason, "state limit of 3");
	assert.deepEqual(found.rules, [
		{
			id: "notOne",
			says: "n is never one",
			verdict: "broken",
			path: {
				start: [{ name: "n", value: 0 }],
				steps: [
					{
						action: "up",
						label: "n goes up",
						input: [],
						changes: [{ field: "n", before: 0, after: 1 }],
						state: [{ name: "n", value: 1 }],
					},
				],
			},
		},
		{ id: "notNine", says: "n is never nine", verdict: "unknown", path: null },
	]);
});

test("the README model: two start states, three presses from each, all stuck", () => {
	const found = explore(cart.AddToCartFocus, { exports: cart });
	assert.equal(found.states, 8);
	assert.equal(found.status, "fail");
	assert.deepEqual(found.rules, [
		{
			id: "focusLandsOnInCart",
			says: "After a keyboard add lands, focus is on In cart",
			verdict: "holds",
			path: null,
		},
	]);
	assert.deepEqual(found.findings, [
		{
			kind: "stuck",
			message: STUCK,
			path: {
				start: [
					{ name: "surface", value: "results" },
					{ name: "cta", value: "Add" },
					{ name: "focus", value: "Add" },
					{ name: "pressBy", value: null },
					{ name: "keyboardAdd", value: false },
				],
				steps: [
					{
						action: "press",
						label: "the buyer presses Add",
						input: [{ name: "by", value: "keyboard" }],
						changes: [
							{ field: "cta", before: "Add", after: "Adding" },
							{ field: "pressBy", before: null, after: "keyboard" },
							{ field: "keyboardAdd", before: false, after: true },
						],
						state: [
							{ name: "surface", value: "results" },
							{ name: "cta", value: "Adding" },
							{ name: "focus", value: "Add" },
							{ name: "pressBy", value: "keyboard" },
							{ name: "keyboardAdd", value: true },
						],
					},
				],
			},
		},
	]);
});

test("the package README's turnstile passes with 5 states", () => {
	assert.deepEqual(explore(turnstile.Turnstile, { exports: turnstile }), {
		status: "pass",
		states: 5,
		rules: [
			{
				id: "staysLocked",
				says: "The gate stays locked until the visitor pays",
				verdict: "holds",
				path: null,
			},
		],
		findings: [],
		reason: null,
	});
});

test("the result is plain data that survives JSON", () => {
	const { Hops, ...exports } = hops();
	const found = explore(Hops, { exports });
	assert.deepEqual(JSON.parse(JSON.stringify(found)), found);
});
