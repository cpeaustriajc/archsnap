/**
 * A value as `earnest verify --json` prints it: a set, a sequence and a queue are arrays, a map
 * is an array of `[key, value]` pairs, a record is an object, a lock is `true` when held or its
 * holder, a timer and a request are the name of their stage, and a retry is its attempt count.
 */
export type Json =
  | null
  | boolean
  | number
  | string
  | readonly Json[]
  | { readonly [name: string]: Json };

/** An object read by field name whose values are not yet known to fit any declaration. */
export type Row = Readonly<Record<string, unknown>>;

/**
 * The base of every collection, lock, timer, request and retry value. Two values with the same
 * content are the same object, so `===` compares them by content.
 */
export abstract class ModelValue {
  /** Text that is equal for two values exactly when their content is equal. */
  abstract readonly key: string;

  /** The value in the form `earnest verify --json` prints. */
  abstract toJSON(): Json;
}

/** Tells a record, which is a plain object, from every other value a model can hold. */
export const isRecord = (value: unknown): value is Row =>
  typeof value === "object" &&
  value !== null &&
  !Array.isArray(value) &&
  !(value instanceof ModelValue);

/**
 * Text that is equal for two model values exactly when their content is equal. A record's key
 * does not depend on the order its fields were written in. Throws for anything a model can't
 * hold, such as `undefined` or a function.
 */
export const keyOf = (value: unknown): string => {
  if (value instanceof ModelValue) {
    return value.key;
  }
  if (value === null || typeof value === "boolean") {
    return String(value);
  }
  if (typeof value === "number") {
    return value === 0 ? "0" : String(value);
  }
  if (typeof value === "string") {
    return JSON.stringify(value);
  }
  if (isRecord(value)) {
    const fields = Object.keys(value)
      .sort()
      .map((name) => `${JSON.stringify(name)}:${keyOf(value[name])}`);
    return `{${fields.join(",")}}`;
  }
  throw new Error(`${String(value)} is not a value a model can hold`);
};

/** Turns a model value into the form `earnest verify --json` prints. */
export const toJson = (value: unknown): Json => {
  if (value instanceof ModelValue) {
    return value.toJSON();
  }
  if (value === null || typeof value === "boolean" || typeof value === "string") {
    return value;
  }
  if (typeof value === "number") {
    return value === 0 ? 0 : value;
  }
  if (isRecord(value)) {
    return Object.fromEntries(Object.keys(value).map((name) => [name, toJson(value[name])]));
  }
  throw new Error(`${String(value)} is not a value a model can hold`);
};

/**
 * Makes a record, and every record inside it, read-only, and gives the same value back. Other
 * values are already immutable.
 */
export const frozen = <T>(value: T): T => {
  if (isRecord(value) && !Object.isFrozen(value)) {
    Object.values(value).forEach(frozen);
    Object.freeze(value);
  }
  return value;
};

/** Tells a JSON array from every other JSON value. */
export const isJsonList = (json: Json): json is readonly Json[] => Array.isArray(json);

/** Tells a JSON object from every other JSON value. */
export const isJsonObject = (json: Json): json is { readonly [name: string]: Json } =>
  typeof json === "object" && json !== null && !Array.isArray(json);

const rank = (json: Json): number => {
  if (json === null) {
    return 0;
  }
  if (typeof json === "boolean") {
    return 1;
  }
  if (typeof json === "number") {
    return 2;
  }
  if (typeof json === "string") {
    return 3;
  }
  return isJsonList(json) ? 5 : 4;
};

const compareLists = (left: readonly Json[], right: readonly Json[]): number => {
  for (const [position, item] of left.entries()) {
    const other = right[position];
    if (other === undefined) {
      return 1;
    }
    const order = compareJson(item, other);
    if (order !== 0) {
      return order;
    }
  }
  return left.length - right.length;
};

const flattened = (json: { readonly [name: string]: Json }): Json[] =>
  Object.keys(json)
    .sort()
    .flatMap((name) => [name, json[name] ?? null]);

/**
 * The order set members and map keys are kept and printed in: `null`, then booleans, numbers,
 * text in code-unit order, records and collections. Returns a negative number, zero or a
 * positive number, as a sort comparator does.
 */
export const compareJson = (left: Json, right: Json): number => {
  if (typeof left === "boolean" && typeof right === "boolean") {
    return Number(left) - Number(right);
  }
  if (typeof left === "number" && typeof right === "number") {
    return left - right;
  }
  if (typeof left === "string" && typeof right === "string") {
    return left < right ? -1 : left > right ? 1 : 0;
  }
  if (left === null || right === null || typeof left !== "object" || typeof right !== "object") {
    return rank(left) - rank(right);
  }
  if (isJsonList(left) && isJsonList(right)) {
    return compareLists(left, right);
  }
  if (!isJsonList(left) && !isJsonList(right)) {
    return compareLists(flattened(left), flattened(right));
  }
  return rank(left) - rank(right);
};

const shared = new Map<string, WeakRef<ModelValue>>();

const forgotten = new FinalizationRegistry<string>((key) => {
  if (shared.get(key)?.deref() === undefined) {
    shared.delete(key);
  }
});

const isSameValue = <V extends ModelValue>(found: ModelValue | undefined, made: V): found is V =>
  found !== undefined && found.constructor === made.constructor && found.key === made.key;

const one = <V extends ModelValue>(made: V): V => {
  const found = shared.get(made.key)?.deref();
  if (isSameValue(found, made)) {
    return found;
  }
  shared.set(made.key, new WeakRef(made));
  forgotten.register(made, made.key);
  return made;
};

interface Member<T> {
  readonly item: T;
  readonly key: string;
  readonly json: Json;
}

const member = <T>(item: T): Member<T> => ({
  item: frozen(item),
  key: keyOf(item),
  json: toJson(item),
});

const inOrder = (left: Member<unknown>, right: Member<unknown>): number =>
  compareJson(left.json, right.json) || (left.key < right.key ? -1 : left.key > right.key ? 1 : 0);

const sortedMembers = <T>(items: Iterable<T>): Member<T>[] => {
  const byKey = new Map<string, Member<T>>([...items].map(member).map((each) => [each.key, each]));
  return [...byKey.values()].sort(inOrder);
};

const listed = <T>(items: Iterable<T>): readonly T[] => [...items].map(frozen);

const orNull = <T>(found: T | undefined): T | null => (found === undefined ? null : found);

/**
 * An immutable set. Members are compared by content and kept in the order of `compareJson`,
 * whatever order they were added in. `has`, `isSubset`, `size`, `isEmpty`, `every` and `some`
 * read the set; `add`, `delete`, `union`, `intersect`, `subtract` and `filter` give back another
 * set and leave this one as it was.
 */
export class SetValue<T> extends ModelValue implements Iterable<T> {
  readonly #items: readonly T[];
  readonly #keys: ReadonlySet<string>;
  override readonly key: string;

  private constructor(members: readonly Member<T>[]) {
    super();
    this.#items = members.map((each) => each.item);
    this.#keys = new Set(members.map((each) => each.key));
    this.key = `set(${members.map((each) => each.key).join(",")})`;
  }

  /** The set of the given items; an item given twice is in the set once. */
  static of<T>(items: Iterable<T>): SetValue<T> {
    return one(new SetValue(sortedMembers(items)));
  }

  get size(): number {
    return this.#items.length;
  }

  get isEmpty(): boolean {
    return this.#items.length === 0;
  }

  has(item: T): boolean {
    return this.#keys.has(keyOf(item));
  }

  add(item: T): SetValue<T> {
    return SetValue.of([...this.#items, item]);
  }

  delete(item: T): SetValue<T> {
    const gone = keyOf(item);
    return SetValue.of(this.#items.filter((each) => keyOf(each) !== gone));
  }

  union(other: SetValue<T>): SetValue<T> {
    return SetValue.of([...this.#items, ...other]);
  }

  intersect(other: SetValue<T>): SetValue<T> {
    return SetValue.of(this.#items.filter((each) => other.has(each)));
  }

  subtract(other: SetValue<T>): SetValue<T> {
    return SetValue.of(this.#items.filter((each) => !other.has(each)));
  }

  isSubset(other: SetValue<T>): boolean {
    return this.#items.every((each) => other.has(each));
  }

  every(test: (item: T) => boolean): boolean {
    return this.#items.every((each) => test(each));
  }

  some(test: (item: T) => boolean): boolean {
    return this.#items.some((each) => test(each));
  }

  filter(test: (item: T) => boolean): SetValue<T> {
    return SetValue.of(this.#items.filter((each) => test(each)));
  }

  [Symbol.iterator](): Iterator<T> {
    return this.#items[Symbol.iterator]();
  }

  override toJSON(): Json {
    return this.#items.map(toJson);
  }
}

/**
 * An immutable map. Keys are compared by content and kept in the order of `compareJson`.
 * `get` gives `null` for a key the map does not have. `set` and `delete` give back another map
 * and leave this one as it was. Iterating gives `[key, value]` pairs.
 */
export class MapValue<K, V> extends ModelValue implements Iterable<readonly [K, V]> {
  readonly #entries: ReadonlyMap<string, readonly [K, V]>;
  override readonly key: string;

  private constructor(entries: readonly (readonly [K, V])[]) {
    super();
    this.#entries = new Map<string, readonly [K, V]>(
      entries.map((entry) => [keyOf(entry[0]), entry]),
    );
    const shown = entries.map(([key, value]) => `${keyOf(key)}=>${keyOf(value)}`);
    this.key = `map(${shown.join(",")})`;
  }

  /** The map of the given pairs; when a key is given twice, the later pair wins. */
  static of<K, V>(entries: Iterable<readonly [K, V]>): MapValue<K, V> {
    const latest = new Map<string, readonly [K, V]>(
      [...entries].map((entry) => [keyOf(entry[0]), entry]),
    );
    const ordered = [...latest.values()]
      .map(([key, value]) => ({ at: member(key), value: frozen(value) }))
      .sort((left, right) => inOrder(left.at, right.at));
    return one(new MapValue(ordered.map((each) => [each.at.item, each.value])));
  }

  get size(): number {
    return this.#entries.size;
  }

  get isEmpty(): boolean {
    return this.#entries.size === 0;
  }

  get keys(): SetValue<K> {
    return SetValue.of([...this.#entries.values()].map(([key]) => key));
  }

  has(key: K): boolean {
    return this.#entries.has(keyOf(key));
  }

  get(key: K): V | null {
    const found = this.#entries.get(keyOf(key));
    return found === undefined ? null : found[1];
  }

  set(key: K, value: V): MapValue<K, V> {
    return MapValue.of([...this.#entries.values(), [key, value]]);
  }

  delete(key: K): MapValue<K, V> {
    const gone = keyOf(key);
    return MapValue.of(
      [...this.#entries].filter(([known]) => known !== gone).map(([, entry]) => entry),
    );
  }

  [Symbol.iterator](): Iterator<readonly [K, V]> {
    return this.#entries.values();
  }

  override toJSON(): Json {
    return [...this.#entries.values()].map(([key, value]) => [toJson(key), toJson(value)]);
  }
}

/**
 * An immutable sequence. `get`, `first` and `last` give `null` where there is no item. `push`
 * adds at the end, `shift` drops the first item and `pop` drops the last; each gives back
 * another sequence, and `shift` and `pop` on an empty sequence give the empty sequence. Pushing
 * past the declared `max` is not an error here: the checker reports the state as out of range.
 */
export class SeqValue<T> extends ModelValue implements Iterable<T> {
  readonly #items: readonly T[];
  override readonly key: string;

  private constructor(items: readonly T[]) {
    super();
    this.#items = items;
    this.key = `seq(${items.map(keyOf).join(",")})`;
  }

  /** The sequence of the given items, in the order given. */
  static of<T>(items: Iterable<T>): SeqValue<T> {
    return one(new SeqValue(listed(items)));
  }

  get size(): number {
    return this.#items.length;
  }

  get isEmpty(): boolean {
    return this.#items.length === 0;
  }

  get first(): T | null {
    return orNull(this.#items[0]);
  }

  get last(): T | null {
    return orNull(this.#items[this.#items.length - 1]);
  }

  get(position: number): T | null {
    return orNull(this.#items[position]);
  }

  push(item: T): SeqValue<T> {
    return SeqValue.of([...this.#items, item]);
  }

  shift(): SeqValue<T> {
    return SeqValue.of(this.#items.slice(1));
  }

  pop(): SeqValue<T> {
    return SeqValue.of(this.#items.slice(0, -1));
  }

  includes(item: T): boolean {
    const wanted = keyOf(item);
    return this.#items.some((each) => keyOf(each) === wanted);
  }

  every(test: (item: T) => boolean): boolean {
    return this.#items.every((each) => test(each));
  }

  some(test: (item: T) => boolean): boolean {
    return this.#items.some((each) => test(each));
  }

  filter(test: (item: T) => boolean): SeqValue<T> {
    return SeqValue.of(this.#items.filter((each) => test(each)));
  }

  [Symbol.iterator](): Iterator<T> {
    return this.#items[Symbol.iterator]();
  }

  override toJSON(): Json {
    return this.#items.map(toJson);
  }
}

/**
 * An immutable first-in, first-out queue with a capacity. `head` is the item `receive` would
 * drop, or `null`. `send`, `receive` and `clear` give back another queue. Sending to a full
 * queue is not an error here: the checker reports the state as out of range.
 */
export class QueueValue<T> extends ModelValue implements Iterable<T> {
  readonly #items: readonly T[];
  readonly #capacity: number;
  override readonly key: string;

  private constructor(capacity: number, items: readonly T[]) {
    super();
    this.#capacity = capacity;
    this.#items = items;
    this.key = `queue/${capacity}(${items.map(keyOf).join(",")})`;
  }

  /** The queue of the given capacity holding the given items, the first of them at the head. */
  static of<T>(capacity: number, items: Iterable<T>): QueueValue<T> {
    return one(new QueueValue(capacity, listed(items)));
  }

  get size(): number {
    return this.#items.length;
  }

  get isEmpty(): boolean {
    return this.#items.length === 0;
  }

  get capacity(): number {
    return this.#capacity;
  }

  get isFull(): boolean {
    return this.#items.length >= this.#capacity;
  }

  get head(): T | null {
    return orNull(this.#items[0]);
  }

  send(item: T): QueueValue<T> {
    return QueueValue.of(this.#capacity, [...this.#items, item]);
  }

  receive(): QueueValue<T> {
    return QueueValue.of(this.#capacity, this.#items.slice(1));
  }

  includes(item: T): boolean {
    const wanted = keyOf(item);
    return this.#items.some((each) => keyOf(each) === wanted);
  }

  clear(): QueueValue<T> {
    return QueueValue.of<T>(this.#capacity, []);
  }

  every(test: (item: T) => boolean): boolean {
    return this.#items.every((each) => test(each));
  }

  some(test: (item: T) => boolean): boolean {
    return this.#items.some((each) => test(each));
  }

  [Symbol.iterator](): Iterator<T> {
    return this.#items[Symbol.iterator]();
  }

  override toJSON(): Json {
    return this.#items.map(toJson);
  }
}

/**
 * A lock with no owner: free or held. `acquire` and `release` give back another lock; acquiring
 * a held lock gives a held lock, so guard with `isFree` in the action's `when`.
 */
export class LockValue extends ModelValue {
  readonly #isHeld: boolean;
  override readonly key: string;

  private constructor(isHeld: boolean) {
    super();
    this.#isHeld = isHeld;
    this.key = `lock(${isHeld ? "held" : "free"})`;
  }

  /** The held lock when `isHeld` is true, the free lock otherwise. */
  static of(isHeld: boolean): LockValue {
    return one(new LockValue(isHeld));
  }

  get isFree(): boolean {
    return !this.#isHeld;
  }

  get isHeld(): boolean {
    return this.#isHeld;
  }

  acquire(): LockValue {
    return LockValue.of(true);
  }

  release(): LockValue {
    return LockValue.of(false);
  }

  override toJSON(): Json {
    return this.#isHeld;
  }
}

/**
 * A lock that remembers who holds it. `holder` is `null` when the lock is free. `acquire(owner)`
 * and `release` give back another lock; acquiring a held lock hands it to the new owner, so
 * guard with `isFree` in the action's `when`.
 */
export class OwnedLockValue<Owner> extends ModelValue {
  readonly #holder: Owner | null;
  override readonly key: string;

  private constructor(holder: Owner | null) {
    super();
    this.#holder = holder;
    this.key = `owned-lock(${keyOf(holder)})`;
  }

  /** The lock held by `holder`, or the free lock for `null`. */
  static of<Owner>(holder: Owner | null): OwnedLockValue<Owner> {
    return one(new OwnedLockValue(frozen(holder)));
  }

  get isFree(): boolean {
    return this.#holder === null;
  }

  get isHeld(): boolean {
    return this.#holder !== null;
  }

  get holder(): Owner | null {
    return this.#holder;
  }

  isHeldBy(owner: Owner): boolean {
    return this.#holder !== null && keyOf(this.#holder) === keyOf(owner);
  }

  acquire(owner: Owner): OwnedLockValue<Owner> {
    return OwnedLockValue.of(owner);
  }

  release(): OwnedLockValue<Owner> {
    return OwnedLockValue.of<Owner>(null);
  }

  override toJSON(): Json {
    return toJson(this.#holder);
  }
}

/** The stages of a timer, in the order a declaration lists them. */
export const TIMER_STAGES: readonly TimerStage[] = ["idle", "running", "fired"];

/** The name of one stage of a timer. */
export type TimerStage = "idle" | "running" | "fired";

/**
 * A timer: idle, running or fired. `start`, `fire`, `cancel` and `reset` give back another timer
 * from any stage; `cancel` and `reset` both give the idle timer.
 */
export class TimerValue extends ModelValue {
  readonly #stage: TimerStage;
  override readonly key: string;

  private constructor(stage: TimerStage) {
    super();
    this.#stage = stage;
    this.key = `timer(${stage})`;
  }

  /** The timer at the given stage. */
  static at(stage: TimerStage): TimerValue {
    return one(new TimerValue(stage));
  }

  get isIdle(): boolean {
    return this.#stage === "idle";
  }

  get isRunning(): boolean {
    return this.#stage === "running";
  }

  get hasFired(): boolean {
    return this.#stage === "fired";
  }

  start(): TimerValue {
    return TimerValue.at("running");
  }

  fire(): TimerValue {
    return TimerValue.at("fired");
  }

  cancel(): TimerValue {
    return TimerValue.at("idle");
  }

  reset(): TimerValue {
    return TimerValue.at("idle");
  }

  override toJSON(): Json {
    return this.#stage;
  }
}

/** The stages of an async request, in the order a declaration lists them. */
export const REQUEST_STAGES: readonly RequestStage[] = ["idle", "pending", "succeeded", "failed"];

/** The name of one stage of an async request. */
export type RequestStage = "idle" | "pending" | "succeeded" | "failed";

/**
 * An async request: idle, pending, succeeded or failed. `isSettled` is true once it succeeded or
 * failed. `send`, `succeed`, `fail` and `reset` give back another request from any stage.
 */
export class RequestValue extends ModelValue {
  readonly #stage: RequestStage;
  override readonly key: string;

  private constructor(stage: RequestStage) {
    super();
    this.#stage = stage;
    this.key = `request(${stage})`;
  }

  /** The request at the given stage. */
  static at(stage: RequestStage): RequestValue {
    return one(new RequestValue(stage));
  }

  get isIdle(): boolean {
    return this.#stage === "idle";
  }

  get isPending(): boolean {
    return this.#stage === "pending";
  }

  get hasSucceeded(): boolean {
    return this.#stage === "succeeded";
  }

  get hasFailed(): boolean {
    return this.#stage === "failed";
  }

  get isSettled(): boolean {
    return this.#stage === "succeeded" || this.#stage === "failed";
  }

  send(): RequestValue {
    return RequestValue.at("pending");
  }

  succeed(): RequestValue {
    return RequestValue.at("succeeded");
  }

  fail(): RequestValue {
    return RequestValue.at("failed");
  }

  reset(): RequestValue {
    return RequestValue.at("idle");
  }

  override toJSON(): Json {
    return this.#stage;
  }
}

/**
 * A count of attempts with a most. `canRetry` is true below the most and `isExhausted` at it.
 * `again` counts one more attempt and `reset` goes back to zero; both give back another counter.
 * Counting past the most is not an error here: the checker reports the state as out of range.
 */
export class RetryValue extends ModelValue {
  readonly #max: number;
  readonly #attempts: number;
  override readonly key: string;

  private constructor(max: number, attempts: number) {
    super();
    this.#max = max;
    this.#attempts = attempts;
    this.key = `retry/${max}(${attempts})`;
  }

  /** The counter that allows `max` attempts and has counted `attempts` of them. */
  static of(max: number, attempts: number): RetryValue {
    return one(new RetryValue(max, attempts));
  }

  get max(): number {
    return this.#max;
  }

  get attempts(): number {
    return this.#attempts;
  }

  get canRetry(): boolean {
    return this.#attempts < this.#max;
  }

  get isExhausted(): boolean {
    return this.#attempts >= this.#max;
  }

  again(): RetryValue {
    return RetryValue.of(this.#max, this.#attempts + 1);
  }

  reset(): RetryValue {
    return RetryValue.of(this.#max, 0);
  }

  override toJSON(): Json {
    return this.#attempts;
  }
}

/** Writes a set value: `m.setOf("a", "b")`. Members keep their literal types. */
export const setOf = <const T>(...items: readonly T[]): SetValue<T> => SetValue.of(items);

/** Writes a sequence value: `m.seqOf("a", "b")`. It is also how a queue's `.initial` is written. */
export const seqOf = <const T>(...items: readonly T[]): SeqValue<T> => SeqValue.of(items);

/** Writes a map value from `[key, value]` pairs: `m.mapOf(["a", 1], ["b", 2])`. */
export const mapOf = <const K, const V>(...entries: readonly (readonly [K, V])[]): MapValue<K, V> =>
  MapValue.of(entries);

/** Thrown by `rem` for a remainder the contract makes a fault; `explore` and `replay` report it. */
export class RemainderFault extends Error {
  constructor() {
    super("takes a remainder with a negative number or a divisor below 1");
    this.name = "RemainderFault";
  }
}

/** Thrown by `div` for a division the contract makes a fault; `explore` and `replay` report it. */
export class DivisionFault extends Error {
  constructor() {
    super("takes a division with a negative number or a divisor below 1");
    this.name = "DivisionFault";
  }
}

/** Thrown by `some` for a `null`; `explore` and `replay` report it as a read of a null value. */
export class NullFault extends Error {
  constructor() {
    super("reads a value that is null");
    this.name = "NullFault";
  }
}

/**
 * Writes the value inside an optional: `m.some(s.chosen).qty`. A `null` is a fault, as in
 * `earnest verify`, because JavaScript would go on with `null` as a number. Use it where
 * TypeScript would want a postfix `!`, which a model file may not have.
 */
export const some = <T>(value: T | null): T => {
  if (value === null || value === undefined) {
    throw new NullFault();
  }
  return value;
};

/**
 * Writes the remainder of `a` divided by `b`: `m.rem(7, 3)` is 1. A negative `a` or a `b`
 * below 1 is a fault, as in `earnest verify`, because languages disagree on the result there.
 * Use it where JavaScript would use `%`, which a model file may not.
 */
export const rem = (a: number, b: number): number => {
  if (a < 0 || b < 1 || Number.isNaN(a) || Number.isNaN(b)) {
    throw new RemainderFault();
  }
  return a % b;
};

/**
 * Writes the whole number of times `b` goes into `a`: `m.div(7, 2)` is 3. A negative `a` or a
 * `b` below 1 is a fault, as in `earnest verify`, because languages disagree on the result
 * there and Rust panics on a divisor of zero. Use it where JavaScript would use `/`, which a
 * model file may not, since it gives a fraction.
 */
export const div = (a: number, b: number): number => {
  if (a < 0 || b < 1 || Number.isNaN(a) || Number.isNaN(b)) {
    throw new DivisionFault();
  }
  return (a - (a % b)) / b;
};
