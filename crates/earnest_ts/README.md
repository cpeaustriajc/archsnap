# earnest_ts

The TypeScript reader: model file to tree.

`read(file, source)` parses one TypeScript model file and gives a `Reading` from `earnest_ast`: a
`Draft` for each `m.model(...)` in the file, in file order, with the actions, rules, ends and view
declared on it, and a `Problem` for each construct the root `README.md` does not allow in a model
file. It checks what the file is written with, not what it means: a name the file does not define
and a value of the wrong kind are found by `earnest_semantic`.

A top-level `const` is exactly one of the forms the root `README.md` lists, or a problem,
whether or not anything reads it; the name of a model is a problem anywhere but in a direct call
at the top level. In an action, `when` and `changes` each bring their own names into scope, and the
tree says which inputs each one takes. A rule's text is stored with each run of white space as
one space.

The name of a called method reaches the tree in `snake_case`. A property read keeps the name the
file writes: whether `value.isFull` is a record's field or a queue's property depends on what
`value` is, which `earnest_semantic` knows and a reader does not.

`reaches_library(file, source)` says whether a file is a model file: it resolves the specifier of
each import as JavaScript does, so an escape or a line continuation in `"@earnest/model"` is the
library, and a file that can't be shown to leave the library alone counts as reaching it. A
number a model writes or bounds a declaration with is refused past 2^53 - 1, the last whole number
a JavaScript number holds, and a `const` of a block that is read before its declaration is refused,
as JavaScript does not give it the constant of the same name outside. A file that nests more than
1000 levels deep is refused before the parser runs.

A rule, an action and an end must be bound to an exported constant, because the id is the name it
is exported under; the reader refuses one bound to a constant that is not exported. It also refuses what
JavaScript refuses or what erases an export: a name exported twice in any spelling, and a
`type` modifier on an exported part; and an export of the library or of any name imported from
it. A type assertion (`as`, `satisfies`, `<Type>value`) in a term is refused except `as const`. A name is
declared once in its scope, as JavaScript requires, and `m.some(value)` is read as the value
inside an optional while a postfix `!` is refused.
