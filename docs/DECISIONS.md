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
