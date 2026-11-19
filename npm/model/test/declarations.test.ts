import { expect, test } from "vitest";
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
  expect(m.boolean().values()).toStrictEqual([false, true]);
  expect(m.int(-1, 2).values()).toStrictEqual([-1, 0, 1, 2]);
  expect(m.enum(["b", "a", "c"]).values()).toStrictEqual(["b", "a", "c"]);
  expect(m.oneOf([3, 1, 2]).values()).toStrictEqual([3, 1, 2]);
  expect([m.boolean().count, m.int(-1, 2).count, m.enum(["b", "a", "c"]).count]).toStrictEqual([
    2, 4, 3,
  ]);
});

test("membership is exact", () => {
  expect(m.boolean().has(true)).toBe(true);
  expect(m.boolean().has(1)).toBe(false);
  expect(m.int(0, 3).has(3)).toBe(true);
  expect(m.int(0, 3).has(4)).toBe(false);
  expect(m.int(0, 3).has(-1)).toBe(false);
  expect(m.int(0, 3).has(1.5)).toBe(false);
  expect(m.int(0, 3).has("1")).toBe(false);
  expect(Letter.has("a")).toBe(true);
  expect(Letter.has("c")).toBe(false);
  expect(Letter.has(null)).toBe(false);
  expect(m.oneOf([1, 2]).has(2)).toBe(true);
  expect(m.oneOf([1, 2]).has("2")).toBe(false);
});

test("an optional lists null first and has one more value", () => {
  expect(Letter.optional().values()).toStrictEqual([null, "a", "b"]);
  expect(Letter.optional().count).toBe(3);
  expect(Letter.optional().has(null)).toBe(true);
  expect(Letter.optional().has("b")).toBe(true);
  expect(Letter.optional().has("c")).toBe(false);
});

test("a record lists every combination, the first field varying slowest", () => {
  const Point = m.record({ on: m.boolean(), letter: Letter });
  expect(Point.values()).toStrictEqual([
    { on: false, letter: "a" },
    { on: false, letter: "b" },
    { on: true, letter: "a" },
    { on: true, letter: "b" },
  ]);
  expect(Point.count).toBe(4);
  expect(Point.has({ letter: "a", on: true })).toBe(true);
  expect(Point.has({ on: true })).toBe(false);
  expect(Point.has({ on: true, letter: "a", extra: 1 })).toBe(false);
  expect(Point.has({ on: true, letter: "c" })).toBe(false);
  expect(Point.has(null)).toBe(false);
});

test("a set lists every subset", () => {
  const Letters = m.set(Letter);
  expect(Letters.values()).toStrictEqual([
    m.setOf(),
    m.setOf("a"),
    m.setOf("b"),
    m.setOf("a", "b"),
  ]);
  expect(Letters.count).toBe(4);
  expect(Letters.has(m.setOf("b"))).toBe(true);
  expect(Letters.has(m.setOf("c"))).toBe(false);
  expect(Letters.has(m.seqOf("a"))).toBe(false);
});

test("a map lists every choice of keys and values", () => {
  const Votes = m.map(Letter, m.boolean());
  expect(Votes.count).toBe(9);
  expect(Votes.values()).toStrictEqual([
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
  expect(Votes.has(m.mapOf(["b", true]))).toBe(true);
  expect(Votes.has(m.mapOf(["c", true]))).toBe(false);
  expect(Votes.has(m.mapOf(["a", 1]))).toBe(false);
});

test("a sequence lists every list up to its max, shortest first", () => {
  const Steps = m.seq(Letter, { max: 2 });
  expect(Steps.count).toBe(7);
  expect(Steps.values()).toStrictEqual([
    m.seqOf(),
    m.seqOf("a"),
    m.seqOf("b"),
    m.seqOf("a", "a"),
    m.seqOf("a", "b"),
    m.seqOf("b", "a"),
    m.seqOf("b", "b"),
  ]);
  expect(Steps.has(m.seqOf("b", "a"))).toBe(true);
  expect(Steps.has(m.seqOf("a", "a", "a"))).toBe(false);
  expect(Steps.has(m.seqOf("c"))).toBe(false);
});

test("a queue lists every content up to its capacity", () => {
  const Inbox = m.queue(Letter, { capacity: 2 });
  expect(Inbox.count).toBe(7);
  expect(Inbox.values().map((queue) => queue.toJSON())).toStrictEqual([
    [],
    ["a"],
    ["b"],
    ["a", "a"],
    ["a", "b"],
    ["b", "a"],
    ["b", "b"],
  ]);
  expect(Inbox.has(QueueValue.of(2, ["a", "b"]))).toBe(true);
  expect(Inbox.has(QueueValue.of(2, ["a", "b", "a"]))).toBe(false);
  expect(Inbox.has(QueueValue.of(2, ["c"]))).toBe(false);
});

test("locks, timers, requests and retries list their few values", () => {
  expect(m.lock().values()).toStrictEqual([LockValue.of(false), LockValue.of(true)]);
  expect(m.lock(Letter).values()).toStrictEqual([
    OwnedLockValue.of(null),
    OwnedLockValue.of("a"),
    OwnedLockValue.of("b"),
  ]);
  expect(m.lock(Letter).has(OwnedLockValue.of("c"))).toBe(false);
  expect(m.timer().values()).toStrictEqual([
    TimerValue.at("idle"),
    TimerValue.at("running"),
    TimerValue.at("fired"),
  ]);
  expect(m.request().values()).toStrictEqual([
    RequestValue.at("idle"),
    RequestValue.at("pending"),
    RequestValue.at("succeeded"),
    RequestValue.at("failed"),
  ]);
  expect(m.retry({ max: 2 }).values()).toStrictEqual([
    RetryValue.of(2, 0),
    RetryValue.of(2, 1),
    RetryValue.of(2, 2),
  ]);
  expect(m.retry({ max: 2 }).has(RetryValue.of(2, 3))).toBe(false);
  expect(m.timer().has(RequestValue.at("idle"))).toBe(false);
});

test("a field with .initial starts at that value", () => {
  expect(m.boolean().initial(true).starts()).toStrictEqual([true]);
  expect(m.int(0, 3).initial(2).starts()).toStrictEqual([2]);
  expect(Letter.initial("b").starts()).toStrictEqual(["b"]);
  expect(Letter.optional().initial(null).starts()).toStrictEqual([null]);
  expect(Letter.optional().initial("a").starts()).toStrictEqual(["a"]);
  expect(Letter.initial("b").optional().starts()).toStrictEqual(["b"]);
  expect(m.set(Letter).initial(m.setOf("a")).starts()).toStrictEqual([m.setOf("a")]);
  expect(m.record({ on: m.boolean() }).initial({ on: true }).starts()).toStrictEqual([
    { on: true },
  ]);
  expect(m.queue(Letter, { capacity: 2 }).initial(m.seqOf("b", "a")).starts()).toStrictEqual([
    QueueValue.of(2, ["b", "a"]),
  ]);
});

test("a boolean, number, enum, optional or record without .initial starts at every value", () => {
  expect(m.boolean().starts()).toStrictEqual([false, true]);
  expect(m.int(0, 2).starts()).toStrictEqual([0, 1, 2]);
  expect(Letter.starts()).toStrictEqual(["a", "b"]);
  expect(Letter.optional().starts()).toStrictEqual([null, "a", "b"]);
  expect(m.record({ on: m.boolean() }).starts()).toStrictEqual([{ on: false }, { on: true }]);
});

test("a collection, lock, timer, request or retry without .initial starts empty, free, idle or at zero", () => {
  expect(m.set(Letter).starts()).toStrictEqual([m.setOf()]);
  expect(m.map(Letter, Letter).starts()).toStrictEqual([m.mapOf()]);
  expect(m.seq(Letter, { max: 2 }).starts()).toStrictEqual([m.seqOf()]);
  expect(m.queue(Letter, { capacity: 2 }).starts()).toStrictEqual([QueueValue.of(2, [])]);
  expect(m.lock().starts()).toStrictEqual([LockValue.of(false)]);
  expect(m.lock(Letter).starts()).toStrictEqual([OwnedLockValue.of(null)]);
  expect(m.timer().starts()).toStrictEqual([TimerValue.at("idle")]);
  expect(m.request().starts()).toStrictEqual([RequestValue.at("idle")]);
  expect(m.retry({ max: 3 }).starts()).toStrictEqual([RetryValue.of(3, 0)]);
});

test(".initial and .optional give new declarations and leave the old one as it was", () => {
  const Count = m.int(0, 2);
  const Started = Count.initial(1);
  const Maybe = Count.optional();
  expect(Started).not.toBe(Count);
  expect(Count.starts()).toStrictEqual([0, 1, 2]);
  expect(Started.starts()).toStrictEqual([1]);
  expect(Maybe.starts()).toStrictEqual([null, 0, 1, 2]);
  expect(Count.has(null)).toBe(false);
  expect(Count.kind).toBe("number");
  expect(Maybe.kind).toBe("optional");
});

test("a declaration that makes no sense is refused when it is written", () => {
  expect(() => m.int(3, 1)).toThrow(/lower one first/);
  expect(() => m.int(0, 1.5)).toThrow(/whole numbers/);
  expect(() => m.enum(["a", "a"])).toThrow(/different values/);
  expect(() => m.enum([])).toThrow(/one or more/);
  expect(() => m.oneOf([1, "one"])).toThrow(/all text, all numbers or all booleans/);
  expect(() => m.int(0, 3).initial(7)).toThrow(/7 is not a value of this number/);
  expect(() => m.set(m.int(0, 1)).initial(m.setOf(0, 5))).toThrow(/not a value of this set/);
  expect(() => Letter.optional().optional()).toThrow(/can't be made optional/);
  expect(() => m.lock(Letter).optional()).toThrow(/can't be made optional/);
  expect(() => m.lock(Letter.optional())).toThrow(/owner can't be optional/);
  expect(() => m.map(Letter, m.lock(Letter))).toThrow(/m\.map can't hold a lock with an owner/);
  expect(() => m.seq(m.lock(Letter), { max: 1 })).toThrow(/m\.seq can't hold a lock with an owner/);
  expect(() => m.queue(m.lock(Letter), { capacity: 1 })).toThrow(
    /m\.queue can't hold a lock with an owner/,
  );
  expect(m.map(Letter, m.lock()).kind).toBe("map");
  expect(m.set(m.lock(Letter)).kind).toBe("set");
  expect(() => m.seq(Letter, { max: -1 })).toThrow(/not negative/);
  expect(() => m.queue(Letter, { capacity: 0.5 })).toThrow(/whole number/);
  expect(() => m.retry({ max: -2 })).toThrow(/not negative/);
  expect(() => m.set(m.int(0, 30)).values()).toThrow(/too many to list/);
});

test("a value is read back from its JSON form", () => {
  expect(m.set(Letter).fromJson(["b", "a"])).toBe(m.setOf("a", "b"));
  expect(m.seq(Letter, { max: 2 }).fromJson(["b", "a"])).toBe(m.seqOf("b", "a"));
  expect(m.map(Letter, m.int(0, 1)).fromJson([["a", 1]])).toBe(m.mapOf(["a", 1]));
  expect(m.queue(Letter, { capacity: 2 }).fromJson(["a"])).toBe(QueueValue.of(2, ["a"]));
  expect(m.lock().fromJson(true)).toBe(LockValue.of(true));
  expect(m.lock(Letter).fromJson(null)).toBe(OwnedLockValue.of(null));
  expect(m.lock(Letter).fromJson("a")).toBe(OwnedLockValue.of("a"));
  expect(m.timer().fromJson("running")).toBe(TimerValue.at("running"));
  expect(m.request().fromJson("failed")).toBe(RequestValue.at("failed"));
  expect(m.retry({ max: 2 }).fromJson(1)).toBe(RetryValue.of(2, 1));
  expect(Letter.optional().fromJson(null)).toBe(null);
  expect(m.int(0, 3).fromJson(9)).toBe(9);
  expect(m.record({ at: m.set(Letter) }).fromJson({ at: ["a"] })).toStrictEqual({
    at: m.setOf("a"),
  });
});

test("JSON of another kind is refused", () => {
  expect(() => m.set(Letter).fromJson("a")).toThrow(/can't be read as a set/);
  expect(() => m.int(0, 3).fromJson([1])).toThrow(/can't be read as a number/);
  expect(() => m.timer().fromJson("melted")).toThrow(/can't be read as a timer/);
  expect(() => m.lock().fromJson("held")).toThrow(/can't be read as a lock/);
  expect(() => m.map(Letter, Letter).fromJson([["a"]])).toThrow(/can't be read as a map entry/);
  expect(() => m.record({ on: m.boolean() }).fromJson({ off: true })).toThrow(/can't be read/);
});

test("a model starts at every combination of its fields' start values, the first field slowest", () => {
  const Pair = m.model("Pair", {
    on: m.boolean(),
    count: m.int(0, 2),
    letter: Letter.initial("b"),
    seen: m.set(Letter),
  });
  const none = m.setOf();
  expect(Pair.starts()).toStrictEqual([
    { on: false, count: 0, letter: "b", seen: none },
    { on: false, count: 1, letter: "b", seen: none },
    { on: false, count: 2, letter: "b", seen: none },
    { on: true, count: 0, letter: "b", seen: none },
    { on: true, count: 1, letter: "b", seen: none },
    { on: true, count: 2, letter: "b", seen: none },
  ]);
  expect(Pair.has({ on: true, count: 2, letter: "a", seen: m.setOf("a") })).toBe(true);
  expect(Pair.has({ on: true, count: 3, letter: "a", seen: none })).toBe(false);
  expect(Pair.has({ on: true, count: 2, letter: "a" })).toBe(false);
});

test("a model whose fields all have one start value has one start state", () => {
  const Solo = m.model("Solo", { count: m.int(0, 2).initial(0), door: m.lock() });
  expect(Solo.starts()).toStrictEqual([{ count: 0, door: LockValue.of(false) }]);
});
