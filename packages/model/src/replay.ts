import type { Declaration } from "./declarations.ts";
import type { ActionRunner, AnyModel } from "./model.ts";
import { type Exports, type Named, attempt, namer, nextState } from "./search.ts";
import { type Json, type Row, keyOf, toJson } from "./values.ts";

/**
 * One step of a path to replay. `action` is the action's id and `label` its sentence; the
 * action is found by id when the model file's exports are passed, and by sentence otherwise.
 * `state` is the whole state the step must lead to.
 */
export interface ReplayStep {
  readonly action: string;
  readonly label?: string;
  readonly input?: readonly Named[];
  readonly state: readonly Named[];
}

/** A path as `earnest verify --json` and `explore` give it; `changes` is not needed. */
export interface ReplayPath {
  readonly start: readonly Named[];
  readonly steps: readonly ReplayStep[];
}

/** `exports` is the model file's exports, which let a step's `action` id be matched. */
export interface ReplayOptions {
  readonly exports?: Exports;
}

/** A state with each field in JSON form. */
export type StateJson = { readonly [field: string]: Json };

type Declared = readonly (readonly [string, Declaration<unknown, never>])[];

const messageOf = (thrown: unknown): string =>
  thrown instanceof Error ? thrown.message : String(thrown);

const readAll = (where: string, declared: Declared, named: readonly Named[]): Row => {
  const given = new Map<string, Json>(named.map((each) => [each.name, each.value]));
  const stray = named.find((each) => !declared.some(([name]) => name === each.name));
  if (stray !== undefined) {
    throw new Error(`${where} names \`${stray.name}\`, which is not declared`);
  }
  return Object.fromEntries(
    declared.map(([name, declaration]) => {
      const json = given.get(name);
      if (json === undefined) {
        throw new Error(`${where} has no value for \`${name}\``);
      }
      try {
        return [name, declaration.fromJson(json)];
      } catch (thrown) {
        throw new Error(`${where} can't be read at \`${name}\`: ${messageOf(thrown)}`);
      }
    }),
  );
};

const outside = (declared: Declared, row: Row): string | null => {
  const found = declared.find(([name, declaration]) => !declaration.has(row[name]));
  return found === undefined ? null : found[0];
};

const stateKey = (fields: Declared, row: Row): string =>
  fields.map(([name]) => keyOf(row[name])).join(";");

const text = (value: unknown): string => JSON.stringify(toJson(value));

const actionFor = (
  where: string,
  actions: readonly ActionRunner[],
  step: ReplayStep,
  options: ReplayOptions,
): ActionRunner => {
  const name = namer(options.exports);
  const byId = actions.filter((each) => name(each.part, each.sentence) === step.action);
  const matching = byId.length > 0 ? byId : actions.filter((each) => each.sentence === step.label);
  const [found, ...others] = matching;
  if (found === undefined) {
    throw new Error(
      `${where}: the model has no action with this id or label; pass the model file's exports so that ids can be matched`,
    );
  }
  if (others.length > 0) {
    throw new Error(
      `${where}: more than one action matches; pass the model file's exports so that ids can be matched`,
    );
  }
  return found;
};

const applied = (where: string, fields: Declared, action: ActionRunner, state: Row, input: Row) => {
  const unfit = outside(fields, state);
  if (unfit !== null) {
    throw new Error(
      `${where}: the state before it is out of range at \`${unfit}\`, so no action can follow`,
    );
  }
  const strange = outside(action.inputs, input);
  if (strange !== null) {
    throw new Error(
      `${where}: the input \`${strange}\` is ${text(input[strange])}, which is not a value it declares`,
    );
  }
  const can = attempt(() => action.when(state, input));
  if (can.fault !== null) {
    throw new Error(`${where}: the \`when\` of the action ${can.fault} in this state`);
  }
  if (!can.value) {
    throw new Error(`${where}: the \`when\` of the action is false in this state`);
  }
  const next = nextState(action, state, input);
  if (next.fault !== null) {
    throw new Error(`${where}: the \`then\` of the action ${next.fault} in this state`);
  }
  return next.value;
};

/**
 * Applies a path one action at a time and gives every state it passes through, the start state
 * first, each with its fields in JSON form. Throws an error that names the step when the path
 * does not begin at a start state, a step names no action of the model, an input is missing or
 * undeclared, the action's `when` is false, the action can't be evaluated, or the state after
 * the action differs from the step's `state`. Set members may be listed in any order.
 */
export const replay = (
  model: AnyModel,
  path: ReplayPath,
  options: ReplayOptions = {},
): StateJson[] => {
  const fields = Object.entries(model.fields);
  const actions = model.runners().actions;
  const given = readAll(`${model.name}: the start of the path`, fields, path.start);
  const start = model.starts().find((each) => stateKey(fields, each) === stateKey(fields, given));
  if (start === undefined) {
    throw new Error(`${model.name}: the path does not begin at a start state of the model`);
  }
  const states: Row[] = [start];
  let state: Row = start;
  for (const [position, step] of path.steps.entries()) {
    const where = `${model.name}: step ${position + 1} (${step.action})`;
    const action = actionFor(where, actions, step, options);
    const input = readAll(`${where}: the input`, action.inputs, step.input ?? []);
    const next = applied(where, fields, action, state, input);
    const expected = readAll(`${where}: the state`, fields, step.state);
    const differing = fields
      .filter(([name]) => keyOf(next[name]) !== keyOf(expected[name]))
      .map(
        ([name]) => `\`${name}\` is ${text(next[name])} but the path has ${text(expected[name])}`,
      );
    if (differing.length > 0) {
      throw new Error(
        `${where}: the state after the action differs from the path: ${differing.join("; ")}`,
      );
    }
    states.push(next);
    state = next;
  }
  return states.map((row) => Object.fromEntries(fields.map(([name]) => [name, toJson(row[name])])));
};
