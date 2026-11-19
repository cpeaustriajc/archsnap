# Publish

Step 5 of SKILL.md.

- **Show me new or changed pages before a first write to a shared space.** Publishing to a team's
  Notion, Confluence or public wiki is visible to everyone. After I approve the shape once, a
  routine new decision is published and then reported.
- **Save after every change**, as the Backend section says (a wiki is a git repo and needs a push).
- **Parallel writers**: give each agent its own working copy, or keep a single writer.
- **Before publishing**, for a log held as files (GitHub wiki, Obsidian) run
  `python3 -I <this skill's base directory>/scripts/check_pages.py <folder>`: it checks names,
  Status and Date, sections, links, Home, index order and mentions of Claude or agents. Fix each
  line it prints, or say why one stays (a `check:` line, such as a support agent, is a
  judgment). On Notion, Linear or Confluence check those by hand. Then rewrite every pattern in
  `references/plain-prose.md`.
