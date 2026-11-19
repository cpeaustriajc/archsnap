# earnest_value

Values, interned names and finite domains.

- `Value` is what one field of a model holds in one state: `null`, a boolean, a whole number, a
  text value, a record, a set, a map or a sequence. Values compare, hash and sort, so a state can
  be a key and a set prints in the same order on every run.
- `Sym` is an interned name. `Symbols` hands them out, gives the text back, and prints a value as
  the text report and the JSON output show it.
- `Domain` is the finite set of values a field or an input declares. It can count its values
  without listing them, list them, and say whether a value belongs.
