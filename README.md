# Action-Based-Generation

A Rust project. What it does is not written down yet — see `CLAUDE.md`, which
says so plainly rather than guessing.

## What is in here

| Path | What it is |
| --- | --- |
| `src/` | the `ActionBasedGeneration` crate — still a stub |
| `crates/abg-gen/` | the tile-space generator: pure, deterministic, no dependencies |
| `crates/abg-viewer/` | a window for looking at what the generator produces |
| `crates/git-ui/` | `abg-git-ui`, a desktop window for commit, pull and push |
| `docs/` | the project map and the decision log |
| `CLAUDE.md` | the working rules for this repository |
| `CONTRIBUTING.md` | setup and the everyday workflow |

## Getting started

```bash
cargo build --workspace
```

On Windows, `git-ui.cmd` builds and launches the git UI. Toolchain setup is in
`CONTRIBUTING.md`; it is not the usual one, so read it before building.
