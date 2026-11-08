//! Generator for the Action-Based-Generation tile space.
//!
//! The world is a 3^k hierarchy: each level is a 3x3 arrangement of the level
//! below, from a single tile up. A tile is either a wall or open space.
//!
//! Nothing is stored globally. A unit is produced from a seed and its position,
//! and generation is a pure function with no shared state — so it is
//! reproducible, trivially parallel, and costs nothing when nobody is looking.
//!
//! Structure comes from generating top-down rather than per tile. A corridor
//! can run the width of a block because the *block* decided so, not because
//! adjacent tiles happened to agree. Hashing each tile independently would give
//! texture; this gives architecture.

pub mod cells;
pub mod similarity;

use std::collections::HashMap;

/// Which cells have been pushed away from what the seed alone would produce,
/// and by how far.
///
/// This is the whole of "replace a space with a very similar one". A step
/// moves one cell to a legal neighbour of itself — a single tile different —
/// so the count is a budget for how wrong a place is allowed to look. Nothing
/// is stored for untouched cells, which is almost all of them.
/// One kind of change the space can make to a cell. Each is a single step,
/// the same step the comparison counts degrees in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    /// Move to a legal cell one tile different. The gentlest change there is.
    Flip,
    /// Turn a quarter clockwise.
    Turn,
    /// Mirror left to right.
    Mirror,
    /// Slide by `(dx, dy)`, wrapping round the cell edges.
    Slide(u8, u8),
}

impl Kind {
    /// The four slides that are genuinely one step: one tile, one direction.
    ///
    /// A slide of (1, 1) is two steps, not one, because it moves along both
    /// axes. Anything offering "walk once" has to pick from these, or the
    /// degree the comparison reports will not be the degree that was asked
    /// for.
    pub const SINGLE_SLIDES: [Kind; 4] = [
        Kind::Slide(1, 0),
        Kind::Slide(2, 0),
        Kind::Slide(0, 1),
        Kind::Slide(0, 2),
    ];
}

/// Which kinds the space is allowed to use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kinds {
    pub flip: bool,
    pub turn: bool,
    pub mirror: bool,
    pub slide: bool,
}

impl Kinds {
    /// Only single-tile walks: what the generator could do before turns,
    /// mirrors and slides existed.
    pub const FLIPS_ONLY: Kinds = Kinds {
        flip: true,
        turn: false,
        mirror: false,
        slide: false,
    };

    pub const ALL: Kinds = Kinds {
        flip: true,
        turn: true,
        mirror: true,
        slide: true,
    };

    /// The allowed kinds, for picking one at random. A slide comes back with
    /// a placeholder offset for the caller to replace.
    pub fn list(self) -> Vec<Kind> {
        let mut out = Vec::new();
        if self.flip {
            out.push(Kind::Flip);
        }
        if self.turn {
            out.push(Kind::Turn);
        }
        if self.mirror {
            out.push(Kind::Mirror);
        }
        if self.slide {
            out.push(Kind::Slide(1, 0));
        }
        out
    }

    pub fn any(self) -> bool {
        self.flip || self.turn || self.mirror || self.slide
    }
}

impl Default for Kinds {
    fn default() -> Self {
        Kinds::FLIPS_ONLY
    }
}

/// Everything that has happened to one cell.
///
/// Turning and mirroring are always safe: the legal alphabet is closed under
/// them, so the centre stays open and corridor carving still lands. Sliding is
/// not, which is why it is stored as an intention and checked when applied.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Change {
    /// Single-tile walks through the legal alphabet.
    pub flips: u32,
    /// Quarter turns clockwise, taken modulo four.
    pub turns: u8,
    /// Whether the cell is mirrored. With `turns` this reaches all eight ways
    /// a square can be set down.
    pub mirror: bool,
    /// Slide offsets, each modulo three.
    pub slide: (u8, u8),
}

impl Change {
    /// How many single things this amounts to, counted the way the
    /// comparison counts them.
    ///
    /// A rotation is one thing however far it turns, because any multiple of
    /// ninety degrees is a single rotation. A mirror is one thing. A walk of
    /// one tile along one axis is one; along both axes it is two, which is
    /// why only single-axis walks are offered.
    pub fn steps(&self) -> u32 {
        let (dx, dy) = (u32::from(self.slide.0 % 3), u32::from(self.slide.1 % 3));
        self.flips
            + u32::from(!self.turns.is_multiple_of(4))
            + u32::from(self.mirror)
            + dx.min(3 - dx)
            + dy.min(3 - dy)
    }

    pub fn is_empty(&self) -> bool {
        *self == Change::default()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Nudges {
    /// Keyed by the cell's top-left tile, which is always a multiple of three.
    changes: HashMap<(u32, u32), Change>,
}

impl Nudges {
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything that has happened to the cell containing `(x, y)`.
    pub fn change_at(&self, x: u32, y: u32) -> Change {
        self.changes
            .get(&(x - x % 3, y - y % 3))
            .copied()
            .unwrap_or_default()
    }

    /// How many single-tile walks the cell containing `(x, y)` has taken.
    pub fn steps_at(&self, x: u32, y: u32) -> u32 {
        self.change_at(x, y).flips
    }

    fn entry(&mut self, x: u32, y: u32) -> &mut Change {
        self.changes.entry((x - x % 3, y - y % 3)).or_default()
    }

    /// Walk the cell containing `(x, y)` one more legal single-tile step.
    pub fn push(&mut self, x: u32, y: u32) {
        self.entry(x, y).flips += 1;
    }

    /// Turn it another quarter.
    pub fn turn(&mut self, x: u32, y: u32) {
        let change = self.entry(x, y);
        change.turns = (change.turns + 1) % 4;
    }

    /// Mirror it, or put it back if it was already mirrored.
    pub fn mirror(&mut self, x: u32, y: u32) {
        let change = self.entry(x, y);
        change.mirror = !change.mirror;
    }

    /// Slide it further by `(dx, dy)`, wrapping.
    pub fn slide(&mut self, x: u32, y: u32, dx: u8, dy: u8) {
        let change = self.entry(x, y);
        change.slide.0 = (change.slide.0 + dx) % 3;
        change.slide.1 = (change.slide.1 + dy) % 3;
    }

    /// Apply one change of the given kind.
    pub fn apply(&mut self, x: u32, y: u32, kind: Kind) {
        match kind {
            Kind::Flip => self.push(x, y),
            Kind::Turn => self.turn(x, y),
            Kind::Mirror => self.mirror(x, y),
            Kind::Slide(dx, dy) => self.slide(x, y, dx, dy),
        }
    }

    /// How many cells have been touched at all.
    pub fn touched_cells(&self) -> usize {
        self.changes.len()
    }

    /// Every single step applied, of every kind, across all cells.
    pub fn total_steps(&self) -> u32 {
        self.changes.values().map(Change::steps).sum()
    }

    /// Bytes held by the record of what has changed. Untouched cells cost
    /// nothing, which is almost all of them.
    pub fn bytes(&self) -> usize {
        self.changes.len() * (std::mem::size_of::<(u32, u32)>() + std::mem::size_of::<Change>())
    }

    pub fn clear(&mut self) {
        self.changes.clear();
    }

    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }
}

/// A level of the hierarchy. Each is a 3x3 arrangement of the one below.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Level {
    Tile,
    Cell,
    Room,
    House,
    Lot,
    Block,
}

impl Level {
    pub const ALL: [Level; 6] = [
        Level::Tile,
        Level::Cell,
        Level::Room,
        Level::House,
        Level::Lot,
        Level::Block,
    ];

    /// How many 3x3 subdivisions deep this level sits. `Tile` is 0.
    pub fn depth(self) -> u32 {
        match self {
            Level::Tile => 0,
            Level::Cell => 1,
            Level::Room => 2,
            Level::House => 3,
            Level::Lot => 4,
            Level::Block => 5,
        }
    }

    /// Side length in tiles: 3^depth.
    pub fn side(self) -> u32 {
        3u32.pow(self.depth())
    }

    /// Total tiles in a unit of this level.
    pub fn tiles(self) -> u64 {
        let side = u64::from(self.side());
        side * side
    }

    pub fn name(self) -> &'static str {
        match self {
            Level::Tile => "tile",
            Level::Cell => "cell",
            Level::Room => "room",
            Level::House => "house",
            Level::Lot => "lot",
            Level::Block => "block",
        }
    }
}

/// What to generate, and how open it should be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Params {
    pub seed: u64,
    /// 0 leaves as little open as the alphabet allows, 255 as much.
    ///
    /// An integer rather than a float on purpose: generation has to be
    /// reproducible, and float results are not guaranteed bit-identical across
    /// platforms or optimisation levels.
    pub openness: u8,
    /// Reserved for player-driven change: a per-unit nudge away from what the
    /// seed alone would produce.
    ///
    /// Always zero today. It is in the signature now because retrofitting it
    /// later would mean touching every call site, and because the whole second
    /// half of this project is "replace a space with a very similar one" —
    /// which is this parameter moving by one.
    pub nudge: u32,
    /// How many connections beyond the bare minimum to carve, 0 to 255.
    ///
    /// Nine children need eight corridors to all be reachable, and there are
    /// twelve places a corridor could go. At 0 only the eight are carved, so
    /// there is exactly one route between any two places — a perfect maze. At
    /// 255 all twelve are carved and the unit becomes an open grid of
    /// corridors with loops everywhere.
    ///
    /// This only ever *adds* corridors, so the space stays fully connected at
    /// every setting. Allowing less than full connectivity — sealed rooms,
    /// space behind a wall — would be a separate knob, because it breaks the
    /// guarantee rather than tuning it.
    pub connectivity: u8,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            seed: 1,
            openness: 128,
            nudge: 0,
            connectivity: 0,
        }
    }
}

/// A square grid of tiles, one bit each. A set bit is a wall.
///
/// One bit per tile is not an optimisation, it is the only honest
/// representation: the tile really does carry exactly one bit of information.
#[derive(Clone)]
pub struct Grid {
    side: u32,
    words: Vec<u64>,
}

impl Grid {
    /// A grid of solid wall. Generation carves open space out of it.
    pub fn solid(side: u32) -> Self {
        let tiles = (side as usize) * (side as usize);
        Self {
            side,
            words: vec![u64::MAX; tiles.div_ceil(64)],
        }
    }

    pub fn side(&self) -> u32 {
        self.side
    }

    /// Bytes actually held by the tile data.
    pub fn bytes(&self) -> usize {
        self.words.len() * 8
    }

    fn index(&self, x: u32, y: u32) -> Option<(usize, u64)> {
        if x >= self.side || y >= self.side {
            return None;
        }
        let i = (y as usize) * (self.side as usize) + (x as usize);
        Some((i / 64, 1u64 << (i % 64)))
    }

    pub fn is_wall(&self, x: u32, y: u32) -> bool {
        match self.index(x, y) {
            // Outside the grid counts as wall, so edge handling needs no
            // special case anywhere else.
            None => true,
            Some((word, bit)) => self.words[word] & bit != 0,
        }
    }

    pub fn set_wall(&mut self, x: u32, y: u32, wall: bool) {
        if let Some((word, bit)) = self.index(x, y) {
            if wall {
                self.words[word] |= bit;
            } else {
                self.words[word] &= !bit;
            }
        }
    }

    /// How many tiles are open.
    pub fn open_tiles(&self) -> u64 {
        let mut open = 0;
        for y in 0..self.side {
            for x in 0..self.side {
                if !self.is_wall(x, y) {
                    open += 1;
                }
            }
        }
        open
    }

    /// How many open tiles can be walked to from the grid's centre.
    ///
    /// The generator is built so this always equals [`Grid::open_tiles`]. It is
    /// worth measuring rather than assuming: open space the player can see but
    /// never reach is the most common way a generator like this goes wrong.
    pub fn reachable_from_centre(&self) -> u64 {
        let centre = self.side / 2;
        if self.side == 0 || self.is_wall(centre, centre) {
            return 0;
        }

        let mut seen = vec![false; (self.side as usize) * (self.side as usize)];
        let at = |x: u32, y: u32| (y as usize) * (self.side as usize) + (x as usize);

        seen[at(centre, centre)] = true;
        let mut stack = vec![(centre, centre)];
        let mut count = 0;

        while let Some((x, y)) = stack.pop() {
            count += 1;
            let neighbours = [
                (x.wrapping_sub(1), y),
                (x + 1, y),
                (x, y.wrapping_sub(1)),
                (x, y + 1),
            ];
            for (nx, ny) in neighbours {
                if nx < self.side && ny < self.side && !self.is_wall(nx, ny) && !seen[at(nx, ny)] {
                    seen[at(nx, ny)] = true;
                    stack.push((nx, ny));
                }
            }
        }

        count
    }

    /// Whether every open tile can be reached from the centre.
    pub fn is_fully_connected(&self) -> bool {
        self.reachable_from_centre() == self.open_tiles()
    }
}

/// Generate one unit of the given level.
///
/// A single allocation of the final size, filled in place. Generating children
/// into their own grids and copying them up would be the obvious way to write
/// this, and would move every tile once per level above it.
pub fn generate(level: Level, params: &Params) -> Grid {
    generate_with(level, params, &Nudges::new())
}

/// Generate one unit, with some of its cells pushed away from what the seed
/// alone would give.
pub fn generate_with(level: Level, params: &Params, nudges: &Nudges) -> Grid {
    let mut grid = Grid::solid(level.side());
    fill(&mut grid, level.depth(), 0, 0, params, nudges);
    grid
}

/// Fill the square at `(x0, y0)` of side 3^depth.
fn fill(grid: &mut Grid, depth: u32, x0: u32, y0: u32, params: &Params, nudges: &Nudges) {
    if depth == 0 {
        // A lone tile is the degenerate case of the centre rule below: the
        // centre of any unit is open, and a tile is its own centre.
        grid.set_wall(x0, y0, false);
        return;
    }

    if depth == 1 {
        fill_cell(grid, x0, y0, params, nudges);
        return;
    }

    let child = 3u32.pow(depth - 1);
    for j in 0..3 {
        for i in 0..3 {
            fill(
                grid,
                depth - 1,
                x0 + i * child,
                y0 + j * child,
                params,
                nudges,
            );
        }
    }

    // The children are generated independently, so their open space does not
    // line up across the boundaries between them. Connect them deliberately: a
    // spanning tree over the nine children, carved centre to centre. Every
    // child centre is open by construction, so every carve lands on open space
    // at both ends, and the result is one connected unit at every level.
    let mut rng = seed_at(params, depth, x0, y0);
    let tree = spanning_tree(&mut rng);
    for &(a, b) in &tree {
        carve_between(grid, x0, y0, child, a, b);
    }

    // The spanning tree is the minimum: every child reachable, exactly one
    // route between any two of them. That is a maze. Carving any of the four
    // leftover adjacencies adds loops, and loops are what turn a maze into
    // somewhere you can walk around in. Only ever adds corridors, so full
    // connectivity survives every setting.
    if params.connectivity > 0 {
        for &(a, b) in &ADJACENT {
            let already = tree
                .iter()
                .any(|&(c, d)| (c, d) == (a, b) || (c, d) == (b, a));
            if already {
                continue;
            }
            if next(&mut rng) % 256 < u64::from(params.connectivity) {
                carve_between(grid, x0, y0, child, a, b);
            }
        }
    }
}

/// Every pair of children that share an edge, as indices into the 3x3.
///
/// Twelve of them: six horizontal, six vertical.
const ADJACENT: [(usize, usize); 12] = [
    (0, 1),
    (1, 2),
    (3, 4),
    (4, 5),
    (6, 7),
    (7, 8),
    (0, 3),
    (3, 6),
    (1, 4),
    (4, 7),
    (2, 5),
    (5, 8),
];

/// Choose one of the legal cells and write it in.
fn fill_cell(grid: &mut Grid, x0: u32, y0: u32, params: &Params, nudges: &Nudges) {
    let alphabet = cells::alphabet();

    // Map openness onto a target number of open tiles. One is the bare centre,
    // nine is a fully open cell.
    let target = 1 + (u32::from(params.openness) * 8 / 255) as usize;
    let candidates = alphabet.nearest_with_open_count(target);

    let mut rng = seed_at(params, 1, x0, y0);
    let mut bits = candidates[(next(&mut rng) % candidates.len() as u64) as usize];

    let change = nudges.change_at(x0, y0);

    // Walk away from that cell, one legal single-tile change at a time.
    //
    // Each step lands on a cell differing by exactly one tile, and every one
    // is legal, so the centre stays open and nothing inside gets sealed off.
    // Steps can undo each other, so n steps means *at most* n tiles different,
    // not exactly n.
    for step in 0..change.flips {
        let neighbours = cells::legal_neighbours(bits);
        if neighbours.is_empty() {
            break;
        }
        let mut walk = seed_at(params, 1, x0, y0) ^ (u64::from(step) + 1).wrapping_mul(0x9E37_79B9);
        bits = neighbours[(next(&mut walk) % neighbours.len() as u64) as usize];
    }

    // Turning and mirroring need no check. The legal alphabet is closed under
    // the eight ways a square can be set down, so the centre is still open and
    // nothing has been sealed off.
    for _ in 0..(change.turns % 4) {
        bits = similarity::transformed_cell(bits, similarity::Rigid::Turn90);
    }
    if change.mirror {
        bits = similarity::transformed_cell(bits, similarity::Rigid::MirrorX);
    }

    // Sliding does need one. It moves the centre tile, and a cell with a
    // walled centre is not legal — roughly half of all slides have to be
    // refused. A refused slide leaves the cell where it was rather than
    // substituting something else, so the space never produces a unit the
    // player could be sealed inside.
    let (dx, dy) = (u32::from(change.slide.0 % 3), u32::from(change.slide.1 % 3));
    if dx != 0 || dy != 0 {
        let slid = similarity::slid_cell(bits, dx, dy);
        if cells::is_legal(slid) {
            bits = slid;
        }
    }

    for i in 0..9 {
        grid.set_wall(x0 + i % 3, y0 + i / 3, !cells::is_open(bits, i));
    }
}

/// Carve a corridor between the centres of two adjacent children.
///
/// Children in a 3x3 share a row or a column with every neighbour, so their
/// centres line up and the corridor is always a straight line.
fn carve_between(grid: &mut Grid, x0: u32, y0: u32, child: u32, a: usize, b: usize) {
    let centre = |n: usize| {
        (
            x0 + (n as u32 % 3) * child + child / 2,
            y0 + (n as u32 / 3) * child + child / 2,
        )
    };
    let (ax, ay) = centre(a);
    let (bx, by) = centre(b);

    if ay == by {
        for x in ax.min(bx)..=ax.max(bx) {
            grid.set_wall(x, ay, false);
        }
    } else {
        for y in ay.min(by)..=ay.max(by) {
            grid.set_wall(ax, y, false);
        }
    }
}

/// A random spanning tree over the nine children, as edges between indices.
///
/// Randomised depth-first search: it favours long winding runs over short
/// stubby ones, which reads as corridors rather than as a blob.
fn spanning_tree(rng: &mut u64) -> Vec<(usize, usize)> {
    let mut visited = [false; 9];
    let mut edges = Vec::with_capacity(8);

    let start = (next(rng) % 9) as usize;
    visited[start] = true;
    let mut stack = vec![start];

    while let Some(&current) = stack.last() {
        let (x, y) = (current % 3, current / 3);
        let mut candidates = [0usize; 4];
        let mut count = 0;
        let consider = |n: usize, candidates: &mut [usize; 4], count: &mut usize| {
            if !visited[n] {
                candidates[*count] = n;
                *count += 1;
            }
        };
        if x > 0 {
            consider(current - 1, &mut candidates, &mut count);
        }
        if x < 2 {
            consider(current + 1, &mut candidates, &mut count);
        }
        if y > 0 {
            consider(current - 3, &mut candidates, &mut count);
        }
        if y < 2 {
            consider(current + 3, &mut candidates, &mut count);
        }

        if count == 0 {
            stack.pop();
            continue;
        }

        let next_node = candidates[(next(rng) % count as u64) as usize];
        visited[next_node] = true;
        edges.push((current, next_node));
        stack.push(next_node);
    }

    edges
}

/// The random state for one unit, from the seed and where the unit sits.
///
/// Position is part of the input rather than carried along as mutable state,
/// which is what keeps generation a pure function of the address.
fn seed_at(params: &Params, depth: u32, x: u32, y: u32) -> u64 {
    let mut state = params
        .seed
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(u64::from(depth))
        .rotate_left(17)
        ^ (u64::from(x) << 32)
        ^ u64::from(y)
        ^ (u64::from(params.nudge) << 16);
    // Mix once so neighbouring positions do not produce neighbouring states.
    next(&mut state);
    state
}

/// splitmix64. Small, fast, and identical on every platform, which `rand`'s
/// default generator does not promise across versions.
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sides_follow_powers_of_three() {
        assert_eq!(Level::Tile.side(), 1);
        assert_eq!(Level::Cell.side(), 3);
        assert_eq!(Level::Room.side(), 9);
        assert_eq!(Level::House.side(), 27);
        assert_eq!(Level::Lot.side(), 81);
        assert_eq!(Level::Block.side(), 243);
        assert_eq!(Level::Block.tiles(), 59_049);
    }

    #[test]
    fn same_seed_gives_the_same_unit() {
        let params = Params {
            seed: 12345,
            openness: 90,
            nudge: 0,
            connectivity: 120,
        };
        let a = generate(Level::House, &params);
        let b = generate(Level::House, &params);
        for y in 0..a.side() {
            for x in 0..a.side() {
                assert_eq!(a.is_wall(x, y), b.is_wall(x, y), "differs at {x},{y}");
            }
        }
    }

    #[test]
    fn every_unit_is_one_connected_space() {
        // The claim the whole corridor-carving design exists to make good on.
        for level in Level::ALL {
            for seed in 1..20u64 {
                for openness in [0, 64, 128, 200, 255] {
                    for connectivity in [0, 128, 255] {
                        let grid = generate(
                            level,
                            &Params {
                                seed,
                                openness,
                                nudge: 0,
                                connectivity,
                            },
                        );
                        assert!(
                            grid.is_fully_connected(),
                            "{} seed {seed} openness {openness} connectivity \
                             {connectivity}: {} of {} open tiles reachable",
                            level.name(),
                            grid.reachable_from_centre(),
                            grid.open_tiles(),
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_centre_is_always_open() {
        for level in Level::ALL {
            for seed in 1..40u64 {
                let grid = generate(
                    level,
                    &Params {
                        seed,
                        openness: 0,
                        nudge: 0,
                        connectivity: 0,
                    },
                );
                let centre = grid.side() / 2;
                assert!(!grid.is_wall(centre, centre), "{}", level.name());
            }
        }
    }

    #[test]
    fn openness_spans_its_full_range() {
        let tiles = Level::Lot.tiles();
        let closed = generate(
            Level::Lot,
            &Params {
                seed: 7,
                openness: 0,
                nudge: 0,
                connectivity: 0,
            },
        );
        let open = generate(
            Level::Lot,
            &Params {
                seed: 7,
                openness: 255,
                nudge: 0,
                connectivity: 0,
            },
        );

        assert_eq!(open.open_tiles(), tiles, "255 should leave nothing walled");

        // Note what the floor actually is. At openness 0 every cell is reduced
        // to its bare centre, yet roughly a third of the unit is still open,
        // because the corridors carved between child centres at each level cut
        // through regardless. Connectivity is not negotiable, so it sets a
        // minimum on how solid a unit can be — worth knowing before trying to
        // design a mostly-solid space with this generator.
        let floor = closed.open_tiles() as f64 / tiles as f64;
        assert!(
            (0.2..0.5).contains(&floor),
            "corridor floor moved to {floor:.3}; the generator's minimum \
             openness changed, which affects every level above"
        );
    }

    #[test]
    fn a_block_costs_about_seven_kilobytes() {
        // 243 x 243 tiles at one bit each. Worth asserting: the moment this
        // starts costing a byte per tile, the numbers that make the hierarchy
        // workable stop holding.
        let grid = generate(Level::Block, &Params::default());
        assert!(grid.bytes() < 8 * 1024, "{} bytes", grid.bytes());
    }

    fn differences(a: &Grid, b: &Grid) -> u64 {
        let mut count = 0;
        for y in 0..a.side() {
            for x in 0..a.side() {
                if a.is_wall(x, y) != b.is_wall(x, y) {
                    count += 1;
                }
            }
        }
        count
    }

    #[test]
    fn one_step_changes_exactly_one_tile() {
        // Checked on a bare cell, where no corridor carving can cover a
        // changed tile up. This is the smallest possible change to the world,
        // and it has to actually be the smallest or "replace it with a very
        // similar one" means nothing.
        let params = Params::default();
        for seed in 1..60u64 {
            let params = Params { seed, ..params };
            let before = generate(Level::Cell, &params);

            let mut nudges = Nudges::new();
            nudges.push(1, 1);
            let after = generate_with(Level::Cell, &params, &nudges);

            assert_eq!(
                differences(&before, &after),
                1,
                "seed {seed}: one step moved {} tiles",
                differences(&before, &after)
            );
        }
    }

    #[test]
    fn a_nudge_stays_inside_its_own_cell() {
        // The reason this mechanic can be cheap: changing one cell cannot
        // disturb anything outside it, so nothing else has to be regenerated
        // and no change can cascade.
        let params = Params::default();
        let before = generate(Level::Lot, &params);

        let mut nudges = Nudges::new();
        nudges.push(40, 40);
        let after = generate_with(Level::Lot, &params, &nudges);

        for y in 0..before.side() {
            for x in 0..before.side() {
                let inside = (39..42).contains(&x) && (39..42).contains(&y);
                if !inside {
                    assert_eq!(
                        before.is_wall(x, y),
                        after.is_wall(x, y),
                        "tile {x},{y} changed, but it is outside the nudged cell"
                    );
                }
            }
        }
    }

    #[test]
    fn nudged_space_is_still_one_connected_space() {
        // Every step lands on a legal cell, so the centre stays open and the
        // corridor carving still has somewhere to land. If that ever stops
        // holding, a change could seal the player in.
        let params = Params::default();
        let mut nudges = Nudges::new();
        let mut state = 99u64;

        for round in 0..40 {
            let x = (next(&mut state) % 81) as u32;
            let y = (next(&mut state) % 81) as u32;
            nudges.push(x, y);

            let grid = generate_with(Level::Lot, &params, &nudges);
            assert!(
                grid.is_fully_connected(),
                "round {round}: {} of {} open tiles reachable after {} steps",
                grid.reachable_from_centre(),
                grid.open_tiles(),
                nudges.total_steps(),
            );
        }
    }

    #[test]
    fn nudges_cost_nothing_until_used() {
        let mut nudges = Nudges::new();
        assert!(nudges.is_empty());
        nudges.push(7, 7);
        nudges.push(8, 8);
        // Both land in the same cell, so this is one entry with two steps —
        // storage grows with places touched, not with how often.
        assert_eq!(nudges.touched_cells(), 1);
        assert_eq!(nudges.total_steps(), 2);
        assert_eq!(nudges.steps_at(6, 6), 2);
        assert_eq!(nudges.steps_at(9, 9), 0);
    }

    // ---- turns, mirrors and slides as changes the space can make --------

    /// A cell on its own: no corridor is carved over it, so what the change
    /// did is exactly what you can read back out.
    fn bare_cell(seed: u64, nudges: &Nudges) -> u16 {
        let params = Params {
            seed,
            ..Params::default()
        };
        let grid = generate_with(Level::Cell, &params, nudges);
        let mut bits = 0u16;
        for i in 0..9u32 {
            if grid.is_wall(i % 3, i / 3) {
                bits |= 1 << i;
            }
        }
        bits
    }

    #[test]
    fn a_turn_really_turns_the_cell() {
        for seed in 0..40u64 {
            let before = bare_cell(seed, &Nudges::new());
            let mut nudges = Nudges::new();
            nudges.turn(0, 0);
            let after = bare_cell(seed, &nudges);
            assert_eq!(
                after,
                similarity::transformed_cell(before, similarity::Rigid::Turn90),
                "seed {seed}: a turn did not produce the turned cell"
            );
            assert!(
                cells::is_legal(after),
                "seed {seed}: turning made it illegal"
            );
        }
    }

    #[test]
    fn a_mirror_really_mirrors_the_cell() {
        for seed in 0..40u64 {
            let before = bare_cell(seed, &Nudges::new());
            let mut nudges = Nudges::new();
            nudges.mirror(0, 0);
            let after = bare_cell(seed, &nudges);
            assert_eq!(
                after,
                similarity::transformed_cell(before, similarity::Rigid::MirrorX),
                "seed {seed}: a mirror did not produce the mirrored cell"
            );
            assert!(cells::is_legal(after));
        }
    }

    #[test]
    fn a_slide_is_taken_or_refused_but_never_breaks_the_cell() {
        // Sliding moves the centre tile, and a walled centre is illegal. A
        // slide that would do that has to be refused outright: substituting
        // something else would be a change the player never asked for, and
        // taking it anyway could seal them in.
        let mut taken = 0;
        let mut refused = 0;
        for seed in 0..60u64 {
            let before = bare_cell(seed, &Nudges::new());
            for step in 1..9u32 {
                let (dx, dy) = (step % 3, step / 3);
                let mut nudges = Nudges::new();
                nudges.slide(0, 0, dx as u8, dy as u8);
                let after = bare_cell(seed, &nudges);

                let slid = similarity::slid_cell(before, dx, dy);
                if cells::is_legal(slid) {
                    assert_eq!(after, slid, "a legal slide was not taken");
                    taken += 1;
                } else {
                    assert_eq!(after, before, "an illegal slide was not refused");
                    refused += 1;
                }
                assert!(cells::is_legal(after), "a slide left an illegal cell");
            }
        }
        assert!(taken > 0 && refused > 0, "expected both outcomes to occur");
    }

    #[test]
    fn turning_four_times_is_no_change_at_all() {
        let before = bare_cell(5, &Nudges::new());
        let mut nudges = Nudges::new();
        for _ in 0..4 {
            nudges.turn(0, 0);
        }
        assert_eq!(bare_cell(5, &nudges), before);
        assert_eq!(nudges.total_steps(), 0, "four turns should cost no steps");
    }

    #[test]
    fn every_kind_of_change_leaves_one_connected_space() {
        // The whole point of the legality rules. Turning and mirroring are
        // closed over the alphabet; a slide that would break it is refused.
        // If any of that were wrong, a player could be sealed into a pocket
        // of a unit with no way out.
        let mut rng = 0x9E37_79B9_7F4A_7C15u64;
        let mut roll = move || {
            rng = rng.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = rng;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        };

        for level in [Level::Cell, Level::Room, Level::House, Level::Lot] {
            let side = level.side();
            for seed in 0..6u64 {
                let params = Params {
                    seed,
                    openness: (roll() % 256) as u8,
                    connectivity: (roll() % 256) as u8,
                    nudge: 0,
                };
                let mut nudges = Nudges::new();
                for _ in 0..(side * side / 4).max(4) {
                    let x = (roll() % u64::from(side)) as u32;
                    let y = (roll() % u64::from(side)) as u32;
                    match roll() % 4 {
                        0 => nudges.push(x, y),
                        1 => nudges.turn(x, y),
                        2 => nudges.mirror(x, y),
                        _ => {
                            let step = 1 + roll() % 8;
                            nudges.slide(x, y, (step % 3) as u8, (step / 3) as u8);
                        }
                    }
                }
                let grid = generate_with(level, &params, &nudges);
                assert!(
                    grid.is_fully_connected(),
                    "{} seed {seed}: {} of {} open tiles reachable after {} changes",
                    level.name(),
                    grid.reachable_from_centre(),
                    grid.open_tiles(),
                    nudges.total_steps()
                );
            }
        }
    }

    #[test]
    fn a_change_of_any_kind_stays_inside_its_own_cell() {
        let params = Params::default();
        let plain = generate(Level::Lot, &params);

        for kind in [Kind::Flip, Kind::Turn, Kind::Mirror, Kind::Slide(1, 1)] {
            let mut nudges = Nudges::new();
            nudges.apply(40, 40, kind);
            let changed = generate_with(Level::Lot, &params, &nudges);

            let (cx, cy) = (40 - 40 % 3, 40 - 40 % 3);
            for y in 0..Level::Lot.side() {
                for x in 0..Level::Lot.side() {
                    let inside = (cx..cx + 3).contains(&x) && (cy..cy + 3).contains(&y);
                    if !inside {
                        assert_eq!(
                            plain.is_wall(x, y),
                            changed.is_wall(x, y),
                            "{kind:?} changed ({x}, {y}), outside the cell it was aimed at"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn a_change_is_exact_in_a_bare_cell_and_partly_covered_above_one() {
        // Corridors are carved after cells, so above cell level a corridor can
        // run back over part of a changed cell and open it up again. The change
        // still happened, but the finished unit no longer reads as one clean
        // step. Anyone reading the comparison screen has to know that, so it is
        // pinned here rather than left to be rediscovered.
        let params = Params::default();

        // A bare cell has nothing carved over it, so a turn is exactly a turn.
        for seed in 0..20u64 {
            let p = Params { seed, ..params };
            let before = generate(Level::Cell, &p);
            let mut nudges = Nudges::new();
            nudges.turn(0, 0);
            let after = generate_with(Level::Cell, &p, &nudges);
            let report = similarity::compare(&before, &after);
            assert!(
                report.steps() <= 1 && report.is_exact(),
                "seed {seed}: a bare cell came out {} away, exact: {}",
                report.steps(),
                report.is_exact()
            );
        }

        // A house does, and some of its turns get covered.
        let plain = generate(Level::House, &params);
        let side = Level::House.side();
        let mut covered = 0;
        for cy in 0..side / 3 {
            for cx in 0..side / 3 {
                let mut nudges = Nudges::new();
                nudges.turn(cx * 3, cy * 3);
                let after = generate_with(Level::House, &params, &nudges);
                if !similarity::compare(&plain, &after).is_exact() {
                    covered += 1;
                }
            }
        }
        assert!(
            covered > 0,
            "no turn was covered by a corridor, so the UI no longer needs to explain this"
        );
    }

    #[test]
    fn every_single_slide_really_is_one_step() {
        for kind in Kind::SINGLE_SLIDES {
            let mut nudges = Nudges::new();
            nudges.apply(0, 0, kind);
            assert_eq!(
                nudges.total_steps(),
                1,
                "{kind:?} was offered as one step but costs more"
            );
        }
    }

    #[test]
    fn a_change_costs_the_steps_the_comparison_would_count() {
        // `Change::steps` has to agree with how the comparison measures
        // degrees, or the generator and the screen disagree about how far a
        // place has moved.
        let cases = [
            (
                Change {
                    flips: 5,
                    ..Change::default()
                },
                5,
            ),
            (
                Change {
                    turns: 1,
                    ..Change::default()
                },
                1,
            ),
            (
                // A half turn is one rotation, not two quarter ones.
                Change {
                    turns: 2,
                    ..Change::default()
                },
                1,
            ),
            (
                Change {
                    turns: 3,
                    ..Change::default()
                },
                1,
            ),
            (
                Change {
                    turns: 4,
                    ..Change::default()
                },
                0,
            ),
            (
                Change {
                    mirror: true,
                    ..Change::default()
                },
                1,
            ),
            (
                Change {
                    slide: (1, 0),
                    ..Change::default()
                },
                1,
            ),
            (
                Change {
                    slide: (2, 0),
                    ..Change::default()
                },
                1,
            ),
            (
                Change {
                    slide: (1, 1),
                    ..Change::default()
                },
                2,
            ),
            (
                Change {
                    slide: (2, 2),
                    ..Change::default()
                },
                2,
            ),
        ];
        for (change, expected) in cases {
            assert_eq!(change.steps(), expected, "{change:?}");
        }
    }

    #[test]
    fn what_has_changed_still_costs_almost_nothing_to_remember() {
        let mut nudges = Nudges::new();
        for i in 0..100u32 {
            nudges.turn(i * 3, 0);
        }
        assert_eq!(nudges.touched_cells(), 100);
        // Twelve bytes a cell was the figure before turns, mirrors and slides
        // existed; they must not have made remembering a change expensive.
        assert!(
            nudges.bytes() <= 100 * 24,
            "a touched cell now costs {} bytes",
            nudges.bytes() / 100
        );
    }
}
