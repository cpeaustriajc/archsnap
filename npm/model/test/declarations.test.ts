import assert from "node:assert/strict";
import { test } from "node:test";
import {
	LockValue,
	OwnedLockValue,
	QueueValue,
	RequestValue,
	RetryValue,
	TimerValue,
	m,
} from "@earnest/model";

const Letter = m.enum(["a", "b"]);

test("a boolean, a number, an enum and a oneOf list their values in order", () => {
	assert.deepEqual(m.boolean().values(), [false, true]);
	assert.deepEqual(m.int(-1, 2).values(), [-1, 0, 1, 2]);
	assert.deepEqual(m.enum(["b", "a", "c"]).values(), ["b", "a", "c"]);
	assert.deepEqual(m.oneOf([3, 1, 2]).values(), [3, 1, 2]);
	assert.deepEqual(
		[m.boolean().count, m.int(-1, 2).count, m.enum(["b", "a", "c"]).count],
		[2, 4, 3],
	);
});

test("membership is exact", () => {
	assert.equal(m.boolean().has(true), true);
	assert.equal(m.boolean().has(1), false);
	assert.equal(m.int(0, 3).has(3), true);
	assert.equal(m.int(0, 3).has(4), false);
	assert.equal(m.int(0, 3).has(-1), false);
	assert.equal(m.int(0, 3).has(1.5), false);
	assert.equal(m.int(0, 3).has("1"), false);
	assert.equal(Letter.has("a"), true);
	assert.equal(Letter.has("c"), false);
	assert.equal(Letter.has(null), false);
	assert.equal(m.oneOf([1, 2]).has(2), true);
	assert.equal(m.oneOf([1, 2]).has("2"), false);
});

test("an optional lists null first and has one more value", () => {
	assert.deepEqual(Letter.optional().values(), [null, "a", "b"]);
	assert.equal(Letter.optional().count, 3);
	assert.equal(Letter.optional().has(null), true);
	assert.equal(Letter.optional().has("b"), true);
	assert.equal(Letter.optional().has("c"), false);
});

test("a record lists every combination, the first field varying slowest", () => {
	const Point = m.record({ on: m.boolean(), letter: Letter });
	assert.deepEqual(Point.values(), [
		{ on: false, letter: "a" },
		{ on: false, letter: "b" },
		{ on: true, letter: "a" },
		{ on: true, letter: "b" },
	]);
	assert.equal(Point.count, 4);
	assert.equal(Point.has({ letter: "a", on: true }), true);
	assert.equal(Point.has({ on: true }), false);
	assert.equal(Point.has({ on: true, letter: "a", extra: 1 }), false);
	assert.equal(Point.has({ on: true, letter: "c" }), false);
	assert.equal(Point.has(null), false);
});

test("a set lists every subset", () => {
	const Letters = m.set(Letter);
	assert.deepEqual(Letters.values(), [m.setOf(), m.setOf("a"), m.setOf("b"), m.setOf("a", "b")]);
	assert.equal(Letters.count, 4);
	assert.equal(Letters.has(m.setOf("b")), true);
	assert.equal(Letters.has(m.setOf("c")), false);
	assert.equal(Letters.has(m.seqOf("a")), false);
});

test("a map lists every choice of keys and values", () => {
	const Votes = m.map(Letter, m.boolean());
	assert.equal(Votes.count, 9);
	assert.deepEqual(Votes.values(), [
		m.mapOf(),
		m.mapOf(["a", false]),
		m.mapOf(["a", true]),
		m.mapOf(["b", false]),
		m.mapOf(["b", true]),
		m.mapOf(["a", false], ["b", false]),
		m.mapOf(["a", false], ["b", true]),
		m.mapOf(["a", true], ["b", false]),
		m.mapOf(["a", true], ["b", true]),
	]);
	assert.equal(Votes.has(m.mapOf(["b", true])), true);
	assert.equal(Votes.has(m.mapOf(["c", true])), false);
	assert.equal(Votes.has(m.mapOf(["a", 1])), false);
});

test("a sequence lists every list up to its max, shortest first", () => {
	const Steps = m.seq(Letter, { max: 2 });
	assert.equal(Steps.count, 7);
	assert.deepEqual(Steps.values(), [
		m.seqOf(),
		m.seqOf("a"),
		m.seqOf("b"),
		m.seqOf("a", "a"),
		m.seqOf("a", "b"),
		m.seqOf("b", "a"),
		m.seqOf("b", "b"),
	]);
	assert.equal(Steps.has(m.seqOf("b", "a")), true);
	assert.equal(Steps.has(m.seqOf("a", "a", "a")), false);
	assert.equal(Steps.has(m.seqOf("c")), false);
});

test("a queue lists every content up to its capacity", () => {
	const Inbox = m.queue(Letter, { capacity: 2 });
	assert.equal(Inbox.count, 7);
	assert.deepEqual(
		Inbox.values().map((queue) => queue.toJSON()),
		[[], ["a"], ["b"], ["a", "a"], ["a", "b"], ["b", "a"], ["b", "b"]],
	);
	assert.equal(Inbox.has(QueueValue.of(2, ["a", "b"])), true);
	assert.equal(Inbox.has(QueueValue.of(2, ["a", "b", "a"])), false);
	assert.equal(Inbox.has(QueueValue.of(2, ["c"])), false);
});

test("locks, timers, requests and retries list their few values", () => {
	assert.deepEqual(m.lock().values(), [LockValue.of(false), LockValue.of(true)]);
	assert.deepEqual(m.lock(Letter).values(), [
		OwnedLockValue.of(null),
		OwnedLockValue.of("a"),
		OwnedLockValue.of("b"),
	]);
	assert.equal(m.lock(Letter).has(OwnedLockValue.of("c")), false);
	assert.deepEqual(m.timer().values(), [
		TimerValue.at("idle"),
		TimerValue.at("running"),
		TimerValue.at("fired"),
	]);
	assert.deepEqual(m.request().values(), [
		RequestValue.at("idle"),
		RequestValue.at("pending"),
		RequestValue.at("succeeded"),
		RequestValue.at("failed"),
	]);
	assert.deepEqual(m.retry({ max: 2 }).values(), [
		RetryValue.of(2, 0),
		RetryValue.of(2, 1),
		RetryValue.of(2, 2),
	]);
	assert.equal(m.retry({ max: 2 }).has(RetryValue.of(2, 3)), false);
	assert.equal(m.timer().has(RequestValue.at("idle")), false);
});

test("a field with .initial starts at that value", () => {
	assert.deepEqual(m.boolean().initial(true).starts(), [true]);
	assert.deepEqual(m.int(0, 3).initial(2).starts(), [2]);
	assert.deepEqual(Letter.initial("b").starts(), ["b"]);
	assert.deepEqual(Letter.optional().initial(null).starts(), [null]);
	assert.deepEqual(Letter.optional().initial("a").starts(), ["a"]);
	assert.deepEqual(Letter.initial("b").optional().starts(), ["b"]);
	assert.deepEqual(m.set(Letter).initial(m.setOf("a")).starts(), [m.setOf("a")]);
	assert.deepEqual(m.record({ on: m.boolean() }).initial({ on: true }).starts(), [{ on: true }]);
	assert.deepEqual(m.queue(Letter, { capacity: 2 }).initial(m.seqOf("b", "a")).starts(), [
		QueueValue.of(2, ["b", "a"]),
	]);
});

test("a boolean, number, enum, optional or record without .initial starts at every value", () => {
	assert.deepEqual(m.boolean().starts(), [false, true]);
	assert.deepEqual(m.int(0, 2).starts(), [0, 1, 2]);
	assert.deepEqual(Letter.starts(), ["a", "b"]);
	assert.deepEqual(Letter.optional().starts(), [null, "a", "b"]);
	assert.deepEqual(m.record({ on: m.boolean() }).starts(), [{ on: false }, { on: true }]);
});

test("a collection, lock, timer, request or retry without .initial starts empty, free, idle or at zero", () => {
	assert.deepEqual(m.set(Letter).starts(), [m.setOf()]);
	assert.deepEqual(m.map(Letter, Letter).starts(), [m.mapOf()]);
	assert.deepEqual(m.seq(Letter, { max: 2 }).starts(), [m.seqOf()]);
	assert.deepEqual(m.queue(Letter, { capacity: 2 }).starts(), [QueueValue.of(2, [])]);
	assert.deepEqual(m.lock().starts(), [LockValue.of(false)]);
	assert.deepEqual(m.lock(Letter).starts(), [OwnedLockValue.of(null)]);
	assert.deepEqual(m.timer().starts(), [TimerValue.at("idle")]);
	assert.deepEqual(m.request().starts(), [RequestValue.at("idle")]);
	assert.deepEqual(m.retry({ max: 3 }).starts(), [RetryValue.of(3, 0)]);
});

test(".initial and .optional give new declarations and leave the old one as it was", () => {
	const Count = m.int(0, 2);
	const Started = Count.initial(1);
	const Maybe = Count.optional();
	assert.notEqual(Started, Count);
	assert.deepEqual(Count.starts(), [0, 1, 2]);
	assert.deepEqual(Started.starts(), [1]);
	assert.deepEqual(Maybe.starts(), [null, 0, 1, 2]);
	assert.equal(Count.has(null), false);
	assert.equal(Count.kind, "number");
	assert.equal(Maybe.kind, "optional");
});

test("a declaration that makes no sense is refused when it is written", () => {
	assert.throws(() => m.int(3, 1), /lower one first/);
	assert.throws(() => m.int(0, 1.5), /whole numbers/);
	assert.throws(() => m.enum(["a", "a"]), /different values/);
	assert.throws(() => m.enum([]), /one or more/);
	assert.throws(() => m.oneOf([1, "one"]), /all text, all numbers or all booleans/);
	assert.throws(() => m.int(0, 3).initial(7), /7 is not a value of this number/);
	assert.throws(() => m.set(m.int(0, 1)).initial(m.setOf(0, 5)), /not a value of this set/);
	assert.throws(() => Letter.optional().optional(), /can't be made optional/);
	assert.throws(() => m.lock(Letter).optional(), /can't be made optional/);
	assert.throws(() => m.lock(Letter.optional()), /owner can't be optional/);
	assert.throws(() => m.seq(Letter, { max: -1 }), /not negative/);
	assert.throws(() => m.queue(Letter, { capacity: 0.5 }), /whole number/);
	assert.throws(() => m.retry({ max: -2 }), /not negative/);
	assert.throws(() => m.set(m.int(0, 30)).values(), /too many to list/);
});

test("a value is read back from its JSON form", () => {
	assert.equal(m.set(Letter).fromJson(["b", "a"]), m.setOf("a", "b"));
	assert.equal(m.seq(Letter, { max: 2 }).fromJson(["b", "a"]), m.seqOf("b", "a"));
	assert.equal(m.map(Letter, m.int(0, 1)).fromJson([["a", 1]]), m.mapOf(["a", 1]));
	assert.equal(m.queue(Letter, { capacity: 2 }).fromJson(["a"]), QueueValue.of(2, ["a"]));
	assert.equal(m.lock().fromJson(true), LockValue.of(true));
	assert.equal(m.lock(Letter).fromJson(null), OwnedLockValue.of(null));
	assert.equal(m.lock(Letter).fromJson("a"), OwnedLockValue.of("a"));
	assert.equal(m.timer().fromJson("running"), TimerValue.at("running"));
	assert.equal(m.request().fromJson("failed"), RequestValue.at("failed"));
	assert.equal(m.retry({ max: 2 }).fromJson(1), RetryValue.of(2, 1));
	assert.equal(Letter.optional().fromJson(null), null);
	assert.equal(m.int(0, 3).fromJson(9), 9);
	assert.deepEqual(m.record({ at: m.set(Letter) }).fromJson({ at: ["a"] }), { at: m.setOf("a") });
});

test("JSON of another kind is refused", () => {
	assert.throws(() => m.set(Letter).fromJson("a"), /can't be read as a set/);
	assert.throws(() => m.int(0, 3).fromJson([1]), /can't be read as a number/);
	assert.throws(() => m.timer().fromJson("melted"), /can't be read as a timer/);
	assert.throws(() => m.lock().fromJson("held"), /can't be read as a lock/);
	assert.throws(() => m.map(Letter, Letter).fromJson([["a"]]), /can't be read as a map entry/);
	assert.throws(() => m.record({ on: m.boolean() }).fromJson({ off: true }), /can't be read/);
});

test("a model starts at every combination of its fields' start values, the first field slowest", () => {
	const Pair = m.model("Pair", {
		on: m.boolean(),
		count: m.int(0, 2),
		letter: Letter.initial("b"),
		seen: m.set(Letter),
	});
	const none = m.setOf();
	assert.deepEqual(Pair.starts(), [
		{ on: false, count: 0, letter: "b", seen: none },
		{ on: false, count: 1, letter: "b", seen: none },
		{ on: false, count: 2, letter: "b", seen: none },
		{ on: true, count: 0, letter: "b", seen: none },
		{ on: true, count: 1, letter: "b", seen: none },
		{ on: true, count: 2, letter: "b", seen: none },
	]);
	assert.equal(Pair.has({ on: true, count: 2, letter: "a", seen: m.setOf("a") }), true);
	assert.equal(Pair.has({ on: true, count: 3, letter: "a", seen: none }), false);
	assert.equal(Pair.has({ on: true, count: 2, letter: "a" }), false);
});

test("a model whose fields all have one start value has one start state", () => {
	const Solo = m.model("Solo", { count: m.int(0, 2).initial(0), door: m.lock() });
	assert.deepEqual(Solo.starts(), [{ count: 0, door: LockValue.of(false) }]);
});
