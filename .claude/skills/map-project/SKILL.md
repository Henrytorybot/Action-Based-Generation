---
name: map-project
description: Rebuild or refresh docs/PROJECT_MAP.md so it matches the code as it actually is — every crate and module, what each owns, how data flows between them. Use when the structure changed, when the map looks stale, or when someone asks what the project looks like or where something lives.
---

# Map the project

`docs/PROJECT_MAP.md` is the answer to "where does this live and what owns it".
It is only worth having if it is true, so verify against the tree — never
against memory or against what the map already says.

## 1. Read the real structure

```bash
git ls-files
cat Cargo.toml
find crates -name Cargo.toml
```

Use `git ls-files` rather than a bare `find`: it skips `target/` and anything
ignored, which is exactly what the map should skip too.

For each crate, read its `Cargo.toml` for the binary/library names and the real
dependency list, then read the module files themselves — `main.rs`, `lib.rs`,
`mod.rs` and the modules they declare.

## 2. Work out what each module owns

For every module, answer in one line: **what is this the only place for?**

If you cannot say it in one line, say so in the map — an honest "this module
has two unrelated responsibilities" is more useful than a tidy fiction, and it
is the kind of thing that earns an entry in `docs/DECISIONS.md`.

Then trace the flow between them: what calls what, what type crosses each
boundary, what runs on which thread. The flow matters more than the file list;
a reader can get the file list from `ls`.

## 3. Write it

Keep the existing shape of `docs/PROJECT_MAP.md`:

- **At a glance** — the crates and how they relate, as a short tree.
- **One section per crate** — a small table (path, binary, dependencies, what it
  owns), then module responsibilities, then data flow, then key decisions with a
  pointer to `docs/DECISIONS.md`.
- **Build and tooling** — the files that are not code but that someone needs to
  know about.
- Update the **"Last structural change"** date at the top.

Rules for the prose:

- Describe what is there. Where something is unwritten or undecided, say that
  plainly and leave it as an open question — do not invent a design.
- No file-by-file inventory of every function. The map is for orientation.
- Mark anything deliberately excluded and why, like `git-ui` never being a
  dependency of the root package.

## 4. Commit it with the change that caused it

If the structure changed in work you are doing now, the map update belongs in
**that same commit**, not a follow-up. If you are refreshing a map that drifted,
commit it on its own:

```
docs(docs): refresh project map

- docs/PROJECT_MAP.md: <what was out of date>

Why: <what had changed in the code without the map following>
```
