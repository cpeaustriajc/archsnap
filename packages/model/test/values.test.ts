import { expect, test } from "vitest";
import {
  type Json,
  LockValue,
  ModelValue,
  OwnedLockValue,
  QueueValue,
  RequestValue,
  RetryValue,
  TimerValue,
  compareText,
  m,
} from "@earnest/model";

const json = (value: ModelValue): Json => value.toJSON();

test("a set holds each member once and reads by content", () => {
  const letters = m.setOf<string>("b", "a", "b");
  expect(letters.size).toBe(2);
  expect(letters.isEmpty).toBe(false);
  expect(m.setOf().isEmpty).toBe(true);
  expect(m.setOf().size).toBe(0);
  expect(letters.has("a")).toBe(true);
  expect(letters.has("c")).toBe(false);
  expect([...letters]).toStrictEqual(["a", "b"]);
  expect(json(letters)).toStrictEqual(["a", "b"]);
});

test("set operations give new sets and leave the old one as it was", () => {
  const letters = m.setOf<string>("a", "b");
  expect(json(letters.add("c"))).toStrictEqual(["a", "b", "c"]);
  expect(json(letters.delete("a"))).toStrictEqual(["b"]);
  expect(json(letters.delete("z"))).toStrictEqual(["a", "b"]);
  expect(json(letters.union(m.setOf("b", "c")))).toStrictEqual(["a", "b", "c"]);
  expect(json(letters.intersect(m.setOf("b", "c")))).toStrictEqual(["b"]);
  expect(json(letters.subtract(m.setOf("b", "c")))).toStrictEqual(["a"]);
  expect(json(letters.filter((letter) => letter !== "a"))).toStrictEqual(["b"]);
  expect(json(letters)).toStrictEqual(["a", "b"]);
});

test("set questions", () => {
  const letters = m.setOf<string>("a", "b");
  expect(letters.isSubset(m.setOf("a", "b", "c"))).toBe(true);
  expect(letters.isSubset(m.setOf("a"))).toBe(false);
  expect(m.setOf<string>().isSubset(letters)).toBe(true);
  expect(letters.every((letter) => letter < "c")).toBe(true);
  expect(letters.every((letter) => letter < "b")).toBe(false);
  expect(letters.some((letter) => letter === "b")).toBe(true);
  expect(letters.some((letter) => letter === "z")).toBe(false);
  expect(m.setOf<string>().every(() => false)).toBe(true);
  expect(m.setOf<string>().some(() => true)).toBe(false);
});

test("two sets with the same members are the same value", () => {
  expect(m.setOf("a", "b")).toBe(m.setOf("b", "a"));
  expect(m.setOf<string>("a").add("b")).toBe(m.setOf("a", "b"));
  expect(m.setOf<string>("a", "b").delete("b")).toBe(m.setOf("a"));
  expect(m.setOf("a")).not.toBe(m.setOf("b"));
  expect(m.setOf("a")).not.toBe(m.seqOf("a"));
  expect(m.setOf(1, 2).key).toBe(m.setOf(2, 1).key);
});

test("set members are compared by content, records included", () => {
  const points = m.setOf<{ x: number; y: number }>({ x: 1, y: 2 });
  expect(points.has({ y: 2, x: 1 })).toBe(true);
  expect(points.has({ x: 1, y: 3 })).toBe(false);
  expect(points.add({ y: 2, x: 1 }).size).toBe(1);
  expect(m.setOf(m.setOf(1), m.setOf(1)).size).toBe(1);
});

test("set members are kept in one order: null, booleans, numbers, then text", () => {
  expect(
    json(m.setOf<string | number | boolean | null>("b", 10, true, null, 2, false, "a")),
  ).toStrictEqual([null, false, true, 2, 10, "a", "b"]);
});

test("a map reads by key and gives null for a missing key", () => {
  const ages = m.mapOf(["bob", 2], ["ann", 1]);
  expect(ages.size).toBe(2);
  expect(ages.isEmpty).toBe(false);
  expect(m.mapOf().isEmpty).toBe(true);
  expect(ages.has("ann")).toBe(true);
  expect(ages.get("ann")).toBe(1);
  expect(ages.keys).toBe(m.setOf("ann", "bob"));
  expect(json(ages)).toStrictEqual([
    ["ann", 1],
    ["bob", 2],
  ]);
  const wider = m.mapOf<string, number>(["ann", 1]);
  expect(wider.has("cy")).toBe(false);
  expect(wider.get("cy")).toBe(null);
});

test("map operations give new maps and leave the old one as it was", () => {
  const ages = m.mapOf<string, number>(["ann", 1]);
  expect(json(ages.set("bob", 2))).toStrictEqual([
    ["ann", 1],
    ["bob", 2],
  ]);
  expect(json(ages.set("ann", 5))).toStrictEqual([["ann", 5]]);
  expect(json(ages.delete("ann"))).toStrictEqual([]);
  expect(json(ages.delete("zed"))).toStrictEqual([["ann", 1]]);
  expect(json(ages)).toStrictEqual([["ann", 1]]);
  expect(ages.set("bob", 2).delete("bob")).toBe(ages);
  expect(m.mapOf(["a", 1], ["b", 2])).toBe(m.mapOf(["b", 2], ["a", 1]));
  expect(m.mapOf(["a", 1])).not.toBe(m.mapOf(["a", 2]));
  expect(m.mapOf(["a", 1], ["a", 2]).get("a")).toBe(2);
});

test("a sequence keeps order and reads by position", () => {
  const steps = m.seqOf("x", "y", "z");
  expect(steps.size).toBe(3);
  expect(steps.isEmpty).toBe(false);
  expect(steps.get(0)).toBe("x");
  expect(steps.get(2)).toBe("z");
  expect(steps.get(3)).toBe(null);
  expect(steps.get(-1)).toBe(null);
  expect(steps.first).toBe("x");
  expect(steps.last).toBe("z");
  expect(steps.includes("y")).toBe(true);
  expect(json(steps)).toStrictEqual(["x", "y", "z"]);
  const none = m.seqOf<string>();
  expect(none.isEmpty).toBe(true);
  expect(none.first).toBe(null);
  expect(none.last).toBe(null);
  expect(none.includes("x")).toBe(false);
});

test("sequence operations give new sequences and leave the old one as it was", () => {
  const steps = m.seqOf<string>("x", "y");
  expect(json(steps.push("x"))).toStrictEqual(["x", "y", "x"]);
  expect(json(steps.shift())).toStrictEqual(["y"]);
  expect(json(steps.pop())).toStrictEqual(["x"]);
  expect(json(steps.filter((step) => step === "y"))).toStrictEqual(["y"]);
  expect(json(m.seqOf<string>().shift())).toStrictEqual([]);
  expect(json(m.seqOf<string>().pop())).toStrictEqual([]);
  expect(json(steps)).toStrictEqual(["x", "y"]);
  expect(steps.every((step) => step !== "z")).toBe(true);
  expect(steps.some((step) => step === "z")).toBe(false);
  expect(steps.push("z").pop()).toBe(steps);
  expect(m.seqOf("x", "y")).toBe(m.seqOf("x", "y"));
  expect(m.seqOf("x", "y")).not.toBe(m.seqOf("y", "x"));
});

test("a queue is first in, first out and knows when it is full", () => {
  const empty = QueueValue.of<string>(2, []);
  expect(empty.size).toBe(0);
  expect(empty.isEmpty).toBe(true);
  expect(empty.isFull).toBe(false);
  expect(empty.head).toBe(null);
  const one = empty.send("a");
  const two = one.send("b");
  expect(one.isEmpty).toBe(false);
  expect(one.isFull).toBe(false);
  expect(two.isFull).toBe(true);
  expect(two.size).toBe(2);
  expect(two.head).toBe("a");
  expect(two.includes("b")).toBe(true);
  expect(two.includes("c")).toBe(false);
  expect(json(two)).toStrictEqual(["a", "b"]);
  expect(json(two.receive())).toStrictEqual(["b"]);
  expect(two.receive().head).toBe("b");
  expect(two.clear()).toBe(empty);
  expect(empty.receive()).toBe(empty);
  expect(two.every((item) => item !== "c")).toBe(true);
  expect(two.some((item) => item === "c")).toBe(false);
  expect(json(one)).toStrictEqual(["a"]);
  expect(json(empty)).toStrictEqual([]);
  expect(empty.send("a").send("b")).toBe(two);
});

test("sending to a full queue is allowed and gives a queue over its capacity", () => {
  const over = QueueValue.of<string>(1, ["a"]).send("b");
  expect(over.size).toBe(2);
  expect(over.isFull).toBe(true);
});

test("a lock without an owner is free or held", () => {
  const free = LockValue.of(false);
  expect(free.isFree).toBe(true);
  expect(free.isHeld).toBe(false);
  const held = free.acquire();
  expect(held.isFree).toBe(false);
  expect(held.isHeld).toBe(true);
  expect(held.release()).toBe(free);
  expect(free.isFree).toBe(true);
  expect(held).toBe(LockValue.of(true));
  expect(json(free)).toBe(false);
  expect(json(held)).toBe(true);
});

test("a lock with an owner remembers who holds it", () => {
  const free = OwnedLockValue.of<string>(null);
  expect(free.isFree).toBe(true);
  expect(free.isHeld).toBe(false);
  expect(free.holder).toBe(null);
  expect(free.isHeldBy("ann")).toBe(false);
  const held = free.acquire("ann");
  expect(held.isFree).toBe(false);
  expect(held.isHeld).toBe(true);
  expect(held.holder).toBe("ann");
  expect(held.isHeldBy("ann")).toBe(true);
  expect(held.isHeldBy("bob")).toBe(false);
  expect(held.release()).toBe(free);
  expect(free.holder).toBe(null);
  expect(held).toBe(OwnedLockValue.of("ann"));
  expect(json(free)).toBe(null);
  expect(json(held)).toBe("ann");
});

test("a timer is idle, running or fired", () => {
  const idle = TimerValue.at("idle");
  expect([idle.isIdle, idle.isRunning, idle.hasFired]).toStrictEqual([true, false, false]);
  const running = idle.start();
  expect([running.isIdle, running.isRunning, running.hasFired]).toStrictEqual([false, true, false]);
  const fired = running.fire();
  expect([fired.isIdle, fired.isRunning, fired.hasFired]).toStrictEqual([false, false, true]);
  expect(running.cancel()).toBe(idle);
  expect(fired.reset()).toBe(idle);
  expect(idle.isIdle).toBe(true);
  expect(fired).toBe(TimerValue.at("fired"));
  expect([json(idle), json(running), json(fired)]).toStrictEqual(["idle", "running", "fired"]);
});

test("a request is idle, pending, succeeded or failed", () => {
  const idle = RequestValue.at("idle");
  const pending = idle.send();
  const succeeded = pending.succeed();
  const failed = pending.fail();
  expect([
    idle.isIdle,
    idle.isPending,
    idle.hasSucceeded,
    idle.hasFailed,
    idle.isSettled,
  ]).toStrictEqual([true, false, false, false, false]);
  expect([
    pending.isIdle,
    pending.isPending,
    pending.hasSucceeded,
    pending.hasFailed,
    pending.isSettled,
  ]).toStrictEqual([false, true, false, false, false]);
  expect([
    succeeded.isPending,
    succeeded.hasSucceeded,
    succeeded.hasFailed,
    succeeded.isSettled,
  ]).toStrictEqual([false, true, false, true]);
  expect([failed.isPending, failed.hasSucceeded, failed.hasFailed, failed.isSettled]).toStrictEqual(
    [false, false, true, true],
  );
  expect(failed.reset()).toBe(idle);
  expect(pending.isPending).toBe(true);
  expect(succeeded).toBe(RequestValue.at("succeeded"));
  expect([json(idle), json(pending), json(succeeded), json(failed)]).toStrictEqual([
    "idle",
    "pending",
    "succeeded",
    "failed",
  ]);
});

test("a retry counts attempts up to its most", () => {
  const fresh = RetryValue.of(2, 0);
  expect([fresh.attempts, fresh.canRetry, fresh.isExhausted]).toStrictEqual([0, true, false]);
  const once = fresh.again();
  expect([once.attempts, once.canRetry, once.isExhausted]).toStrictEqual([1, true, false]);
  const twice = once.again();
  expect([twice.attempts, twice.canRetry, twice.isExhausted]).toStrictEqual([2, false, true]);
  expect(twice.again().attempts).toBe(3);
  expect(twice.reset()).toBe(fresh);
  expect(fresh.attempts).toBe(0);
  expect(once).toBe(RetryValue.of(2, 1));
  expect(RetryValue.of(3, 1)).not.toBe(RetryValue.of(2, 1));
  expect(json(twice)).toBe(2);
});

test("forall and exists range over a declaration or a collection", () => {
  const Digit = m.int(0, 3);
  expect(m.forall(Digit, (digit) => digit < 4)).toBe(true);
  expect(m.forall(Digit, (digit) => digit < 3)).toBe(false);
  expect(m.exists(Digit, (digit) => digit === 3)).toBe(true);
  expect(m.exists(Digit, (digit) => digit === 4)).toBe(false);
  expect(m.forall(m.setOf(1, 2), (digit) => digit > 0)).toBe(true);
  expect(m.exists(m.seqOf(1, 2), (digit) => digit > 1)).toBe(true);
  expect(m.exists(QueueValue.of(2, [1, 2]), (digit) => digit > 2)).toBe(false);
  expect(m.forall(m.setOf<number>(), () => false)).toBe(true);
  expect(m.exists(m.setOf<number>(), () => true)).toBe(false);
  expect(m.exists(m.mapOf(["a", 1]), ([key, value]) => key === "a" && value === 1)).toBe(true);
});

test("texts are walked by code point, as Rust and the checker walk them", () => {
  const letters = m.setOf<string>("\u{1f600}", "\u{ff5e}", "z", "\u{e9}", "B", "b", "a");
  expect([...letters]).toStrictEqual(["B", "a", "b", "z", "\u{e9}", "\u{ff5e}", "\u{1f600}"]);
  const [grin, tilde] = ["\u{1f600}", "\u{ff5e}"];
  expect(compareText(grin, tilde)).toBeGreaterThan(0);
  expect(grin < tilde).toBe(true);
  expect(compareText("ab", "a")).toBeGreaterThan(0);
  expect(compareText("a", "a")).toBe(0);
});

test("records are walked and printed by field name, whatever order the fields were written in", () => {
  const spots = m.setOf({ zebra: 0, apple: 2 }, { zebra: 2, apple: 1 }, { apple: 1, zebra: 1 });
  expect(json(spots)).toStrictEqual([
    { apple: 1, zebra: 1 },
    { apple: 1, zebra: 2 },
    { apple: 2, zebra: 0 },
  ]);
  expect(JSON.stringify(json(m.setOf({ zebra: 1, apple: 2 })))).toBe('[{"apple":2,"zebra":1}]');
});

test("collections are walked item by item and a prefix comes first", () => {
  const groups = m.setOf(
    m.setOf(2),
    m.setOf(1, 3),
    m.setOf(1, 2, 3),
    m.setOf(1, 2),
    m.setOf<number>(),
  );
  expect(json(groups)).toStrictEqual([[], [1, 2], [1, 2, 3], [1, 3], [2]]);
  const maps = m.setOf(
    m.mapOf<string, number>(["b", 1]),
    m.mapOf<string, number>(["a", 2]),
    m.mapOf<string, number>(["a", 1], ["b", 1]),
  );
  expect(json(maps)).toStrictEqual([
    [
      ["a", 1],
      ["b", 1],
    ],
    [["a", 2]],
    [["b", 1]],
  ]);
});

test("a quantifier over a set stops at the first item in that order", () => {
  const seen: string[] = [];
  const found = m.exists(m.setOf("c", "a", "b"), (letter) => {
    seen.push(letter);
    return letter === "b";
  });
  expect(found).toBe(true);
  expect(seen).toStrictEqual(["a", "b"]);
});

test("a record put in a collection can no longer be changed", () => {
  const point = { x: 1 };
  const points = m.seqOf(point);
  expect(Object.isFrozen(point)).toBe(true);
  expect(json(points)).toStrictEqual([{ x: 1 }]);
});
