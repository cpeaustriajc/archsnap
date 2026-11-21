import { expect, test } from "vitest";
import * as library from "@earnest/model";
import { explore, m, replay } from "@earnest/model";
import * as cart from "./models/cart.model.ts";
import * as turnstile from "./models/turnstile.model.ts";

const settled = async <Value>(value: Value): Promise<Value> => value;

const carriesThen = (value: unknown): boolean =>
  (typeof value === "object" || typeof value === "function") && value !== null && "then" in value;

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
  const members = ["changes", "input", "kind", "sentence", "when"];
  expect(Object.keys(cart.press).sort()).toStrictEqual(members);
  expect(Object.keys(turnstile.pay).sort()).toStrictEqual(members);
  expect(turnstile.pay.input).toBeUndefined();
  expect(turnstile.pay.when({ gate: "locked", paid: false, visitors: 0 }, {})).toBe(true);
  expect(cart.press.kind).toBe("action");
  expect(cart.press.sentence).toBe("the buyer presses Add");
});

test("a rule, an end and a view keep their members", () => {
  expect(Object.keys(cart.focusLandsOnInCart).sort()).toStrictEqual(["holds", "kind", "says"]);
  expect(Object.keys(cart.inCart).sort()).toStrictEqual(["kind", "says", "when"]);
  expect(cart.view.kind).toBe("view");
  expect(cart.view.title).toBe("Add to cart focus");
  expect(cart.AddToCartFocus.views).toStrictEqual([cart.view]);
});

test("no value the package exports, and no part it makes, has a member named then", () => {
  const { actions, rules, ends } = cart.AddToCartFocus.runners();
  const exported = [...Object.values(library), ...Object.values(library.m)];
  const prototypes = exported.flatMap((value) =>
    typeof value === "function" ? [value.prototype] : [],
  );
  const made = [
    cart.press,
    cart.focusLandsOnInCart,
    cart.inCart,
    cart.view,
    cart.AddToCartFocus,
    cart.AddToCartFocus.runners(),
    ...actions,
    ...rules,
    ...ends,
    ...Object.values(cart.AddToCartFocus.fields),
    library.m.int(0, 1),
  ];
  expect(exported.length).toBeGreaterThan(0);
  expect([...exported, ...prototypes, ...made].filter(carriesThen)).toStrictEqual([]);
});

test("an async function that returns an action settles to that action", async () => {
  expect(await settled(cart.press)).toBe(cart.press);
  expect(await settled(cart.focusLandsOnInCart)).toBe(cart.focusLandsOnInCart);
  expect(await settled(cart.inCart)).toBe(cart.inCart);
  expect(await settled(cart.view)).toBe(cart.view);
  expect(await settled(cart.AddToCartFocus)).toBe(cart.AddToCartFocus);
});

test("the parts m.action, m.rule, m.end and m.view return are frozen", () => {
  expect(Object.isFrozen(cart.press)).toBe(true);
  expect(Object.isFrozen(cart.focusLandsOnInCart)).toBe(true);
  expect(Object.isFrozen(cart.inCart)).toBe(true);
  expect(Object.isFrozen(cart.view)).toBe(true);
});
