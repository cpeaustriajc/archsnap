# Rules for agents

- Never silence a lint. Change the code.
- Never write a comment, except the doc comment a lint requires on a public item. Never write a
  doc attribute in its place. Comments go stale: `// keep 3 weeks` above code that keeps 5
  misleads the next agent. Use names: `const WEEKS_KEPT: usize = 5;`
- Run `just check` before you finish.
