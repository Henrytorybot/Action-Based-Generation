# Project map

What exists, what each piece owns, and how they fit together.

**Rule:** this file is updated in the same commit that changes the structure it
describes. See `CLAUDE.md` §6.

Last structural change: 2026-11-07 — workspace created, `git-ui` added.

---

## At a glance

```
ActionBasedGeneration (root package)   ← the project itself; still a stub
└── workspace member
    └── abg-git-ui                     ← developer tool, not part of the product
```

The workspace has two independent halves. The root package is the thing being
built. `crates/git-ui` is tooling for working on it and ships to nobody — it
must never become a dependency of the root package.

---

## `ActionBasedGeneration` — the root package

| | |
| --- | --- |
| Path | `Cargo.toml`, `src/` |
| Binary | `ActionBasedGeneration` |
| Depends on | nothing yet |
| Owns | the actual subject of this project |

### Status

A `fn main()` printing `Hello, world!`. Nothing about action-based generation
is implemented or specified yet.

### To be filled in

The sections below are the shape this document should take once the design
exists. Leave them as open questions rather than guessing at answers.

- **What "action-based generation" means here** — the problem being solved, and
  what an *action* is in this system.
- **Core types** — what an action is as data, what a generation step consumes
  and produces.
- **Pipeline** — the stages a generation run passes through, in order, and what
  crosses each boundary.
- **Extension points** — what is meant to be swappable.
- **External interfaces** — CLI, library API, file formats, anything persisted.

---

## `abg-git-ui` — git desktop UI

| | |
| --- | --- |
| Path | `crates/git-ui/` |
| Binary | `abg-git-ui` |
| Depends on | `eframe` + `egui` 0.31 (GUI), `rfd` (native folder picker), `serde` (persisted settings) |
| Also needs | the `git` CLI on `PATH` at runtime |
| Owns | clone / status / pull / push, driven from a window instead of a terminal |

### Modules

| File | Owns |
| --- | --- |
| `src/main.rs` | Window setup and `eframe` entry point. Hides the console in release builds. |
| `src/git.rs` | Every call to the `git` binary, and the parsing of its output. Knows nothing about the UI. |
| `src/app.rs` | All egui drawing, the worker-thread job queue, and persisted settings. Knows nothing about git's argument syntax beyond the lists it builds. |

### Data flow

```
button click
   └─> app.rs builds a Vec<Vec<String>> of git argument lists
        └─> spawned worker thread
             └─> git.rs::run()  →  CommandOutput        ─┐
             └─> git.rs::status() →  RepoStatus          ├─> mpsc::Sender<Msg>
                                                         ─┘
   UI thread drains the channel each frame, repaints
```

One job runs at a time; buttons are disabled while one is in flight. The UI
thread never blocks on git.

### Key decisions

- Drives the `git` CLI rather than linking libgit2, so authentication is
  whatever the user's `git` already uses. See `docs/DECISIONS.md`.
- Nothing destructive is exposed: no force push, no reset, no discard, no branch
  deletion. Those stay in a terminal deliberately.

---

## Build and tooling

| File | Owns |
| --- | --- |
| `Cargo.toml` (root) | The root package *and* the workspace member list. Both live in one file. |
| `git-ui.cmd` | Windows launcher: puts MinGW's binutils on `PATH`, builds release, starts the exe. |
| `.gitignore` | `target/`, `*.zip`, editor and debug leftovers. |

The MinGW `PATH` dance exists because the toolchain is `windows-gnu`; see
`CLAUDE.md` §2.
