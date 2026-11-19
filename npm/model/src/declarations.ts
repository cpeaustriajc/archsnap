import {
  type Json,
  LockValue,
  MapValue,
  ModelValue,
  OwnedLockValue,
  QueueValue,
  REQUEST_STAGES,
  RequestValue,
  RetryValue,
  type Row,
  SeqValue,
  SetValue,
  TIMER_STAGES,
  TimerValue,
  frozen,
  isJsonList,
  isJsonObject,
  isRecord,
  toJson,
} from "./values.ts";

/** The most values `Declaration.values` will list before it throws instead. */
export const MOST_VALUES_LISTED = 1_000_000;

/**
 * What one kind of declaration knows about its values. `rest` is where a field of this kind
 * starts when it has no `.initial`; `null` means it starts at every value. `adopt` turns what
 * `.initial` accepts into a value, and `decode` reads a value back from its JSON form, throwing
 * when the JSON is of another kind.
 */
export interface Shape<T, In = T> {
  readonly kind: string;
  readonly count: number;
  readonly rest: { readonly value: T } | null;
  values(): readonly T[];
  has(value: unknown): value is T;
  adopt(value: In): T;
  decode(json: Json): unknown;
}

const shown = (value: unknown): string =>
  value === undefined || typeof value === "function" || typeof value === "symbol"
    ? String(value)
    : JSON.stringify(isRecord(value) || value instanceof ModelValue ? toJson(value) : value);

const unreadable = (kind: string, json: Json): Error =>
  new Error(`${JSON.stringify(json)} can't be read as a ${kind}`);

const optionalOf = <T, In>(inner: Shape<T, In>): Shape<T | null, In | null> => ({
  kind: "optional",
  count: inner.count + 1,
  rest: null,
  values: () => [null, ...inner.values()],
  has: (value): value is T | null => value === null || inner.has(value),
  adopt: (value) => (value === null ? null : inner.adopt(value)),
  decode: (json) => (json === null ? null : inner.decode(json)),
});

/**
 * The finite set of values a field, an input or a collection member can take. Build one with
 * `m.boolean()`, `m.int(lo, hi)` and the other builders; it is a plain value, so one declaration
 * can be shared by several fields and models. `.optional()` and `.initial(value)` give back a
 * new declaration and leave this one as it was.
 */
export class Declaration<T, In = T> {
  readonly #shape: Shape<T, In>;
  readonly #initial: { readonly value: T } | null;

  /** Model files use the `m` builders instead of calling this. */
  constructor(shape: Shape<T, In>, initial: { readonly value: T } | null = null) {
    this.#shape = shape;
    this.#initial = initial;
  }

  /** What was declared, in words: `"enum"`, `"number"`, `"set"` and so on. */
  get kind(): string {
    return this.#shape.kind;
  }

  /** How many values the declaration has. Very large counts are approximate. */
  get count(): number {
    return this.#shape.count;
  }

  /**
   * Every value, in the order the checker tries them: `false` before `true`, numbers upward,
   * enum values as listed, `null` before the values of an optional. Throws when there are more
   * than `MOST_VALUES_LISTED`.
   */
  values(): readonly T[] {
    if (this.#shape.count > MOST_VALUES_LISTED) {
      throw new Error(
        `this ${this.kind} has ${this.#shape.count} values, too many to list; declare a smaller one`,
      );
    }
    return this.#shape.values();
  }

  /** Whether `value` is one of the declared values. Any value may be asked about. */
  has(value: unknown): value is T {
    return this.#shape.has(value);
  }

  /**
   * The values a field declared this way starts at: the `.initial` value when there is one;
   * otherwise the empty, free, idle or zero value of a collection, lock, timer, request or
   * retry; otherwise every value.
   */
  starts(): readonly T[] {
    if (this.#initial !== null) {
      return [this.#initial.value];
    }
    return this.#shape.rest === null ? this.values() : [this.#shape.rest.value];
  }

  /**
   * Reads a value from the form `earnest verify --json` prints. The result may lie outside the
   * declaration, as a value on an out-of-range path does, so check it with `has`. Throws when
   * the JSON is of another kind altogether, such as text where a set is declared.
   */
  fromJson(json: Json): unknown {
    return this.#shape.decode(json);
  }

  /** The same declaration, starting at `value`. Throws when `value` is not a declared value. */
  initial(value: In): Declaration<T, In> {
    const adopted = frozen(this.#shape.adopt(value));
    if (!this.#shape.has(adopted)) {
      throw new Error(`${shown(adopted)} is not a value of this ${this.kind}`);
    }
    return new Declaration(this.#shape, { value: adopted });
  }

  /**
   * The same declaration with `null` as one more value. Without `.initial` it then starts at
   * every value, `null` included. Throws for a declaration that is already optional and for a
   * lock with an owner, whose free state is already `null`.
   */
  optional(): Declaration<T | null, In | null> {
    if (this.kind === "optional" || this.kind === "owned lock") {
      throw new Error(`an ${this.kind} can't be made optional: it already has a null value`);
    }
    return new Declaration(optionalOf(this.#shape), this.#initial);
  }
}

/** The fields of a model or a record, or the inputs of an action: a declaration for each name. */
export type Fields = Readonly<Record<string, Declaration<unknown, never>>>;

/** The type of the values a declaration has. */
export type ValueOf<D> = D extends Declaration<infer T, infer _In> ? T : never;

/** The object type with one read-only property per declared name. */
export type StateOf<F extends Fields> = { readonly [Name in keyof F]: ValueOf<F[Name]> };

const plain = (kind: string, json: Json): Json => {
  if (typeof json === "object") {
    throw unreadable(kind, json);
  }
  return json;
};

const same = <T>(value: T): T => value;

/** Declares a field that is `true` or `false`. */
export const booleanOf = (): Declaration<boolean> =>
  new Declaration({
    kind: "boolean",
    count: 2,
    rest: null,
    values: () => [false, true],
    has: (value): value is boolean => typeof value === "boolean",
    adopt: same,
    decode: (json) => plain("boolean", json),
  });

/**
 * Declares a whole number from `lo` to `hi`, both included. Arithmetic that leaves the range is
 * not an error in the action: the checker reports the state as out of range. Throws when a bound
 * is not a whole number or `lo` is above `hi`.
 */
export const int = (lo: number, hi: number): Declaration<number> => {
  if (!Number.isSafeInteger(lo) || !Number.isSafeInteger(hi) || lo > hi) {
    throw new Error(`m.int(${lo}, ${hi}) needs two whole numbers, the lower one first`);
  }
  return new Declaration({
    kind: "number",
    count: hi - lo + 1,
    rest: null,
    values: () => Array.from({ length: hi - lo + 1 }, (_, position) => lo + position),
    has: (value): value is number =>
      typeof value === "number" && Number.isInteger(value) && lo <= value && value <= hi,
    adopt: same,
    decode: (json) => plain("number", json),
  });
};

const choice = <T extends string | number | boolean>(
  builder: string,
  options: readonly T[],
): Declaration<T> => {
  const listed = [...options];
  if (listed.length === 0 || new Set(listed).size !== listed.length) {
    throw new Error(`${builder} needs a list of one or more different values`);
  }
  if (new Set(listed.map((option) => typeof option)).size !== 1) {
    throw new Error(`${builder} needs values that are all text, all numbers or all booleans`);
  }
  return new Declaration({
    kind: "enum",
    count: listed.length,
    rest: null,
    values: () => listed,
    has: (value): value is T => listed.some((option) => option === value),
    adopt: same,
    decode: (json) => plain("enum", json),
  });
};

/** Declares a field that is one of the listed texts. Its type is the union of those literals. */
export const enumOf = <const T extends string>(options: readonly T[]): Declaration<T> =>
  choice("m.enum", options);

/**
 * Declares a field that is one of the listed values, which are all text, all numbers or all
 * booleans. Its type is the union of those literals.
 */
export const oneOf = <const T extends string | number | boolean>(
  options: readonly T[],
): Declaration<T> => choice("m.oneOf", options);

const product = <T>(prefixes: readonly (readonly T[])[], options: readonly T[]): T[][] =>
  prefixes.flatMap((prefix) => options.map((option) => [...prefix, option]));

/**
 * Declares a record: an object with exactly the given fields. Read a field with `r.field` and
 * change one with `{ ...r, field: value }`. Compare records field by field: `===` on two records
 * compares the objects, not their content.
 */
export const record = <F extends Fields>(fields: F): Declaration<StateOf<F>> => {
  const entries = Object.entries(fields);
  const has = (value: unknown): value is StateOf<F> =>
    isRecord(value) &&
    Object.keys(value).length === entries.length &&
    entries.every(([name, declared]) => declared.has(value[name]));
  return new Declaration({
    kind: "record",
    count: entries.reduce((total, [, declared]) => total * declared.count, 1),
    rest: null,
    values: () =>
      entries
        .reduce<Row[]>(
          (partial, [name, declared]) =>
            partial.flatMap((prefix) =>
              declared.values().map((value) => ({ ...prefix, [name]: value })),
            ),
          [{}],
        )
        .map(frozen)
        .filter(has),
    has,
    adopt: same,
    decode: (json) => {
      if (!isJsonObject(json) || Object.keys(json).length !== entries.length) {
        throw unreadable(
          `record with the fields ${entries.map(([name]) => name).join(", ")}`,
          json,
        );
      }
      return Object.fromEntries(
        entries.map(([name, declared]) => {
          const found = json[name];
          if (found === undefined) {
            throw unreadable(`record with the field ${name}`, json);
          }
          return [name, declared.fromJson(found)];
        }),
      );
    },
  });
};

const listOf = (kind: string, json: Json): readonly Json[] => {
  if (!isJsonList(json)) {
    throw unreadable(kind, json);
  }
  return json;
};

const subsets = <T>(items: readonly T[]): SetValue<T>[] =>
  items.reduce(
    (smaller, item) => [...smaller, ...smaller.map((subset) => subset.add(item))],
    [SetValue.of<T>([])],
  );

const mapsOver = <K, V>(keys: readonly K[], values: readonly V[]): MapValue<K, V>[] =>
  keys.reduce(
    (smaller, key) => [
      ...smaller,
      ...smaller.flatMap((known) => values.map((value) => known.set(key, value))),
    ],
    [MapValue.of<K, V>([])],
  );

/** Declares a set of values of `item`. Without `.initial` it starts empty. */
export const set = <T>(item: Declaration<T, never>): Declaration<SetValue<T>> =>
  new Declaration({
    kind: "set",
    count: 2 ** item.count,
    rest: { value: SetValue.of<T>([]) },
    values: () => subsets(item.values()),
    has: (value): value is SetValue<T> =>
      value instanceof SetValue && value.every((each) => item.has(each)),
    adopt: same,
    decode: (json) => SetValue.of(listOf("set", json).map((each) => item.fromJson(each))),
  });

const holdable = <T>(builder: string, inside: Declaration<T, never>): Declaration<T, never> => {
  if (inside.kind === "owned lock") {
    throw new Error(
      `${builder} can't hold a lock with an owner: a free lock is already null, so reading it could not be told from reading nothing`,
    );
  }
  return inside;
};

/**
 * Declares a map from values of `key` to values of `value`. Without `.initial` it starts empty.
 * Throws when `value` is a lock with an owner.
 */
export const map = <K, V>(
  key: Declaration<K, never>,
  valueDeclared: Declaration<V, never>,
): Declaration<MapValue<K, V>> => {
  const value = holdable("m.map", valueDeclared);
  return new Declaration({
    kind: "map",
    count: (value.count + 1) ** key.count,
    rest: { value: MapValue.of<K, V>([]) },
    values: () => mapsOver(key.values(), value.values()),
    has: (found): found is MapValue<K, V> =>
      found instanceof MapValue &&
      [...found].every(([each, held]) => key.has(each) && value.has(held)),
    adopt: same,
    decode: (json) =>
      MapValue.of(
        listOf("map", json).map((pair): readonly [unknown, unknown] => {
          const [each, held, ...rest] = listOf("map entry", pair);
          if (each === undefined || held === undefined || rest.length > 0) {
            throw unreadable("map entry", pair);
          }
          return [key.fromJson(each), value.fromJson(held)];
        }),
      ),
  });
};

const lists = <T>(items: readonly T[], longest: number): T[][] => {
  const all: T[][] = [[]];
  let last: T[][] = [[]];
  for (let length = 0; length < longest; length += 1) {
    last = product(last, items);
    all.push(...last);
  }
  return all;
};

const listCount = (each: number, longest: number): number =>
  Array.from({ length: longest + 1 }, (_, length) => each ** length).reduce(
    (total, ofLength) => total + ofLength,
    0,
  );

const wholeLimit = (builder: string, limit: number): number => {
  if (!Number.isSafeInteger(limit) || limit < 0) {
    throw new Error(`${builder} needs a whole number that is not negative, not ${limit}`);
  }
  return limit;
};

/**
 * Declares a sequence of at most `max` values of `item`. Without `.initial` it starts empty.
 * Pushing past `max` is not an error in the action: the checker reports the state as out of
 * range. Throws when `item` is a lock with an owner.
 */
export const seq = <T>(
  itemDeclared: Declaration<T, never>,
  limit: { readonly max: number },
): Declaration<SeqValue<T>> => {
  const item = holdable("m.seq", itemDeclared);
  const max = wholeLimit("m.seq's max", limit.max);
  return new Declaration({
    kind: "sequence",
    count: listCount(item.count, max),
    rest: { value: SeqValue.of<T>([]) },
    values: () => lists(item.values(), max).map((items) => SeqValue.of(items)),
    has: (value): value is SeqValue<T> =>
      value instanceof SeqValue && value.size <= max && value.every((each) => item.has(each)),
    adopt: same,
    decode: (json) => SeqValue.of(listOf("sequence", json).map((each) => item.fromJson(each))),
  });
};

/**
 * Declares a first-in, first-out queue of at most `capacity` values of `item`. Without
 * `.initial` it starts empty; `.initial(m.seqOf(a, b))` starts it with `a` at the head. Sending
 * to a full queue is not an error in the action: the checker reports the state as out of range.
 * Throws when `item` is a lock with an owner.
 */
export const queue = <T>(
  itemDeclared: Declaration<T, never>,
  limit: { readonly capacity: number },
): Declaration<QueueValue<T>, QueueValue<T> | SeqValue<T>> => {
  const item = holdable("m.queue", itemDeclared);
  const capacity = wholeLimit("m.queue's capacity", limit.capacity);
  return new Declaration({
    kind: "queue",
    count: listCount(item.count, capacity),
    rest: { value: QueueValue.of<T>(capacity, []) },
    values: () => lists(item.values(), capacity).map((items) => QueueValue.of(capacity, items)),
    has: (value): value is QueueValue<T> =>
      value instanceof QueueValue &&
      value.capacity === capacity &&
      value.size <= capacity &&
      value.every((each) => item.has(each)),
    adopt: (value) => QueueValue.of(capacity, value),
    decode: (json) =>
      QueueValue.of(
        capacity,
        listOf("queue", json).map((each) => item.fromJson(each)),
      ),
  });
};

const plainLock = (): Declaration<LockValue> =>
  new Declaration({
    kind: "lock",
    count: 2,
    rest: { value: LockValue.of(false) },
    values: () => [LockValue.of(false), LockValue.of(true)],
    has: (value): value is LockValue => value instanceof LockValue,
    adopt: same,
    decode: (json) => {
      if (typeof json !== "boolean") {
        throw unreadable("lock", json);
      }
      return LockValue.of(json);
    },
  });

const ownedLock = <Owner>(owner: Declaration<Owner, never>): Declaration<OwnedLockValue<Owner>> => {
  if (owner.kind === "optional") {
    throw new Error("a free lock already has no owner, so the owner can't be optional");
  }
  return new Declaration({
    kind: "owned lock",
    count: owner.count + 1,
    rest: { value: OwnedLockValue.of<Owner>(null) },
    values: () => [
      OwnedLockValue.of<Owner>(null),
      ...owner.values().map((each) => OwnedLockValue.of(each)),
    ],
    has: (value): value is OwnedLockValue<Owner> =>
      value instanceof OwnedLockValue && (value.holder === null || owner.has(value.holder)),
    adopt: same,
    decode: (json) => OwnedLockValue.of(json === null ? null : owner.fromJson(json)),
  });
};

/**
 * Declares a lock. `m.lock()` is free or held; `m.lock(Owner)` also remembers which value of
 * `Owner` holds it. Without `.initial` it starts free.
 */
export function lock(): Declaration<LockValue>;
export function lock<Owner>(owner: Declaration<Owner, never>): Declaration<OwnedLockValue<Owner>>;
export function lock(
  owner?: Declaration<unknown, never>,
): Declaration<LockValue> | Declaration<OwnedLockValue<unknown>> {
  return owner === undefined ? plainLock() : ownedLock(owner);
}

const staged = <Stage extends string, T extends ModelValue>(
  kind: string,
  stages: readonly Stage[],
  at: (stage: Stage) => T,
  has: (value: unknown) => value is T,
): Declaration<T> => {
  const values = stages.map(at);
  return new Declaration({
    kind,
    count: values.length,
    rest: values[0] === undefined ? null : { value: values[0] },
    values: () => values,
    has,
    adopt: same,
    decode: (json) => {
      const stage = stages.find((each) => each === json);
      if (stage === undefined) {
        throw unreadable(kind, json);
      }
      return at(stage);
    },
  });
};

/** Declares a timer that is idle, running or fired. Without `.initial` it starts idle. */
export const timer = (): Declaration<TimerValue> =>
  staged(
    "timer",
    TIMER_STAGES,
    TimerValue.at,
    (value): value is TimerValue => value instanceof TimerValue,
  );

/**
 * Declares an async request that is idle, pending, succeeded or failed. Without `.initial` it
 * starts idle.
 */
export const request = (): Declaration<RequestValue> =>
  staged(
    "request",
    REQUEST_STAGES,
    RequestValue.at,
    (value): value is RequestValue => value instanceof RequestValue,
  );

/**
 * Declares a count of attempts from zero to `max`. Without `.initial` it starts at zero.
 * Counting past `max` is not an error in the action: the checker reports the state as out of
 * range, so guard `again()` with `canRetry`.
 */
export const retry = (limit: { readonly max: number }): Declaration<RetryValue> => {
  const max = wholeLimit("m.retry's max", limit.max);
  return new Declaration({
    kind: "retry",
    count: max + 1,
    rest: { value: RetryValue.of(max, 0) },
    values: () => Array.from({ length: max + 1 }, (_, attempts) => RetryValue.of(max, attempts)),
    has: (value): value is RetryValue =>
      value instanceof RetryValue && value.max === max && value.attempts <= max,
    adopt: same,
    decode: (json) => {
      if (typeof json !== "number") {
        throw unreadable("retry", json);
      }
      return RetryValue.of(max, json);
    },
  });
};

const itemsOf = <T>(over: Declaration<T, never> | Iterable<T>): readonly T[] =>
  over instanceof Declaration ? over.values() : [...over];

/**
 * Whether `test` is true for every value of a declaration, or for every member of a set,
 * sequence or queue. Over a map it is given `[key, value]` pairs. True when there is nothing to
 * test.
 */
export const forall = <T>(
  over: Declaration<T, never> | Iterable<T>,
  test: (item: T) => boolean,
): boolean => itemsOf(over).every((item) => test(item));

/**
 * Whether `test` is true for some value of a declaration, or for some member of a set, sequence
 * or queue. Over a map it is given `[key, value]` pairs. False when there is nothing to test.
 */
export const exists = <T>(
  over: Declaration<T, never> | Iterable<T>,
  test: (item: T) => boolean,
): boolean => itemsOf(over).some((item) => test(item));
