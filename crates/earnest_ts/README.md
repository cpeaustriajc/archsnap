# earnest_ts

The TypeScript reader: model file to tree.

`read(file, source)` parses one TypeScript model file and gives a `Reading` from `earnest_ast`: a
`Draft` for each `m.model(...)` in the file, in file order, with the actions, rules, ends and view
declared on it, and a `Problem` for each construct the root `README.md` does not allow in a model
file. It checks what the file is written with, not what it means: a name the file does not define
and a value of the wrong kind are found by `earnest_semantic`.

A top-level `const` is exactly one of the forms the root `README.md` lists, or a problem,
whether or not anything reads it; the name of a model is a problem anywhere but in a direct call
at the top level. In an action, `when` and `then` each bring their own names into scope, and the
tree says which inputs each one takes. A rule's text is stored with each run of white space as
one space.

The name of a called method reaches the tree in `snake_case`. A property read keeps the name the
file writes: whether `value.isFull` is a record's field or a queue's property depends on what
`value` is, which `earnest_semantic` knows and a reader does not.
