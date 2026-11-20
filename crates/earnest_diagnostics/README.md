# earnest_diagnostics

A place in a model file and a problem at that place.

A reader or the lowering reports a `Problem` when a model file uses something the checker can't
give a meaning to. The CLI prints each one as `file:line:column`, the feature in backticks and a
sentence, and no model in that file is checked.
