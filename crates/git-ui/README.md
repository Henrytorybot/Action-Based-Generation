# abg-git-ui

A small desktop window for keeping this project in sync with GitHub: see what
changed, commit, pull, push. No GitHub token to manage — it drives the `git`
command line, so it reuses whatever credentials the `git` CLI already has
(on Windows that's Git Credential Manager).

## Running it

Double-click `git-ui.cmd` in the repository root, or:

```bash
cargo run --release -p abg-git-ui
```

## What it does

| Area | What's there |
| --- | --- |
| Top bar | Folder picker, recent repositories, `Clone…` form for a GitHub URL |
| Left panel | Branch, upstream, ahead/behind counts, origin URL, action buttons, branch switcher, recent commits |
| Centre | Changed files with their status, commit message box, `Commit all` / `Commit all + Push` |
| Bottom | The exact git commands that ran, with their output |

The big **Sync** button does the usual round trip in one click: commit (if a
message is typed and there are changes), pull, push.

**Pull with rebase** is on by default and uses `--rebase --autostash`, so local
edits are stashed, the remote commits are replayed underneath, and the edits
come back. Turning it off switches to `pull --ff-only`, which refuses to pull
when histories have diverged rather than creating a merge commit.

Pushing a branch with no upstream automatically uses `push -u origin <branch>`.

If you point it at a folder that isn't a repository yet, the left panel offers
to run `git init` + `git remote add origin <url>` + `git fetch origin` for you.

## Design notes

- **Why the CLI and not libgit2?** Authentication. `git2` would mean handling
  tokens, SSH keys and 2FA by hand; shelling out gets all of that for free from
  the user's existing setup.
- `GIT_TERMINAL_PROMPT=0` is set on every call, so git can never hang waiting
  for input on a terminal this GUI doesn't have. Credential helpers, which open
  their own window, still work.
- On Windows each `git` call uses `CREATE_NO_WINDOW` so no console flashes up.
- Commands run on a worker thread and report back over a channel; the UI never
  blocks. One job at a time — buttons grey out while something is running.
- Status comes from `git status --porcelain=v2 --branch`, which gives the
  ahead/behind counts directly instead of needing extra `rev-list` calls.

## Deliberately not included

Nothing destructive: no force push, no reset, no discarding changes, no branch
deletion. Those stay in the terminal on purpose.
