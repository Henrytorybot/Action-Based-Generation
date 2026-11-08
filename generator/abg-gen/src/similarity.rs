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

/// The four walks: one tile along one axis, wrapping round the cell.
///
/// A slide along both axes at once is two walks, not one, so it is not here.
const WALKS: [(u32, u32); 4] = [(1, 0), (2, 0), (0, 1), (0, 2)];

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
    /// Cells that are already equal, needing nothing done to them. `None`
    /// for a unit smaller than one cell.
    ///
    /// This and the four below it are what a player has a chance of
    /// noticing. A cell is a shape, and people recognise a shape they have
    /// seen before even when it has been turned round or slid over. A tile
    /// count across a whole unit is not a shape; it is an accident of where
    /// walls happened to fall.
    ///
    /// Only the first degree is counted. Two cells are called alike when one
    /// single thing would make them equal, and otherwise they are not alike
    /// at all. Each figure below includes the cells that were already equal,
    /// so the five of them nest rather than crossing over.
    pub cells: Option<f32>,
    /// Cells equal after changing at most one tile.
    pub cells_one_tile: Option<f32>,
    /// Cells equal after one rotation, of a quarter, a half or three
    /// quarters. Any multiple of ninety degrees is one rotation; a half turn
    /// is not counted as two quarter ones.
    pub cells_one_turn: Option<f32>,
    /// Cells equal after mirroring on one axis through the middle, at
    /// forty-five degree increments: across, down, or either diagonal.
    pub cells_one_mirror: Option<f32>,
    /// Cells equal after walking one tile along one axis, wrapping round the
    /// cell edges.
    ///
    /// A walked cell is often not a legal cell at all, because walking moves
    /// the centre tile and a walled centre is illegal. It still happens: 274
    /// pairs of legal cells are walks of each other, slightly more than the
    /// 227 related by a rotation.
    pub cells_one_walk: Option<f32>,
    /// Cells alike by any of the four: one single thing would make them
    /// equal. Anything beyond that is not alike at all.
    pub cells_similar: Option<f32>,
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
    /// The ways one cell can be the same shape as another. Read these first.
    pub fn patterns(&self) -> [(&'static str, Option<f32>); 6] {
        [
            ("already equal", self.cells),
            ("one tile changed", self.cells_one_tile),
            ("one rotation", self.cells_one_turn),
            ("one mirror", self.cells_one_mirror),
            ("one walk", self.cells_one_walk),
            ("alike at all", self.cells_similar),
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
            cells: None,
            cells_one_tile: None,
            cells_one_turn: None,
            cells_one_mirror: None,
            cells_one_walk: None,
            cells_similar: None,
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

    let tally = classify_cells(a, b);

    Report {
        tiles: tile_agreement(a, b),
        turned,
        motion,
        cells: tally.map(|t| percent(t.identical, t.total)),
        cells_one_tile: tally.map(|t| percent(t.one_tile, t.total)),
        cells_one_turn: tally.map(|t| percent(t.one_turn, t.total)),
        cells_one_mirror: tally.map(|t| percent(t.one_mirror, t.total)),
        cells_one_walk: tally.map(|t| percent(t.one_walk, t.total)),
        cells_similar: tally.map(|t| percent(t.similar, t.total)),
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

/// How two units line up cell by cell, counted every way one single thing
/// could make a pair of cells equal.
#[derive(Clone, Copy)]
struct CellTally {
    identical: u64,
    one_tile: u64,
    one_turn: u64,
    one_mirror: u64,
    one_walk: u64,
    similar: u64,
    total: u64,
}

/// `None` when the unit is smaller than a single cell and so has none.
fn classify_cells(a: &Grid, b: &Grid) -> Option<CellTally> {
    let side = a.side();
    if side < 3 {
        return None;
    }
    let across = side / 3;
    let mut tally = CellTally {
        identical: 0,
        one_tile: 0,
        one_turn: 0,
        one_mirror: 0,
        one_walk: 0,
        similar: 0,
        total: u64::from(across) * u64::from(across),
    };

    for cy in 0..across {
        for cx in 0..across {
            let left = cell_bits(a, cx * 3, cy * 3);
            let right = cell_bits(b, cx * 3, cy * 3);

            // Each figure allows doing nothing as well, so the five nest:
            // already equal is below each of the others, and all are below
            // alike at all. Without that, a unit compared with itself would
            // read 100 on the first and almost nothing on the rest, which
            // looks broken rather than informative.
            let identical = left == right;
            let one_tile = (left ^ right).count_ones() <= 1;
            let one_turn = identical || TURNS.iter().any(|&m| transformed_cell(right, m) == left);
            let one_mirror =
                identical || MIRRORS.iter().any(|&m| transformed_cell(right, m) == left);
            let one_walk = identical
                || WALKS
                    .iter()
                    .any(|&(dx, dy)| slid_cell(right, dx, dy) == left);

            tally.identical += u64::from(identical);
            tally.one_tile += u64::from(one_tile);
            tally.one_turn += u64::from(one_turn);
            tally.one_mirror += u64::from(one_mirror);
            tally.one_walk += u64::from(one_walk);
            tally.similar += u64::from(one_tile || one_turn || one_mirror || one_walk);
        }
    }
    Some(tally)
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

    // ---- first-degree likeness ----------------------------------------

    #[test]
    fn a_unit_is_already_equal_to_itself() {
        let grid = unit(Level::House, 7);
        let report = compare(&grid, &grid);
        for (label, value) in report.patterns() {
            assert_eq!(
                value,
                Some(100.0),
                "{label} did not read 100 comparing a unit with itself"
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
    fn one_tile_changed_is_alike_but_not_equal() {
        let grid = unit(Level::House, 41);
        let mut nudged = grid.clone();
        nudged.set_wall(4, 4, !grid.is_wall(4, 4));
        let report = compare(&grid, &nudged);

        assert_eq!(report.cells_one_tile, Some(100.0));
        assert_eq!(report.cells_similar, Some(100.0));
        assert!(
            report.cells.is_some_and(|v| v < 100.0),
            "the changed cell should not count as already equal"
        );
    }

    #[test]
    fn two_tiles_changed_is_not_alike_at_all() {
        // The whole point of stopping at the first degree: two changes is not
        // "slightly less similar", it is not similar.
        let grid = unit(Level::Cell, 12);
        let mut far = grid.clone();
        let mut changed = 0;
        for i in 0..9u32 {
            let (x, y) = (i % 3, i / 3);
            if (x, y) != (1, 1) && changed < 2 {
                far.set_wall(x, y, !grid.is_wall(x, y));
                changed += 1;
            }
        }
        assert_eq!(changed, 2);
        let report = compare(&grid, &far);
        assert_eq!(report.cells_one_tile, Some(0.0));
        assert_eq!(report.cells, Some(0.0));
    }

    #[test]
    fn every_rotation_is_one_rotation() {
        // A half turn is one rotation, not two quarter ones.
        let grid = unit(Level::House, 17);
        for motion in TURNS {
            let other = per_cell(&grid, |bits| transformed_cell(bits, motion));
            let report = compare(&grid, &other);
            assert_eq!(
                report.cells_one_turn,
                Some(100.0),
                "{} was not counted as one rotation",
                motion.name()
            );
            assert_eq!(report.cells_similar, Some(100.0));
        }
    }

    #[test]
    fn every_axis_through_the_middle_is_one_mirror() {
        let grid = unit(Level::House, 19);
        for motion in MIRRORS {
            let other = per_cell(&grid, |bits| transformed_cell(bits, motion));
            let report = compare(&grid, &other);
            assert_eq!(
                report.cells_one_mirror,
                Some(100.0),
                "{} was not counted as one mirror",
                motion.name()
            );
            assert_eq!(report.cells_similar, Some(100.0));
        }
    }

    #[test]
    fn a_walk_along_one_axis_is_one_walk() {
        let grid = unit(Level::House, 23);
        for (dx, dy) in WALKS {
            let other = per_cell(&grid, |bits| slid_cell(bits, dx, dy));
            let report = compare(&grid, &other);
            assert_eq!(
                report.cells_one_walk,
                Some(100.0),
                "a walk of ({dx}, {dy}) was not counted as one walk"
            );
        }
    }

    #[test]
    fn a_walk_along_both_axes_is_not_one_walk() {
        // Two walks is two things, so it is not alike at the first degree.
        // If this ever passed, "one walk" would quietly mean "up to two".
        let grid = unit(Level::Cell, 31);
        let diagonal = per_cell(&grid, |bits| slid_cell(bits, 1, 1));
        let report = compare(&grid, &diagonal);
        if report.cells == Some(0.0) {
            assert_eq!(
                report.cells_one_walk,
                Some(0.0),
                "a diagonal walk was counted as a single walk"
            );
        }
    }

    #[test]
    fn the_five_figures_nest() {
        // Each allows everything "already equal" allows and more, and all of
        // them are below "alike at all". Numbers that crossed would make the
        // screen lie about what allowing a rotation buys you.
        for seed in 0..12u64 {
            let a = unit(Level::House, seed);
            let b = unit(Level::House, seed + 60);
            let r = compare(&a, &b);
            let (Some(equal), Some(any)) = (r.cells, r.cells_similar) else {
                panic!("a house has cells");
            };
            for (label, value) in r.patterns() {
                let Some(value) = value else {
                    panic!("a house has cells");
                };
                assert!(
                    value >= equal - 0.001,
                    "{label} scored {value}, below already-equal at {equal}"
                );
                assert!(
                    value <= any + 0.001,
                    "{label} scored {value}, above alike-at-all at {any}"
                );
            }
        }
    }

    #[test]
    fn every_figure_stays_inside_nought_to_a_hundred() {
        for seed in 0..12u64 {
            let a = unit(Level::Room, seed);
            let b = unit(Level::Room, seed + 100);
            let report = compare(&a, &b);
            for (label, value) in report.patterns() {
                let Some(value) = value else {
                    panic!("a room has cells");
                };
                assert!(
                    (0.0..=100.0).contains(&value),
                    "{label} returned {value}, which is not a percentage"
                );
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
        assert_eq!(compare(&grid, &there_and_back).cells, Some(100.0));
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
        // Rotate every cell in place. The whole-unit tile figure falls away
        // while the rotation figure still reads 100. That gap is the entire
        // reason these are separate figures.
        let grid = unit(Level::House, 37);
        let turned = per_cell(&grid, |bits| transformed_cell(bits, Rigid::Turn90));
        let report = compare(&grid, &turned);
        assert_eq!(report.cells_one_turn, Some(100.0));
        assert!(
            report.tiles < 95.0,
            "tiles scored {}, so the tile count saw the rotation after all",
            report.tiles
        );
    }

    #[test]
    fn different_sizes_have_nothing_to_compare() {
        let report = compare(&unit(Level::Room, 1), &unit(Level::House, 1));
        assert_eq!(report.tiles, 0.0);
        assert!(report.cells_similar.is_none());
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
