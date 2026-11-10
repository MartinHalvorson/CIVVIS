//! Appeal and national parks.
//!
//! A tile's appeal (`tile_appeal`), the rhombus of tiles a park covers and
//! the diamond it may be founded on, whether a site is valid, the sites open
//! to a Naturalist (`national_park_sites`, `national_park_site_at`), and the
//! parks established so far (`established_national_parks`).
//!
//! Moved verbatim out of the `impl Game` block in `game/city.rs`
//! (2026-11-05). Methods the rest of `city` calls are `pub(super)`;
//! methods that were `pub(super)` (all of `game`) are
//! `pub(in crate::game)`, the same reach; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Gathering Storm appeal of a tile from adjacent terrain, features,
    /// improvements, wonders, and districts. River appeal belongs to the
    /// evaluated tile itself; the other modifiers come from its neighbors.
    /// Appeal of a tile, memoized for the life of a [`Game::query_memo`] scope.
    ///
    /// The answer is a pure function of the tile, its six neighbours, and the
    /// owning city's wonders and Governor -- none of which a read-only scope
    /// changes, which is the same contract [`Game::city_yields`] is cached
    /// under. Outside a scope this computes as it always did, so a caller that
    /// is mutating between reads cannot see a stale figure.
    pub fn tile_appeal(&self, position: Pos) -> i32 {
        if let Some(memo) = self.query_memo.appeal.borrow().as_ref() {
            if let Some(appeal) = memo.get(&position) {
                return *appeal;
            }
        }
        let appeal = self.tile_appeal_uncached(position);
        if let Some(memo) = self.query_memo.appeal.borrow_mut().as_mut() {
            memo.insert(position, appeal);
        }
        appeal
    }

    pub(in crate::game) fn tile_appeal_uncached(&self, position: Pos) -> i32 {
        // The host's own count where the export carried one. The derivation
        // below reads the six neighbours the board can see; the host's
        // reads the ones it has, fog included, plus every modifier the model
        // does not carry.
        if let Some(appeal) = self.observed_appeal.get(&position) {
            return *appeal;
        }
        let Some(tile) = self.map.get(position) else {
            return 0;
        };
        let owner_city_id = tile.owner_city;
        let owner = owner_city_id
            .and_then(|city_id| self.cities.get(&city_id))
            .map(|city| city.owner);
        let biosphere = owner
            .map(|pid| self.empire_wonder_effect(pid, "rainforest_marsh_adjacent_appeal"))
            .unwrap_or(0.0) as i32;
        let mut appeal = i32::from(tile.has_river());
        for neighbor in self.nbrs(position) {
            let Some(adjacent) = self.map.get(neighbor) else {
                continue;
            };
            if matches!(adjacent.terrain.as_str(), "mountain" | "coast" | "lake") {
                appeal += 1;
            }
            // Features.Appeal, read rather than listed: Woods and an Oasis are
            // +1, Rainforest, Marsh and Floodplains -1, and a natural wonder is
            // +2 -- except the Cliffs of Dover and Uluru, which are +4.
            appeal += adjacent
                .feature
                .as_deref()
                .and_then(|feature| self.rules.features.get(feature))
                .map_or(0, |spec| spec.appeal.round() as i32);
            if adjacent.owner_city == owner_city_id
                && adjacent.improvement.is_none()
                && adjacent.feature.is_some()
            {
                if let Some(city_id) = owner_city_id {
                    if let Some(city) = self.cities.get(&city_id) {
                        appeal += self.governor_effect(city.owner, city.id, "appeal") as i32;
                    }
                }
            }
            if biosphere != 0 && matches!(adjacent.feature.as_deref(), Some("jungle" | "marsh")) {
                appeal += biosphere;
            }

            if adjacent.wonder.is_some() {
                appeal += 1;
            }
            if adjacent.pillaged {
                appeal -= 1;
            }
            if let Some(district) = adjacent.district {
                appeal += self
                    .rules
                    .districts
                    .get(district.as_str())
                    .map(|spec| spec.appeal.round() as i32)
                    .unwrap_or(0);
            }

            if !adjacent.pillaged {
                appeal += adjacent
                    .improvement
                    .as_deref()
                    .and_then(|improvement| self.rules.improvements.get(improvement))
                    .and_then(|improvement| improvement.effects.get("adjacent_appeal"))
                    .copied()
                    .unwrap_or(0.0) as i32;
            }
        }
        if let Some(owner) = owner {
            appeal += self.empire_wonder_effect(owner, "empire_appeal") as i32;
        }
        if let Some(city) = tile
            .owner_city
            .and_then(|city_id| self.cities.get(&city_id))
        {
            appeal += city
                .wonders
                .keys()
                .map(|wonder| {
                    self.rules.wonders[wonder]
                        .effects
                        .get("city_appeal")
                        .copied()
                        .unwrap_or(0.0) as i32
                })
                .sum::<i32>();
        }
        appeal
    }

    /// The four tiles of a park on a globe: a tile, two of its neighbours that
    /// also touch each other, and the tile opposite. The pair is taken at a
    /// fixed place in the tile's own ring so that one tile names one park.
    pub(in crate::game) fn park_rhombus(&self, top: Pos) -> Option<[Pos; 4]> {
        let ring = self.nbrs(top);
        if ring.len() < 6 {
            return None;
        }
        let (left, right) = (ring[4], ring[5]);
        let far = self
            .nbrs(left)
            .into_iter()
            .find(|pos| *pos != top && self.nbrs(right).contains(pos))?;
        Some([top, left, right, far])
    }

    pub(in crate::game) fn national_park_diamond(&self, top: Pos) -> Option<[Pos; 4]> {
        // A park is a rhombus: a tile, two neighbours that touch each other,
        // and the tile those two share on the far side. On a flat map that is
        // a fixed set of coordinate offsets; on a globe the same shape is
        // built from the tile's own ring of neighbours, which is the only
        // definition that survives a world with no fixed compass.
        let positions = match self.map.sphere() {
            Some(_) => self.park_rhombus(top)?,
            None => [
                top,
                (top.0 - 1, top.1 + 1),
                (top.0, top.1 + 1),
                (top.0 - 1, top.1 + 2),
            ]
            .map(|position| hex::canon(position, self.map.width)),
        };
        let mut sorted = positions;
        sorted.sort_unstable();
        let distinct = sorted.windows(2).all(|pair| pair[0] != pair[1]);
        (distinct
            && positions
                .iter()
                .all(|position| self.map.tiles.contains_key(position)))
        .then_some(positions)
    }

    pub(in crate::game) fn valid_national_park_site(
        &self,
        pid: usize,
        positions: &[Pos; 4],
    ) -> bool {
        let Some(city_id) = self.map.tiles[&positions[0]].owner_city else {
            return false;
        };
        self.cities
            .get(&city_id)
            .is_some_and(|city| city.owner == pid)
            && positions.iter().all(|position| {
                let tile = &self.map.tiles[position];
                let natural_wonder = tile.feature.as_ref().is_some_and(|feature| {
                    self.rules
                        .features
                        .get(feature.as_str())
                        .is_some_and(|feature| feature.natural_wonder)
                });
                tile.owner_city == Some(city_id)
                    && !tile.flooded
                    && !tile.submerged
                    && tile.improvement.is_none()
                    && tile.district.is_none()
                    && tile.district_foundation.is_none()
                    && tile.wonder.is_none()
                    && self.city_at(*position).is_none()
                    && (tile.terrain == "mountain"
                        || natural_wonder
                        || self.tile_appeal(*position) >= 2)
            })
            && positions
                .iter()
                .any(|position| self.rules.is_passable(&self.map.tiles[position]))
    }

    pub fn national_park_sites(&self, pid: usize) -> Vec<[Pos; 4]> {
        if self.tree_effect(pid, "national_parks") <= 0.0 {
            return Vec::new();
        }
        let mut sites: Vec<[Pos; 4]> = self
            .map
            .tiles
            .keys()
            .copied()
            .filter_map(|top| self.national_park_diamond(top))
            .filter(|positions| self.valid_national_park_site(pid, positions))
            .collect();
        sites.sort();
        sites.dedup();
        sites
    }

    pub(in crate::game) fn national_park_site_at(
        &self,
        pid: usize,
        position: Pos,
    ) -> Option<[Pos; 4]> {
        let possible_tops = [
            position,
            (position.0 + 1, position.1 - 1),
            (position.0, position.1 - 1),
            (position.0 + 1, position.1 - 2),
        ];
        possible_tops
            .into_iter()
            .filter_map(|top| self.national_park_diamond(top))
            .filter(|positions| self.valid_national_park_site(pid, positions))
            .max_by_key(|positions| {
                (
                    positions
                        .iter()
                        .map(|position| self.tile_appeal(*position).max(0))
                        .sum::<i32>(),
                    std::cmp::Reverse(*positions),
                )
            })
    }

    pub(in crate::game) fn established_national_parks(&self, pid: usize) -> Vec<(u32, [Pos; 4])> {
        if let Some(memo) = self.query_memo.national_parks.borrow().as_ref() {
            if let Some(parks) = memo.get(&pid) {
                return parks.clone();
            }
        }
        let parks = self.established_national_parks_uncached(pid);
        if let Some(memo) = self.query_memo.national_parks.borrow_mut().as_mut() {
            memo.insert(pid, parks.clone());
        }
        parks
    }

    pub(in crate::game) fn established_national_parks_uncached(
        &self,
        pid: usize,
    ) -> Vec<(u32, [Pos; 4])> {
        let mut parks = Vec::new();
        let mut used = BTreeSet::new();
        // Every established park has four owned tiles carrying the
        // `national_park` improvement. Derive the small set of possible tops
        // from those tiles instead of probing every tile on the map whenever
        // a city asks for its Amenities.
        let national_park = crate::name!("national_park");
        let candidate_tops: BTreeSet<Pos> = self
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .flat_map(|city| city.owned_tiles.iter().copied())
            .filter(|position| self.map.tiles[position].improvement == Some(national_park))
            .flat_map(|position| -> Vec<Pos> {
                match self.map.sphere() {
                    // Any tile of a rhombus lies within two steps of its top.
                    Some(_) => self.wdisk(position, 2),
                    None => [
                        position,
                        (position.0 + 1, position.1 - 1),
                        (position.0, position.1 - 1),
                        (position.0 + 1, position.1 - 2),
                    ]
                    .map(|top| hex::canon(top, self.map.width))
                    .to_vec(),
                }
            })
            .collect();
        for top in candidate_tops {
            let Some(positions) = self.national_park_diamond(top) else {
                continue;
            };
            let Some(city_id) = self.map.tiles[&positions[0]].owner_city else {
                continue;
            };
            if self
                .cities
                .get(&city_id)
                .is_some_and(|city| city.owner == pid)
                && positions.iter().all(|position| {
                    let tile = &self.map.tiles[position];
                    tile.owner_city == Some(city_id)
                        && tile.improvement.as_deref() == Some("national_park")
                })
                && positions.iter().all(|position| !used.contains(position))
            {
                used.extend(positions);
                parks.push((city_id, positions));
            }
        }
        parks.sort();
        parks.dedup();
        parks
    }
}
