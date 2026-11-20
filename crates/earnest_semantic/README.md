# earnest_semantic

The typed lowering: tree to checked model, where each primitive gets its meaning.

`lower` takes a `Draft` from a reader and gives back a `Model` for the search. On the way it

- turns every declaration into a finite domain and a starting value,
- resolves every name to a state field, an input, a binding, a constant or a helper function,
  where an input is in scope only in the function of an action that takes it,
- gives each primitive of the README's table its meaning in the expression language, reading a
  primitive's property by its `snake_case` or its camelCase name once the value is known not to
  be a record, and
- checks that every value has the kind its place needs, wrapping or unwrapping optional values
  where a plain one meets an optional one, also field by field where two records meet or a
  record meets its declaration, and
- in a draft read from Rust, gives every number its integer type and asks each sum, difference,
  product and negation to stay inside it, with `i32` for a number nothing types and `usize` for
  a size or a position.

Anything it can't give a meaning to becomes a `Problem` at the place of the term: a name the file
does not define, also in a constant nothing reads, a primitive that does not exist, a value of
the wrong kind, two optional values compared by order, a helper function that calls itself, a
starting value outside its field, a value listed twice in a list of choices, a lock with an
owner held in a map, a sequence or a queue, or a declaration with more than a million values to
try. In a draft read from Rust it also refuses what the checker would read differently from
Rust: an optional value held in a map, a sequence or a queue, or read out of a written
collection of optional values; numbers of two integer types that meet; a `let` that binds a
number nothing gives a type; and a value in a view's label that is not a boolean, a number, an
enum variant or a text, because `Debug` prints an optional value, a record or a collection
differently from the chart. A list of choices read from `#[one_of(1, 5)]` is a list of `Declared`
numbers, and the choices keep the integer type. A draft read from TypeScript has JavaScript's numbers: each sum,
difference, product and remainder is wrapped in `Op::Safe`, so a result past 2^53 - 1 is the
fault "computes a number too large to hold". An expression nested more than 1000 levels deep is
refused, counting the helpers and constants it reads, as is a map literal whose entries cannot
join into one type.

`lower_with_earlier` also lowers the rules of an older version of the model, each with the
constants and helper functions of its own file, so `earnest verify --base` can evaluate an old
rule and a new one in the same state.
