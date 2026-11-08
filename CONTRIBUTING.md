# Contributing

Short version: branch, commit in Conventional Commit format, rebase, open a PR.
The long version is below, and `CLAUDE.md` is the same agreement written for
Claude Code.

## Setting up

You need **Rust** and **git**. On Windows this project uses the GNU toolchain,
because it does not require a multi-gigabyte Visual Studio C++ install.

```bash
winget install Rustlang.Rustup
rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal
rustup default stable-x86_64-pc-windows-gnu
winget install BrechtSanders.WinLibs.POSIX.MSVCRT
```

The last one supplies `as.exe` and `dlltool.exe`, which the Windows crates need
to build import libraries. They must be on `PATH` when you run cargo:

```
%LOCALAPPDATA%\Microsoft\WinGet\Packages\BrechtSanders.WinLibs.POSIX.MSVCRT_Microsoft.Winget.Source_8wekyb3d8bbwe\mingw64\bin
```

`git-ui.cmd` does this for you, so if you only want the git UI, double-click it.

```bash
git clone <repo-url>
cd Action-Based-Generation
cargo build
```

## Everyday workflow

```bash
git switch dev/your-name   # your own branch; create it once, then stay on it
# ...work...
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git add -A
git commit            # see the format below
git push
```

That is the whole day-to-day loop, and after the first `switch` it needs no
branch handling at all. Push as often as you like: your branch is yours, and
nothing you put on it reaches anyone else.

When you and the other people on the project agree the work is ready, open a
pull request from your branch into `main`. Afterwards, bring `main` back into
your branch so the two do not drift:

```bash
git pull origin main       # while on your own branch
```

Creating your branch, once:

```bash
git switch main
git pull
git switch -c dev/your-name
git push -u origin dev/your-name
```

There is also a desktop UI for the routine parts — status, commit, pull, push —
in `crates/git-ui`. Run `git-ui.cmd` or `cargo run --release -p abg-git-ui`.

## Branches

Everyone has a long-lived personal branch, `dev/<name>`:

```
dev/cifosi2500
```

That is where your work lives. It is never deleted, and it only reaches `main`
through a pull request opened when the people involved agree it is ready.

For a self-contained change you want reviewed on its own, a short-lived branch
still works, named `<type>/<short-slug>` where the type matches the commit type:

```
feat/action-sampler
fix/git-ui-detached-head
docs/project-map
```

Delete those once their PR is merged. Personal branches stay.

## Commit messages

Conventional Commits, with a body that says why:

```
feat(core): add action sampler

Samples the next action from the policy head instead of argmax,
so generation stops being deterministic.

- src/core/sampler.rs: new TopK sampler
- src/core/mod.rs: wire into Generator

Why: argmax collapsed every run to the same trajectory.
```

- **Types:** `feat` `fix` `docs` `refactor` `perf` `test` `build` `chore` `revert`
- **Scopes:** `core` `git-ui` `docs` `build` `ci` `repo`
- Subject in the imperative, lowercase, no full stop, 72 characters or fewer.
- A body is required for anything bigger than a typo fix, and it ends with a
  `Why:` line. If you cannot write that line, you do not understand the change
  well enough to commit it.
- One logical change per commit. Reformatting goes in its own `chore` commit.

## Pull requests

- Rebase onto current `main` before opening the PR — do not merge `main` into
  your branch.
- PR title uses the same Conventional Commit format as the subject line.
- The body says what changed, why, and how you tested it.
- Merged with **squash merge**, so `main` gets one clean commit per change.

## Things not to do

- Do not commit directly to `main`.
- Do not force-push `main`, or any branch someone else might have checked out.
  On your own feature branch after a rebase, use `--force-with-lease`.
- Do not create merge commits by pulling — always `git pull --rebase`.
- Do not rewrite history that is already on `main`.
- Do not commit `target/`, archives, or editor files.

`Cargo.lock` **is** committed: this workspace builds binaries.

## Documentation that moves with the code

- `docs/PROJECT_MAP.md` — update in the same commit that adds, removes, renames
  or moves a module or crate, or changes how data flows between them.
- `docs/DECISIONS.md` — append an entry whenever a choice rules out a real
  alternative. Append-only; supersede rather than edit.
