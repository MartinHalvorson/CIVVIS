//! The Planet map: a closed globe of hexagons and twelve pentagons.
//!
//! Every other CIVVIS map is a cylinder — a rectangle that wraps east to west
//! and stops at a north and a south edge. A sphere cannot be covered by
//! hexagons alone; Euler's formula insists on twelve pentagons, which is why a
//! football is built the way it is. Planet uses exactly that shape: subdivide
//! each face of an icosahedron into a triangular lattice of frequency `n`,
//! push the lattice points onto the unit sphere, and treat every point as one
//! tile. Two points are neighbours when the lattice joins them, so a tile has
//! six neighbours everywhere except at the icosahedron's twelve corners, which
//! have five. The tiles are the faces of the dual Goldberg polyhedron — the
//! hexagons and pentagons the map is drawn with.
//!
//! A frequency-`n` sphere holds `10n² + 2` tiles.
//!
//! ## Where this construction comes from
//!
//! It is the same shape Uber's H3 geospatial index uses, and for the same
//! reasons. A lattice point is placed by barycentric coordinates on the flat
//! icosahedron face and then pushed out to the sphere, which is exactly a
//! gnomonic projection of that face — H3 describes itself as "gnomonic
//! projections centered on icosahedron faces", laying its grid on the faces
//! themselves rather than on an unfolded net, and this module does the same.
//! Two differences follow from being a game rather than an index: there is one
//! resolution per map size instead of H3's sixteen nested ones, since a game
//! is played at a single scale; and where H3 orients its icosahedron so that
//! the twelve pentagons fall in the ocean, a generated world can simply be
//! told to leave them under water (see [`Sphere::pentagons`]).
//!
//! The twelve pentagons are the only irregularity, and they are handled by
//! never assuming a tile has six of anything: adjacency, distance and rings
//! all come from the tile graph, not from coordinate arithmetic. That is the
//! part of a hex grid that quietly breaks on a sphere.
//!
//! ## Laying a sphere out in the engine's rectangle
//!
//! Tiles are addressed by the same offset column/row pair the flat maps use,
//! because [`crate::world::TileGrid`] stores hexes in a dense rectangle and
//! every rule in the engine speaks [`Pos`]. The sphere is cut into the ten
//! rhombi formed by pairing adjacent icosahedron faces — two per each of the
//! five longitude lunes — and each rhombus owns an `n × n` half-open block of
//! its own lattice. That is a partition: `10 · n² = 10n²` tiles, leaving the
//! two poles, which get a row of their own at each end. The rectangle is
//! therefore `5n` columns by `2n + 2` rows, with the two pole rows holding one
//! tile each.
//!
//! The rectangle is storage, not geography: a row is not a parallel of
//! latitude. Anything that wants latitude asks for it ([`Sphere::latitude`]),
//! and anything that wants adjacency or distance asks the graph. The sphere's
//! geometry is a pure function of its frequency, so it is built once per
//! frequency and shared by every map and every cloned game that uses it.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use crate::hex;
use crate::Pos;

/// Exact distances are precomputed out to this radius for every tile. It
/// covers the radii the rules actually ask about — a city's workable tiles,
/// district adjacency, unit sight — so the common query is a binary search
/// over a small shared table rather than a search of the map.
const RING_RADIUS: i32 = 6;

/// Lattice points are matched between rhombi by rounding to this grid. The
/// closest two distinct tiles ever come is about `2/n` radians of arc, which
/// at the largest frequency this engine builds is still four orders of
/// magnitude above the rounding, so the match is exact in practice while
/// absorbing the last-bit differences between two ways of computing the same
/// corner.
const WELD: f64 = 1.0e6;

/// The shape of one tile, for anything that has to draw the globe.
#[derive(Clone, Debug)]
pub struct Cell {
    pub pos: Pos,
    /// Unit vector from the centre of the world to the middle of the tile.
    pub center: [f64; 3],
    /// The tile's outline, counter-clockwise seen from outside. Corner `k` is
    /// the point shared with neighbours `k` and `k + 1`, so the outline and
    /// the neighbour list run together.
    pub corners: Vec<[f64; 3]>,
}

/// The geometry and topology of one frequency of globe.
///
/// Adjacency is held as flat arrays rather than inside [`Cell`]: neighbour
/// lookup and distance sit under every rule in the engine, and keeping the six
/// neighbouring *positions* next to each other means answering one costs a
/// single lookup instead of a walk through six separate tiles.
pub struct Sphere {
    frequency: i32,
    width: i32,
    height: i32,
    /// Storage position of each tile, in storage order.
    pos: Vec<Pos>,
    /// Each tile's neighbouring positions, counter-clockwise from outside.
    around: Vec<[Pos; 6]>,
    /// Each tile's neighbouring cell indices, in the same order.
    adjacent: Vec<[u32; 6]>,
    /// Five at a pentagon, six everywhere else.
    degree: Vec<u8>,
    /// Drawing geometry, which no rule reads.
    cells: Vec<Cell>,
    /// `row * width + col` -> cell index, or `u32::MAX` where the rectangle
    /// has no tile (the empty part of the two pole rows).
    slot: Vec<u32>,
    /// Every tile within [`RING_RADIUS`] of each tile, including the tile
    /// itself, sorted by position and carrying its distance. Sorted by
    /// position rather than by distance so that a distance query is a binary
    /// search over one contiguous run, and a disk is a single pass over it
    /// that comes out already in map order.
    rings: Vec<Box<[(Pos, u8)]>>,
    /// Full distance rows, searched out the first time a tile is asked about
    /// something further away than the rings reach and kept afterwards. One
    /// byte per tile per filled row, and only rows that were actually asked
    /// for are ever filled.
    rows: Vec<OnceLock<Box<[u8]>>>,
}

const EMPTY: u32 = u32::MAX;

impl Sphere {
    pub fn frequency(&self) -> i32 {
        self.frequency
    }

    /// Columns in the storage rectangle.
    pub const fn width_for(frequency: i32) -> i32 {
        5 * frequency
    }

    /// Rows in the storage rectangle, including the two single-tile pole rows.
    pub const fn height_for(frequency: i32) -> i32 {
        2 * frequency + 2
    }

    /// Tiles on a globe of this frequency.
    pub const fn tiles_for(frequency: i32) -> usize {
        (10 * frequency * frequency + 2) as usize
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn cells(&self) -> &[Cell] {
        &self.cells
    }

    pub fn len(&self) -> usize {
        self.pos.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pos.is_empty()
    }

    #[inline]
    pub fn index_of(&self, pos: Pos) -> Option<u32> {
        let (col, row) = hex::axial_to_offset(pos.0, pos.1);
        if col < 0 || col >= self.width || row < 0 || row >= self.height {
            return None;
        }
        let slot = *self.slot.get((row * self.width + col) as usize)?;
        (slot != EMPTY).then_some(slot)
    }

    #[inline]
    pub fn cell(&self, pos: Pos) -> Option<&Cell> {
        self.index_of(pos).map(|index| &self.cells[index as usize])
    }

    pub fn contains(&self, pos: Pos) -> bool {
        self.index_of(pos).is_some()
    }

    /// Every tile of the globe, in storage order.
    pub fn positions(&self) -> impl Iterator<Item = Pos> + '_ {
        self.pos.iter().copied()
    }

    /// How many neighbours a tile has: five at the twelve pentagons, six
    /// everywhere else.
    pub fn degree(&self, pos: Pos) -> usize {
        self.index_of(pos)
            .map_or(0, |index| self.degree[index as usize] as usize)
    }

    /// The twelve tiles that have five neighbours instead of six: the corners
    /// of the icosahedron the globe is subdivided from.
    ///
    /// Uber's H3 grid, which is the same construction, orients its icosahedron
    /// the way Buckminster Fuller did so that all twelve corners fall in open
    /// ocean and the pentagons never surface in the data. A generated world
    /// can do better than orient itself: the map generator is told where these
    /// twelve are and leaves them under water, so no city, district or
    /// adjacency bonus ever sits on a tile with a neighbour missing.
    pub fn pentagons(&self) -> Vec<Pos> {
        self.pos
            .iter()
            .zip(&self.degree)
            .filter(|(_, degree)| **degree == 5)
            .map(|(pos, _)| *pos)
            .collect()
    }

    /// The tile's neighbours, counter-clockwise from outside.
    #[inline]
    pub fn neighbors(&self, pos: Pos) -> hex::Neighbors {
        let mut out = hex::Neighbors::new();
        let Some(index) = self.index_of(pos) else {
            return out;
        };
        let index = index as usize;
        for neighbor in &self.around[index][..self.degree[index] as usize] {
            out.push(*neighbor);
        }
        out
    }

    /// Which of `from`'s edges faces `to`, or `None` when they do not touch.
    /// This is the index river and cliff segments are stored under, so it has
    /// to agree with [`Sphere::neighbors`].
    pub fn direction_to(&self, from: Pos, to: Pos) -> Option<usize> {
        let index = self.index_of(from)? as usize;
        self.around[index][..self.degree[index] as usize]
            .iter()
            .position(|neighbor| *neighbor == to)
    }

    /// Latitude in radians, negative south of the equator.
    pub fn latitude(&self, pos: Pos) -> f64 {
        self.cell(pos).map_or(0.0, |cell| cell.center[2].asin())
    }

    /// Longitude in radians, zero on the prime meridian.
    pub fn longitude(&self, pos: Pos) -> f64 {
        self.cell(pos)
            .map_or(0.0, |cell| cell.center[1].atan2(cell.center[0]))
    }

    /// Steps along the tile graph between two tiles. Exact: within
    /// [`RING_RADIUS`] it is a table lookup, and past it the map is searched.
    pub fn distance(&self, a: Pos, b: Pos) -> i32 {
        let (Some(from), Some(to)) = (self.index_of(a), self.index_of(b)) else {
            return hex::wdistance(a, b, self.width);
        };
        if from == to {
            return 0;
        }
        let ring = &self.rings[from as usize];
        if let Ok(at) = ring.binary_search_by_key(&b, |(pos, _)| *pos) {
            return ring[at].1 as i32;
        }
        self.row(from)[to as usize] as i32
    }

    /// The middle of a tile, as a unit vector.
    pub fn center(&self, pos: Pos) -> Option<[f64; 3]> {
        self.cell(pos).map(|cell| cell.center)
    }

    /// A tile's outline, counter-clockwise seen from outside the globe.
    pub fn corners(&self, pos: Pos) -> &[[f64; 3]] {
        self.cell(pos).map_or(&[], |cell| &cell.corners)
    }

    /// Every tile within `radius` steps, including the centre.
    pub fn disk(&self, center: Pos, radius: i32) -> Vec<Pos> {
        let Some(from) = self.index_of(center) else {
            return Vec::new();
        };
        if radius <= 0 {
            return vec![center];
        }
        if radius <= RING_RADIUS {
            // The ring is already in map order, so the disk comes out sorted
            // without a pass over it.
            return self.rings[from as usize]
                .iter()
                .filter(|(_, distance)| *distance as i32 <= radius)
                .map(|(pos, _)| *pos)
                .collect();
        }
        let mut out: Vec<Pos> = self
            .row(from)
            .iter()
            .enumerate()
            .filter(|(_, distance)| **distance as i32 <= radius)
            .map(|(index, _)| self.pos[index])
            .collect();
        out.sort();
        out
    }

    /// Distances from one tile to the whole globe, searched once and kept.
    fn row(&self, from: u32) -> &[u8] {
        self.rows[from as usize].get_or_init(|| self.search(from))
    }

    fn search(&self, from: u32) -> Box<[u8]> {
        let mut distance = vec![u8::MAX; self.len()];
        distance[from as usize] = 0;
        let mut frontier = vec![from];
        let mut next = Vec::new();
        let mut step = 0u8;
        while !frontier.is_empty() {
            step = step.saturating_add(1);
            for cell in frontier.drain(..) {
                let cell = cell as usize;
                for neighbor in &self.adjacent[cell][..self.degree[cell] as usize] {
                    if distance[*neighbor as usize] == u8::MAX {
                        distance[*neighbor as usize] = step;
                        next.push(*neighbor);
                    }
                }
            }
            std::mem::swap(&mut frontier, &mut next);
        }
        distance.into_boxed_slice()
    }
}

/// The icosahedron this globe is subdivided from: a pole, a ring of five
/// vertices in the northern hemisphere, a ring of five offset by 36° in the
/// southern, and the opposite pole.
fn icosahedron() -> [[f64; 3]; 12] {
    let mut out = [[0.0; 3]; 12];
    out[0] = [0.0, 0.0, 1.0];
    let ring = (0.5f64).atan();
    for k in 0..5 {
        let upper = (72.0 * k as f64).to_radians();
        out[1 + k] = [
            ring.cos() * upper.cos(),
            ring.cos() * upper.sin(),
            ring.sin(),
        ];
        let lower = (72.0 * k as f64 + 36.0).to_radians();
        out[6 + k] = [
            ring.cos() * lower.cos(),
            ring.cos() * lower.sin(),
            -ring.sin(),
        ];
    }
    out[11] = [0.0, 0.0, -1.0];
    out
}

const NORTH: usize = 0;
const SOUTH: usize = 11;

fn upper(k: usize) -> usize {
    1 + k % 5
}

fn lower(k: usize) -> usize {
    6 + k % 5
}

/// The ten rhombi, as `(A, B, C, D)` corners.
///
/// `A` sits at local `(0, 0)`, `B` at `(n, 0)`, `C` at `(0, n)` and `D` at
/// `(n, n)`; `B`–`C` is the diagonal the two icosahedron faces share. Each
/// rhombus owns the half-open block `u ∈ [1, n]`, `v ∈ [0, n)`, which is its
/// `B` corner, the two icosahedron edges that meet there, and its interior.
/// Taken together the ten claims cover every corner but the two poles, and
/// every edge exactly once.
fn rhombi() -> [[usize; 4]; 10] {
    let mut out = [[0; 4]; 10];
    for c in 0..5 {
        out[c] = [lower(c), upper(c), upper(c + 1), NORTH];
        out[5 + c] = [upper(c + 1), lower(c), lower(c + 1), SOUTH];
    }
    out
}

fn normalize(p: [f64; 3]) -> [f64; 3] {
    let length = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    [p[0] / length, p[1] / length, p[2] / length]
}

fn blend(weights: [f64; 3], points: [[f64; 3]; 3]) -> [f64; 3] {
    normalize([
        weights[0] * points[0][0] + weights[1] * points[1][0] + weights[2] * points[2][0],
        weights[0] * points[0][1] + weights[1] * points[1][1] + weights[2] * points[2][1],
        weights[0] * points[0][2] + weights[1] * points[1][2] + weights[2] * points[2][2],
    ])
}

/// Where local coordinate `(u, v)` of a rhombus lands on the globe. Below the
/// shared diagonal the point is barycentric in face `A B C`, above it in face
/// `B C D`; the two agree along the diagonal itself.
fn rhombus_point(corners: &[[f64; 3]; 12], rhombus: [usize; 4], n: i32, u: i32, v: i32) -> [f64; 3] {
    let [a, b, c, d] = rhombus.map(|index| corners[index]);
    let (n, u, v) = (n as f64, u as f64, v as f64);
    if u + v <= n {
        blend([n - u - v, u, v], [a, b, c])
    } else {
        blend([n - v, n - u, u + v - n], [b, c, d])
    }
}

/// Matches lattice points that two rhombi both compute, so a shared edge is
/// one tile rather than two.
struct Weld {
    buckets: HashMap<(i64, i64, i64), u32>,
}

impl Weld {
    fn new() -> Weld {
        Weld {
            buckets: HashMap::new(),
        }
    }

    fn intern(&mut self, point: [f64; 3], next: u32) -> u32 {
        let key = |p: [f64; 3]| {
            (
                (p[0] * WELD).round() as i64,
                (p[1] * WELD).round() as i64,
                (p[2] * WELD).round() as i64,
            )
        };
        let (x, y, z) = key(point);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(found) = self.buckets.get(&(x + dx, y + dy, z + dz)) {
                        return *found;
                    }
                }
            }
        }
        self.buckets.insert((x, y, z), next);
        next
    }
}

fn build(frequency: i32) -> Sphere {
    assert!(frequency >= 1, "a globe needs at least one subdivision");
    let n = frequency;
    let corners = icosahedron();
    let rhombi = rhombi();
    let width = Sphere::width_for(n);
    let height = Sphere::height_for(n);

    // Every lattice point of every rhombus, welded so that shared edges and
    // corners are one tile. `lattice[r][u][v]` is the tile at that local
    // coordinate; the same tile appears in more than one rhombus.
    let mut weld = Weld::new();
    let mut points: Vec<[f64; 3]> = Vec::with_capacity(Sphere::tiles_for(n));
    let mut lattice = vec![vec![vec![0u32; (n + 1) as usize]; (n + 1) as usize]; 10];
    for (index, rhombus) in rhombi.iter().enumerate() {
        for u in 0..=n {
            for v in 0..=n {
                let point = rhombus_point(&corners, *rhombus, n, u, v);
                let id = weld.intern(point, points.len() as u32);
                if id as usize == points.len() {
                    points.push(point);
                }
                lattice[index][u as usize][v as usize] = id;
            }
        }
    }
    debug_assert_eq!(points.len(), Sphere::tiles_for(n));

    // Each rhombus owns u ∈ [1, n], v ∈ [0, n): its own quarter of the globe,
    // written into the rectangle as an n-wide, n-tall block.
    let mut position = vec![None; points.len()];
    for (index, block) in lattice.iter().enumerate() {
        let lune = (index % 5) as i32;
        let half = (index / 5) as i32;
        for u in 1..=n {
            for v in 0..n {
                let id = block[u as usize][v as usize] as usize;
                debug_assert!(position[id].is_none(), "two rhombi claimed one tile");
                let col = lune * n + (u - 1);
                let row = 1 + half * n + v;
                position[id] = Some(hex::offset_to_axial(col, row));
            }
        }
    }
    // What is left over is the pair of poles, which get the spare row at each
    // end of the rectangle.
    for (id, slot) in position.iter_mut().enumerate() {
        if slot.is_none() {
            let row = if points[id][2] > 0.0 { 0 } else { height - 1 };
            *slot = Some(hex::offset_to_axial(0, row));
        }
    }

    // Adjacency comes from the lattice: inside a rhombus the six axial
    // directions are neighbours, and because the rhombi are welded together the
    // same walk crosses their seams.
    let mut adjacency: Vec<Vec<u32>> = vec![Vec::new(); points.len()];
    for block in &lattice {
        for u in 0..=n {
            for v in 0..=n {
                let id = block[u as usize][v as usize];
                for (du, dv) in hex::DIRS {
                    let (nu, nv) = (u + du, v + dv);
                    if nu < 0 || nu > n || nv < 0 || nv > n {
                        continue;
                    }
                    let other = block[nu as usize][nv as usize];
                    if other != id && !adjacency[id as usize].contains(&other) {
                        adjacency[id as usize].push(other);
                    }
                }
            }
        }
    }

    // Storage order follows the rectangle, so the map iterates in the same
    // order it always has.
    let mut order: Vec<u32> = (0..points.len() as u32).collect();
    order.sort_by_key(|id| position[*id as usize].unwrap());
    let mut rank = vec![0u32; points.len()];
    for (index, id) in order.iter().enumerate() {
        rank[*id as usize] = index as u32;
    }

    let mut pos: Vec<Pos> = Vec::with_capacity(points.len());
    let mut around: Vec<[Pos; 6]> = Vec::with_capacity(points.len());
    let mut adjacent: Vec<[u32; 6]> = Vec::with_capacity(points.len());
    let mut degree: Vec<u8> = Vec::with_capacity(points.len());
    let mut cells: Vec<Cell> = Vec::with_capacity(points.len());
    for id in &order {
        let id = *id as usize;
        let center = points[id];
        // Counter-clockwise seen from outside, so the outline is wound
        // consistently everywhere and an edge index means the same thing on
        // both tiles that share it.
        let east = if center[2].abs() > 0.999_999 {
            [1.0, 0.0, 0.0]
        } else {
            normalize([-center[1], center[0], 0.0])
        };
        let north = [
            center[1] * east[2] - center[2] * east[1],
            center[2] * east[0] - center[0] * east[2],
            center[0] * east[1] - center[1] * east[0],
        ];
        let mut sorted: Vec<(f64, u32)> = adjacency[id]
            .iter()
            .map(|other| {
                let point = points[*other as usize];
                let delta = [
                    point[0] - center[0],
                    point[1] - center[1],
                    point[2] - center[2],
                ];
                let x = delta[0] * east[0] + delta[1] * east[1] + delta[2] * east[2];
                let y = delta[0] * north[0] + delta[1] * north[1] + delta[2] * north[2];
                (y.atan2(x), rank[*other as usize])
            })
            .collect();
        sorted.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let mut neighbors = [0u32; 6];
        let mut positions = [(0, 0); 6];
        for (at, (_, cell)) in sorted.iter().enumerate() {
            neighbors[at] = *cell;
            positions[at] = position[order[*cell as usize] as usize].unwrap();
        }
        pos.push(position[id].unwrap());
        around.push(positions);
        adjacent.push(neighbors);
        degree.push(sorted.len() as u8);
        cells.push(Cell {
            pos: position[id].unwrap(),
            center,
            corners: Vec::new(),
        });
    }

    // A corner of the tiling is where three tiles meet, which on a geodesic
    // grid is the middle of the lattice triangle they form.
    for index in 0..cells.len() {
        let center = cells[index].center;
        let sides = degree[index] as usize;
        let mut corners = Vec::with_capacity(sides);
        for at in 0..sides {
            let a = cells[adjacent[index][at] as usize].center;
            let b = cells[adjacent[index][(at + 1) % sides] as usize].center;
            corners.push(normalize([
                center[0] + a[0] + b[0],
                center[1] + a[1] + b[1],
                center[2] + a[2] + b[2],
            ]));
        }
        cells[index].corners = corners;
    }

    let mut slot = vec![EMPTY; (width * height) as usize];
    for (index, cell) in cells.iter().enumerate() {
        let (col, row) = hex::axial_to_offset(cell.pos.0, cell.pos.1);
        slot[(row * width + col) as usize] = index as u32;
    }

    let mut sphere = Sphere {
        frequency: n,
        width,
        height,
        rows: (0..pos.len()).map(|_| OnceLock::new()).collect(),
        pos,
        around,
        adjacent,
        degree,
        cells,
        slot,
        rings: Vec::new(),
    };
    sphere.rings = (0..sphere.len())
        .map(|index| sphere.ring_of(index as u32))
        .collect();
    sphere
}

impl Sphere {
    /// Exact distances from one tile out to [`RING_RADIUS`], the tile itself
    /// included.
    fn ring_of(&self, from: u32) -> Box<[(Pos, u8)]> {
        let mut seen: HashMap<u32, u8> = HashMap::new();
        seen.insert(from, 0);
        let mut frontier = vec![from];
        let mut next = Vec::new();
        for step in 1..=RING_RADIUS as u8 {
            for cell in frontier.drain(..) {
                let cell = cell as usize;
                for neighbor in &self.adjacent[cell][..self.degree[cell] as usize] {
                    seen.entry(*neighbor).or_insert_with(|| {
                        next.push(*neighbor);
                        step
                    });
                }
            }
            std::mem::swap(&mut frontier, &mut next);
        }
        let mut out: Vec<(Pos, u8)> = seen
            .into_iter()
            .map(|(cell, distance)| (self.pos[cell as usize], distance))
            .collect();
        out.sort_by_key(|(pos, _)| *pos);
        out.into_boxed_slice()
    }
}

/// The globe of a given frequency, built once and shared.
pub fn sphere(frequency: i32) -> Arc<Sphere> {
    static BUILT: RwLock<Option<HashMap<i32, Arc<Sphere>>>> = RwLock::new(None);
    {
        let guard = BUILT.read().unwrap();
        if let Some(found) = guard.as_ref().and_then(|built| built.get(&frequency)) {
            return Arc::clone(found);
        }
    }
    let made = Arc::new(build(frequency));
    let mut guard = BUILT.write().unwrap();
    let built = guard.get_or_insert_with(HashMap::new);
    Arc::clone(built.entry(frequency).or_insert(made))
}

#[cfg(test)]
mod tests {
    use std::collections::{HashSet, VecDeque};

    use super::*;

    #[test]
    fn a_globe_is_hexagons_and_exactly_twelve_pentagons() {
        for frequency in [1, 2, 3, 7, 11] {
            let globe = sphere(frequency);
            assert_eq!(globe.len(), Sphere::tiles_for(frequency));
            let pentagons = globe
                .cells()
                .iter()
                .filter(|cell| globe.degree(cell.pos) == 5)
                .count();
            let hexagons = globe
                .cells()
                .iter()
                .filter(|cell| globe.degree(cell.pos) == 6)
                .count();
            assert_eq!(pentagons, 12, "frequency {frequency}");
            assert_eq!(hexagons, globe.len() - 12, "frequency {frequency}");
            // A tile's outline has one corner per neighbour.
            for cell in globe.cells() {
                assert_eq!(cell.corners.len(), globe.degree(cell.pos));
            }
        }
    }

    #[test]
    fn adjacency_is_mutual_and_the_globe_is_one_surface() {
        let globe = sphere(7);
        for cell in globe.cells() {
            for neighbor in globe.neighbors(cell.pos) {
                assert!(
                    globe.neighbors(neighbor).contains(&cell.pos),
                    "{:?} and {neighbor:?} disagree",
                    cell.pos
                );
                assert_eq!(globe.distance(cell.pos, neighbor), 1);
            }
        }
        let start = globe.cells()[0].pos;
        let mut seen: HashSet<Pos> = HashSet::from([start]);
        let mut queue = VecDeque::from([start]);
        while let Some(pos) = queue.pop_front() {
            for neighbor in globe.neighbors(pos) {
                if seen.insert(neighbor) {
                    queue.push_back(neighbor);
                }
            }
        }
        assert_eq!(seen.len(), globe.len(), "the globe is not connected");
    }

    #[test]
    fn every_tile_has_its_own_place_in_the_rectangle() {
        for frequency in [3, 11] {
            let globe = sphere(frequency);
            let mut seen: HashSet<Pos> = HashSet::new();
            for cell in globe.cells() {
                assert!(seen.insert(cell.pos), "two tiles share {:?}", cell.pos);
                let (col, row) = hex::axial_to_offset(cell.pos.0, cell.pos.1);
                assert!((0..globe.width()).contains(&col));
                assert!((0..globe.height()).contains(&row));
                assert_eq!(globe.cell(cell.pos).map(|found| found.pos), Some(cell.pos));
            }
            // The poles are the only tiles in the first and last rows.
            for row in [0, globe.height() - 1] {
                let count = globe
                    .cells()
                    .iter()
                    .filter(|cell| hex::axial_to_offset(cell.pos.0, cell.pos.1).1 == row)
                    .count();
                assert_eq!(count, 1, "row {row} of a frequency-{frequency} globe");
            }
        }
    }

    #[test]
    fn walking_the_globe_never_leaves_it() {
        // Every direction out of every tile lands on another tile: there is no
        // edge of the world to fall off, which is the whole point of Planet.
        let globe = sphere(5);
        for cell in globe.cells() {
            let neighbors = globe.neighbors(cell.pos);
            assert!(neighbors.len() == 5 || neighbors.len() == 6);
            for neighbor in neighbors {
                assert!(globe.contains(neighbor));
            }
        }
    }

    #[test]
    fn distance_matches_a_search_of_the_map() {
        let globe = sphere(6);
        let sources = [0usize, 17, 133, globe.len() - 1];
        for source in sources {
            let from = globe.cells()[source].pos;
            let mut walked: HashMap<Pos, i32> = HashMap::from([(from, 0)]);
            let mut queue = VecDeque::from([from]);
            while let Some(pos) = queue.pop_front() {
                let step = walked[&pos] + 1;
                for neighbor in globe.neighbors(pos) {
                    walked.entry(neighbor).or_insert_with(|| {
                        queue.push_back(neighbor);
                        step
                    });
                }
            }
            assert_eq!(walked.len(), globe.len());
            for (pos, steps) in &walked {
                assert_eq!(globe.distance(from, *pos), *steps, "{from:?} -> {pos:?}");
            }
            for radius in [0, 1, 3, RING_RADIUS, RING_RADIUS + 4] {
                let disk = globe.disk(from, radius);
                let expected = walked.values().filter(|steps| **steps <= radius).count();
                assert_eq!(disk.len(), expected, "radius {radius}");
                assert!(disk.windows(2).all(|pair| pair[0] < pair[1]));
            }
        }
    }

    #[test]
    fn latitude_runs_from_pole_to_pole_and_edges_are_two_sided() {
        let globe = sphere(8);
        let north = globe
            .cells()
            .iter()
            .max_by(|a, b| a.center[2].partial_cmp(&b.center[2]).unwrap())
            .unwrap();
        assert!(globe.latitude(north.pos) > 1.5, "a pole is near ±π/2");
        assert_eq!(globe.degree(north.pos), 5, "the poles are pentagons");
        for cell in globe.cells().iter().take(200) {
            for (direction, neighbor) in globe.neighbors(cell.pos).into_iter().enumerate() {
                assert_eq!(globe.direction_to(cell.pos, neighbor), Some(direction));
                assert!(globe.direction_to(neighbor, cell.pos).is_some());
            }
        }
    }

    #[test]
    fn tiles_are_within_a_few_percent_of_the_stock_map_sizes() {
        // Planet keeps the shipped sizes recognizable: the globe a size builds
        // holds about as many tiles as that size's rectangle.
        for (frequency, rectangle) in [
            (11, 44 * 26),
            (15, 60 * 38),
            (18, 74 * 46),
            (21, 84 * 54),
            (24, 96 * 60),
            (26, 106 * 66),
        ] {
            let tiles = Sphere::tiles_for(frequency) as f64;
            let ratio = tiles / rectangle as f64;
            assert!(
                (0.93..=1.07).contains(&ratio),
                "frequency {frequency}: {tiles} tiles against {rectangle}"
            );
        }
    }
}
