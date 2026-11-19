# Rules for agents

- Never write a lint-level attribute: no `allow`, `expect`, `warn`, `deny` or `forbid`, alone or
  inside `cfg_attr`. When a lint fires, change the code.
- Run `just check` before you call work done. It must pass.
