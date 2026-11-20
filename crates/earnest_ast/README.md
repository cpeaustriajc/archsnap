# earnest_ast

The language-neutral tree both readers produce.

The TypeScript reader and the Rust reader each turn a model file into a `Reading`: one `Draft` per
model and one `Problem` per thing the reader refused. A draft is untyped. Its expressions are
`Term`s and its declarations are `Shape`s, each with the `Place` it starts at. The name of a called
method is `snake_case` whichever language it came from; a field or property read keeps the name
the file writes. `earnest_semantic` gives the tree its meaning.

A draft also says which `Language` it was read from, because two things mean something else in
each: in Rust a number has an integer type that its results must stay inside, and `Some(None)`
is another value than `None`. A number's `Shape` carries its integer type, a `Declared` term is
a value with its type written out, and a helper is a `Function` of the file or a `Method` of
the model, which may share a name.

`nesting_problem` and `MOST_NESTING` are the one limit on how deep a model file or an expression
may nest, which both readers and the lowering apply, and `is_color` is what a view's color may be.
