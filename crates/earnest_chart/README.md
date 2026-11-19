# earnest_chart

The Mermaid chart and Markdown report writer.

`chart` draws the `Graph` of a search as one fenced `mermaid` block holding a `stateDiagram-v2`
chart, and `report` writes the Markdown for a PR body: one section per model with its result line,
its chart and the path to each broken rule and each finding, then the rule changes when the caller
passes them. `path_lines`, `detail_lines` and `rule_change_lines` give the paths and the rule
changes as the plain lines the text output documents, so the CLI and the report print them the
same way.

The crate reads no file and calls no git. The caller passes each model's name, file and `Outcome`,
and the rule changes it found.

## What a chart holds

- A state is `s<n>`, its position in the graph's nodes, with its label attached in quotes. A group
  of the view is a composite state `g<n>`.
- An arrow inside one group is drawn inside it, and so are the `[*]` arrows of its start and end
  states. Every other arrow is drawn at the top level.
- An arrow carries the label of each of its actions, joined with ` / `.
- A state that breaks a rule or is out of range has the class `broken`. The view's `broken` color
  replaces the default; its `start` and `end` colors add the classes `starting` and `ending`. A
  state has one class: `broken` before `ending` before `starting`. A color is used only when it is
  `#` with 3, 4, 6 or 8 hex digits or a plain color name. The text on a hex color is black or
  white, whichever reads better.
- The view's title is the chart's title, in Mermaid front matter.

## Text a developer wrote

In a chart, every ASCII character other than a letter, a digit, a space and `,.'()=+/!?-_` is
written as a numbered entity such as `#34;`, which Mermaid turns back into the character when it
draws the label. A line break or a tab becomes a space. So a label can't close its quotes, start
an arrow, open a directive or end the block.

In a report, the model's name is escaped as Markdown, the file is a code span, and everything else
a developer wrote sits inside a fenced block on a line that starts with a word of the writer. A
line break inside such text becomes a space.

## The 20-arrow rule

A chart draws at most `ARROW_LIMIT` arrows, not counting the `[*]` arrows.

1. Each state gets the arrow that first reaches it in a breadth-first walk from the start states,
   taking states and arrows in the graph's order. Following those arrows back from a state gives a
   shortest route to it.
2. For each broken state, in the graph's order, the arrows of its route are kept when all of them
   still fit. Of a route that does not fit, the arrows nearest the start are kept as far as the
   limit allows, and the broken state itself is not drawn.
3. The rest is filled one arrow at a time: the first arrow in the graph's order that is not kept
   and leaves a state the chart already reaches from a start.
4. A state is drawn when a kept arrow touches it, when it is a broken state whose route was kept,
   or when it is a start state with no arrow leaving it.

When a state or an arrow is left out, the block is followed by one line such as
`Not shown: 15 of 36 states and 40 of 60 arrows.` When a broken state is among them because its
route did not fit, the line says so: `Not shown: 5 of 26 states and 5 of 25 arrows. A broken
state lies beyond what is shown.` The same graph always gives the same chart.
