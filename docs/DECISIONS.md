# Decisions

Append-only. Newest at the bottom. One entry per choice that ruled out a real
alternative — a dependency, a format, a toolchain, an architectural split.

To reverse a decision, **add a new entry that supersedes it**; never edit or
delete an old one. The value of this file is being able to see what was believed
at the time.

Template:

```
## NNNN — <short title>

- **Date:** YYYY-MM-DD
- **Status:** accepted | superseded by NNNN
- **Decision:** <what was chosen, in one sentence>
- **Alternatives:** <what was rejected, and the cost of each>
- **Why:** <the reasoning, including what would make this wrong>
```

---

## 0001 — Drive the `git` CLI instead of linking libgit2

- **Date:** 2026-11-07
- **Status:** accepted
- **Decision:** `abg-git-ui` shells out to the `git` binary and parses its
  output; it does not use the `git2` crate.
- **Alternatives:** `git2` (libgit2 bindings) would give typed results and no
  output parsing, but every credential path — HTTPS tokens, SSH keys, 2FA —
  becomes our problem to implement and store.
- **Why:** authentication. Shelling out inherits whatever the user's `git`
  already has; on Windows that is Git Credential Manager, so pushing to GitHub
  works on day one with no token handling in this codebase. The cost is parsing
  `--porcelain=v2`, which is a stable, documented format. This would be wrong if
  the tool ever needed to operate on repositories without a `git` binary present,
  or needed fine-grained object access.

## 0002 — Use the `stable-x86_64-pc-windows-gnu` Rust toolchain

- **Date:** 2026-11-07
- **Status:** accepted
- **Decision:** the default toolchain is the GNU one, with MinGW-w64 from
  WinLibs supplying `as.exe` and `dlltool.exe`.
- **Alternatives:** the usual `stable-x86_64-pc-windows-msvc` needs Visual
  Studio's "Desktop development with C++" workload — a 4–6 GB install that was
  not present on the development machine.
- **Why:** roughly 500 MB instead of 5 GB, and it builds the egui stack fine.
  The cost is that MinGW's binutils must be on `PATH` for the Windows crates'
  `raw-dylib` import libraries, which `git-ui.cmd` handles. This would be wrong
  if a dependency ever requires MSVC specifically, or if debugger support on
  Windows becomes important.

## 0003 — Conventional Commits, feature branches, pull requests

- **Date:** 2026-11-07
- **Status:** accepted
- **Decision:** commits follow Conventional Commits with a mandatory `Why:` line;
  all work happens on `<type>/<slug>` branches and reaches `main` through a
  squash-merged pull request. Pulling always rebases.
- **Alternatives:** committing straight to `main` is simpler and was the first
  instinct for a small project.
- **Why:** more than one person works on this repository. Shared history makes
  rebase-on-pull and no-direct-commits-to-`main` worth the ceremony, and the
  commit format keeps the log readable without anyone having to remember what a
  change was for. This would be wrong if the project returns to a single author.

## 0004 — Untrusted strings never reach a shell

- **Date:** 2026-11-08
- **Status:** accepted
- **Decision:** anything that opens an external target does so through the
  platform's handler API (`webbrowser` for URLs) or by passing a single process
  argument (`explorer.exe` / `open` / `xdg-open` for folders). No `cmd /C`, no
  shell string interpolation. Values that originate outside this codebase are
  validated against a character allow-list before use, not escaped.
- **Alternatives:** escaping the value for the shell. Rejected because the
  correct quoting rules differ for every handler the string might reach — Rust
  escapes process arguments by `CommandLineToArgvW` rules, which `cmd.exe` does
  not follow, so an escape that looks right is not.
- **Why:** this qualifies 0001. Driving the `git` CLI is still the right call
  for authentication, but it means this codebase passes strings to subprocesses,
  and some of those strings are attacker-controlled. The `origin` URL comes from
  `.git/config`, which arrives with any cloned repository; an unquoted `&` in it
  reached `cmd /C start` and ran as a second command. 0001 named parsing
  `--porcelain=v2` as the only cost of shelling out; the real second cost is
  this boundary, and it needs a rule rather than care. This would be wrong if a
  value could be proven to originate in this codebase — but `origin` looked like
  that too.

## 0005 — Generate top-down and carve corridors, rather than hashing tiles

- **Date:** 2026-11-08
- **Status:** accepted
- **Decision:** space is produced by descending the 3x3 hierarchy, choosing a
  legal cell at the bottom and carving a random spanning tree between child
  centres on the way back up. A tile's value is never computed on its own.
- **Alternatives:**
  - *Hash each tile from its coordinates.* O(1) per tile, no recursion, no
    state. Rejected: it produces noise. Nothing can be larger than one tile,
    so there are no corridors, no rooms, and no connectivity at all.
  - *Constraint solving over the whole region (wave-function collapse and
    relatives).* Much stronger local agreement than this. Rejected for now:
    it needs global search, can fail and need backtracking, and its cost is
    not bounded by depth — all of which are hard to reconcile with generating
    a region on demand with no stored world.
- **Why:** the thing being built needs the player to form a mental map, so the
  space has to look like it obeys rules at every scale. Top-down is what
  produces that: a corridor spans a block because the block decided so, not
  because adjacent tiles happened to agree.

  Carving centre-to-centre also converts connectivity from a hope into a
  guarantee. Every unit's centre is open by construction, so every carve lands
  on open space at both ends, and every open tile ends up reachable from the
  centre. Cost is O(depth) per unit with no backtracking.

  This would be wrong if the generator ever needed to answer "what is this one
  tile" without generating its ancestors — a renderer that samples scattered
  points rather than regions would want the hashing approach instead.
