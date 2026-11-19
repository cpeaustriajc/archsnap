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
  where a plain one meets an optional one.

Anything it can't give a meaning to becomes a `Problem` at the place of the term: a name the file
does not define, also in a constant nothing reads, a primitive that does not exist, a value of
the wrong kind, two optional values compared by order, a helper function that calls itself, a
starting value outside its field, or a declaration with more than a million values to try.

`lower_with_earlier` also lowers the rules of an older version of the model, each with the
constants and helper functions of its own file, so `earnest verify --base` can evaluate an old
rule and a new one in the same state.
