import { type Action, type End, type Rule, type View, m } from "@earnest/model";

type Same<Left, Right> =
  (<T>() => T extends Left ? 1 : 2) extends <T>() => T extends Right ? 1 : 2 ? true : false;

type Accepts<Target, Value> = [Value] extends [Target] ? true : false;

type Rejects<Target, Value> = [Value] extends [Target] ? false : true;

type Holds<Claim extends true> = Claim;

type PressedBy = "keyboard" | "virtual" | "mouse";

const Press = m.enum(["keyboard", "virtual", "mouse"]);

const Cart = m.model("Cart", {
  cta: m.enum(["Add", "Adding", "InCart"]).initial("Add"),
  pressBy: Press.optional().initial(null),
  count: m.int(0, 3).initial(0),
});

type State = m.Infer<typeof Cart>;

const press = m.action(Cart, "the buyer presses Add", {
  input: { by: Press },
  when: (s, { by }) => s.cta === "Add" && by !== "virtual",
  changes: (_, { by }) => ({ cta: "Adding", pressBy: by }),
});

const changesFirst = m.action(Cart, "changes written first", {
  changes: (_, { by }) => ({ pressBy: by }),
  when: (_, { by }) => by === "mouse",
  input: { by: Press },
});

const whenFirst = m.action(Cart, "when written first", {
  when: (_, { by }) => by === "mouse",
  input: { by: Press },
  changes: (_, { by }) => ({ pressBy: by }),
});

const inputLast = m.action(Cart, "input written last", {
  changes: (_, { by }) => ({ pressBy: by }),
  input: { by: Press },
  when: (_, { by }) => by === "mouse",
});

const noInput = m.action(Cart, "no input", { changes: () => ({ cta: "InCart" }) });

const bothMembers = m.action(Cart, "when and changes without input", {
  changes: (s) => ({ count: s.count + 1 }),
  when: (s) => s.count < 3,
});

const pressed = m.rule(Cart, "A press is recorded", (s) => s.cta === "Add" || s.pressBy !== null);

const inCart = m.end(Cart, "The item is in the cart", (s) => s.cta === "InCart");

const view = m.view(Cart, { title: "Cart", step: { press: "press Add" } });

const emptyView = m.view(Cart);

type Changes = ReturnType<typeof press.changes>;

type PressInput = Parameters<typeof press.when>[1];

type RuleGiven = Parameters<typeof m.rule<typeof Cart.fields>>[2];

type ActionKeys<Given> = Extract<keyof Given, "then">;

export type Proofs = [
  Holds<Same<typeof press, Action<typeof Cart.fields, { by: typeof Press }>>>,
  Holds<Same<typeof changesFirst, typeof press>>,
  Holds<Same<typeof whenFirst, typeof press>>,
  Holds<Same<typeof inputLast, typeof press>>,
  Holds<Same<typeof noInput, Action<typeof Cart.fields>>>,
  Holds<Same<typeof bothMembers, Action<typeof Cart.fields>>>,
  Holds<Same<typeof pressed, Rule<typeof Cart.fields>>>,
  Holds<Same<typeof inCart, End<typeof Cart.fields>>>,
  Holds<Same<typeof view, View<typeof Cart.fields>>>,
  Holds<Same<typeof emptyView, View<typeof Cart.fields>>>,

  Holds<Same<PressInput, { readonly by: PressedBy }>>,
  Holds<Same<Parameters<typeof press.changes>[0], State>>,
  Holds<Same<Parameters<typeof press.changes>[1], PressInput>>,
  Holds<Same<Parameters<typeof changesFirst.changes>[1], PressInput>>,
  Holds<Same<Parameters<typeof whenFirst.when>[1], PressInput>>,
  Holds<Same<Parameters<typeof inputLast.when>[1], PressInput>>,
  Holds<Same<Parameters<typeof noInput.changes>[1], Readonly<Record<never, never>>>>,

  Holds<Accepts<Changes, { cta: "Adding"; pressBy: "mouse" }>>,
  Holds<Accepts<Changes, { pressBy: null }>>,
  Holds<Accepts<Changes, Record<never, never>>>,
  Holds<Rejects<Changes, { cta: "Nope" }>>,
  Holds<Rejects<Changes, { cta: null }>>,
  Holds<Rejects<Changes, { cta: undefined }>>,
  Holds<Rejects<Changes, { count: "one" }>>,
  Holds<Rejects<Changes, { basket: 1 }>>,

  Holds<Rejects<PressInput, { by: "touch" }>>,
  Holds<Rejects<PressInput, { by: null }>>,
  Holds<Rejects<PressInput, Record<never, never>>>,

  Holds<Accepts<RuleGiven, (state: State) => boolean>>,
  Holds<Rejects<RuleGiven, (state: State) => number>>,
  Holds<Rejects<RuleGiven, (state: { missing: 1 }) => boolean>>,

  Holds<Same<ActionKeys<typeof press>, never>>,
  Holds<Same<ActionKeys<typeof pressed>, never>>,
  Holds<Same<ActionKeys<typeof inCart>, never>>,
  Holds<Same<ActionKeys<typeof view>, never>>,
  Holds<Same<ActionKeys<ReturnType<typeof Cart.runners>["actions"][number]>, never>>,
];
