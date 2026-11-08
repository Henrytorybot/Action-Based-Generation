//! The cell alphabet: every 3x3 arrangement of walls and open tiles that this
//! generator is allowed to use.
//!
//! A cell is 9 tiles, so there are 2^9 = 512 arrangements in total. This is the
//! one level of the hierarchy small enough to enumerate exhaustively, and it is
//! worth doing: the legal subset is a fixed, inspectable table of a few hundred
//! entries that everything above is built out of.
//!
//! Two rules decide legality:
//!
//! 1. **The centre tile is open.** Every level's centre is the centre of its
//!    centre child, all the way down to a single tile, so this one rule gives
//!    every node at every level a guaranteed-open centre. Corridors between
//!    nodes are carved centre-to-centre, which is what makes that useful.
//! 2. **Every open tile connects to the centre**, moving only up/down/left/right.
//!    Without this a cell can contain an open tile sealed off by walls, which is
//!    space the player can see but never reach.

use std::sync::OnceLock;

/// Bit `i` of a cell is the tile at `(i % 3, i / 3)`. A set bit is a wall.
pub type CellBits = u16;

/// Index of the centre tile within a cell.
pub const CENTRE: u32 = 4;

/// The legal cells, grouped by how many tiles they leave open.
pub struct Alphabet {
    all: Vec<CellBits>,
    /// `by_open[n]` holds the legal cells with exactly `n` open tiles.
    by_open: [Vec<CellBits>; 10],
}

impl Alphabet {
    /// Every legal cell.
    pub fn all(&self) -> &[CellBits] {
        &self.all
    }

    /// The legal cells with exactly `open` open tiles, which may be empty.
    pub fn with_open_count(&self, open: usize) -> &[CellBits] {
        self.by_open.get(open).map_or(&[], Vec::as_slice)
    }

    /// The legal cells with `open` open tiles, or the nearest count that has
    /// any. Never empty: an all-walls-but-the-centre cell is always legal.
    pub fn nearest_with_open_count(&self, open: usize) -> &[CellBits] {
        for delta in 0..10usize {
            if let Some(found) = open
                .checked_sub(delta)
                .map(|n| self.with_open_count(n))
                .filter(|s| !s.is_empty())
            {
                return found;
            }
            let up = self.with_open_count(open + delta);
            if !up.is_empty() {
                return up;
            }
        }
        &self.all
    }
}

/// The alphabet, computed once on first use.
pub fn alphabet() -> &'static Alphabet {
    static ALPHABET: OnceLock<Alphabet> = OnceLock::new();
    ALPHABET.get_or_init(build)
}

fn build() -> Alphabet {
    let mut all = Vec::new();
    let mut by_open: [Vec<CellBits>; 10] = Default::default();

    for bits in 0u16..512 {
        if !is_legal(bits) {
            continue;
        }
        all.push(bits);
        by_open[open_count(bits)].push(bits);
    }

    Alphabet { all, by_open }
}

/// Whether tile `i` of a cell is open.
pub fn is_open(bits: CellBits, i: u32) -> bool {
    bits & (1 << i) == 0
}

/// How many of a cell's nine tiles are open.
pub fn open_count(bits: CellBits) -> usize {
    9 - (bits & 0x1FF).count_ones() as usize
}

/// Whether a cell satisfies both legality rules.
pub fn is_legal(bits: CellBits) -> bool {
    if !is_open(bits, CENTRE) {
        return false;
    }
    reachable_from_centre(bits) == open_count(bits)
}

/// How many open tiles can be reached from the centre by orthogonal steps.
fn reachable_from_centre(bits: CellBits) -> usize {
    let mut seen = 1u16 << CENTRE;
    let mut stack = vec![CENTRE];
    let mut count = 1;

    while let Some(i) = stack.pop() {
        let (x, y) = (i % 3, i / 3);
        let mut visit = |nx: u32, ny: u32, stack: &mut Vec<u32>| {
            let n = ny * 3 + nx;
            if is_open(bits, n) && seen & (1 << n) == 0 {
                seen |= 1 << n;
                count += 1;
                stack.push(n);
            }
        };
        if x > 0 {
            visit(x - 1, y, &mut stack);
        }
        if x < 2 {
            visit(x + 1, y, &mut stack);
        }
        if y > 0 {
            visit(x, y - 1, &mut stack);
        }
        if y < 2 {
            visit(x, y + 1, &mut stack);
        }
    }

    count
}

/// The cells one single-tile change away from `bits`, legal ones only.
///
/// Nothing uses this yet. It is here because "replace a space with a very
/// similar one" needs a notion of distance between cells, and the natural one
/// is a single flipped tile — which makes the alphabet a graph rather than a
/// list. Whether that graph is connected decides whether every legal cell can
/// be reached from every other by changes small enough to go unnoticed.
pub fn legal_neighbours(bits: CellBits) -> Vec<CellBits> {
    (0..9)
        .map(|i| bits ^ (1 << i))
        .filter(|&b| is_legal(b))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn the_alphabet_is_one_hundred_and_sixty_one_cells() {
        // Of the 512 arrangements, 256 have an open centre, and of those the
        // ones with no sealed-off tiles come to 161. A corner tile is only
        // reachable through one of its two adjacent edge tiles, which is what
        // removes the rest.
        //
        // Pinned as a number because every level above is built from this set:
        // if a legality rule changes, this is where it should be noticed.
        assert_eq!(alphabet().all().len(), 161);
    }

    #[test]
    fn centre_is_always_open() {
        assert!(alphabet().all().iter().all(|&b| is_open(b, CENTRE)));
    }

    #[test]
    fn no_legal_cell_has_unreachable_space() {
        for &bits in alphabet().all() {
            assert_eq!(
                reachable_from_centre(bits),
                open_count(bits),
                "cell {bits:#011b} has open tiles sealed off from its centre"
            );
        }
    }

    #[test]
    fn every_open_count_is_represented() {
        // One open tile is the bare centre; nine is a fully open cell. Both
        // ends have to exist or `openness` cannot span its full range.
        for n in 1..=9 {
            assert!(
                !alphabet().with_open_count(n).is_empty(),
                "no legal cell leaves exactly {n} tiles open"
            );
        }
        assert!(alphabet().with_open_count(0).is_empty());
    }

    /// The question worth answering before anything is built on top of this
    /// alphabet: can you walk from any legal cell to any other by changing one
    /// tile at a time, never passing through an illegal cell on the way?
    ///
    /// If this fails, some cells are only reachable by a jump big enough for a
    /// player to notice, and "replace it with a very similar one" cannot reach
    /// the whole alphabet.
    #[test]
    fn legal_cells_are_connected_by_single_tile_changes() {
        let all = alphabet().all();
        let legal: HashSet<CellBits> = all.iter().copied().collect();

        let start = all[0];
        let mut seen = HashSet::from([start]);
        let mut stack = vec![start];
        while let Some(bits) = stack.pop() {
            for n in legal_neighbours(bits) {
                if legal.contains(&n) && seen.insert(n) {
                    stack.push(n);
                }
            }
        }

        assert_eq!(
            seen.len(),
            all.len(),
            "the legal alphabet splits into islands: {} of {} cells reachable",
            seen.len(),
            all.len()
        );
    }
}
