# Project map

What exists, what each piece owns, and how they fit together.

**Rule:** this file is updated in the same commit that changes the structure it
describes. See `CLAUDE.md` §6.

Last structural change: 2026-11-08 — `abg-gen` and `abg-viewer` added.

---

## At a glance

```
ActionBasedGeneration (root package)   ← the project itself; still a stub
└── workspace members
    ├── abg-gen                        ← the generator; the actual subject
    ├── abg-viewer                     ← looks at abg-gen's output
    └── abg-git-ui                     ← developer tool, not part of the product
```

`abg-gen` is the project. `abg-viewer` exists to look at it. `crates/git-ui` is
tooling for working on the repository and ships to nobody — it must never
become a dependency of anything else here.

---

## The space being generated

Enough of the design is settled to write down. The rest is marked as open
rather than guessed at.

**Settled:**

- The world is a grid of tiles. A tile is **one bit**: wall, or open space.
- Levels are **3×3 of the level below**, so level *k* is 3^k tiles across:
  `tile, cell, room, house, lot, block, neighborhood, district, town, city,
  metropolis, region, state, country, subcontinent, continent, hemisphere,
  planet`. The names are identifiers, not descriptions.
- **Nothing is stored.** A planet is 3³⁴ ≈ 1.67×10¹⁶ tiles, about 2 PB at one
  bit each, so the hierarchy is an *address space*, not an object. Space is
  produced from a seed and a position, on demand.
- Generation runs **top-down**, never per tile. A corridor can span a block
  because the block decided so. Hashing tiles independently gives texture;
  this gives structure.

**Built, but not yet driven by anything:**

- A change replaces a unit with a **very similar** one — not a different one.
  The point is to undermine the player's confidence in their own memory, which
  needs the replacement to be almost right. A wholly different space reads as a
  teleport and is merely surprising.
- Which level gets replaced sets the scale of the wrongness: a cell is a tile
  out of place, a district is an unmistakably different area.
- See **Changing a space** below. It works; what is missing is the trigger.

**Designed, not built:**

- Space changes in response to how the player *moves through it*. Today a
  change happens because somebody clicked. What the real trigger should be —
  crossing a boundary, leaving a unit, looking away — is undecided, and it
  determines what the generator has to know about the player.

**Open:**

- Whether revisiting a place is stable by default, decays with time or
  distance, or always re-rolls.
- Whether a unit should be generated standing alone or as it appears at a
  given address inside its parents.
- Whether unreachable space is forbidden or a deliberate tool.

---

## `abg-gen` — the generator

| | |
| --- | --- |
| Path | `crates/abg-gen/` |
| Library | `abg_gen` |
| Depends on | **nothing** |
| Owns | turning a seed and a level into a grid of walls and open space |

No dependencies is deliberate: generation has to be reproducible, and anything
external is something that can change the output between versions. That
includes the random number generator, which is a hand-written splitmix64 rather
than `rand`.

### Modules

| File | Owns |
| --- | --- |
| `src/lib.rs` | `Level`, `Params`, `Grid`, and the recursive generator. |
| `src/cells.rs` | The 3×3 cell alphabet: all 512 arrangements, which are legal, and the distance between them. |

### How a unit is generated

```
generate(level, params)
  └─ allocate ONE Grid of side 3^depth, all wall
     └─ fill(depth, x, y)                       recursive, writes in place
          depth 0  → open the single tile
          depth 1  → choose a legal cell, write its 9 tiles
          depth n  → fill the 9 children
                     carve a random spanning tree between their centres
                     carve some of the 4 spare links  (connectivity)
```

Children are generated independently, so their open space does not line up at
the boundaries. Corridors are carved deliberately rather than hoped for.

### The invariant everything rests on

**The centre tile of every unit at every level is open.** It holds because the
centre of a unit is the centre of its centre child, down to a single tile, and
the cell alphabet excludes any cell with a walled centre.

That is what makes corridor carving work: every carve runs centre-to-centre and
is guaranteed to land on open space at both ends. Which in turn gives the real
guarantee — **every open tile in a unit is reachable from its centre**, at
every level and every parameter setting. `Grid::reachable_from_centre` measures
it rather than assuming it, and the test suite checks it across levels, seeds,
openness and connectivity.

### The cell alphabet

512 possible 3×3 arrangements; **161 are legal**. Two rules: the centre is
open, and no open tile is sealed off from it. A corner tile is reachable only
through one of its two adjacent edges, which is what eliminates the rest.

The legal set is **connected under single-tile changes** — any legal cell can
be reached from any other by flipping one tile at a time without passing
through an illegal cell. That is asserted by a test, because it is the property
the not-yet-built "replace with a very similar one" mechanic depends on.

### Parameters

| Field | Does |
| --- | --- |
| `seed` | Picks the world. |
| `openness` | How much space cells leave open, 0–255. Has a floor near 37%: corridors must connect regardless, so a unit cannot be made mostly solid. |
| `connectivity` | Corridors beyond the eight needed. 0 is a perfect maze, 255 links all twelve neighbours. Only ever adds, so full connectivity survives. |
| `nudge` | Unused. A per-unit offset kept for whatever the eventual trigger needs; changes today go through `Nudges` instead. |

### Changing a space

`generate_with(level, params, &Nudges)` is the same generator with some cells
pushed away from what the seed alone would give. A `Nudges` is a sparse map
from a cell's origin to a step count; untouched cells — almost all of them —
cost nothing.

**One step moves one cell to a legal neighbour of itself**: exactly one tile
different, and still a legal cell. Two properties follow, and both are tested
rather than assumed:

- **A change cannot seal anything off.** The replacement still has an open
  centre and nothing walled away inside it, so corridor carving still lands.
- **A change cannot cascade.** Nudging one cell in an 81×81 lot leaves every
  tile outside that 3×3 bit-identical. Mutation is O(1) and strictly local,
  which is what makes it affordable at world scale.

Steps accumulate: pushing the same cell repeatedly walks it further through
the alphabet, so a place drifts rather than jumping. Because the walk can
retrace itself, *n* steps means at most *n* tiles different, not exactly *n*.

Corridors are carved after cells, so a changed tile underneath a corridor is
invisible. The change still happened; it is covered.

Integers, not floats — float results are not guaranteed bit-identical across
platforms, and generation has to be reproducible.

---

## `abg-viewer` — the generator's window

| | |
| --- | --- |
| Path | `crates/abg-viewer/` |
| Binary | `abg-viewer` |
| Depends on | `abg-gen`, `eframe` + `egui` 0.31 |
| Owns | drawing a generated unit and the controls that produce it |

White is wall, black is open. There is no player, but the space can be changed:
clicking a grid square replaces that unit with a very similar one, which is the
mutation mechanic standing in for a trigger that does not exist yet.

Controls are unit, seed, openness, connectivity, a grid overlay that can
outline any level below the one on screen, and how many tiles a click is
allowed to change. The clicked unit is outlined so it is clear what moved.

The stats panel reports size, open percentage, bytes held, bytes spent on
changes, generation time, and how much of the open space is actually reachable
— that last one measured every frame, so a connectivity bug would show rather
than hide.

Tiles are drawn at a whole number of screen pixels each. Without that snap,
some tiles get two pixels and some get one, which reads as structure that is
not in the data.

---

## `abg-git-ui` — git desktop UI

| | |
| --- | --- |
| Path | `crates/git-ui/` |
| Binary | `abg-git-ui` |
| Depends on | `eframe` + `egui` 0.31, `rfd`, `serde`, `webbrowser` |
| Also needs | the `git` CLI on `PATH` at runtime |
| Owns | clone / status / pull / push, driven from a window instead of a terminal |

### Modules

| File | Owns |
| --- | --- |
| `src/main.rs` | Window setup and `eframe` entry point. Hides the console in release builds. |
| `src/git.rs` | Every call to the `git` binary, and the parsing of its output. Knows nothing about the UI. |
| `src/app.rs` | All egui drawing, the worker-thread job queue, and persisted settings. |

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
  whatever the user's `git` already uses. See `docs/DECISIONS.md` 0001.
- External targets are opened through the platform's handler API, never a
  shell, and remote URLs are validated against an allow-list. See 0004.
- Nothing destructive is exposed: no force push, no reset, no discard, no
  branch deletion.

---

## Build and tooling

| File | Owns |
| --- | --- |
| `Cargo.toml` (root) | The root package *and* the workspace member list. Both live in one file. |
| `viewer.cmd` | Windows launcher for the generator viewer. |
| `git-ui.cmd` | Windows launcher for the git UI. |
| `.gitattributes` | LF in the repository; CRLF in the working tree for `.cmd`, `.bat`, `.ps1`. |
| `.gitignore` | `target/`, `*.zip`, editor and debug leftovers. |

Both `.cmd` launchers put MinGW's binutils on `PATH` because the toolchain is
`windows-gnu`; see `CLAUDE.md` §2.
