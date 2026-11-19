# Rules for agents

- Never write a lint-level attribute: no `allow`, `expect`, `warn`, `deny` or `forbid`, alone or
  inside `cfg_attr`. When a lint fires, change the code.
- Never write a comment, doc comments included. Put the meaning in names and types.
- Run `just check` before you call work done. It must pass.

## Why no comments

A comment is not compiled or tested, so nothing fails when it stops being true. Agents edit code
often and leave the comments around it behind. The next agent then reads the comment as a fact.

```rust
// Keep the newest three weeks.
fn trim(weeks: &[Week]) -> &[Week] {
    &weeks[weeks.len().saturating_sub(5)..]
}
```

Someone changed 3 to 5 and the comment stayed. The next agent must now guess which one is right. It
may "fix" the code back to 3, or build on the wrong number, and it spends steps on the question
either way. Code that says the same thing cannot go stale without a failing build or test:

```rust
const WEEKS_KEPT: usize = 5;

fn newest_weeks(weeks: &[Week]) -> &[Week] {
    &weeks[weeks.len().saturating_sub(WEEKS_KEPT)..]
}
```
