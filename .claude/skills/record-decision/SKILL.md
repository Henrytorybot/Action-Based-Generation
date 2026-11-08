---
name: record-decision
description: Append an entry to docs/DECISIONS.md when a choice rules out a real alternative — a dependency, a data format, a toolchain, an architectural split. Use after making such a choice, or when the user asks to write down or revisit a decision.
---

# Record a decision

`docs/DECISIONS.md` exists so that months from now the reasoning is still
available, including what was believed at the time and what would make it wrong.

## When an entry is warranted

Write one when the choice **closed off something real**:

- adding, replacing or dropping a dependency
- a data format, protocol or on-disk layout
- a toolchain, target or build approach
- splitting or merging crates or modules
- a convention everyone now has to follow

Do **not** write one for ordinary implementation detail, a bug fix, or a choice
with no rejected alternative. A file full of non-decisions is as useless as an
empty one.

## How to write it

Read the end of `docs/DECISIONS.md` first for the next number and the house
style, then append:

```markdown
## NNNN — <short title>

- **Date:** YYYY-MM-DD
- **Status:** accepted
- **Decision:** <what was chosen, in one sentence>
- **Alternatives:** <what was rejected, and what each would have cost>
- **Why:** <the reasoning, ending with what would make this wrong>
```

- Numbers are sequential and never reused. Newest goes at the bottom.
- **Alternatives** must name something that was genuinely considered. "No
  alternative" means this is not a decision worth recording.
- **Why** ends by naming the conditions under which this would be the wrong
  call. That sentence is what makes the entry useful later.
- Use today's real date, not a relative one.

## Reversing a decision

The file is append-only. To change course:

1. Add a **new** entry with the next number, describing the new choice, and in
   its `Why:` explain what changed since the old one.
2. Edit only the **Status** line of the superseded entry:
   `- **Status:** superseded by NNNN`

Never delete or rewrite the body of an old entry. The record of what was
believed at the time is the whole point.

## Commit it

```
docs(docs): record decision NNNN — <short title>

- docs/DECISIONS.md: <the choice, in a few words>

Why: <why this needed writing down>
```

If the decision was made as part of a code change, the entry belongs in that
same commit.
