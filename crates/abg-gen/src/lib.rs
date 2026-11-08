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
    let mut grid = Grid::solid(level.side());
    fill(&mut grid, level.depth(), 0, 0, params);
    grid
}

/// Fill the square at `(x0, y0)` of side 3^depth.
fn fill(grid: &mut Grid, depth: u32, x0: u32, y0: u32, params: &Params) {
    if depth == 0 {
        // A lone tile is the degenerate case of the centre rule below: the
        // centre of any unit is open, and a tile is its own centre.
        grid.set_wall(x0, y0, false);
        return;
    }

    if depth == 1 {
        fill_cell(grid, x0, y0, params);
        return;
    }

    let child = 3u32.pow(depth - 1);
    for j in 0..3 {
        for i in 0..3 {
            fill(grid, depth - 1, x0 + i * child, y0 + j * child, params);
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
fn fill_cell(grid: &mut Grid, x0: u32, y0: u32, params: &Params) {
    let alphabet = cells::alphabet();

    // Map openness onto a target number of open tiles. One is the bare centre,
    // nine is a fully open cell.
    let target = 1 + (u32::from(params.openness) * 8 / 255) as usize;
    let candidates = alphabet.nearest_with_open_count(target);

    let mut rng = seed_at(params, 1, x0, y0);
    let bits = candidates[(next(&mut rng) % candidates.len() as u64) as usize];

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
}
