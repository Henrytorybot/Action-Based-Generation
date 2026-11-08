//! How alike two units are, and in how many different senses at once.
//!
//! "Similar" is not one number. Two rooms can share every tile but one, or
//! share no tile at all and still be the same room turned ninety degrees, or
//! be built from exactly the same nine cells fitted together differently.
//! Those are different kinds of alike, and the mechanic this project is built
//! around cares which: turning a room and moving one tile does not feel like
//! moving four tiles. So a comparison reports each sense separately and never
//! averages them into one score.
//!
//! Every figure is a percentage, where 100 means "the same in this sense".
//!
//! One number to keep in mind while reading them: two *unrelated* units do not
//! score 0 on tiles. Wall agrees with wall by chance, so the floor sits nearer
//! 50 than 0. `an_unrelated_unit_scores_near_the_chance_floor` pins that down,
//! because a percentage whose floor you do not know cannot be read.

use crate::{Grid, Kind, Kinds, Level, Nudges, Params, generate_with};

/// One of the eight ways a square can be set down: four turns, and the same
/// four mirrored.
///
/// This is the symmetry group of the square. The cell alphabet is closed under
/// it — turning or mirroring a legal cell always gives another legal cell —
/// which is what makes it usable as a notion of sameness here. Sliding is not:
/// shifting a cell moves the centre tile, and a cell with a walled centre is
/// not legal.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Rigid {
    None,
    Turn90,
    Turn180,
    Turn270,
    MirrorX,
    MirrorY,
    MirrorMain,
    MirrorAnti,
}

impl Rigid {
    pub const ALL: [Rigid; 8] = [
        Rigid::None,
        Rigid::Turn90,
        Rigid::Turn180,
        Rigid::Turn270,
        Rigid::MirrorX,
        Rigid::MirrorY,
        Rigid::MirrorMain,
        Rigid::MirrorAnti,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Rigid::None => "as it is",
            Rigid::Turn90 => "turned 90",
            Rigid::Turn180 => "turned 180",
            Rigid::Turn270 => "turned 270",
            Rigid::MirrorX => "mirrored left-right",
            Rigid::MirrorY => "mirrored top-bottom",
            Rigid::MirrorMain => "mirrored on the main diagonal",
            Rigid::MirrorAnti => "mirrored on the anti-diagonal",
        }
    }

    /// Where the tile at `(x, y)` ends up. `last` is the highest coordinate,
    /// so `side - 1`.
    fn apply(self, x: u32, y: u32, last: u32) -> (u32, u32) {
        match self {
            Rigid::None => (x, y),
            Rigid::Turn90 => (last - y, x),
            Rigid::Turn180 => (last - x, last - y),
            Rigid::Turn270 => (y, last - x),
            Rigid::MirrorX => (last - x, y),
            Rigid::MirrorY => (x, last - y),
            Rigid::MirrorMain => (y, x),
            Rigid::MirrorAnti => (last - y, last - x),
        }
    }
}

impl Grid {
    /// This grid turned or mirrored.
    pub fn transformed(&self, motion: Rigid) -> Grid {
        let side = self.side();
        let last = side.saturating_sub(1);
        let mut out = Grid::solid(side);
        for y in 0..side {
            for x in 0..side {
                let (tx, ty) = motion.apply(x, y, last);
                out.set_wall(tx, ty, self.is_wall(x, y));
            }
        }
        out
    }
}

/// How many single steps of one kind it takes to make two cells equal.
///
/// A step is the smallest move of that kind: flip one tile, turn a quarter,
/// mirror once, slide one tile. Degree 0 means they already match.
///
/// Turns, mirrors and walks all run out quickly. Any turn is at most two
/// quarter-turns away once you may turn either direction; any member of the
/// turn-and-mirror family is at most two reflections away; and on a three by
/// three that wraps, no slide is more than one step in each direction. So
/// those three stop moving at degree 2. Only tiles keep climbing, up to the
/// eight that can differ.
pub const MAX_DEGREE: usize = 4;

/// The share of cells reachable within each degree, from 0 up to
/// [`MAX_DEGREE`]. It never decreases along the way: allowing more steps can
/// only let more cells through.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ladder {
    pub within: [f32; MAX_DEGREE + 1],
}

impl Ladder {
    /// The share already equal with no steps at all.
    pub fn identical(&self) -> f32 {
        self.within[0]
    }

    /// The share this kind of step can account for at all.
    pub fn reach(&self) -> f32 {
        self.within[MAX_DEGREE]
    }
}

/// Every sense in which two units were found to be alike, each 0 to 100.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Report {
    /// Tiles that are the same thing in the same place.
    pub tiles: f32,
    /// The best `tiles` score over all eight turns and mirrors, and which one
    /// reached it. Never below `tiles`, because "as it is" is one of the
    /// eight.
    pub turned: f32,
    pub motion: Rigid,
    /// How many tiles have to be flipped before each cell matches. `None`
    /// for a unit smaller than one cell.
    ///
    /// These four, and not the whole-unit figures below them, are what a
    /// player has a chance of noticing. A cell is a shape, and people
    /// recognise a shape they have seen before even when it has been turned
    /// round or slid over. A tile count across a whole unit is not a shape;
    /// it is an accident of where walls happened to fall.
    pub cell_tiles: Option<Ladder>,
    /// How many quarter-turns, either direction, before each cell matches.
    pub cell_turns: Option<Ladder>,
    /// How many reflections before each cell matches. Two reflections make a
    /// turn, which is why this reaches exactly as far as the turn ladder.
    pub cell_mirrors: Option<Ladder>,
    /// How many single-tile slides, wrapping round the edges, before each
    /// cell matches.
    ///
    /// A slid cell is often not a legal cell at all, because sliding moves
    /// the centre tile and a walled centre is illegal. It still happens: 274
    /// pairs of legal cells are slides of each other, slightly more than the
    /// 227 related by a turn.
    pub cell_walks: Option<Ladder>,
    /// The best of the four for each cell: the fewest steps of any kind.
    pub cell_any: Option<Ladder>,
    /// Overlap of the two local-pattern mixes — how far the place is made of
    /// the same small shapes, wherever those sit. Position-blind.
    pub texture: f32,
    /// How close the two are in sheer quantity of open space, ignoring where
    /// any of it is.
    pub openness: f32,
    /// How close the two are in number of independent loops: whether the walk
    /// through them offers the same number of ways round.
    pub loops: f32,
}

impl Report {
    /// The figures in the order they are worth reading, with a label for each.
    /// `None` where a figure does not apply to this size.
    /// The four kinds of step, plus the best of them. Read these first.
    pub fn patterns(&self) -> [(&'static str, Option<Ladder>); 5] {
        [
            ("tiles flipped", self.cell_tiles),
            ("quarter-turns", self.cell_turns),
            ("mirrors", self.cell_mirrors),
            ("walks", self.cell_walks),
            ("fewest of any kind", self.cell_any),
        ]
    }

    /// Bulk measures over the whole unit. They move smoothly and are good for
    /// calibration, but none of them is a shape, so none is a thing anyone
    /// could point at and say they had seen it before.
    pub fn background(&self) -> [(&'static str, f32); 5] {
        [
            ("tiles in place", self.tiles),
            ("whole unit, best turn", self.turned),
            ("texture", self.texture),
            ("amount of open space", self.openness),
            ("loops to walk", self.loops),
        ]
    }
}

/// Compare two units of the same size.
///
/// Grids of different sizes have nothing to compare, so every figure comes
/// back 0.
pub fn compare(a: &Grid, b: &Grid) -> Report {
    if a.side() != b.side() || a.side() == 0 {
        return Report {
            tiles: 0.0,
            turned: 0.0,
            motion: Rigid::None,
            cell_tiles: None,
            cell_turns: None,
            cell_mirrors: None,
            cell_walks: None,
            cell_any: None,
            texture: 0.0,
            openness: 0.0,
            loops: 0.0,
        };
    }

    let mut turned = 0.0f32;
    let mut motion = Rigid::None;
    for candidate in Rigid::ALL {
        let score = tile_agreement(a, &b.transformed(candidate));
        if score > turned {
            turned = score;
            motion = candidate;
        }
    }

    let ladders = cell_ladders(a, b);

    Report {
        tiles: tile_agreement(a, b),
        turned,
        motion,
        cell_tiles: ladders.as_ref().map(|l| l.tiles),
        cell_turns: ladders.as_ref().map(|l| l.turns),
        cell_mirrors: ladders.as_ref().map(|l| l.mirrors),
        cell_walks: ladders.as_ref().map(|l| l.walks),
        cell_any: ladders.as_ref().map(|l| l.any),
        texture: texture_overlap(a, b),
        openness: openness_agreement(a, b),
        loops: loop_agreement(a, b),
    }
}

/// Share of tiles that are the same thing in the same place.
fn tile_agreement(a: &Grid, b: &Grid) -> f32 {
    let side = a.side();
    let mut same = 0u64;
    for y in 0..side {
        for x in 0..side {
            if a.is_wall(x, y) == b.is_wall(x, y) {
                same += 1;
            }
        }
    }
    percent(same, u64::from(side) * u64::from(side))
}

/// The nine tiles of the cell whose top-left corner is `(ox, oy)`, packed the
/// way `cells.rs` packs them: bit `i` is the tile at `(i % 3, i / 3)`, set
/// meaning wall.
fn cell_bits(grid: &Grid, ox: u32, oy: u32) -> u16 {
    let mut bits = 0u16;
    for i in 0..9u32 {
        if grid.is_wall(ox + i % 3, oy + i / 3) {
            bits |= 1 << i;
        }
    }
    bits
}

/// Those same nine tiles, turned or mirrored.
///
/// The generator uses this too, so the thing that changes a cell and the
/// thing that measures the change cannot drift apart.
pub fn transformed_cell(bits: u16, motion: Rigid) -> u16 {
    let mut out = 0u16;
    for i in 0..9u32 {
        if bits & (1 << i) != 0 {
            let (x, y) = motion.apply(i % 3, i / 3, 2);
            out |= 1 << (y * 3 + x);
        }
    }
    out
}

/// The turns that are not simply leaving the cell alone.
const TURNS: [Rigid; 3] = [Rigid::Turn90, Rigid::Turn180, Rigid::Turn270];

/// Every way a cell can be mirrored.
const MIRRORS: [Rigid; 4] = [
    Rigid::MirrorX,
    Rigid::MirrorY,
    Rigid::MirrorMain,
    Rigid::MirrorAnti,
];

/// Those same nine tiles slid over by `(dx, dy)`, wrapping round the edges.
///
/// The result is often not a legal cell, because sliding moves the centre
/// tile and a walled centre is illegal. Callers that mean to keep a cell
/// usable must check with [`crate::cells::is_legal`] before taking it.
pub fn slid_cell(bits: u16, dx: u32, dy: u32) -> u16 {
    let mut out = 0u16;
    for i in 0..9u32 {
        if bits & (1 << i) != 0 {
            let x = (i % 3 + dx) % 3;
            let y = (i / 3 + dy) % 3;
            out |= 1 << (y * 3 + x);
        }
    }
    out
}

/// The fewest steps of each kind that would make one cell equal to another.
/// `usize::MAX` where no number of steps of that kind ever would.
#[derive(Clone, Copy)]
struct CellDegrees {
    tiles: usize,
    turns: usize,
    mirrors: usize,
    walks: usize,
}

impl CellDegrees {
    fn best(&self) -> usize {
        self.tiles.min(self.turns).min(self.mirrors).min(self.walks)
    }
}

/// How far apart one pair of cells is, in each kind of step.
fn cell_degrees(left: u16, right: u16) -> CellDegrees {
    let tiles = (left ^ right).count_ones() as usize;

    // Turning either way, so three quarter-turns clockwise counts as one.
    let mut turns = usize::MAX;
    let mut rotated = right;
    for quarters in 0..4usize {
        if rotated == left {
            turns = turns.min(quarters.min(4 - quarters) % 4);
        }
        rotated = transformed_cell(rotated, Rigid::Turn90);
    }

    // One reflection is a reflection; two make a turn. Anything outside the
    // turn-and-mirror family stays out of reach however many you use.
    let mirrors = if left == right {
        0
    } else if MIRRORS.iter().any(|&m| transformed_cell(right, m) == left) {
        1
    } else if TURNS.iter().any(|&m| transformed_cell(right, m) == left) {
        2
    } else {
        usize::MAX
    };

    // Sliding wraps, so two along is one step back the other way.
    let mut walks = usize::MAX;
    for dy in 0..3u32 {
        for dx in 0..3u32 {
            if slid_cell(right, dx, dy) == left {
                let steps = (dx.min(3 - dx) + dy.min(3 - dy)) as usize;
                walks = walks.min(steps);
            }
        }
    }

    CellDegrees {
        tiles,
        turns,
        mirrors,
        walks,
    }
}

/// One ladder per kind of step, over every cell of the two units.
struct Ladders {
    tiles: Ladder,
    turns: Ladder,
    mirrors: Ladder,
    walks: Ladder,
    any: Ladder,
}

/// `None` when the unit is smaller than a single cell and so has none.
fn cell_ladders(a: &Grid, b: &Grid) -> Option<Ladders> {
    let side = a.side();
    if side < 3 {
        return None;
    }
    let across = side / 3;
    let total = u64::from(across) * u64::from(across);

    // counts[kind][degree]: how many cells need exactly that many steps.
    let mut counts = [[0u64; MAX_DEGREE + 1]; 5];
    for cy in 0..across {
        for cx in 0..across {
            let degrees = cell_degrees(cell_bits(a, cx * 3, cy * 3), cell_bits(b, cx * 3, cy * 3));
            let each = [
                degrees.tiles,
                degrees.turns,
                degrees.mirrors,
                degrees.walks,
                degrees.best(),
            ];
            for (kind, steps) in each.into_iter().enumerate() {
                if steps <= MAX_DEGREE {
                    counts[kind][steps] += 1;
                }
            }
        }
    }

    // Running totals, so `within[d]` answers "within d steps" rather than "at
    // exactly d steps", which is the question anyone actually asks.
    let ladder = |row: [u64; MAX_DEGREE + 1]| {
        let mut within = [0.0f32; MAX_DEGREE + 1];
        let mut running = 0u64;
        for (degree, count) in row.into_iter().enumerate() {
            running += count;
            within[degree] = percent(running, total);
        }
        Ladder { within }
    };

    Some(Ladders {
        tiles: ladder(counts[0]),
        turns: ladder(counts[1]),
        mirrors: ladder(counts[2]),
        walks: ladder(counts[3]),
        any: ladder(counts[4]),
    })
}

/// Count of every 3x3 neighbourhood shape in the grid. Outside counts as
/// wall, as it does everywhere else.
fn pattern_histogram(grid: &Grid) -> Vec<u64> {
    let mut counts = vec![0u64; 512];
    for y in 0..grid.side() {
        for x in 0..grid.side() {
            let mut code = 0usize;
            for i in 0..9u32 {
                // Wrapping is deliberate: at x = 0 this reads a very large
                // coordinate, which is outside the grid, which is wall.
                let sx = x.wrapping_add(i % 3).wrapping_sub(1);
                let sy = y.wrapping_add(i / 3).wrapping_sub(1);
                if grid.is_wall(sx, sy) {
                    code |= 1 << i;
                }
            }
            counts[code] += 1;
        }
    }
    counts
}

/// How much of the local-pattern mix the two share, regardless of where any
/// of it sits.
fn texture_overlap(a: &Grid, b: &Grid) -> f32 {
    let left = pattern_histogram(a);
    let right = pattern_histogram(b);
    let shared: u64 = left.iter().zip(right.iter()).map(|(x, y)| *x.min(y)).sum();
    percent(shared, left.iter().sum::<u64>())
}

fn openness_agreement(a: &Grid, b: &Grid) -> f32 {
    let total = u64::from(a.side()) * u64::from(a.side());
    let gap = a.open_tiles().abs_diff(b.open_tiles());
    percent(total.saturating_sub(gap), total)
}

/// Independent loops in the open space: edges minus tiles plus one, the
/// cyclomatic number of the graph the player walks.
///
/// The plus one assumes the open space is a single connected piece, which the
/// generator guarantees by construction and `Grid::is_fully_connected`
/// measures rather than assumes.
fn independent_loops(grid: &Grid) -> u64 {
    let side = grid.side();
    let mut tiles = 0i64;
    let mut edges = 0i64;
    for y in 0..side {
        for x in 0..side {
            if grid.is_wall(x, y) {
                continue;
            }
            tiles += 1;
            if !grid.is_wall(x + 1, y) {
                edges += 1;
            }
            if !grid.is_wall(x, y + 1) {
                edges += 1;
            }
        }
    }
    if tiles == 0 {
        return 0;
    }
    (edges - tiles + 1).max(0) as u64
}

fn loop_agreement(a: &Grid, b: &Grid) -> f32 {
    let left = independent_loops(a);
    let right = independent_loops(b);
    let scale = left.max(right);
    if scale == 0 {
        // Neither offers a way round, which makes them the same in this sense.
        return 100.0;
    }
    percent(scale - left.abs_diff(right), scale)
}

fn percent(part: u64, whole: u64) -> f32 {
    if whole == 0 {
        return 0.0;
    }
    (part as f64 * 100.0 / whole as f64) as f32
}

/// The most a single cell is pushed while searching for a target similarity.
///
/// Past this the walk has nothing left to find. A cell has at most eight legal
/// neighbours and the walk can retrace itself, so pushing one cell forever
/// stops making it more different: by twenty steps a unit has drifted as far
/// from where it started as an unrelated unit already is, which is as far as
/// "similar" goes. `nudging_far_enough_is_as_good_as_a_stranger` holds that
/// down.
const MAX_STEPS_PER_CELL: u32 = 20;

/// Build nudges that make the unit roughly `target` percent the same, by tile,
/// as the untouched one. Returns them together with the agreement actually
/// reached, which is the figure worth believing.
///
/// The search bisects over how many pushes to apply from one fixed random
/// order, so more pushes only ever means further away, and the whole thing is
/// reproducible from `seed`. It is approximate on purpose: pushes can undo
/// each other and corridors are carved over the top of them, so no exact
/// number of pushes corresponds to an exact percentage. Below some floor —
/// set by how many cells there are to push — no number of pushes will reach
/// the target, and the returned figure says so.
pub fn nudges_for_tile_similarity(
    level: Level,
    params: &Params,
    target: f32,
    seed: u64,
    kinds: Kinds,
) -> (Nudges, f32) {
    let base = generate_with(level, params, &Nudges::new());
    let order = push_order(level, seed, kinds);

    // A unit smaller than a cell has nothing to push.
    if order.is_empty() {
        return (Nudges::new(), 100.0);
    }

    let agreement_after = |count: usize| {
        let mut nudges = Nudges::new();
        for &(x, y, kind) in order.iter().take(count) {
            nudges.apply(x, y, kind);
        }
        let grid = generate_with(level, params, &nudges);
        let score = tile_agreement(&base, &grid);
        (nudges, score)
    };

    let mut low = 0usize;
    let mut high = order.len();
    while low < high {
        let mid = low + (high - low) / 2;
        if agreement_after(mid).1 <= target {
            high = mid;
        } else {
            low = mid + 1;
        }
    }

    // `low` is the first count at or below the target; the one before it sits
    // just above. Whichever is nearer to what was asked is the answer.
    let at = agreement_after(low);
    if low == 0 {
        return at;
    }
    let before = agreement_after(low - 1);
    if (before.1 - target).abs() < (at.1 - target).abs() {
        before
    } else {
        at
    }
}

/// Every cell of the unit in a reproducible shuffled order, repeated so that a
/// cell can be pushed again before every other cell has been pushed once.
fn push_order(level: Level, seed: u64, kinds: Kinds) -> Vec<(u32, u32, Kind)> {
    let side = level.side();
    if side < 3 {
        return Vec::new();
    }
    let across = side / 3;
    let mut cells: Vec<(u32, u32)> = (0..across)
        .flat_map(|cy| (0..across).map(move |cx| (cx * 3, cy * 3)))
        .collect();

    let mut state = seed ^ 0x51ED_2701_A5C3_9B4F;
    let mut next = move || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };

    // A fresh shuffle each round, so later rounds scatter their pushes
    // differently instead of walking the same cells in the same order.
    let allowed = kinds.list();
    if allowed.is_empty() {
        return Vec::new();
    }

    let mut order = Vec::with_capacity(cells.len() * MAX_STEPS_PER_CELL as usize);
    for _ in 0..MAX_STEPS_PER_CELL {
        for i in (1..cells.len()).rev() {
            let j = (next() % (i as u64 + 1)) as usize;
            cells.swap(i, j);
        }
        for &(x, y) in &cells {
            let mut kind = allowed[(next() % allowed.len() as u64) as usize];
            if let Kind::Slide(_, _) = kind {
                // Which way it slides is decided here, so the order stays the
                // one fixed sequence the bisection needs. Only single-step
                // slides, so every entry in the order costs exactly one step.
                kind = Kind::SINGLE_SLIDES[(next() % 4) as usize];
            }
            order.push((x, y, kind));
        }
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate;

    fn unit(level: Level, seed: u64) -> Grid {
        generate(
            level,
            &Params {
                seed,
                ..Params::default()
            },
        )
    }

    /// The same unit with every one of its cells put through `change`, so a
    /// step can be tested cell by cell rather than on the whole square.
    fn per_cell(grid: &Grid, change: impl Fn(u16) -> u16) -> Grid {
        let mut out = Grid::solid(grid.side());
        let across = grid.side() / 3;
        for cy in 0..across {
            for cx in 0..across {
                let bits = change(cell_bits(grid, cx * 3, cy * 3));
                for i in 0..9u32 {
                    out.set_wall(cx * 3 + i % 3, cy * 3 + i / 3, bits & (1 << i) != 0);
                }
            }
        }
        out
    }

    fn ladders_of(report: &Report) -> Vec<(&str, Ladder)> {
        report
            .patterns()
            .into_iter()
            .filter_map(|(label, ladder)| ladder.map(|l| (label, l)))
            .collect()
    }

    // ---- the degree ladders -------------------------------------------

    #[test]
    fn a_unit_needs_no_steps_to_equal_itself() {
        let grid = unit(Level::House, 7);
        let report = compare(&grid, &grid);
        for (label, ladder) in ladders_of(&report) {
            assert_eq!(
                ladder.identical(),
                100.0,
                "{label} wanted steps to match a unit with itself"
            );
        }
        for (label, value) in report.background() {
            assert!(
                (value - 100.0).abs() < 0.001,
                "{label} scored {value} comparing a unit with itself"
            );
        }
    }

    #[test]
    fn one_tile_flipped_is_one_step() {
        let grid = unit(Level::House, 41);
        let mut nudged = grid.clone();
        nudged.set_wall(4, 4, !grid.is_wall(4, 4));

        let tiles = compare(&grid, &nudged)
            .cell_tiles
            .expect("a house has cells");
        assert_eq!(
            tiles.within[1], 100.0,
            "one flipped tile should put every cell within one step"
        );
        assert!(
            tiles.identical() < 100.0,
            "the flipped cell should not count as already identical"
        );
    }

    #[test]
    fn a_quarter_turn_is_one_step_and_a_half_turn_is_two() {
        let grid = unit(Level::House, 17);
        for (motion, expected) in [(Rigid::Turn90, 1), (Rigid::Turn180, 2), (Rigid::Turn270, 1)] {
            let other = per_cell(&grid, |bits| transformed_cell(bits, motion));
            let turns = compare(&grid, &other)
                .cell_turns
                .expect("a house has cells");
            assert_eq!(
                turns.within[expected],
                100.0,
                "{} should be {expected} step(s), but the ladder reads {:?}",
                motion.name(),
                turns.within
            );
        }
    }

    #[test]
    fn a_mirror_is_one_step() {
        let grid = unit(Level::House, 19);
        for motion in MIRRORS {
            let other = per_cell(&grid, |bits| transformed_cell(bits, motion));
            let mirrors = compare(&grid, &other)
                .cell_mirrors
                .expect("a house has cells");
            assert_eq!(
                mirrors.within[1],
                100.0,
                "{} should be one step, but the ladder reads {:?}",
                motion.name(),
                mirrors.within
            );
        }
    }

    #[test]
    fn a_walk_costs_one_step_per_direction_moved() {
        let grid = unit(Level::House, 23);
        for step in 1..9u32 {
            let (dx, dy) = (step % 3, step / 3);
            // Sliding wraps, so two along is one step back the other way.
            let expected = (dx.min(3 - dx) + dy.min(3 - dy)) as usize;
            let other = per_cell(&grid, |bits| slid_cell(bits, dx, dy));
            let walks = compare(&grid, &other)
                .cell_walks
                .expect("a house has cells");
            assert_eq!(
                walks.within[expected], 100.0,
                "a walk of ({dx}, {dy}) should be {expected} step(s), ladder reads {:?}",
                walks.within
            );
        }
    }

    #[test]
    fn a_ladder_never_goes_down() {
        for seed in 0..10u64 {
            let a = unit(Level::House, seed);
            let b = unit(Level::House, seed + 60);
            for (label, ladder) in ladders_of(&compare(&a, &b)) {
                for degree in 1..=MAX_DEGREE {
                    assert!(
                        ladder.within[degree] >= ladder.within[degree - 1],
                        "{label} fell from {} to {} between degree {} and {degree}",
                        ladder.within[degree - 1],
                        ladder.within[degree],
                        degree - 1
                    );
                }
            }
        }
    }

    #[test]
    fn turns_mirrors_and_walks_run_out_at_two_steps() {
        // The doc comment on MAX_DEGREE claims these three cannot use a third
        // step. If that were wrong, the last columns of the table would be
        // quietly meaningless.
        for seed in 0..10u64 {
            let a = unit(Level::House, seed);
            let b = unit(Level::House, seed + 70);
            let report = compare(&a, &b);
            for (label, ladder) in [
                ("turns", report.cell_turns),
                ("mirrors", report.cell_mirrors),
                ("walks", report.cell_walks),
            ] {
                let ladder = ladder.expect("a house has cells");
                for degree in 3..=MAX_DEGREE {
                    assert_eq!(
                        ladder.within[degree], ladder.within[2],
                        "{label} moved at degree {degree}: {:?}",
                        ladder.within
                    );
                }
            }
        }
    }

    #[test]
    fn the_best_of_the_four_is_never_worse_than_any_one_of_them() {
        for seed in 0..10u64 {
            let a = unit(Level::House, seed);
            let b = unit(Level::House, seed + 80);
            let report = compare(&a, &b);
            let any = report.cell_any.expect("a house has cells");
            for (label, ladder) in ladders_of(&report) {
                for degree in 0..=MAX_DEGREE {
                    assert!(
                        any.within[degree] >= ladder.within[degree] - 0.001,
                        "{label} reached {} at degree {degree} but the best of all four only {}",
                        ladder.within[degree],
                        any.within[degree]
                    );
                }
            }
        }
    }

    #[test]
    fn every_figure_stays_inside_nought_to_a_hundred() {
        for seed in 0..12u64 {
            let a = unit(Level::Room, seed);
            let b = unit(Level::Room, seed + 100);
            let report = compare(&a, &b);
            for (label, ladder) in ladders_of(&report) {
                for (degree, value) in ladder.within.into_iter().enumerate() {
                    assert!(
                        (0.0..=100.0).contains(&value),
                        "{label} returned {value} at degree {degree}"
                    );
                }
            }
            for (label, value) in report.background() {
                assert!(
                    (0.0..=100.0).contains(&value),
                    "{label} returned {value}, which is not a percentage"
                );
            }
        }
    }

    #[test]
    fn walking_all_the_way_round_comes_back() {
        let grid = unit(Level::Room, 29);
        let there_and_back = per_cell(&grid, |bits| {
            slid_cell(slid_cell(slid_cell(bits, 1, 2), 1, 2), 1, 2)
        });
        let any = compare(&grid, &there_and_back)
            .cell_any
            .expect("a room has cells");
        assert_eq!(any.identical(), 100.0);
    }

    #[test]
    fn walking_never_changes_how_much_is_open() {
        let grid = unit(Level::House, 31);
        for step in 0..9u32 {
            let walked = per_cell(&grid, |bits| slid_cell(bits, step % 3, step / 3));
            assert_eq!(grid.open_tiles(), walked.open_tiles());
        }
    }

    #[test]
    fn a_shape_is_noticed_where_a_tile_count_is_not() {
        // Turn every cell in place. The whole-unit tile figure falls away
        // while the turn ladder still reads 100 at one step. That gap is the
        // entire reason these are separate figures.
        let grid = unit(Level::House, 37);
        let turned = per_cell(&grid, |bits| transformed_cell(bits, Rigid::Turn90));
        let report = compare(&grid, &turned);
        assert_eq!(
            report.cell_turns.expect("a house has cells").within[1],
            100.0
        );
        assert!(
            report.tiles < 95.0,
            "tiles scored {}, so the tile count saw the turn after all",
            report.tiles
        );
    }

    #[test]
    fn different_sizes_have_nothing_to_compare() {
        let report = compare(&unit(Level::Room, 1), &unit(Level::House, 1));
        assert_eq!(report.tiles, 0.0);
        assert!(report.cell_any.is_none());
    }

    // ---- whole-unit motions -------------------------------------------

    #[test]
    fn a_turned_unit_is_recognised_as_turned() {
        for motion in Rigid::ALL {
            let grid = unit(Level::House, 11);
            let report = compare(&grid, &grid.transformed(motion));
            assert!(
                (report.turned - 100.0).abs() < 0.001,
                "{} was not recognised: best turn scored {}",
                motion.name(),
                report.turned
            );
        }
    }

    #[test]
    fn turning_is_reported_but_never_hidden() {
        // A turned unit really does differ tile by tile. If `tiles` quietly
        // took the best orientation, the two could not be told apart.
        let grid = unit(Level::House, 3);
        let report = compare(&grid, &grid.transformed(Rigid::Turn90));
        assert!(report.turned >= report.tiles);
        assert!(
            report.tiles < 99.0,
            "a turned unit scored {} on raw tiles, so turning was absorbed",
            report.tiles
        );
    }

    #[test]
    fn four_quarter_turns_come_back_to_the_start() {
        let grid = unit(Level::Room, 5);
        let mut turned = grid.clone();
        for _ in 0..4 {
            turned = turned.transformed(Rigid::Turn90);
        }
        assert!((compare(&grid, &turned).tiles - 100.0).abs() < 0.001);
    }

    #[test]
    fn turning_never_changes_how_much_is_open() {
        let grid = unit(Level::House, 13);
        for motion in Rigid::ALL {
            assert_eq!(
                grid.open_tiles(),
                grid.transformed(motion).open_tiles(),
                "{} changed the amount of open space",
                motion.name()
            );
        }
    }

    #[test]
    fn an_unrelated_unit_scores_near_the_chance_floor() {
        // Two unrelated units agree wherever wall happens to meet wall, which
        // is most of the grid. The floor is nowhere near zero, and reading
        // these percentages without knowing that would be reading them wrong.
        let a = unit(Level::House, 1);
        let b = unit(Level::House, 999);
        let tiles = compare(&a, &b).tiles;
        assert!(
            (30.0..90.0).contains(&tiles),
            "unrelated units scored {tiles} on tiles, outside the expected band"
        );
    }

    // ---- building a unit to order --------------------------------------

    #[test]
    fn asking_for_an_identical_unit_changes_nothing() {
        let params = Params::default();
        let (nudges, reached) =
            nudges_for_tile_similarity(Level::House, &params, 100.0, 4, Kinds::FLIPS_ONLY);
        assert!(nudges.is_empty());
        assert!((reached - 100.0).abs() < 0.001);
    }

    #[test]
    fn the_search_lands_near_what_was_asked() {
        let params = Params::default();
        for target in [97.0f32, 94.0, 90.0, 85.0] {
            let (_, reached) =
                nudges_for_tile_similarity(Level::House, &params, target, 4, Kinds::FLIPS_ONLY);
            assert!(
                (reached - target).abs() <= 3.0,
                "asked for {target}%, reached {reached}%"
            );
        }
    }

    #[test]
    fn asking_for_less_similar_never_gives_more_similar() {
        let params = Params::default();
        let mut previous = 100.0f32;
        for target in [95.0f32, 90.0, 85.0, 80.0, 75.0, 70.0] {
            let (_, reached) =
                nudges_for_tile_similarity(Level::House, &params, target, 9, Kinds::FLIPS_ONLY);
            assert!(
                reached <= previous + 0.001,
                "target {target}% reached {reached}%, above the previous {previous}%"
            );
            previous = reached;
        }
    }

    #[test]
    fn the_search_gives_the_same_answer_twice() {
        let params = Params::default();
        let first = nudges_for_tile_similarity(Level::House, &params, 88.0, 21, Kinds::FLIPS_ONLY);
        let second = nudges_for_tile_similarity(Level::House, &params, 88.0, 21, Kinds::FLIPS_ONLY);
        assert_eq!(first.0, second.0);
        assert_eq!(first.1, second.1);
    }

    #[test]
    fn a_unit_with_no_cells_cannot_be_nudged() {
        let params = Params::default();
        let (nudges, reached) =
            nudges_for_tile_similarity(Level::Tile, &params, 50.0, 1, Kinds::FLIPS_ONLY);
        assert!(nudges.is_empty());
        assert_eq!(reached, 100.0);
    }

    #[test]
    fn what_the_search_builds_really_is_that_similar() {
        // The search reports a figure; this checks the figure describes the
        // grid you actually get back out of the generator with those nudges.
        let params = Params::default();
        let (nudges, reached) =
            nudges_for_tile_similarity(Level::House, &params, 85.0, 2, Kinds::FLIPS_ONLY);
        let base = generate(Level::House, &params);
        let built = generate_with(Level::House, &params, &nudges);
        assert!((compare(&base, &built).tiles - reached).abs() < 0.001);
    }

    #[test]
    fn nudging_far_enough_is_as_good_as_a_stranger() {
        // Pushing every cell as far as the walk goes should leave the unit no
        // more recognisable than one generated from an unrelated seed. If this
        // drifts apart, the mutation mechanic has a ceiling on how wrong it can
        // make a place, and that ceiling belongs in the UI rather than here.
        let params = Params::default();
        let base = generate(Level::House, &params);
        let side = Level::House.side();

        let mut nudges = Nudges::new();
        for _ in 0..MAX_STEPS_PER_CELL {
            for cy in 0..side / 3 {
                for cx in 0..side / 3 {
                    nudges.push(cx * 3, cy * 3);
                }
            }
        }
        let drifted = compare(&base, &generate_with(Level::House, &params, &nudges)).tiles;
        let stranger = compare(
            &base,
            &generate(
                Level::House,
                &Params {
                    seed: 999,
                    ..params
                },
            ),
        )
        .tiles;

        assert!(
            drifted - stranger < 6.0,
            "a fully drifted unit scored {drifted}% but a stranger scores {stranger}%"
        );
    }
}
