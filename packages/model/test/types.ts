import {
  type LockValue,
  type MapValue,
  type OwnedLockValue,
  type QueueValue,
  type RequestValue,
  type RetryValue,
  type SeqValue,
  type SetValue,
  type TimerValue,
  m,
} from "@earnest/model";
import type { AddToCartFocus, State, press } from "./models/cart.model.ts";

type Same<Left, Right> =
  (<T>() => T extends Left ? 1 : 2) extends <T>() => T extends Right ? 1 : 2 ? true : false;

type Accepts<Target, Value> = [Value] extends [Target] ? true : false;

type Rejects<Target, Value> = [Value] extends [Target] ? false : true;

type Holds<Claim extends true> = Claim;

type PressedBy = "keyboard" | "virtual" | "mouse";

type Changes = ReturnType<typeof press.changes>;

type PressInput = Parameters<typeof press.when>[1];

type RuleGiven = Parameters<typeof AddToCartFocus.rule>[0];

type EndGiven = Parameters<typeof AddToCartFocus.end>[0];

type ViewGiven = NonNullable<Parameters<typeof AddToCartFocus.view>[0]>;

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

  Holds<Accepts<RuleGiven, { says: string; holds: (state: State) => boolean }>>,
  Holds<Rejects<RuleGiven, { says: string; holds: (state: State) => number }>>,
  Holds<Rejects<RuleGiven, { holds: (state: State) => boolean }>>,
  Holds<Rejects<RuleGiven, { says: string; holds: (state: { missing: 1 }) => boolean }>>,
  Holds<Rejects<EndGiven, { says: string; when: (state: State) => string }>>,
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
];
