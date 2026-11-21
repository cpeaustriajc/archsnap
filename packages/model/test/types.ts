import {
  type Action,
  type End,
  type LockValue,
  type MapValue,
  type Model,
  type OwnedLockValue,
  type QueueValue,
  type RequestValue,
  type RetryValue,
  type Rule,
  type SeqValue,
  type SetValue,
  type TimerValue,
  type View,
  m,
} from "@earnest/model";
import type { AddToCartFocus, State, press } from "./models/cart.model.ts";

type Same<Left, Right> =
  (<T>() => T extends Left ? 1 : 2) extends <T>() => T extends Right ? 1 : 2 ? true : false;

type Accepts<Target, Value> = [Value] extends [Target] ? true : false;

type Rejects<Target, Value> = [Value] extends [Target] ? false : true;

type Holds<Claim extends true> = Claim;

type PressedBy = "keyboard" | "virtual" | "mouse";

const Press = m.enum(["keyboard", "virtual", "mouse"]);

type Changes = ReturnType<typeof press.changes>;

type PressInput = Parameters<typeof press.when>[1];

type RuleGiven = Parameters<typeof m.rule<typeof AddToCartFocus.fields>>[2];

type EndGiven = Parameters<typeof m.end<typeof AddToCartFocus.fields>>[2];

type ViewGiven = NonNullable<Parameters<typeof m.view<typeof AddToCartFocus.fields>>[1]>;

type ThenKeys<Given> = Extract<keyof Given, "then">;

type PartMakers<Given> = Extract<
  keyof Given,
  | "action"
  | "rule"
  | "end"
  | "view"
  | "declareAction"
  | "declareRule"
  | "declareEnd"
  | "declareView"
>;

const Cart = m.model("Cart", {
  cta: m.enum(["Add", "Adding", "InCart"]).initial("Add"),
  pressBy: Press.optional().initial(null),
  count: m.int(0, 3).initial(0),
});

const cartPress = m.action(Cart, "the buyer presses Add", {
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

const cartView = m.view(Cart, { title: "Cart", step: { press: "press Add" } });

const emptyView = m.view(Cart);

const Owner = m.enum(["ann", "bob"]);

const Point = m.record({ x: m.int(0, 2), tag: m.oneOf([1, 2, 3]) });

const Everything = m.model("Everything", {
  flag: m.boolean(),
  count: m.int(0, 3),
  owner: Owner.optional(),
  point: Point,
  seen: m.set(Owner),
  scores: m.map(Owner, m.int(0, 1)),
  history: m.seq(Owner, { max: 2 }),
  inbox: m.queue(Owner, { capacity: 2 }),
  door: m.lock(),
  desk: m.lock(Owner),
  alarm: m.timer(),
  fetch: m.request(),
  tries: m.retry({ max: 2 }),
});

const Letters = m.enum(["a", "b"]);

const InitialOfLetters = Letters.initial;

const InitialOfQueue = m.queue(Letters, { capacity: 1 }).initial;

const forallOverDeclaration = (): boolean => m.forall(Letters, (letter) => letter === "a");

const existsOverSet = (seen: SetValue<"a" | "b">): boolean =>
  m.exists(seen, (letter) => letter === "b");

export type Proofs = [
  Holds<
    Same<
      State,
      {
        readonly surface: "results" | "shortlist";
        readonly cta: "Add" | "Adding" | "InCart";
        readonly focus: "Add" | "InCart" | "Heart" | "Away" | "Lost";
        readonly pressBy: PressedBy | null;
        readonly keyboardAdd: boolean;
      }
    >
  >,
  Holds<Same<m.Infer<typeof AddToCartFocus>, State>>,
  Holds<Same<m.Infer<typeof Owner>, "ann" | "bob">>,
  Holds<Same<m.Infer<typeof Point>, { readonly x: number; readonly tag: 1 | 2 | 3 }>>,
  Holds<
    Same<
      m.Infer<typeof Everything>,
      {
        readonly flag: boolean;
        readonly count: number;
        readonly owner: "ann" | "bob" | null;
        readonly point: { readonly x: number; readonly tag: 1 | 2 | 3 };
        readonly seen: SetValue<"ann" | "bob">;
        readonly scores: MapValue<"ann" | "bob", number>;
        readonly history: SeqValue<"ann" | "bob">;
        readonly inbox: QueueValue<"ann" | "bob">;
        readonly door: LockValue;
        readonly desk: OwnedLockValue<"ann" | "bob">;
        readonly alarm: TimerValue;
        readonly fetch: RequestValue;
        readonly tries: RetryValue;
      }
    >
  >,
  Holds<Same<PressInput, { readonly by: PressedBy }>>,
  Holds<Same<ReturnType<typeof forallOverDeclaration>, boolean>>,
  Holds<Same<ReturnType<typeof existsOverSet>, boolean>>,

  Holds<Accepts<Changes, { cta: "Adding"; pressBy: "mouse"; keyboardAdd: false }>>,
  Holds<Accepts<Changes, { pressBy: null }>>,
  Holds<Accepts<Changes, Record<never, never>>>,
  Holds<Rejects<Changes, { cta: "Nope" }>>,
  Holds<Rejects<Changes, { cta: null }>>,
  Holds<Rejects<Changes, { cta: undefined }>>,
  Holds<Rejects<Changes, { keyboardAdd: "yes" }>>,
  Holds<Rejects<Changes, { pressBy: "touch" }>>,
  Holds<Rejects<Changes, { basket: 1 }>>,

  Holds<Rejects<PressInput, { by: "touch" }>>,
  Holds<Rejects<PressInput, { by: null }>>,
  Holds<Rejects<PressInput, Record<never, never>>>,

  Holds<Accepts<RuleGiven, (state: State) => boolean>>,
  Holds<Rejects<RuleGiven, (state: State) => number>>,
  Holds<Rejects<RuleGiven, (state: { missing: 1 }) => boolean>>,
  Holds<Rejects<EndGiven, (state: State) => string>>,
  Holds<Rejects<ViewGiven, { colors: { broken: 1 } }>>,
  Holds<Rejects<ViewGiven, { state: (state: State) => number }>>,

  Holds<Accepts<Parameters<typeof InitialOfLetters>[0], "a">>,
  Holds<Rejects<Parameters<typeof InitialOfLetters>[0], "c">>,
  Holds<Rejects<Parameters<typeof InitialOfLetters>[0], null>>,
  Holds<Accepts<Parameters<typeof InitialOfQueue>[0], SeqValue<"a" | "b">>>,
  Holds<Rejects<Parameters<typeof InitialOfQueue>[0], SetValue<"a" | "b">>>,

  Holds<Rejects<Parameters<SetValue<"a" | "b">["has"]>[0], "c">>,
  Holds<Rejects<Parameters<SeqValue<"a" | "b">["push"]>[0], 1>>,
  Holds<Rejects<Parameters<MapValue<"a" | "b", number>["set"]>[1], "one">>,
  Holds<Rejects<Parameters<OwnedLockValue<"ann" | "bob">["acquire"]>[0], "cat">>,
  Holds<Rejects<SetValue<"a" | "b">, SeqValue<"a" | "b">>>,
  Holds<Rejects<SetValue<"a" | "b">, SetValue<"c">>>,
  Holds<Rejects<LockValue, TimerValue>>,
  Holds<Same<ReturnType<MapValue<"a", number>["get"]>, number | null>>,
  Holds<Same<SeqValue<"a">["first"], "a" | null>>,
  Holds<Same<QueueValue<"a">["head"], "a" | null>>,
  Holds<Same<OwnedLockValue<"ann">["holder"], "ann" | null>>,

  Holds<Same<typeof cartPress, Action<typeof Cart.fields, { by: typeof Press }>>>,
  Holds<Same<typeof changesFirst, typeof cartPress>>,
  Holds<Same<typeof whenFirst, typeof cartPress>>,
  Holds<Same<typeof inputLast, typeof cartPress>>,
  Holds<Same<typeof noInput, Action<typeof Cart.fields>>>,
  Holds<Same<typeof bothMembers, Action<typeof Cart.fields>>>,
  Holds<Same<typeof pressed, Rule<typeof Cart.fields>>>,
  Holds<Same<typeof inCart, End<typeof Cart.fields>>>,
  Holds<Same<typeof cartView, View<typeof Cart.fields>>>,
  Holds<Same<typeof emptyView, View<typeof Cart.fields>>>,
  Holds<Same<Parameters<typeof cartPress.changes>[0], m.Infer<typeof Cart>>>,
  Holds<Same<Parameters<typeof cartPress.changes>[1], PressInput>>,
  Holds<Same<Parameters<typeof changesFirst.changes>[1], PressInput>>,
  Holds<Same<Parameters<typeof whenFirst.when>[1], PressInput>>,
  Holds<Same<Parameters<typeof inputLast.when>[1], PressInput>>,
  Holds<Same<Parameters<typeof noInput.changes>[1], Readonly<Record<never, never>>>>,

  Holds<Same<ThenKeys<typeof cartPress>, never>>,
  Holds<Same<ThenKeys<typeof pressed>, never>>,
  Holds<Same<ThenKeys<typeof inCart>, never>>,
  Holds<Same<ThenKeys<typeof cartView>, never>>,
  Holds<Same<ThenKeys<ReturnType<typeof Cart.runners>["actions"][number]>, never>>,
  Holds<Same<PartMakers<Model<typeof Cart.fields>>, never>>,
];
