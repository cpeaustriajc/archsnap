import { expect, test } from "vitest";
import { type Named, explore, m, replay } from "@earnest/model";
import * as oldCart from "./models/cart.model.ts";
import * as oldTurnstile from "./models/turnstile.model.ts";

const Press = m.enum(["keyboard", "virtual", "mouse"]);

const AddToCartFocus = m.model("AddToCartFocus", {
  surface: m.enum(["results", "shortlist"]),
  cta: m.enum(["Add", "Adding", "InCart"]).initial("Add"),
  focus: m.enum(["Add", "InCart", "Heart", "Away", "Lost"]).initial("Add"),
  pressBy: Press.optional().initial(null),
  keyboardAdd: m.boolean().initial(false),
});

const press = m.action(AddToCartFocus, "the buyer presses Add", {
  input: { by: Press },
  when: (s, { by }) => s.cta === "Add" && (by === "mouse" || s.focus === "Add"),
  changes: (_, { by }) => ({ cta: "Adding", pressBy: by, keyboardAdd: by === "keyboard" }),
});

const focusLandsOnInCart = m.rule(
  AddToCartFocus,
  "After a keyboard add lands, focus is on In cart",
  (s) => s.pressBy !== "keyboard" || s.cta !== "InCart" || s.focus === "InCart",
);

const inCart = m.end(AddToCartFocus, "The item is in the cart", (s) => s.cta === "InCart");

const view = m.view(AddToCartFocus, {
  title: "Add to cart focus",
  state: (s) => `${s.cta}, focus on ${s.focus}`,
  group: (s) => s.surface,
  step: { press: "press Add" },
  colors: { broken: "#c62828" },
});

const newCart = { AddToCartFocus, press, focusLandsOnInCart, inCart, view };

const Turnstile = m.model("Turnstile", {
  gate: m.enum(["locked", "open"]).initial("locked"),
  paid: m.boolean().initial(false),
  visitors: m.int(0, 3).initial(0),
});

const pay = m.action(Turnstile, "the visitor pays", {
  when: (s) => !s.paid,
  changes: () => ({ paid: true }),
});

const push = m.action(Turnstile, "the visitor pushes", {
  changes: (s) => ({ gate: "open", visitors: s.visitors + 1 }),
  when: (s) => s.paid && s.visitors < 3,
});

const staysLocked = m.rule(
  Turnstile,
  "The gate stays locked until the visitor pays",
  (s) => s.gate === "locked" || s.paid,
);

const full = m.end(Turnstile, "Three visitors went through", (s) => s.visitors === 3);

const newTurnstile = { Turnstile, pay, push, staysLocked, full };

const turnstileState = (gate: string, paid: boolean, visitors: number): Named[] => [
  { name: "gate", value: gate },
  { name: "paid", value: paid },
  { name: "visitors", value: visitors },
];

const turnstilePath = {
  start: turnstileState("locked", false, 0),
  steps: [
    { action: "pay", state: turnstileState("locked", true, 0) },
    { action: "push", state: turnstileState("open", true, 1) },
  ],
};

const settled = async <Value>(value: Value): Promise<Value> => value;

test("explore gives the cart the results it gives the cart written with the methods", () => {
  const found = explore(AddToCartFocus, { exports: newCart });
  expect(found).toStrictEqual(explore(oldCart.AddToCartFocus, { exports: oldCart }));
  expect(found.states).toBe(8);
  expect(found.rules.map((rule) => rule.id)).toStrictEqual(["focusLandsOnInCart"]);
  expect(found.findings.map((finding) => finding.kind)).toStrictEqual(["stuck"]);
});

test("explore gives the turnstile the results it gives the turnstile written with the methods", () => {
  const found = explore(Turnstile, { exports: newTurnstile });
  expect(found).toStrictEqual(explore(oldTurnstile.Turnstile, { exports: oldTurnstile }));
  expect(found.status).toBe("pass");
  expect(found.states).toBe(5);
});

test("explore without exports names a part by its sentence in both forms", () => {
  expect(explore(AddToCartFocus)).toStrictEqual(explore(oldCart.AddToCartFocus));
});

test("replay follows the path explore found in the cart in both forms", () => {
  const [stuck] = explore(AddToCartFocus, { exports: newCart }).findings;
  const path = stuck?.path;
  expect(path?.steps.length).toBe(1);
  if (path === undefined) {
    return;
  }
  const states = replay(AddToCartFocus, path, { exports: newCart });
  expect(states).toStrictEqual(replay(oldCart.AddToCartFocus, path, { exports: oldCart }));
  expect(states).toStrictEqual([
    { surface: "results", cta: "Add", focus: "Add", pressBy: null, keyboardAdd: false },
    { surface: "results", cta: "Adding", focus: "Add", pressBy: "keyboard", keyboardAdd: true },
  ]);
});

test("replay follows a path through the turnstile in both forms", () => {
  const states = replay(Turnstile, turnstilePath, { exports: newTurnstile });
  expect(states).toStrictEqual(
    replay(oldTurnstile.Turnstile, turnstilePath, { exports: oldTurnstile }),
  );
  expect(states.at(-1)).toStrictEqual({ gate: "open", paid: true, visitors: 1 });
});

test("replay refuses a wrong state with the same words in both forms", () => {
  const wrong = {
    ...turnstilePath,
    steps: [{ action: "pay", state: turnstileState("open", true, 0) }],
  };
  const messageOf = (run: () => unknown): string => {
    try {
      run();
    } catch (thrown) {
      return thrown instanceof Error ? thrown.message : String(thrown);
    }
    return "";
  };
  const message = messageOf(() => replay(Turnstile, wrong, { exports: newTurnstile }));
  expect(message).not.toBe("");
  expect(message).toBe(
    messageOf(() => replay(oldTurnstile.Turnstile, wrong, { exports: oldTurnstile })),
  );
});

test("the keys of an action may be written in any order", () => {
  const Counter = m.model("Counter", { count: m.int(0, 3).initial(0) });
  const Step = m.int(1, 2);
  const lastFirst = m.action(Counter, "last first", {
    changes: (s, { by }) => ({ count: s.count + by }),
    when: (s, { by }) => s.count + by <= 3,
    input: { by: Step },
  });
  const inputLast = m.action(Counter, "input last", {
    when: (s, { by }) => s.count + by <= 3,
    changes: (s, { by }) => ({ count: s.count + by }),
    input: { by: Step },
  });
  const inOrder = m.action(Counter, "in order", {
    input: { by: Step },
    when: (s, { by }) => s.count + by <= 3,
    changes: (s, { by }) => ({ count: s.count + by }),
  });
  const found = explore(Counter, { exports: { lastFirst, inputLast, inOrder } });
  expect(found.states).toBe(4);
  expect(found.findings.map((finding) => finding.kind)).toStrictEqual(["stuck"]);
});

test("an action without input or when happens in every state", () => {
  const Flag = m.model("Flag", { up: m.boolean().initial(false) });
  const raise = m.action(Flag, "the flag goes up", { changes: () => ({ up: true }) });
  const states = replay(
    Flag,
    {
      start: [{ name: "up", value: false }],
      steps: [{ action: "raise", state: [{ name: "up", value: true }] }],
    },
    { exports: { Flag, raise } },
  );
  expect(states).toStrictEqual([{ up: false }, { up: true }]);
});

test("an action made by m.action has the members kind, sentence, input, when and changes", () => {
  expect(Object.keys(press).sort()).toStrictEqual(["changes", "input", "kind", "sentence", "when"]);
  expect(Object.keys(pay).sort()).toStrictEqual(["changes", "input", "kind", "sentence", "when"]);
  expect(pay.input).toBeUndefined();
  expect(pay.when({ gate: "locked", paid: false, visitors: 0 }, {})).toBe(true);
  expect(press.kind).toBe("action");
  expect(press.sentence).toBe("the buyer presses Add");
});

test("a rule, an end and a view keep their members", () => {
  expect(Object.keys(focusLandsOnInCart).sort()).toStrictEqual(["holds", "kind", "says"]);
  expect(Object.keys(inCart).sort()).toStrictEqual(["kind", "says", "when"]);
  expect(view.kind).toBe("view");
  expect(view.title).toBe("Add to cart focus");
  expect(AddToCartFocus.views).toStrictEqual([view]);
});

test("nothing m.action, m.rule, m.end or m.view makes has a member named then", () => {
  const { actions, rules, ends } = AddToCartFocus.runners();
  const made = [press, focusLandsOnInCart, inCart, view, ...actions, ...rules, ...ends];
  expect(made.length).toBe(7);
  for (const each of made) {
    expect("then" in each).toBe(false);
  }
  expect("then" in AddToCartFocus).toBe(false);
  expect("then" in AddToCartFocus.runners()).toBe(false);
});

test("an async function that returns an action settles to that action", async () => {
  expect(await settled(press)).toBe(press);
  expect(await settled(focusLandsOnInCart)).toBe(focusLandsOnInCart);
  expect(await settled(inCart)).toBe(inCart);
  expect(await settled(view)).toBe(view);
  expect(await settled(AddToCartFocus)).toBe(AddToCartFocus);
});

test("the parts m.action, m.rule, m.end and m.view return are frozen", () => {
  expect(Object.isFrozen(press)).toBe(true);
  expect(Object.isFrozen(focusLandsOnInCart)).toBe(true);
  expect(Object.isFrozen(inCart)).toBe(true);
  expect(Object.isFrozen(view)).toBe(true);
});

test("both forms can declare on the same model, in the order they are called", () => {
  const Mixed = m.model("Mixed", { n: m.int(0, 3).initial(0) });
  const byFunction = m.action(Mixed, "by function", {
    when: (s) => s.n < 3,
    changes: (s) => ({ n: s.n + 1 }),
  });
  const byMethod = Mixed.action("by method", {
    when: (s) => s.n < 3,
    then: (s) => ({ n: s.n + 1 }),
  });
  expect(Mixed.runners().actions.map((action) => action.part)).toStrictEqual([
    byFunction,
    byMethod,
  ]);
  expect(Object.keys(byMethod).sort()).toStrictEqual([
    "changes",
    "input",
    "kind",
    "sentence",
    "when",
  ]);
});
