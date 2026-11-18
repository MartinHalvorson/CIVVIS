//! The game's index of what stands where, by map position. See
//! [`crate::fast_map`].

/// What stands at each position, found by hashing the position.
pub(crate) type PosMap<V> = crate::fast_map::FastMap<crate::Pos, V>;

/// The city standing on each tile, read through the tile grid's own index.
///
/// `city_at` is asked for every neighbour of every tile a zone-of-control
/// test, sight line or movement flood looks at; a hash lookup was 2.4% of a
/// late site game. Cities are founded and lost rarely, so the ids sit in a
/// vector shared between the game's speculative clones until one of them
/// founds or loses a city. A position the grid has no index for (a hex a
/// save omitted, or one off the map) is kept by position, so every position
/// answers exactly the city last put there and not taken away.
#[derive(Clone, Default)]
pub(crate) struct CityIndex {
    /// By `TileGrid::index_of`: the city's id, or [`CityIndex::NONE`].
    slots: std::sync::Arc<Vec<u32>>,
    /// Cities on positions the grid has no index for.
    off_grid: PosMap<u32>,
}

impl CityIndex {
    const NONE: u32 = u32::MAX;

    #[inline]
    pub(crate) fn get(&self, grid: &crate::world::TileGrid, pos: crate::Pos) -> Option<u32> {
        match grid.index_of(pos) {
            Some(index) => self
                .slots
                .get(index)
                .copied()
                .filter(|city| *city != Self::NONE),
            None => self.off_grid.get(&pos).copied(),
        }
    }

    pub(crate) fn insert(&mut self, grid: &crate::world::TileGrid, pos: crate::Pos, city: u32) {
        match grid.index_of(pos) {
            Some(index) => {
                let slots = std::sync::Arc::make_mut(&mut self.slots);
                if slots.len() <= index {
                    slots.resize(grid.len().max(index + 1), Self::NONE);
                }
                slots[index] = city;
            }
            None => {
                self.off_grid.insert(pos, city);
            }
        }
    }

    pub(crate) fn remove(&mut self, grid: &crate::world::TileGrid, pos: crate::Pos) {
        match grid.index_of(pos) {
            Some(index) => {
                if self
                    .slots
                    .get(index)
                    .is_some_and(|city| *city != Self::NONE)
                {
                    std::sync::Arc::make_mut(&mut self.slots)[index] = Self::NONE;
                }
            }
            None => {
                self.off_grid.remove(&pos);
            }
        }
    }

    pub(crate) fn clear(&mut self) {
        self.slots = std::sync::Arc::default();
        self.off_grid.clear();
    }
}

/// The units standing on each tile, in the order they arrived, read through
/// the tile grid's own index.
///
/// `unit_ids_at` is asked for every neighbour of every tile a movement flood,
/// zone-of-control test or combat reach expands; hashed it was still near 3%
/// of a late site game. The lists sit in a vector indexed the way the grid
/// indexes its tiles, shared between the game's speculative clones until one
/// of them moves, places or removes a unit, which copies it once. A position
/// the grid has no index for is kept by position, so every position answers
/// exactly the units put there and not taken away, in the order they came.
#[derive(Clone, Default)]
pub(crate) struct UnitIndex {
    /// By `TileGrid::index_of`: the units on that tile.
    slots: std::sync::Arc<Vec<Vec<u32>>>,
    /// Units on positions the grid has no index for.
    off_grid: PosMap<Vec<u32>>,
}

impl UnitIndex {
    #[inline]
    pub(crate) fn get(&self, grid: &crate::world::TileGrid, pos: crate::Pos) -> &[u32] {
        match grid.index_of(pos) {
            Some(index) => self.slots.get(index).map_or(&[], Vec::as_slice),
            None => self.off_grid.get(&pos).map_or(&[], Vec::as_slice),
        }
    }

    /// Put `unit` on `pos`, after any unit already there.
    pub(crate) fn push(&mut self, grid: &crate::world::TileGrid, pos: crate::Pos, unit: u32) {
        match grid.index_of(pos) {
            Some(index) => {
                let slots = std::sync::Arc::make_mut(&mut self.slots);
                if slots.len() <= index {
                    slots.resize_with(grid.len().max(index + 1), Vec::new);
                }
                slots[index].push(unit);
            }
            None => self.off_grid.entry(pos).or_default().push(unit),
        }
    }

    /// Take `unit` off `pos`, keeping the others in order.
    pub(crate) fn remove(&mut self, grid: &crate::world::TileGrid, pos: crate::Pos, unit: u32) {
        match grid.index_of(pos) {
            Some(index) => {
                if self.slots.get(index).is_some_and(|ids| ids.contains(&unit)) {
                    std::sync::Arc::make_mut(&mut self.slots)[index].retain(|id| *id != unit);
                }
            }
            None => {
                if let Some(ids) = self.off_grid.get_mut(&pos) {
                    ids.retain(|id| *id != unit);
                    if ids.is_empty() {
                        self.off_grid.remove(&pos);
                    }
                }
            }
        }
    }

    pub(crate) fn clear(&mut self) {
        self.slots = std::sync::Arc::default();
        self.off_grid.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every position, on the grid or off it, answers exactly what a map by
    /// position would: the city last put there and not taken away.
    #[test]
    fn the_city_index_answers_as_a_map_by_position_does() {
        let grid = crate::world::TileGrid::new(12, 8);
        let mut on: Vec<crate::Pos> = grid.keys().copied().collect();
        on.sort();
        let off = [(-40, 3), (99, 99), (0, -50)];
        let all: Vec<crate::Pos> = on.iter().copied().chain(off).collect();
        let mut index = CityIndex::default();
        let mut reference: PosMap<u32> = PosMap::default();
        let check = |index: &CityIndex, reference: &PosMap<u32>| {
            for pos in &all {
                assert_eq!(
                    index.get(&grid, *pos),
                    reference.get(pos).copied(),
                    "{pos:?}"
                );
            }
        };
        check(&index, &reference);
        for (step, pos) in all.iter().enumerate() {
            if step % 3 != 1 {
                index.insert(&grid, *pos, step as u32);
                reference.insert(*pos, step as u32);
            }
        }
        check(&index, &reference);
        // A second city on a position replaces the first, as a map's does.
        index.insert(&grid, on[0], 500);
        reference.insert(on[0], 500);
        index.insert(&grid, off[0], 501);
        reference.insert(off[0], 501);
        check(&index, &reference);
        // A copy shares the ids until one side changes them.
        let copy = index.clone();
        for pos in all.iter().step_by(2) {
            index.remove(&grid, *pos);
            reference.remove(pos);
        }
        check(&index, &reference);
        assert_eq!(copy.get(&grid, on[0]), Some(500));
        index.clear();
        reference.clear();
        check(&index, &reference);
    }

    /// Every position answers exactly the units a map by position of lists
    /// would: those put there and not taken away, in the order they came.
    #[test]
    fn the_unit_index_answers_as_a_map_by_position_does() {
        let grid = crate::world::TileGrid::new(10, 6);
        let mut on: Vec<crate::Pos> = grid.keys().copied().collect();
        on.sort();
        let off = [(-40, 3), (99, 99)];
        let all: Vec<crate::Pos> = on.iter().copied().chain(off).collect();
        let mut index = UnitIndex::default();
        let mut reference: PosMap<Vec<u32>> = PosMap::default();
        let check = |index: &UnitIndex, reference: &PosMap<Vec<u32>>| {
            for pos in &all {
                let want: &[u32] = reference.get(pos).map_or(&[], Vec::as_slice);
                assert_eq!(index.get(&grid, *pos), want, "{pos:?}");
            }
        };
        let mut next = 0u32;
        for round in 0..3 {
            for (step, pos) in all.iter().enumerate() {
                if (step + round) % 2 == 0 {
                    next += 1;
                    index.push(&grid, *pos, next);
                    reference.entry(*pos).or_default().push(next);
                }
            }
        }
        check(&index, &reference);
        let copy = index.clone();
        let before: Vec<u32> = copy.get(&grid, on[0]).to_vec();
        // Take the middle unit off some tiles, an absent one off others.
        for (step, pos) in all.iter().enumerate() {
            let target = if step % 3 == 0 {
                reference.get(pos).and_then(|ids| ids.get(1)).copied()
            } else {
                Some(999_999)
            };
            if let Some(unit) = target {
                index.remove(&grid, *pos, unit);
                if let Some(ids) = reference.get_mut(pos) {
                    ids.retain(|id| *id != unit);
                    if ids.is_empty() {
                        reference.remove(pos);
                    }
                }
            }
        }
        check(&index, &reference);
        assert_eq!(copy.get(&grid, on[0]), before.as_slice());
        index.clear();
        reference.clear();
        check(&index, &reference);
    }
}
