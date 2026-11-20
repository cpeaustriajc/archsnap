import type { Declaration, Fields, StateOf } from "./declarations.ts";
import type { Model } from "./model.ts";

export {
  booleanOf as boolean,
  enumOf as enum,
  exists,
  forall,
  int,
  lock,
  map,
  oneOf,
  queue,
  record,
  request,
  retry,
  seq,
  set,
  timer,
} from "./declarations.ts";
export { model } from "./model.ts";
export { div, mapOf, rem, seqOf, setOf, some } from "./values.ts";

/**
 * The type a model's state has, or the type of a declaration's values:
 * `type State = m.Infer<typeof Cart>` and `type Press = m.Infer<typeof Press>`.
 */
export type Infer<Declared> =
  Declared extends Model<infer F extends Fields>
    ? StateOf<F>
    : Declared extends Declaration<infer T, infer _In>
      ? T
      : never;
