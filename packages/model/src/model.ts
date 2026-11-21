import { type Declaration, type Fields, type StateOf, record } from "./declarations.ts";
import { type Row, frozen, isRecord } from "./values.ts";

/** The inputs of an action that takes none. */
export type NoInput = Readonly<Record<never, never>>;

/**
 * The fields an action's `changes` gives back, with every name that is not a field of the model
 * typed as `never`, so that a misspelt field is a compile error.
 */
export type OnlyFields<F extends Fields, Changes> = Changes & {
  readonly [Stray in Exclude<keyof Changes, keyof F>]: never;
};

/**
 * What `m.action` is given, with its keys in any order. `input` declares what the action is
 * given when it happens, and types the second parameter of `when` and `changes`; leave it out
 * for an action that takes none. `when` says in which states, and for which inputs, the action
 * can happen; leave it out for an action that always can. `changes` gives back only the fields
 * that change, and must not change the state it is given.
 */
export interface ActionSpec<
  F extends Fields,
  I extends Fields,
  Changes extends Partial<StateOf<F>> = Partial<StateOf<F>>,
> {
  readonly input?: I;
  readonly when?: (state: StateOf<F>, input: StateOf<I>) => boolean;
  readonly changes: (state: StateOf<F>, input: StateOf<I>) => OnlyFields<F, Changes>;
}

/**
 * What `Model.action` is given: an `ActionSpec` whose `changes` is called `then`. Write `input`
 * before `when` and `then`, so that their second parameter is typed from it. Kept until models
 * are written with `m.action`.
 */
export interface ThenActionSpec<
  F extends Fields,
  I extends Fields,
  Changes extends Partial<StateOf<F>> = Partial<StateOf<F>>,
> {
  readonly input?: I;
  readonly when?: (state: StateOf<F>, input: StateOf<I>) => boolean;
  readonly then: (state: StateOf<F>, input: StateOf<I>) => OnlyFields<F, Changes>;
}

/**
 * One thing that can happen in a model. Export it from the model file: the name of the export is
 * its id. `when` is always present here, and always true for an action declared without one.
 */
export interface Action<F extends Fields, I extends Fields = NoInput> {
  readonly kind: "action";
  readonly sentence: string;
  readonly input: I | undefined;
  readonly when: (state: StateOf<F>, input: StateOf<I>) => boolean;
  readonly changes: (state: StateOf<F>, input: StateOf<I>) => Partial<StateOf<F>>;
}

/** What `Model.rule` is given: the reviewer's sentence and the condition every state must meet. */
export interface RuleSpec<F extends Fields> {
  readonly says: string;
  readonly holds: (state: StateOf<F>) => boolean;
}

/** A rule every reachable state must meet. The name of its export is its id. */
export interface Rule<F extends Fields> extends RuleSpec<F> {
  readonly kind: "rule";
}

/** What `Model.end` is given: a sentence and the states where stopping is correct. */
export interface EndSpec<F extends Fields> {
  readonly says: string;
  readonly when: (state: StateOf<F>) => boolean;
}

/**
 * States where stopping is correct. A state with no possible action that no end covers is
 * stuck. The name of its export is its id.
 */
export interface End<F extends Fields> extends EndSpec<F> {
  readonly kind: "end";
}

/**
 * What `Model.view` is given, all of it optional: the chart's title, the label of a state, the
 * group a state is drawn in, arrow labels by action id (the default is the action's sentence)
 * and colors.
 */
export interface ViewSpec<F extends Fields> {
  readonly title?: string;
  readonly state?: (state: StateOf<F>) => string;
  readonly group?: (state: StateOf<F>) => string;
  readonly step?: Readonly<Record<string, string>>;
  readonly colors?: { readonly broken?: string; readonly start?: string; readonly end?: string };
}

/** How `earnest chart` draws a model. It changes nothing about what is checked. */
export interface View<F extends Fields> extends ViewSpec<F> {
  readonly kind: "view";
}

/**
 * An action as a search runs it: on states and inputs whose types are not known statically.
 * `part` is the `Action` the model file exports. `when` and `changes` throw when the state or
 * the input is outside what was declared; `changes` gives back the whole next state.
 */
export interface ActionRunner {
  readonly part: object;
  readonly sentence: string;
  readonly inputs: readonly (readonly [string, Declaration<unknown, never>])[];
  when(state: Row, input: Row): boolean;
  changes(state: Row, input: Row): Row;
}

/** A rule as a search runs it. `holds` throws when the state is outside what was declared. */
export interface RuleRunner {
  readonly part: object;
  readonly says: string;
  holds(state: Row): boolean;
}

/** An end as a search runs it. `when` throws when the state is outside what was declared. */
export interface EndRunner {
  readonly part: object;
  readonly says: string;
  when(state: Row): boolean;
}

/** Everything declared on a model so far, each list in declaration order. */
export interface Runners {
  readonly actions: readonly ActionRunner[];
  readonly rules: readonly RuleRunner[];
  readonly ends: readonly EndRunner[];
}

/**
 * What `explore` and `replay` need of a model, whatever its fields are. Every `Model` is one.
 */
export interface AnyModel {
  readonly name: string;
  readonly fields: Fields;
  starts(): readonly Row[];
  runners(): Runners;
}

/**
 * A model: named fields, and the actions, rules, ends and views declared on it. `action`,
 * `rule`, `end` and `view` each give back a plain value to export, and also add it to this
 * model, in the order they are called, which is the order a search tries actions in.
 */
export class Model<F extends Fields> {
  readonly name: string;
  readonly fields: F;
  readonly #state: Declaration<StateOf<F>>;
  readonly #actions: ActionRunner[] = [];
  readonly #rules: RuleRunner[] = [];
  readonly #ends: EndRunner[] = [];
  readonly #views: View<F>[] = [];

  /** Model files call `m.model(name, fields)` instead. */
  constructor(name: string, fields: F) {
    this.name = name;
    this.fields = fields;
    this.#state = record(fields);
  }

  /** Whether `state` has exactly the model's fields, each at a value its declaration has. */
  has(state: unknown): state is StateOf<F> {
    return this.#state.has(state);
  }

  /**
   * Every start state, in the order the checker visits them: fields in declaration order, the
   * first field varying slowest.
   */
  starts(): readonly StateOf<F>[] {
    const isState = (row: unknown): row is StateOf<F> => this.has(row);
    return Object.entries(this.fields)
      .reduce<Row[]>(
        (partial, [name, declared]) =>
          partial.flatMap((prefix) =>
            declared.starts().map((value) => ({ ...prefix, [name]: value })),
          ),
        [{}],
      )
      .map(frozen)
      .filter(isState);
  }

  /**
   * Declares something that can happen, written with `then` where `m.action` writes `changes`.
   * `sentence` says it in the reviewer's words.
   */
  action<
    I extends Fields = NoInput,
    const Changes extends Partial<StateOf<F>> = Partial<StateOf<F>>,
  >(sentence: string, spec: ThenActionSpec<F, I, Changes>): Action<F, I> {
    const { then: changes, ...rest } = spec;
    return this.declareAction(sentence, { ...rest, changes });
  }

  /** Declares a rule: `holds` must be true in every state the model can reach. */
  rule(spec: RuleSpec<F>): Rule<F> {
    return this.declareRule(spec.says, spec.holds);
  }

  /** Declares where stopping is correct, so that those states are not reported as stuck. */
  end(spec: EndSpec<F>): End<F> {
    return this.declareEnd(spec.says, spec.when);
  }

  /** Declares how the model is drawn. A model needs no view. */
  view(spec: ViewSpec<F> = {}): View<F> {
    return this.declareView(spec);
  }

  /**
   * Adds an action to the model and gives it back, as `m.action` does; call that in a model
   * file. Actions are tried in the order they are added.
   */
  declareAction<
    I extends Fields = NoInput,
    const Changes extends Partial<StateOf<F>> = Partial<StateOf<F>>,
  >(sentence: string, spec: ActionSpec<F, I, Changes>): Action<F, I> {
    const declared = spec.input === undefined ? null : record(spec.input);
    const isInput = (input: unknown): input is StateOf<I> =>
      declared === null ? isRecord(input) && Object.keys(input).length === 0 : declared.has(input);
    const known = (input: Row): StateOf<I> => {
      if (!isInput(input)) {
        throw new Error(`${this.name}: "${sentence}" was given an input it does not declare`);
      }
      return input;
    };
    const action: Action<F, I> = Object.freeze({
      kind: "action",
      sentence,
      input: spec.input,
      when: spec.when ?? (() => true),
      changes: spec.changes,
    });
    this.#actions.push({
      part: action,
      sentence,
      inputs: spec.input === undefined ? [] : Object.entries(spec.input),
      when: (state, input) => action.when(this.#known(state), known(input)),
      changes: (state, input) =>
        this.#changed(state, action.changes(this.#known(state), known(input)), sentence),
    });
    return action;
  }

  /** Adds a rule to the model and gives it back, as `m.rule` does; call that in a model file. */
  declareRule(says: string, holds: (state: StateOf<F>) => boolean): Rule<F> {
    const rule: Rule<F> = Object.freeze({ kind: "rule", says, holds });
    this.#rules.push({
      part: rule,
      says,
      holds: (state) => rule.holds(this.#known(state)),
    });
    return rule;
  }

  /** Adds an end to the model and gives it back, as `m.end` does; call that in a model file. */
  declareEnd(says: string, when: (state: StateOf<F>) => boolean): End<F> {
    const end: End<F> = Object.freeze({ kind: "end", says, when });
    this.#ends.push({
      part: end,
      says,
      when: (state) => end.when(this.#known(state)),
    });
    return end;
  }

  /** Adds a view to the model and gives it back, as `m.view` does; call that in a model file. */
  declareView(spec: ViewSpec<F> = {}): View<F> {
    const view: View<F> = Object.freeze({ ...spec, kind: "view" });
    this.#views.push(view);
    return view;
  }

  /** The views declared so far, in declaration order. */
  get views(): readonly View<F>[] {
    return [...this.#views];
  }

  /**
   * The actions, rules and ends declared so far, in a form a search can run without knowing
   * the model's types. `explore` and `replay` are built on this.
   */
  runners(): Runners {
    return { actions: [...this.#actions], rules: [...this.#rules], ends: [...this.#ends] };
  }

  #known(state: Row): StateOf<F> {
    if (!this.has(state)) {
      throw new Error(`${this.name}: the state is outside what the model declares`);
    }
    return state;
  }

  #changed(state: Row, changes: unknown, sentence: string): Row {
    if (!isRecord(changes)) {
      throw new Error(
        `${this.name}: the then of "${sentence}" must give back an object holding the fields that change`,
      );
    }
    const stray = Object.keys(changes).find((name) => !Object.hasOwn(this.fields, name));
    if (stray !== undefined) {
      throw new Error(
        `${this.name}: the then of "${sentence}" gives \`${stray}\`, which is not a field of the model`,
      );
    }
    return frozen({ ...state, ...changes });
  }
}

/**
 * Declares a model: a name and its fields, each built with an `m` builder. The state type is
 * inferred from the fields; `m.Infer<typeof Model>` names it.
 */
export const model = <F extends Fields>(name: string, fields: F): Model<F> =>
  new Model(name, fields);

/**
 * Declares something that can happen in `model`. `sentence` says it in the reviewer's words.
 * Export the result from the model file: the name of the export is the action's id. Actions are
 * tried in the order they are declared. `input`, `when` and `changes` may be written in any
 * order; `changes` is the only one that is required.
 */
export const action = <
  F extends Fields,
  I extends Fields = NoInput,
  const Changes extends Partial<StateOf<F>> = Partial<StateOf<F>>,
>(
  model: Model<F>,
  sentence: string,
  spec: ActionSpec<F, I, Changes>,
): Action<F, I> => model.declareAction(sentence, spec);

/**
 * Declares a rule of `model`: `holds` must be true in every state the model can reach.
 * `sentence` says it in the reviewer's words. Export the result from the model file: the name
 * of the export is the rule's id.
 */
export const rule = <F extends Fields>(
  model: Model<F>,
  sentence: string,
  holds: (state: StateOf<F>) => boolean,
): Rule<F> => model.declareRule(sentence, holds);

/**
 * Declares where stopping is correct in `model`, so that those states are not reported as
 * stuck. `sentence` says it in the reviewer's words. Export the result from the model file: the
 * name of the export is the end's id.
 */
export const end = <F extends Fields>(
  model: Model<F>,
  sentence: string,
  when: (state: StateOf<F>) => boolean,
): End<F> => model.declareEnd(sentence, when);

/**
 * Declares how `earnest chart` draws `model`. A model needs no view. Export the result from the
 * model file: the name of the export is the view's id.
 */
export const view = <F extends Fields>(model: Model<F>, spec: ViewSpec<F> = {}): View<F> =>
  model.declareView(spec);
