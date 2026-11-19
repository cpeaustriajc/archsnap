# earnest_semantic

The typed lowering: tree to checked model, where each primitive gets its meaning.

`lower` takes a `Draft` from a reader and gives back a `Model` for the search. On the way it

- turns every declaration into a finite domain and a starting value,
- resolves every name to a state field, an input, a binding, a constant or a helper function,
- gives each primitive of the README's table its meaning in the expression language, and
- checks that every value has the kind its place needs, wrapping or unwrapping optional values
  where a plain one meets an optional one.

Anything it can't give a meaning to becomes a `Problem` at the place of the term: a name the file
does not define, a primitive that does not exist, a value of the wrong kind, a helper function
that calls itself, a starting value outside its field, or a declaration with more than a million
values to try.
