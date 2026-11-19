# Patterns to detect and fix

Rules 3-25: Wikipedia's "Signs of AI writing" page (en.wikipedia.org/wiki/Wikipedia:Signs_of_AI_writing). Plus one house rule: no em dashes; end the sentence or use a comma.

Rules 26-35 are this repo's additions. Their numbers are stable ids that other files cite.

## Jargon

26. **Abstract metaphor nouns.** Substrate (base), wedge, wedge in (add), vector (way, method), locus, vantage, nexus, primitive as a noun, harness as a metaphor, surface as in "API surface", bedrock, scaffolding as a metaphor, modality, paradigm, gold-plating (more than the job needs), ratchet as a metaphor (the mechanism's real name, or a limit that only tightens), evacuate for moving code (move out), endgame (the last phase), north star, flywheel. Use the plainer concrete word.

## Plain speech

27. **Say what it does, not how it feels.** "SQL you can read" names a feeling; "`.toSQL()` returns the exact string sent to the database" names the mechanism. If you can't restate a sentence as an instruction, a fact or a number, or it could sit unchanged in another project's docs, cut it.
28. **Dense sentences.** One idea per sentence; split one the reader has to parse twice. At most 25 words in a sentence, 20 in a numbered step, and 6 sentences in a paragraph (ASD-STE100).
29. **Passive voice.** Name the actor: "the compiler validates queries". Passive only when the actor does not matter.
30. **Adverbs.** Give the number or a stronger verb: "significantly improves" becomes the measured delta.
31. **Fancy words.** "use", not "utilize" or "leverage"; "if", not "in the event that".
32. **Mannered prose.** Metaphor or flourish where a literal phrase exists: aphorisms ("wire it or delete it"), rhetorical fragments for effect, personified code ("the plan holds it"), figurative verbs ("rides along", "stands on"), stock framing phrases. "A dial worth turning" becomes "a parameter worth varying". Say what you mean. Rule 26 covers the metaphor nouns.
33. **Over-compression.** Dropped articles, verbless fragments, symbol-speak, and abbreviations that make the reader decode instead of read. "Parser rejects bad date → exit 2, no write" becomes "The parser rejects a bad date, exits with code 2, and writes nothing." Write whole sentences with their articles and verbs, and spell out arrows and abbreviations.
34. **Noun stacks.** At most three nouns in a row (ASD-STE100): "the timeout for upload retries", not "the upload retry timeout setting".
35. **One word for one thing** (ASD-STE100). Once you call it a "job", keep "job"; a switch to "task" or "run" reads as a second thing.
