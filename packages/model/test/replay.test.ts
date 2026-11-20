import { assert, expect, test } from "vitest";
import { type Named, explore, m, replay } from "@earnest/model";
import * as cart from "./models/cart.model.ts";

const n = (value: number): Named[] => [{ name: "n", value }];

const hops = () => {
  const Hops = m.model("Hops", { n: m.int(0, 5).initial(0) });
  const one = Hops.action("add one", { when: (s) => s.n < 5, then: (s) => ({ n: s.n + 1 }) });
  const two = Hops.action("add two", { when: (s) => s.n <= 3, then: (s) => ({ n: s.n + 2 }) });
  const belowFour = Hops.rule({ says: "n stays below four", holds: (s) => s.n < 4 });
  const atFive = Hops.end({ says: "n is five", when: (s) => s.n === 5 });
  return { Hops, one, two, belowFour, atFive };
};

const cartStart: Named[] = [
  { name: "surface", value: "shortlist" },
  { name: "cta", value: "Add" },
  { name: "focus", value: "Add" },
  { name: "pressBy", value: null },
  { name: "keyboardAdd", value: false },
];

const cartPressed = (by: string, keyboardAdd: boolean): Named[] => [
  { name: "surface", value: "shortlist" },
  { name: "cta", value: "Adding" },
  { name: "focus", value: "Add" },
  { name: "pressBy", value: by },
  { name: "keyboardAdd", value: keyboardAdd },
];

test("a path is applied one action at a time and gives every state on the way", () => {
  const { Hops, ...exports } = hops();
  const path = {
    start: n(0),
    steps: [
      { action: "two", state: n(2) },
      { action: "one", state: n(3) },
      { action: "two", state: n(5) },
    ],
  };
  expect(replay(Hops, path, { exports })).toStrictEqual([{ n: 0 }, { n: 2 }, { n: 3 }, { n: 5 }]);
});

test("a path with no steps gives its start state", () => {
  const { Hops } = hops();
  expect(replay(Hops, { start: n(0), steps: [] })).toStrictEqual([{ n: 0 }]);
});

test("without exports a step is matched by its label", () => {
  const { Hops } = hops();
  const path = { start: n(0), steps: [{ action: "two", label: "add two", state: n(2) }] };
  expect(replay(Hops, path)).toStrictEqual([{ n: 0 }, { n: 2 }]);
});

test("a path from explore replays to the state that breaks the rule", () => {
  const { Hops, ...exports } = hops();
  const path = explore(Hops, { exports }).rules[0]?.path;
  assert.ok(path);
  expect(replay(Hops, path, { exports })).toStrictEqual([{ n: 0 }, { n: 2 }, { n: 4 }]);
  expect(replay(Hops, path)).toStrictEqual([{ n: 0 }, { n: 2 }, { n: 4 }]);
});

test("inputs are applied, from either start state", () => {
  const path = {
    start: cartStart,
    steps: [
      {
        action: "press",
        input: [{ name: "by", value: "mouse" }],
        state: cartPressed("mouse", false),
      },
    ],
  };
  expect(replay(cart.AddToCartFocus, path, { exports: cart })).toStrictEqual([
    { surface: "shortlist", cta: "Add", focus: "Add", pressBy: null, keyboardAdd: false },
    { surface: "shortlist", cta: "Adding", focus: "Add", pressBy: "mouse", keyboardAdd: false },
  ]);
});

test("collections come back in JSON form, and a set may be listed in any order", () => {
  const Letter = m.enum(["a", "b"]);
  const Seen = m.model("Seen", {
    seen: m.set(Letter),
    order: m.seq(Letter, { max: 2 }),
    count: m.map(Letter, m.int(0, 2)),
  });
  const see = Seen.action("a letter is seen", {
    input: { letter: Letter },
    then: (s, { letter }) => ({
      seen: s.seen.add(letter),
      order: s.order.push(letter),
      count: s.count.set(letter, (s.count.get(letter) ?? 0) + 1),
    }),
  });
  const empty: Named[] = [
    { name: "seen", value: [] },
    { name: "order", value: [] },
    { name: "count", value: [] },
  ];
  const path = {
    start: empty,
    steps: [
      {
        action: "see",
        input: [{ name: "letter", value: "b" }],
        state: [
          { name: "seen", value: ["b"] },
          { name: "order", value: ["b"] },
          { name: "count", value: [["b", 1]] },
        ],
      },
      {
        action: "see",
        input: [{ name: "letter", value: "a" }],
        state: [
          { name: "seen", value: ["b", "a"] },
          { name: "order", value: ["b", "a"] },
          {
            name: "count",
            value: [
              ["b", 1],
              ["a", 1],
            ],
          },
        ],
      },
    ],
  };
  expect(replay(Seen, path, { exports: { see } })).toStrictEqual([
    { seen: [], order: [], count: [] },
    { seen: ["b"], order: ["b"], count: [["b", 1]] },
    {
      seen: ["a", "b"],
      order: ["b", "a"],
      count: [
        ["a", 1],
        ["b", 1],
      ],
    },
  ]);
});

test("a path that does not begin at a start state is refused", () => {
  const { Hops, ...exports } = hops();
  expect(() => replay(Hops, { start: n(1), steps: [] }, { exports })).toThrow(
    /Hops: the path does not begin at a start state of the model/,
  );
});

test("a start that leaves out a field, names a stranger or has the wrong kind is refused", () => {
  const { Hops } = hops();
  expect(() => replay(Hops, { start: [], steps: [] })).toThrow(
    /the start of the path has no value for `n`/,
  );
  expect(() => replay(Hops, { start: [...n(0), { name: "m", value: 0 }], steps: [] })).toThrow(
    /the start of the path names `m`, which is not declared/,
  );
  expect(() => replay(Hops, { start: [{ name: "n", value: [0] }], steps: [] })).toThrow(
    /the start of the path can't be read at `n`: \[0\] can't be read as a number/,
  );
});

test("a step that names no action of the model is refused", () => {
  const { Hops, ...exports } = hops();
  const path = { start: n(0), steps: [{ action: "three", state: n(3) }] };
  expect(() => replay(Hops, path, { exports })).toThrow(
    /Hops: step 1 \(three\): the model has no action with this id or label/,
  );
});

test("a label shared by two actions is refused when there are no exports to tell them apart", () => {
  const Twins = m.model("Twins", { n: m.int(0, 3).initial(0) });
  const first = Twins.action("n changes", { then: () => ({ n: 1 }) });
  const second = Twins.action("n changes", { then: () => ({ n: 2 }) });
  const path = { start: n(0), steps: [{ action: "second", label: "n changes", state: n(2) }] };
  expect(() => replay(Twins, path)).toThrow(/step 1 \(second\): more than one action matches/);
  expect(replay(Twins, path, { exports: { first, second } })).toStrictEqual([{ n: 0 }, { n: 2 }]);
});

test("a step whose when is false is refused", () => {
  const { Hops, ...exports } = hops();
  const path = {
    start: n(0),
    steps: [
      { action: "two", state: n(2) },
      { action: "two", state: n(4) },
      { action: "two", state: n(6) },
    ],
  };
  expect(() => replay(Hops, path, { exports })).toThrow(
    /Hops: step 3 \(two\): the `when` of the action is false in this state/,
  );
});

test("a step whose state differs from what the action gives is refused", () => {
  const { Hops, ...exports } = hops();
  const path = { start: n(0), steps: [{ action: "two", state: n(3) }] };
  expect(() => replay(Hops, path, { exports })).toThrow(
    /Hops: step 1 \(two\): the state after the action differs from the path: `n` is 2 but the path has 3/,
  );
});

test("a step with a missing, undeclared or stray input is refused", () => {
  const step = (input: Named[]) => ({
    start: cartStart,
    steps: [{ action: "press", input, state: cartPressed("mouse", false) }],
  });
  const run = (input: Named[]) => () => replay(cart.AddToCartFocus, step(input), { exports: cart });
  expect(run([])).toThrow(/step 1 \(press\): the input has no value for `by`/);
  expect(run([{ name: "by", value: "touch" }])).toThrow(
    /step 1 \(press\): the input `by` is "touch", which is not a value it declares/,
  );
  expect(
    run([
      { name: "by", value: "mouse" },
      { name: "how", value: "hard" },
    ]),
  ).toThrow(/step 1 \(press\): the input names `how`, which is not declared/);
});

interface Person {
  readonly age: number;
}

const assumedPresent = (who: Person | null): who is Person => who !== undefined;

test("a step whose action can't be evaluated is refused", () => {
  const People = m.model("People", {
    who: m
      .record({ age: m.int(0, 1) })
      .optional()
      .initial(null),
  });
  const age = People.action("a year passes", {
    then: (s) => ({ who: assumedPresent(s.who) ? { age: s.who.age + 1 } : null }),
  });
  const nobody: Named[] = [{ name: "who", value: null }];
  const path = { start: nobody, steps: [{ action: "age", state: nobody }] };
  expect(() => replay(People, path, { exports: { age } })).toThrow(
    /People: step 1 \(age\): the `then` of the action reads a value that is null in this state/,
  );
});

test("a path may end out of range, and nothing can follow that state", () => {
  const Climb = m.model("Climb", { n: m.int(0, 2).initial(0) });
  const up = Climb.action("n goes up", { then: (s) => ({ n: s.n + 1 }) });
  const exports = { up };
  const [finding] = explore(Climb, { exports }).findings;
  assert.ok(finding);
  expect(replay(Climb, finding.path, { exports })).toStrictEqual([
    { n: 0 },
    { n: 1 },
    { n: 2 },
    { n: 3 },
  ]);
  const further = {
    start: finding.path.start,
    steps: [...finding.path.steps, { action: "up", state: n(4) }],
  };
  expect(() => replay(Climb, further, { exports })).toThrow(
    /Climb: step 4 \(up\): the state before it is out of range at `n`, so no action can follow/,
  );
});
