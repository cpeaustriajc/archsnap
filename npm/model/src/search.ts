import type { Declaration } from "./declarations.ts";
import type { ActionRunner, AnyModel, RuleRunner, Runners } from "./model.ts";
import {
  type Json,
  NullFault,
  DivisionFault,
  RemainderFault,
  type Row,
  isJsonList,
  keyOf,
  toJson,
} from "./values.ts";

/** A field or an input with its value in JSON form. */
export interface Named {
  readonly name: string;
  readonly value: Json;
}

/** A field an action changed, with its value before and after. */
export interface Change {
  readonly field: string;
  readonly before: Json;
  readonly after: Json;
}

/**
 * One action on a path: its id, its sentence as `label`, the input it was tried with, the
 * fields it changed and the whole state after it.
 */
export interface Step {
  readonly action: string;
  readonly label: string;
  readonly input: readonly Named[];
  readonly changes: readonly Change[];
  readonly state: readonly Named[];
}

/** A start state and the actions that lead from it, as `earnest verify --json` prints a path. */
export interface Path {
  readonly start: readonly Named[];
  readonly steps: readonly Step[];
}

/**
 * `unknown` means the rule was not seen broken, but the search stopped early, the rule could
 * not be evaluated in a state it reached, or a state out of range or a fault in an action or an
 * end cut the search short, so some state was not judged or not reached.
 */
export type Verdict = "holds" | "broken" | "unknown";

/** One rule's result. `path` is the shortest path to a state that breaks it, or `null`. */
export interface RuleResult {
  readonly id: string;
  readonly says: string;
  readonly verdict: Verdict;
  readonly path: Path | null;
}

/**
 * `stuck`: the `when` of every action is false and so is every end. `out-of-range`: an action
 * gave a field a value outside its declaration. `fault`: a rule, the `when` or the `then` of an
 * action, or an end can't be evaluated in the state. A state where a `when` or an end can't be
 * evaluated is not stuck: nothing says that it should stop there or go on.
 */
export type FindingKind = "stuck" | "out-of-range" | "fault";

/** A problem that is not a broken rule, with the shortest path to the first state showing it. */
export interface Finding {
  readonly kind: FindingKind;
  readonly message: string;
  readonly path: Path;
}

/**
 * `unfinished` means the state limit stopped the search before anything was found wrong. Every
 * state that is counted has its rules judged, also the ones the limit kept the search from
 * going on from.
 */
export type Status = "pass" | "fail" | "unfinished";

/**
 * What `explore` found. `states` counts distinct states, out-of-range ones included. `findings`
 * holds at most one of each kind, in the order stuck, out-of-range, fault. `reason` says why an
 * unfinished search stopped, and is `null` otherwise.
 */
export interface Exploration {
  readonly status: Status;
  readonly states: number;
  readonly rules: readonly RuleResult[];
  readonly findings: readonly Finding[];
  readonly reason: string | null;
}

/** The exports of a model file, by name: `import * as cart from "./cart.model.ts"`. */
export type Exports = Readonly<Record<string, unknown>>;

/**
 * `maxStates` stops the search before it holds more distinct states than this; the default is
 * 5,000,000. `exports` is the model file's exports, which give each action, rule and end its
 * id: the name it is exported under. One that is not among them has its sentence as its id.
 */
export interface ExploreOptions {
  readonly maxStates?: number;
  readonly exports?: Exports;
}

/** The state limit `explore` uses when it is given none, the same as `earnest verify`. */
export const DEFAULT_MAX_STATES = 5_000_000;

/**
 * Gives the function that names a part of a model: the name it is exported under in `exports`,
 * or `fallback`, its sentence, when it is not exported there.
 */
export const namer = (exports: Exports = {}): ((part: object, fallback: string) => string) => {
  const names = new Map<unknown, string>();
  for (const [name, exported] of Object.entries(exports)) {
    if (!names.has(exported)) {
      names.set(exported, name);
    }
  }
  return (part, fallback) => names.get(part) ?? fallback;
};

const NULL_READ = "reads a value that is null";
const OVERFLOW = "computes a number too large to hold";
const REMAINDER = "takes a remainder with a negative number or a divisor below 1";
const DIVISION = "takes a division with a negative number or a divisor below 1";
const WRONG_KIND = "uses a value of the wrong kind";

/** What `attempt` gives: the result, or the sentence part saying why there is none. */
export type Attempt<T> =
  | { readonly fault: null; readonly value: T }
  | { readonly fault: string; readonly value: null };

/**
 * Runs part of a model and turns a fault into a value: reading a field of `null`, or using a
 * value as something it is not. Any other error is a mistake in the model file and is thrown.
 */
export const attempt = <T>(run: () => T): Attempt<T> => {
  try {
    return { fault: null, value: run() };
  } catch (thrown) {
    if (thrown instanceof RemainderFault) {
      return { fault: REMAINDER, value: null };
    }
    if (thrown instanceof DivisionFault) {
      return { fault: DIVISION, value: null };
    }
    if (thrown instanceof NullFault) {
      return { fault: NULL_READ, value: null };
    }
    if (thrown instanceof TypeError) {
      return { fault: /\bnull\b/.test(thrown.message) ? NULL_READ : WRONG_KIND, value: null };
    }
    throw thrown;
  }
};

const numberFault = (json: Json): string | null => {
  if (typeof json === "number") {
    if (Number.isNaN(json)) {
      return REMAINDER;
    }
    return Math.abs(json) > Number.MAX_SAFE_INTEGER ? OVERFLOW : null;
  }
  if (json === null || typeof json !== "object") {
    return null;
  }
  const inside = isJsonList(json) ? json : Object.values(json);
  return inside.map(numberFault).find((fault) => fault !== null) ?? null;
};

const stateLimit = (maxStates: number): number => {
  if (!Number.isInteger(maxStates) || maxStates < 0) {
    throw new Error(`maxStates must be a whole number that is not negative, not ${maxStates}`);
  }
  return maxStates;
};

/**
 * Applies an action and gives the next state, or the fault that stops it. Besides the faults of
 * `attempt`, a next state that holds `NaN`, which `%` by zero gives, or a number too large for
 * JavaScript to hold exactly is a fault.
 */
export const nextState = (action: ActionRunner, state: Row, input: Row): Attempt<Row> => {
  const tried = attempt(() => action.then(state, input));
  if (tried.fault !== null) {
    return tried;
  }
  const fault = numberFault(toJson(tried.value));
  return fault === null ? tried : { fault, value: null };
};

/** Every input an action is tried with, in the checker's order: the first input varies fastest. */
export const inputsOf = (action: ActionRunner): readonly Row[] =>
  action.inputs.reduceRight<Row[]>(
    (rest, [name, declared]) =>
      rest.flatMap((later) => declared.values().map((value) => ({ [name]: value, ...later }))),
    [{}],
  );

interface Parent {
  readonly from: Visited;
  readonly action: ActionRunner;
  readonly input: Row;
}

interface Visited {
  readonly row: Row;
  readonly key: string;
  readonly parent: Parent | null;
}

interface Successor {
  readonly action: ActionRunner;
  readonly input: Row;
  readonly next: Row;
}

interface Expanded {
  readonly outside: string | null;
  readonly broken: RuleRunner[];
  readonly unjudged: RuleRunner[];
  readonly successors: Successor[];
  trouble: string | null;
  isCutShort: boolean;
  isEnabled: boolean;
  isStuck: boolean;
}

type FieldList = readonly (readonly [string, Declaration<unknown, never>])[];

class Search {
  readonly #fields: FieldList;
  readonly #parts: Runners;
  readonly #inputs: ReadonlyMap<ActionRunner, readonly Row[]>;
  readonly #name: (part: object, fallback: string) => string;
  readonly #maxStates: number;
  readonly #seen = new Set<string>();
  readonly #firstBroken = new Map<RuleRunner, Visited>();
  readonly #unjudged = new Set<RuleRunner>();
  #stuck: Visited | null = null;
  #outside: { readonly at: Visited; readonly field: string } | null = null;
  #trouble: { readonly at: Visited; readonly message: string } | null = null;
  #reason: string | null = null;
  #isCutShort = false;

  constructor(fields: FieldList, parts: Runners, options: ExploreOptions) {
    this.#fields = fields;
    this.#parts = parts;
    this.#inputs = new Map(parts.actions.map((action) => [action, inputsOf(action)]));
    this.#name = namer(options.exports);
    this.#maxStates = stateLimit(options.maxStates ?? DEFAULT_MAX_STATES);
  }

  run(starts: readonly Row[]): Exploration {
    const waiting: Visited[] = [];
    for (const start of starts) {
      this.#visit(start, null, waiting);
    }
    let searched = 0;
    for (const visited of waiting) {
      if (this.#reason !== null) {
        break;
      }
      searched += 1;
      const expanded = this.#expand(visited.row);
      this.#record(visited, expanded);
      for (const successor of expanded.successors) {
        const parent = { from: visited, action: successor.action, input: successor.input };
        if (!this.#visit(successor.next, parent, waiting)) {
          break;
        }
      }
    }
    for (const counted of waiting.slice(searched)) {
      this.#record(counted, this.#judge(counted.row));
    }
    return this.#outcome();
  }

  #keyOf(row: Row): string {
    return this.#fields.map(([name]) => keyOf(row[name])).join(";");
  }

  #visit(row: Row, parent: Parent | null, waiting: Visited[]): boolean {
    const key = this.#keyOf(row);
    if (this.#seen.has(key)) {
      return true;
    }
    if (this.#seen.size >= this.#maxStates) {
      this.#reason = `state limit of ${this.#maxStates}`;
      return false;
    }
    this.#seen.add(key);
    waiting.push({ row, key, parent });
    return true;
  }

  #judge(state: Row): Expanded {
    const outside = this.#fields.find(([name, declared]) => !declared.has(state[name]));
    const found: Expanded = {
      outside: outside === undefined ? null : outside[0],
      broken: [],
      unjudged: [],
      successors: [],
      trouble: null,
      isCutShort: outside !== undefined,
      isEnabled: false,
      isStuck: false,
    };
    if (found.outside !== null) {
      return found;
    }
    for (const rule of this.#parts.rules) {
      const held = attempt(() => rule.holds(state));
      if (held.fault !== null) {
        found.trouble ??= `the rule \`${this.#name(rule.part, rule.says)}\` ${held.fault}`;
        found.unjudged.push(rule);
      } else if (!held.value) {
        found.broken.push(rule);
      }
    }
    return found;
  }

  #expand(state: Row): Expanded {
    const found = this.#judge(state);
    if (found.outside !== null) {
      return found;
    }
    let canTell = true;
    for (const action of this.#parts.actions) {
      canTell = this.#expandAction(action, state, found) && canTell;
    }
    if (found.isEnabled) {
      return found;
    }
    let hasEnded = false;
    for (const end of this.#parts.ends) {
      const ended = attempt(() => end.when(state));
      if (ended.fault === null) {
        hasEnded ||= ended.value;
      } else {
        found.trouble ??= `the end \`${this.#name(end.part, end.says)}\` ${ended.fault}`;
        found.isCutShort = true;
        canTell = false;
      }
    }
    found.isStuck = canTell && !hasEnded;
    return found;
  }

  #expandAction(action: ActionRunner, state: Row, found: Expanded): boolean {
    const id = this.#name(action.part, action.sentence);
    let canTell = true;
    for (const input of this.#inputs.get(action) ?? []) {
      const can = attempt(() => action.when(state, input));
      if (can.fault !== null) {
        found.trouble ??= `the \`when\` of the action \`${id}\` ${can.fault}`;
        found.isCutShort = true;
        canTell = false;
        continue;
      }
      if (!can.value) {
        continue;
      }
      found.isEnabled = true;
      const next = nextState(action, state, input);
      if (next.fault === null) {
        found.successors.push({ action, input, next: next.value });
      } else {
        found.trouble ??= `the \`then\` of the action \`${id}\` ${next.fault}`;
        found.isCutShort = true;
      }
    }
    return canTell;
  }

  #record(at: Visited, expanded: Expanded): void {
    if (expanded.outside !== null) {
      this.#outside ??= { at, field: expanded.outside };
    }
    for (const rule of expanded.broken) {
      if (!this.#firstBroken.has(rule)) {
        this.#firstBroken.set(rule, at);
      }
    }
    for (const rule of expanded.unjudged) {
      this.#unjudged.add(rule);
    }
    if (expanded.trouble !== null) {
      this.#trouble ??= { at, message: `${expanded.trouble} in this state` };
    }
    this.#isCutShort ||= expanded.isCutShort;
    if (expanded.isStuck) {
      this.#stuck ??= at;
    }
  }

  #named(row: Row): Named[] {
    return this.#fields.map(([name]) => ({ name, value: toJson(row[name]) }));
  }

  #step(parent: Parent, after: Row): Step {
    const before = parent.from.row;
    return {
      action: this.#name(parent.action.part, parent.action.sentence),
      label: parent.action.sentence,
      input: parent.action.inputs.map(([name]) => ({ name, value: toJson(parent.input[name]) })),
      changes: this.#fields
        .filter(([name]) => keyOf(before[name]) !== keyOf(after[name]))
        .map(([name]) => ({
          field: name,
          before: toJson(before[name]),
          after: toJson(after[name]),
        })),
      state: this.#named(after),
    };
  }

  #pathTo(target: Visited): Path {
    const steps: Step[] = [];
    let at = target;
    while (at.parent !== null) {
      steps.push(this.#step(at.parent, at.row));
      at = at.parent.from;
    }
    return { start: this.#named(at.row), steps: steps.reverse() };
  }

  #findings(): Finding[] {
    const findings: Finding[] = [];
    if (this.#stuck !== null) {
      findings.push({
        kind: "stuck",
        message: "no action can happen here, and the model does not mark it as an end",
        path: this.#pathTo(this.#stuck),
      });
    }
    if (this.#outside !== null) {
      findings.push({
        kind: "out-of-range",
        message: `the field \`${this.#outside.field}\` leaves the values it declares`,
        path: this.#pathTo(this.#outside.at),
      });
    }
    if (this.#trouble !== null) {
      findings.push({
        kind: "fault",
        message: this.#trouble.message,
        path: this.#pathTo(this.#trouble.at),
      });
    }
    return findings;
  }

  #outcome(): Exploration {
    const rules = this.#parts.rules.map((rule): RuleResult => {
      const first = this.#firstBroken.get(rule);
      const isJudged = this.#reason === null && !this.#isCutShort && !this.#unjudged.has(rule);
      const unbroken = isJudged ? "holds" : "unknown";
      return {
        id: this.#name(rule.part, rule.says),
        says: rule.says,
        verdict: first === undefined ? unbroken : "broken",
        path: first === undefined ? null : this.#pathTo(first),
      };
    });
    const findings = this.#findings();
    const hasFailed = findings.length > 0 || rules.some((rule) => rule.verdict === "broken");
    const unfailed = this.#reason === null ? "pass" : "unfinished";
    return {
      status: hasFailed ? "fail" : unfailed,
      states: this.#seen.size,
      rules,
      findings,
      reason: this.#reason,
    };
  }
}

/**
 * Searches every state a model can reach, breadth first, and gives the count of distinct
 * states, each rule's verdict with the shortest path to a state that breaks it, and the first
 * stuck, out-of-range and fault findings. An end is asked only in a state where no action can
 * happen. It visits states in the same order as
 * `earnest verify`, so the two results can be compared. Pass the model file's exports as
 * `exports` to get the same ids.
 */
export const explore = (model: AnyModel, options: ExploreOptions = {}): Exploration =>
  new Search(Object.entries(model.fields), model.runners(), options).run(model.starts());
