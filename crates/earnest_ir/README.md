# earnest_ir

The typed expression language, its evaluator and the checked model.

- `Model` is a model after lowering: fields with finite domains and start values, and actions,
  rules and ends whose conditions are `Expr`s.
- `Expr` reads state fields by position and local values by slot, so evaluating one needs no
  lookup by name.
- `eval` gives an expression's value in one state, and `truth` its value as a condition. Both
  give a `Fault` when there is none: a read of `null`, a number too large to hold, a remainder of
  a negative number, or a number outside the integer type that `Op::Fit` asks it to be in, or past the safe integers
  that `Op::Safe` asks it to be within, which a TypeScript model's arithmetic is wrapped in.

The evaluator lives here, beside the language it gives meaning to, because two later crates need
it: `earnest_semantic` computes starting values with it, and `earnest_check` runs every rule and
action with it.
