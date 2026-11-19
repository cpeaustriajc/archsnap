import assert from "node:assert/strict";
import { test } from "node:test";
import {
	type Json,
	LockValue,
	ModelValue,
	OwnedLockValue,
	QueueValue,
	RequestValue,
	RetryValue,
	TimerValue,
	m,
} from "@earnest/model";

const json = (value: ModelValue): Json => value.toJSON();

test("a set holds each member once and reads by content", () => {
	const letters = m.setOf<string>("b", "a", "b");
	assert.equal(letters.size, 2);
	assert.equal(letters.isEmpty, false);
	assert.equal(m.setOf().isEmpty, true);
	assert.equal(m.setOf().size, 0);
	assert.equal(letters.has("a"), true);
	assert.equal(letters.has("c"), false);
	assert.deepEqual([...letters], ["a", "b"]);
	assert.deepEqual(json(letters), ["a", "b"]);
});

test("set operations give new sets and leave the old one as it was", () => {
	const letters = m.setOf<string>("a", "b");
	assert.deepEqual(json(letters.add("c")), ["a", "b", "c"]);
	assert.deepEqual(json(letters.delete("a")), ["b"]);
	assert.deepEqual(json(letters.delete("z")), ["a", "b"]);
	assert.deepEqual(json(letters.union(m.setOf("b", "c"))), ["a", "b", "c"]);
	assert.deepEqual(json(letters.intersect(m.setOf("b", "c"))), ["b"]);
	assert.deepEqual(json(letters.subtract(m.setOf("b", "c"))), ["a"]);
	assert.deepEqual(json(letters.filter((letter) => letter !== "a")), ["b"]);
	assert.deepEqual(json(letters), ["a", "b"]);
});

test("set questions", () => {
	const letters = m.setOf<string>("a", "b");
	assert.equal(letters.isSubset(m.setOf("a", "b", "c")), true);
	assert.equal(letters.isSubset(m.setOf("a")), false);
	assert.equal(m.setOf<string>().isSubset(letters), true);
	assert.equal(
		letters.every((letter) => letter < "c"),
		true,
	);
	assert.equal(
		letters.every((letter) => letter < "b"),
		false,
	);
	assert.equal(
		letters.some((letter) => letter === "b"),
		true,
	);
	assert.equal(
		letters.some((letter) => letter === "z"),
		false,
	);
	assert.equal(
		m.setOf<string>().every(() => false),
		true,
	);
	assert.equal(
		m.setOf<string>().some(() => true),
		false,
	);
});

test("two sets with the same members are the same value", () => {
	assert.equal(m.setOf("a", "b"), m.setOf("b", "a"));
	assert.equal(m.setOf<string>("a").add("b"), m.setOf("a", "b"));
	assert.equal(m.setOf<string>("a", "b").delete("b"), m.setOf("a"));
	assert.notEqual(m.setOf("a"), m.setOf("b"));
	assert.notEqual(m.setOf("a"), m.seqOf("a"));
	assert.equal(m.setOf(1, 2).key, m.setOf(2, 1).key);
});

test("set members are compared by content, records included", () => {
	const points = m.setOf<{ x: number; y: number }>({ x: 1, y: 2 });
	assert.equal(points.has({ y: 2, x: 1 }), true);
	assert.equal(points.has({ x: 1, y: 3 }), false);
	assert.equal(points.add({ y: 2, x: 1 }).size, 1);
	assert.equal(m.setOf(m.setOf(1), m.setOf(1)).size, 1);
});

test("set members are kept in one order: null, booleans, numbers, then text", () => {
	assert.deepEqual(
		json(m.setOf<string | number | boolean | null>("b", 10, true, null, 2, false, "a")),
		[null, false, true, 2, 10, "a", "b"],
	);
});

test("a map reads by key and gives null for a missing key", () => {
	const ages = m.mapOf(["bob", 2], ["ann", 1]);
	assert.equal(ages.size, 2);
	assert.equal(ages.isEmpty, false);
	assert.equal(m.mapOf().isEmpty, true);
	assert.equal(ages.has("ann"), true);
	assert.equal(ages.get("ann"), 1);
	assert.equal(ages.keys, m.setOf("ann", "bob"));
	assert.deepEqual(json(ages), [
		["ann", 1],
		["bob", 2],
	]);
	const wider = m.mapOf<string, number>(["ann", 1]);
	assert.equal(wider.has("cy"), false);
	assert.equal(wider.get("cy"), null);
});

test("map operations give new maps and leave the old one as it was", () => {
	const ages = m.mapOf<string, number>(["ann", 1]);
	assert.deepEqual(json(ages.set("bob", 2)), [
		["ann", 1],
		["bob", 2],
	]);
	assert.deepEqual(json(ages.set("ann", 5)), [["ann", 5]]);
	assert.deepEqual(json(ages.delete("ann")), []);
	assert.deepEqual(json(ages.delete("zed")), [["ann", 1]]);
	assert.deepEqual(json(ages), [["ann", 1]]);
	assert.equal(ages.set("bob", 2).delete("bob"), ages);
	assert.equal(m.mapOf(["a", 1], ["b", 2]), m.mapOf(["b", 2], ["a", 1]));
	assert.notEqual(m.mapOf(["a", 1]), m.mapOf(["a", 2]));
	assert.equal(m.mapOf(["a", 1], ["a", 2]).get("a"), 2);
});

test("a sequence keeps order and reads by position", () => {
	const steps = m.seqOf("x", "y", "z");
	assert.equal(steps.size, 3);
	assert.equal(steps.isEmpty, false);
	assert.equal(steps.get(0), "x");
	assert.equal(steps.get(2), "z");
	assert.equal(steps.get(3), null);
	assert.equal(steps.get(-1), null);
	assert.equal(steps.first, "x");
	assert.equal(steps.last, "z");
	assert.equal(steps.includes("y"), true);
	assert.deepEqual(json(steps), ["x", "y", "z"]);
	const none = m.seqOf<string>();
	assert.equal(none.isEmpty, true);
	assert.equal(none.first, null);
	assert.equal(none.last, null);
	assert.equal(none.includes("x"), false);
});

test("sequence operations give new sequences and leave the old one as it was", () => {
	const steps = m.seqOf<string>("x", "y");
	assert.deepEqual(json(steps.push("x")), ["x", "y", "x"]);
	assert.deepEqual(json(steps.shift()), ["y"]);
	assert.deepEqual(json(steps.pop()), ["x"]);
	assert.deepEqual(json(steps.filter((step) => step === "y")), ["y"]);
	assert.deepEqual(json(m.seqOf<string>().shift()), []);
	assert.deepEqual(json(m.seqOf<string>().pop()), []);
	assert.deepEqual(json(steps), ["x", "y"]);
	assert.equal(
		steps.every((step) => step !== "z"),
		true,
	);
	assert.equal(
		steps.some((step) => step === "z"),
		false,
	);
	assert.equal(steps.push("z").pop(), steps);
	assert.equal(m.seqOf("x", "y"), m.seqOf("x", "y"));
	assert.notEqual(m.seqOf("x", "y"), m.seqOf("y", "x"));
});

test("a queue is first in, first out and knows when it is full", () => {
	const empty = QueueValue.of<string>(2, []);
	assert.equal(empty.size, 0);
	assert.equal(empty.isEmpty, true);
	assert.equal(empty.isFull, false);
	assert.equal(empty.head, null);
	const one = empty.send("a");
	const two = one.send("b");
	assert.equal(one.isEmpty, false);
	assert.equal(one.isFull, false);
	assert.equal(two.isFull, true);
	assert.equal(two.size, 2);
	assert.equal(two.head, "a");
	assert.equal(two.includes("b"), true);
	assert.equal(two.includes("c"), false);
	assert.deepEqual(json(two), ["a", "b"]);
	assert.deepEqual(json(two.receive()), ["b"]);
	assert.equal(two.receive().head, "b");
	assert.equal(two.clear(), empty);
	assert.equal(empty.receive(), empty);
	assert.equal(
		two.every((item) => item !== "c"),
		true,
	);
	assert.equal(
		two.some((item) => item === "c"),
		false,
	);
	assert.deepEqual(json(one), ["a"]);
	assert.deepEqual(json(empty), []);
	assert.equal(empty.send("a").send("b"), two);
});

test("sending to a full queue is allowed and gives a queue over its capacity", () => {
	const over = QueueValue.of<string>(1, ["a"]).send("b");
	assert.equal(over.size, 2);
	assert.equal(over.isFull, true);
});

test("a lock without an owner is free or held", () => {
	const free = LockValue.of(false);
	assert.equal(free.isFree, true);
	assert.equal(free.isHeld, false);
	const held = free.acquire();
	assert.equal(held.isFree, false);
	assert.equal(held.isHeld, true);
	assert.equal(held.release(), free);
	assert.equal(free.isFree, true);
	assert.equal(held, LockValue.of(true));
	assert.equal(json(free), false);
	assert.equal(json(held), true);
});

test("a lock with an owner remembers who holds it", () => {
	const free = OwnedLockValue.of<string>(null);
	assert.equal(free.isFree, true);
	assert.equal(free.isHeld, false);
	assert.equal(free.holder, null);
	assert.equal(free.isHeldBy("ann"), false);
	const held = free.acquire("ann");
	assert.equal(held.isFree, false);
	assert.equal(held.isHeld, true);
	assert.equal(held.holder, "ann");
	assert.equal(held.isHeldBy("ann"), true);
	assert.equal(held.isHeldBy("bob"), false);
	assert.equal(held.release(), free);
	assert.equal(free.holder, null);
	assert.equal(held, OwnedLockValue.of("ann"));
	assert.equal(json(free), null);
	assert.equal(json(held), "ann");
});

test("a timer is idle, running or fired", () => {
	const idle = TimerValue.at("idle");
	assert.deepEqual([idle.isIdle, idle.isRunning, idle.hasFired], [true, false, false]);
	const running = idle.start();
	assert.deepEqual([running.isIdle, running.isRunning, running.hasFired], [false, true, false]);
	const fired = running.fire();
	assert.deepEqual([fired.isIdle, fired.isRunning, fired.hasFired], [false, false, true]);
	assert.equal(running.cancel(), idle);
	assert.equal(fired.reset(), idle);
	assert.equal(idle.isIdle, true);
	assert.equal(fired, TimerValue.at("fired"));
	assert.deepEqual([json(idle), json(running), json(fired)], ["idle", "running", "fired"]);
});

test("a request is idle, pending, succeeded or failed", () => {
	const idle = RequestValue.at("idle");
	const pending = idle.send();
	const succeeded = pending.succeed();
	const failed = pending.fail();
	assert.deepEqual(
		[idle.isIdle, idle.isPending, idle.hasSucceeded, idle.hasFailed, idle.isSettled],
		[true, false, false, false, false],
	);
	assert.deepEqual(
		[pending.isIdle, pending.isPending, pending.hasSucceeded, pending.hasFailed, pending.isSettled],
		[false, true, false, false, false],
	);
	assert.deepEqual(
		[succeeded.isPending, succeeded.hasSucceeded, succeeded.hasFailed, succeeded.isSettled],
		[false, true, false, true],
	);
	assert.deepEqual(
		[failed.isPending, failed.hasSucceeded, failed.hasFailed, failed.isSettled],
		[false, false, true, true],
	);
	assert.equal(failed.reset(), idle);
	assert.equal(pending.isPending, true);
	assert.equal(succeeded, RequestValue.at("succeeded"));
	assert.deepEqual(
		[json(idle), json(pending), json(succeeded), json(failed)],
		["idle", "pending", "succeeded", "failed"],
	);
});

test("a retry counts attempts up to its most", () => {
	const fresh = RetryValue.of(2, 0);
	assert.deepEqual([fresh.attempts, fresh.canRetry, fresh.isExhausted], [0, true, false]);
	const once = fresh.again();
	assert.deepEqual([once.attempts, once.canRetry, once.isExhausted], [1, true, false]);
	const twice = once.again();
	assert.deepEqual([twice.attempts, twice.canRetry, twice.isExhausted], [2, false, true]);
	assert.equal(twice.again().attempts, 3);
	assert.equal(twice.reset(), fresh);
	assert.equal(fresh.attempts, 0);
	assert.equal(once, RetryValue.of(2, 1));
	assert.notEqual(RetryValue.of(3, 1), RetryValue.of(2, 1));
	assert.equal(json(twice), 2);
});

test("forall and exists range over a declaration or a collection", () => {
	const Digit = m.int(0, 3);
	assert.equal(
		m.forall(Digit, (digit) => digit < 4),
		true,
	);
	assert.equal(
		m.forall(Digit, (digit) => digit < 3),
		false,
	);
	assert.equal(
		m.exists(Digit, (digit) => digit === 3),
		true,
	);
	assert.equal(
		m.exists(Digit, (digit) => digit === 4),
		false,
	);
	assert.equal(
		m.forall(m.setOf(1, 2), (digit) => digit > 0),
		true,
	);
	assert.equal(
		m.exists(m.seqOf(1, 2), (digit) => digit > 1),
		true,
	);
	assert.equal(
		m.exists(QueueValue.of(2, [1, 2]), (digit) => digit > 2),
		false,
	);
	assert.equal(
		m.forall(m.setOf<number>(), () => false),
		true,
	);
	assert.equal(
		m.exists(m.setOf<number>(), () => true),
		false,
	);
	assert.equal(
		m.exists(m.mapOf(["a", 1]), ([key, value]) => key === "a" && value === 1),
		true,
	);
});

test("a record put in a collection can no longer be changed", () => {
	const point = { x: 1 };
	const points = m.seqOf(point);
	assert.equal(Object.isFrozen(point), true);
	assert.deepEqual(json(points), [{ x: 1 }]);
});
