---
name: ship
description: Prepare work in this repository for GitHub — run the checks, update the docs that must move with the code, write a Conventional Commit, put it on a correctly named branch, and push it with a pull request. Use whenever the user asks to commit, push, ship, or open a PR here.
---

# Ship a change

Follow this in order. Stop and report rather than improvising if a step fails.

## 1. See what actually changed

```bash
git status --porcelain=v2 --branch
git diff
git diff --staged
```

Read the diff before writing anything about it. The commit message describes
what the diff does, not what the task was meant to do.

## 2. Run the checks

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Cargo needs MinGW's binutils on `PATH` — see `CLAUDE.md` §2, or run through
`git-ui.cmd` which sets it. If `clippy` or the tests fail, fix them or report
the failure; do not commit over a red check without saying so explicitly.

If `cargo fmt` changed files, that reformatting belongs in its own
`chore(<scope>): apply cargo fmt` commit, separate from the real change.

## 3. Update the docs that move with the code

- Did a module, crate, public entry point or data flow change?
  → update `docs/PROJECT_MAP.md` **in this same commit**, and bump its
  "Last structural change" date.
- Did this change rule out a real alternative — a dependency, a format, a
  toolchain, an architectural split?
  → append an entry to `docs/DECISIONS.md` using the template at the top of it.
- Did a working rule itself change? → update `CLAUDE.md`.

A structural change committed with a stale map is an incomplete change.

## 4. Get on the right branch

Never commit to `main`. Check where you are:

```bash
git branch --show-current
```

If it is `main`, create a branch named `<type>/<short-slug>` matching the
commit type:

```bash
git switch -c feat/action-sampler
```

`feat` `fix` `docs` `refactor` `perf` `test` `build` `chore` `revert`

## 5. Write the commit

One logical change per commit. If the diff does two unrelated things, stage and
commit them separately (`git add -p` if needed).

```
<type>(<scope>): <subject>

<what changed and why, wrapped at 72 columns>

- <path>: <what changed there>
- <path>: <what changed there>

Why: <the reason this change exists>

Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
```

- Scopes in use: `core` `git-ui` `docs` `build` `ci` `repo`. If a new crate
  needs a new scope, add it to `CLAUDE.md` §4 in the same change.
- Subject: imperative, lowercase, no trailing period, ≤ 72 characters.
- The `Why:` line is mandatory for anything beyond a typo fix. If you cannot
  write it, go back and understand the change first.
- The `Co-Authored-By` trailer goes on commits Claude writes.

Use a heredoc so the body keeps its line breaks:

```bash
git commit -F - <<'MSG'
feat(core): add action sampler

...
MSG
```

## 6. Rebase onto main, then push

```bash
git fetch origin
git rebase origin/main
```

Resolve any conflicts, re-run the checks, then:

```bash
git push -u origin <branch>
```

If the branch was already pushed and you rebased, use
`git push --force-with-lease` — **only** on your own branch, never on `main`
or a branch someone else may have checked out.

## 7. Open the pull request

Ask the user before opening it; a PR is public-facing.

Title uses the same Conventional Commit format as the subject line. Body:

```markdown
## What
<the change, in a sentence or two>

## Why
<the reason, same substance as the commit's Why: line>

## How it was tested
<commands run, what you checked by hand>

🤖 Generated with [Claude Code](https://claude.com/claude-code)
```

Merge with **squash merge** so `main` keeps one clean commit per change, then
delete the branch.

## Never, without being asked in so many words

- `push --force` to `main` or any shared branch
- `reset --hard`, `clean -fd`, `checkout --` over uncommitted work
- rewriting history that is already on `main`
- deleting branches other than the one just merged
