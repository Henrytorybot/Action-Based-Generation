# Action-Based-Generation — working rules

Read this before touching anything. These rules are the agreement for the whole
project; prefer them over your own defaults, and say so if a rule blocks the
task rather than quietly working around it.

> **Status:** early. The core idea of Action-Based-Generation is not yet written
> down here. Until it is, do not invent it — ask. Everything about the `git-ui`
> tool below is real and current.

---

## 1. Repository layout

```
Action-Based-Generation/
├── Cargo.toml              workspace root + the `ActionBasedGeneration` package
├── src/main.rs             the main crate (still a stub)
├── crates/
│   └── git-ui/             abg-git-ui — desktop window for commit/pull/push
├── docs/
│   ├── PROJECT_MAP.md      what every module owns and how data flows
│   └── DECISIONS.md        append-only log of decisions and their reasons
├── .claude/skills/         project skills — see §9
├── git-ui.cmd              builds + launches the git UI (Windows)
└── CONTRIBUTING.md         human-facing setup and workflow
```

New top-level crates go in `crates/<name>/` and get added to
`[workspace] members`. The root package stays the project's own entry point.

## 2. Toolchain

| | |
| --- | --- |
| Rust | 1.99.1, **`stable-x86_64-pc-windows-gnu`** (not MSVC) |
| Edition | 2024 |
| Linker | MinGW-w64 from WinLibs, installed via winget |

The GNU toolchain needs MinGW's `as.exe`/`dlltool.exe` on `PATH` or the Windows
crates fail to build import libraries. `git-ui.cmd` adds them itself. For a
plain `cargo` call from a terminal, prepend:

```
%LOCALAPPDATA%\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin
```

Do not switch the default toolchain to MSVC without recording the decision in
`docs/DECISIONS.md` — it needs a 4–6 GB Visual Studio C++ install.

## 3. Commands

```bash
cargo build                     # the main crate only
cargo build --workspace         # the main crate AND every member
cargo run -p abg-git-ui         # the git UI
cargo fmt --all                 # before every commit
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`--workspace` is not optional on the clippy line. The root `Cargo.toml` is both
a package and the workspace root, and in that layout a bare `cargo clippy` only
checks the root package — it will report success while never looking at
`crates/git-ui`.

On Windows, `git-ui.cmd` is the double-click way to launch the git UI.

## 4. Commit messages — Conventional Commits, required

```
<type>(<scope>): <subject>

<body: what changed and why, wrapped at 72 columns>

- <path>: <what changed there>
- <path>: <what changed there>

Why: <the reason this change exists>
```

**Types:** `feat` `fix` `docs` `refactor` `perf` `test` `build` `chore` `revert`

**Scopes:** `core` (root crate) · `git-ui` · `docs` · `build` · `ci` · `repo`
Add a scope to this list when a new crate appears — do not invent one-off scopes.

**Subject:** imperative mood, lowercase, no trailing period, ≤ 72 characters.
"add action sampler", not "Added action sampler." or "adds sampler".

**Body:** required for anything that is not a one-line typo fix. List the files
or areas touched, each with what changed there. Finish with a `Why:` line — the
reason, not a restatement of the diff. If you cannot write the `Why:` line, the
change is not understood well enough to commit.

**Footer:** when Claude writes the commit, end with
`Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`.

One logical change per commit. Formatting-only changes go in their own
`chore(...)` commit so real diffs stay readable.

## 5. Getting changes to GitHub

Other people work on this repository. That drives everything below.

- **Never commit directly to `main`.** `main` is what everyone pulls.
- Work on a branch named `<type>/<short-slug>`, matching the commit type:
  `feat/action-sampler`, `fix/git-ui-detached-head`, `docs/project-map`.
- **Always `git pull --rebase`.** Never create a merge commit by pulling. The
  git UI's "Pull with rebase" checkbox is on by default for this reason.
- Rebase your branch onto the current `main` before opening a pull request.
- Open a **pull request** for every change. PR title follows the same
  Conventional Commit format; the body says what changed, why, and how it was
  tested. Merge with **squash merge** so `main` keeps one clean commit per change.
- **Never force-push `main` or any branch someone else may have checked out.**
  Force-pushing your own unshared feature branch after a rebase is fine, and
  only with `--force-with-lease`.
- Never rewrite history that is already on `main`.
- Delete the branch after the PR merges.

Nothing destructive is automated. No `reset --hard`, no `clean -fd`, no force
push, no branch deletion without being asked for it explicitly.

## 6. Documentation that must stay current

These are not optional extras; they are part of the change that touches them.

- **`docs/PROJECT_MAP.md`** — update in the *same commit* that adds, removes,
  renames or moves a module, crate, or public entry point, or that changes how
  data flows between them. A structural change with a stale map is incomplete.
- **`docs/DECISIONS.md`** — append an entry whenever a choice rules out a real
  alternative: a dependency, a data format, a toolchain, an architecture split.
  Entries are append-only; to reverse one, add a new entry that supersedes it.
- **`CLAUDE.md`** (this file) — update when a rule here actually changes. Do not
  let it drift into a diary.

## 7. Rust conventions

- `cargo fmt` output is the formatting; no hand-alignment arguments.
- `cargo clippy --workspace --all-targets -- -D warnings` must be clean before a
  PR. The `--workspace` is load-bearing; see §3.
- No `unwrap()` or `expect()` on anything that can fail at runtime from user
  input, I/O, or a subprocess. Return `Result` and let the caller decide. In
  tests, `unwrap()` is fine.
- Comments explain *why*, not *what* the line does. A comment restating the code
  is noise; a comment explaining a non-obvious constraint is the point.
- Public items that are not self-evident get a `///` doc comment.
- Keep modules narrow: one responsibility, named for what it owns.
- New dependencies are a decision — justify it, pin a version, log it in
  `docs/DECISIONS.md`.

## 8. Working agreements for Claude

- Finish the task that was asked, not an adjacent one. Flag what you noticed
  instead of fixing it unasked.
- Report honestly: if a build fails, show the error; if a step was skipped, say so.
- Do not commit or push unless asked. Staging and writing the message is fine;
  the push is the user's call.
- `target/`, `*.zip` and build output are never committed — see `.gitignore`.
- `Cargo.lock` **is** committed; this workspace produces binaries.

## 9. Skills — recommend them, never auto-run them

Claude Code loads a *skill* (a packaged set of instructions) by itself whenever a
task looks like a match, and does it silently. **In this repository it must not.**

The rule here is the opposite of the default:

- When a skill would help the task at hand, **say so before starting the work**:
  name it, say in one line what it changes about the approach, and give the exact
  command to invoke it.
- Then do the task unaided. Do not wait for an answer and do not quietly use the
  skill anyway.
- **Unless the skill *is* the task** — e.g. the user asks to push and `ship`
  covers that end to end. Then ask first and wait for a yes.
- Recommend at most one or two per turn, and only where the skill genuinely
  changes the outcome. A skill mentioned on every turn is noise, and noise gets
  ignored.

**Two exceptions, both narrow:**

1. The user invokes it themselves — typing `/ship`, or saying "use the ship
   skill". That is an instruction, not an auto-trigger. Run it immediately.
2. Reference-only skills the harness requires before producing a specific output
   type (artifact design, chart design, API references). Those are documentation
   rather than a workflow; load them silently.

**The shape of a recommendation:**

> `map-project` applies here — it regenerates `docs/PROJECT_MAP.md` from the code
> instead of me hand-editing it, so the map cannot drift. Run `/map-project` if
> you want that. Carrying on with the module split now.

**Why:** the point is to see which skill applies and decide, rather than have the
approach change invisibly mid-task. Recommendation is also how the available
skills get learned in the first place.

### Project skills — `.claude/skills/`

| Skill | Invoke | What it does |
| --- | --- | --- |
| `map-project` | `/map-project` | Rebuilds `docs/PROJECT_MAP.md` from the code as it actually is — crates, modules, what each owns, how data flows. For when structure changed or the map looks stale. |
| `record-decision` | `/record-decision` | Appends an entry to `docs/DECISIONS.md` when a choice rules out a real alternative — a dependency, a data format, a toolchain, an architectural split. |
| `ship` | `/ship` | Runs the checks, updates the docs that must move with the code, writes a Conventional Commit, branches correctly, pushes, opens the PR. The whole §4/§5 routine in one step. |

These three carry the rules in §4, §5 and §6. When a task lands in one of those
areas, that is exactly when to surface the matching skill.

### Built-ins worth surfacing

| Skill | Invoke | Use when |
| --- | --- | --- |
| `code-review` | `/code-review` | Reviewing a diff for correctness bugs before a PR. |
| `simplify` | `/simplify` | Quality pass — reuse, clarity, dead code. Not bug-hunting. |
| `security-review` | `/security-review` | Changes touching credentials, subprocesses, or file paths. |
| `run` | `/run` | Confirming a change works in the real app, not just in tests. |

Add a row here when a skill proves useful on this project; drop one that never
earns its mention.
