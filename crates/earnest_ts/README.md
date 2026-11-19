# earnest_ts

The TypeScript reader: model file to tree.

`read(file, source)` parses one TypeScript model file and gives a `Reading` from `earnest_ast`: a
`Draft` for each `m.model(...)` in the file, in file order, with the actions, rules, ends and view
declared on it, and a `Problem` for each construct the root `README.md` does not allow in a model
file. It checks what the file is written with, not what it means: a name the file does not define
and a value of the wrong kind are found by `earnest_semantic`.

Every method name reaches the tree in `snake_case`. A property keeps its name when the file
declares a field or an input with that name; otherwise a primitive's property such as `isEmpty`
becomes `is_empty`.
