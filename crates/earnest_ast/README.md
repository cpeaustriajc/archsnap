# earnest_ast

The language-neutral tree both readers produce.

The TypeScript reader and the Rust reader each turn a model file into a `Reading`: one `Draft` per
model and one `Problem` per thing the reader refused. A draft is untyped. Its expressions are
`Term`s and its declarations are `Shape`s, each with the `Place` it starts at, and every method or
property name is `snake_case` whichever language it came from. `earnest_semantic` gives the tree
its meaning.
