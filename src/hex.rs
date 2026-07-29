//! Axial hex grid math (mirrors civvis/hexgrid.py).
use crate::Pos;

pub const DIRS: [(i32, i32); 6] = [(1, 0), (1, -1), (0, -1), (-1, 0), (-1, 1), (0, 1)];

pub fn neighbors(p: Pos) -> [Pos; 6] {
    [
        (p.0 + 1, p.1),
        (p.0 + 1, p.1 - 1),
        (p.0, p.1 - 1),
        (p.0 - 1, p.1),
        (p.0 - 1, p.1 + 1),
        (p.0, p.1 + 1),
    ]
}

/// The in-map neighbors of a hex, held inline.
///
/// A hex has at most six neighbors, so the answer never needs the heap.
/// Neighbor queries run inside adjacency bonuses, pathfinding, and every
/// line-of-sight ray, and a `Vec` per query dominated the engine's allocator
/// traffic.
#[derive(Clone, Copy, Debug, Default)]
pub struct Neighbors {
    buf: [Pos; 6],
    len: u8,
}

impl Neighbors {
    pub fn new() -> Neighbors {
        Neighbors::default()
    }

    #[inline]
    pub fn push(&mut self, pos: Pos) {
        self.buf[self.len as usize] = pos;
        self.len += 1;
    }
}

impl std::ops::Deref for Neighbors {
    type Target = [Pos];

    #[inline]
    fn deref(&self) -> &[Pos] {
        &self.buf[..self.len as usize]
    }
}

impl std::ops::DerefMut for Neighbors {
    #[inline]
    fn deref_mut(&mut self) -> &mut [Pos] {
        &mut self.buf[..self.len as usize]
    }
}

impl IntoIterator for Neighbors {
    type Item = Pos;
    type IntoIter = std::iter::Take<std::array::IntoIter<Pos, 6>>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.buf.into_iter().take(self.len as usize)
    }
}

impl<'a> IntoIterator for &'a Neighbors {
    type Item = &'a Pos;
    type IntoIter = std::slice::Iter<'a, Pos>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl FromIterator<Pos> for Neighbors {
    fn from_iter<I: IntoIterator<Item = Pos>>(iter: I) -> Neighbors {
        let mut out = Neighbors::new();
        for pos in iter {
            out.push(pos);
        }
        out
    }
}

pub fn distance(a: Pos, b: Pos) -> i32 {
    let dq = a.0 - b.0;
    let dr = a.1 - b.1;
    dq.abs().max(dr.abs()).max((dq + dr).abs())
}

pub fn disk(c: Pos, radius: i32) -> Vec<Pos> {
    let mut out = Vec::new();
    for dq in -radius..=radius {
        let lo = (-radius).max(-dq - radius);
        let hi = radius.min(-dq + radius);
        for dr in lo..=hi {
            out.push((c.0 + dq, c.1 + dr));
        }
    }
    out
}

/// The hexes exactly `radius` steps from `c`.
///
/// [`disk`] is the union of every ring out to its radius, so walking outward
/// one ring at a time costs `O(radius)` per step instead of rebuilding an
/// `O(radius^2)` disk and throwing away everything inside it. The exploration
/// search does exactly that walk, and it was the largest single cost in the
/// engine's basic AI.
pub fn ring(c: Pos, radius: i32) -> Vec<Pos> {
    if radius <= 0 {
        return vec![c];
    }
    let mut out = Vec::with_capacity(6 * radius as usize);
    // Start on one corner of the ring and walk its six sides. Which corner
    // does not matter to any caller: every one of them treats the result as a
    // set.
    let mut pos = (c.0 + DIRS[4].0 * radius, c.1 + DIRS[4].1 * radius);
    for dir in DIRS {
        for _ in 0..radius {
            out.push(pos);
            pos = (pos.0 + dir.0, pos.1 + dir.1);
        }
    }
    out
}

pub fn offset_to_axial(col: i32, row: i32) -> Pos {
    (col - (row - (row & 1)) / 2, row)
}

/// Canonical position on an east-west wrapping (cylindrical) map.
pub fn canon(p: Pos, width: i32) -> Pos {
    let col = p.0 + (p.1 - (p.1 & 1)) / 2;
    let m = col.rem_euclid(width);
    (p.0 + (m - col), p.1)
}

/// Hex distance on a cylinder of the given width.
pub fn wdistance(a: Pos, b: Pos, width: i32) -> i32 {
    let mut best = i32::MAX;
    for s in [-width, 0, width] {
        best = best.min(distance((a.0 + s, a.1), b));
    }
    best
}

pub fn axial_to_offset(q: i32, r: i32) -> (i32, i32) {
    (q + (r - (r & 1)) / 2, r)
}

#[cfg(test)]
mod ring_tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The ring walk has to produce exactly the hexes a disk gains at that
    /// radius, or the exploration search would skip ground.
    #[test]
    fn a_ring_is_the_shell_a_disk_gains() {
        for radius in 1..8 {
            let inner: BTreeSet<Pos> = disk((3, -7), radius - 1).into_iter().collect();
            let whole: BTreeSet<Pos> = disk((3, -7), radius).into_iter().collect();
            let shell: BTreeSet<Pos> = whole.difference(&inner).copied().collect();
            let walked: BTreeSet<Pos> = ring((3, -7), radius).into_iter().collect();
            assert_eq!(walked, shell, "radius {radius}");
            assert_eq!(ring((3, -7), radius).len(), 6 * radius as usize);
        }
    }
}
