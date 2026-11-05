//! Borders: how a city's territory grows.
//!
//! The culture cost of a city's next tile (`border_influence_cost`),
//! growing its borders by one tile (`expand_borders`), and the culture
//! bomb that claims the ring around a new district or improvement
//! (`culture_bomb`). Forecasting where borders will grow is in
//! `game/border_forecast.rs`.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    pub(super) fn culture_bomb(&mut self, cid: u32, center: Pos) {
        let claims: Vec<Pos> = self
            .nbrs(center)
            .into_iter()
            .filter(|position| {
                self.map
                    .get(*position)
                    .is_some_and(|tile| tile.owner_city.is_none())
            })
            .collect();
        for position in claims {
            self.map.tiles.get_mut(&position).unwrap().owner_city = Some(cid);
            if !self.cities[&cid].owned_tiles.contains(&position) {
                self.cities
                    .get_mut(&cid)
                    .unwrap()
                    .owned_tiles
                    .push(position);
            }
        }
    }

    pub(super) fn expand_borders(&mut self, cid: u32) {
        let city_pos = self.cities[&cid].pos;
        let owned = self.cities[&cid].owned_tiles.clone();
        let mut candidates = BTreeSet::new();
        for pos in &owned {
            for n in self.nbrs(*pos) {
                let t = match self.map.get(n) {
                    Some(t) => t,
                    None => continue,
                };
                // Cultural expansion reaches two rings beyond the workable
                // radius in Civ VI. Gold purchases remain capped at ring 3.
                if t.owner_city.is_some() || self.wdist(n, city_pos) > 5 {
                    continue;
                }
                candidates.insert(n);
            }
        }

        // Distance is priced, not gated: `PLOT_INFLUENCE_RING_COST` adds 100
        // per ring, so a nearer plot normally wins, but a Resource or Natural
        // Wonder at -105 is worth exactly five more than one extra ring and
        // does pull the border outward past a barren neighbour.
        if candidates.is_empty() {
            return;
        }
        let mut scored: Vec<(Pos, f64)> = candidates
            .into_iter()
            .map(|position| (position, self.border_influence_cost(city_pos, position)))
            .collect();
        let best_score = scored
            .iter()
            .map(|(_, score)| *score)
            .min_by(f64::total_cmp)
            .unwrap();
        scored.retain(|(_, score)| *score == best_score);
        // Stock chooses randomly among exactly tied influence scores. Sort
        // first so a fixed game seed remains reproducible across save loads.
        scored.sort_by_key(|(position, _)| *position);
        let n = scored[self.rng.below(scored.len())].0;
        {
            self.map.tiles.get_mut(&n).unwrap().owner_city = Some(cid);
            self.cities.get_mut(&cid).unwrap().owned_tiles.push(n);
        }
    }

    /// Lower is more attractive to Civ VI's cultural tile picker.
    ///
    /// These are the shipped `PLOT_INFLUENCE_*` values: `RING_COST` 100 per
    /// ring out from the City Center, resources and Natural Wonders -105,
    /// water +25, existing improvements -5 (or +100 for a Barbarian Camp),
    /// and each raw yield point -1. Terrain contributes its database
    /// `InfluenceCost`. Nearby unclaimed strategic/luxury resources and
    /// Natural Wonders provide the stock one-point surface-tension nudge.
    ///
    /// The -105 is calibrated against the 100 ring cost on purpose: one
    /// Resource is worth slightly more than one extra ring of distance, so a
    /// city reaches past a barren adjacent plot to take it.
    pub(super) fn border_influence_cost(&self, city_pos: Pos, position: Pos) -> f64 {
        let tile = &self.map.tiles[&position];
        let mut cost = 100.0 * self.wdist(position, city_pos) as f64;
        cost += match tile.terrain.as_str() {
            "grassland" | "plains" | "ocean" => 1.0,
            "desert" | "tundra" | "snow" | "coast" | "lake" => 2.0,
            "mountain" => 3.0,
            _ => 2.0,
        };
        if self.rules.is_water(tile) {
            cost += 25.0;
        }
        match tile.improvement.as_deref() {
            Some("barbarian_camp") => cost += 100.0,
            Some(_) => cost -= 5.0,
            None => {}
        }
        if tile.resource.is_some() {
            cost -= 105.0;
        }
        if self.tile_is_natural_wonder(tile) {
            cost -= 105.0;
        }
        cost -= self.rules.tile_yields(tile).total();

        for neighbor in self.nbrs(position) {
            let adjacent = &self.map.tiles[&neighbor];
            if adjacent.owner_city.is_some() {
                continue;
            }
            if self.tile_is_natural_wonder(adjacent) {
                cost -= 1.0;
            }
            if self.wdist(neighbor, city_pos) <= 3
                && adjacent.resource.as_ref().is_some_and(|resource| {
                    // Borders are not drawn toward a resource nobody can see,
                    // and an Antiquity Site stays buried until Natural History.
                    !matches!(
                        self.rules.resources[resource].class.as_str(),
                        "bonus" | "artifact"
                    )
                })
            {
                cost -= 1.0;
            }
        }
        cost
    }
}
