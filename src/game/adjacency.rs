//! District adjacency calculator: what every district would earn at every
//! candidate plot, before anything is built.
//!
//! [`Game::district_adjacency`] answers what a district *does* earn where it
//! stands.  Planning needs the other direction — every plot a district *could*
//! stand on, ranked, with the ledger that explains each figure — and it needs
//! two assumptions live yields must never make: a city center that does not
//! exist yet (settlement planning) and foundations counted as the districts
//! they will become (build-order planning).  Those assumptions are a
//! [`PlanAssumption`] threaded through the one adjacency implementation in
//! `game.rs`, so the calculator can never drift from what the engine pays.
//!
//! Consumers: `AdvancedAi::settle_value` (a site's district potential),
//! production site ranking (via `district_yields`, unchanged), and the
//! server's `GET /adjacency` for anyone planning against a live game.

use super::{AdjacencySource, Game};
use crate::name::Name;
use crate::rules::Yields;
use crate::Pos;

/// Planning assumptions layered onto the live adjacency rules.  `None`
/// everywhere reproduces live yields exactly.
#[derive(Clone, Copy, Default)]
pub struct PlanAssumption {
    /// Treat this tile as a city center — the city a settler is about to
    /// found.  Pays the `city_center` and `district` adjacency keys.
    pub city_center: Option<Pos>,
    /// Count district foundations under construction as the districts they
    /// will become, so a queued Aqueduct already attracts the Industrial
    /// Zone beside it.
    pub foundations: bool,
}

impl PlanAssumption {
    pub(crate) fn treats_as_city_center(&self, pos: Pos) -> bool {
        self.city_center == Some(pos)
    }
}

/// One candidate plot in a district's forecast.
#[derive(Clone, serde::Serialize)]
pub struct SiteForecast {
    pub pos: Pos,
    /// Adjacency yields alone — base district yields are site-independent.
    pub yields: Yields,
    /// The ledger: every source line that pays at this plot.
    pub sources: Vec<AdjacencySource>,
}

/// Everything one district could earn in one city: every legal (or, for
/// settlement, plausible) plot, best first.
#[derive(Clone, serde::Serialize)]
pub struct DistrictForecast {
    /// The concrete district this civilization builds — `seowon` for Korea
    /// where everyone else forecasts `campus`.
    pub district: Name,
    /// The base family, so callers can group uniques with their stand-ins.
    pub family: Name,
    pub sites: Vec<SiteForecast>,
}

/// One wanted district's forecast in a settlement look-ahead: the plot the
/// greedy assignment gave it and the adjacency it would earn there. `None`
/// when no unclaimed plot in the disk can legally hold it.
pub type LookaheadSite = Option<(Pos, Yields)>;

impl Game {
    /// The district `pid`'s civilization actually builds for `family`: the
    /// civ's unique replacement where one exists, otherwise the family
    /// itself.  (The AI keeps its own copy in `ai.rs`; this is the
    /// game-side canonical form so the calculator cannot depend on an AI.)
    pub fn civ_district_variant(&self, pid: usize, family: &str) -> Name {
        let civ = self.players[pid].civ.as_str();
        self.rules
            .districts
            .iter()
            .find(|(_, spec)| {
                spec.replaces == Some(Name::new(family)) && spec.unique_to.as_deref() == Some(civ)
            })
            .map(|(name, _)| *name)
            .unwrap_or_else(|| Name::new(family))
    }

    /// Every district `pid`'s civilization can ever build, uniques standing
    /// in for the families they replace, in ruleset order.  Tech and civic
    /// gates are deliberately ignored: planning is about the future.
    /// `adjacency_only` limits the list to districts with adjacency rules —
    /// the settlement path wants only those, the city calculator wants all.
    pub fn plannable_districts(&self, pid: usize, adjacency_only: bool) -> Vec<Name> {
        let civ = self.players[pid].civ.as_str();
        self.rules
            .districts
            .iter()
            .filter(|(name, spec)| {
                spec.buildable
                    && (!adjacency_only || !spec.adjacency.is_empty())
                    && spec.unique_to.as_ref().is_none_or(|unique| unique == civ)
                    && !self.rules.districts.values().any(|candidate| {
                        candidate.replaces == Some(**name)
                            && candidate.unique_to.as_deref() == Some(civ)
                    })
            })
            .map(|(name, _)| *name)
            .collect()
    }

    /// The full calculator for a standing city: every buildable district ×
    /// every legal site, adjacency yields and ledger, best site first.
    /// Foundations count as districts, so a build queue's own plan is part
    /// of the forecast.  Sites the city has already founded a district on
    /// are included (they come back from `district_sites`) — their forecast
    /// is what the queued district will earn.
    pub fn district_adjacency_calculator(&self, cid: u32) -> Vec<DistrictForecast> {
        let city = &self.cities[&cid];
        let owner = city.owner;
        let assume = PlanAssumption {
            city_center: None,
            foundations: true,
        };
        self.plannable_districts(owner, false)
            .into_iter()
            .map(|district| {
                let sites = self
                    .district_sites(cid, district)
                    .into_iter()
                    .map(|pos| self.site_forecast(district, pos, &assume))
                    .collect();
                DistrictForecast {
                    district,
                    family: self.district_family(district),
                    sites: rank(sites),
                }
            })
            .collect()
    }

    /// The calculator for a city that does not exist: what would each
    /// adjacency-bearing district earn near a city founded at `center`?
    /// Candidate plots are the unclaimed, physically legal tiles within
    /// `radius` (keep it ≤ 2 on hot paths; every check here is
    /// neighbour-local, so the sphere never pays a distance search).
    /// Ownership and tech gates are ignored — the future city will own the
    /// ground and research the unlocks; permanent terrain is what a settler
    /// can still choose between.
    pub fn settlement_adjacency_potential(
        &self,
        pid: usize,
        center: Pos,
        radius: i32,
    ) -> Vec<DistrictForecast> {
        let assume = PlanAssumption {
            city_center: Some(center),
            foundations: true,
        };
        let candidates = self.settlement_candidates(center, radius);
        self.plannable_districts(pid, true)
            .into_iter()
            .map(|district| {
                let sites = candidates
                    .iter()
                    .copied()
                    .filter(|pos| self.plot_fits_placement(pid, district, *pos, center))
                    .map(|pos| self.site_forecast(district, pos, &assume))
                    .collect();
                DistrictForecast {
                    district,
                    family: self.district_family(district),
                    sites: rank(sites),
                }
            })
            .collect()
    }

    /// The settle-scoring digest: for each adjacency-bearing district family,
    /// the adjacency its best plot within `radius` would pay, summed into one
    /// figure.  No ledgers are allocated — this is the hot path under
    /// `AdvancedAi::settle_value`, which may sweep every land tile on the
    /// map.  A family whose best plot is negative (a hemmed-in Seowon)
    /// contributes nothing rather than a penalty: the planner simply would
    /// not build it there.
    pub fn settlement_adjacency_summary(&self, pid: usize, center: Pos, radius: i32) -> Yields {
        let positions = self.wdisk(center, radius);
        self.settlement_adjacency_summary_from_positions(pid, center, &positions)
    }

    pub(crate) fn settlement_adjacency_summary_from_positions(
        &self,
        pid: usize,
        center: Pos,
        positions: &[Pos],
    ) -> Yields {
        let assume = PlanAssumption {
            city_center: Some(center),
            foundations: true,
        };
        let districts: Vec<(Name, Name)> = self
            .plannable_districts(pid, true)
            .into_iter()
            .map(|district| (district, self.district_family(district)))
            .collect();
        let mut bests: Vec<(f64, Yields)> =
            districts.iter().map(|_| (0.0, Yields::default())).collect();
        for pos in positions
            .iter()
            .copied()
            .filter(|pos| *pos != center && self.plot_could_hold_a_district(*pos))
        {
            let candidate_neighbors = self.nbrs(pos);
            let mut neighbor_tiles = [None; 6];
            for (index, neighbor) in candidate_neighbors.iter().copied().enumerate() {
                neighbor_tiles[index] = self.map.get(neighbor);
            }
            let district_count =
                self.settlement_neighbor_district_count(pos, &neighbor_tiles, &assume);
            for (district_index, (district, family)) in districts.iter().copied().enumerate() {
                if !self.plot_fits_placement_with_neighbors(
                    pid,
                    district,
                    pos,
                    center,
                    &candidate_neighbors,
                ) {
                    continue;
                }
                let yields = self
                    .district_adjacency_assuming_with_family_and_neighbors_and_district_count(
                        district,
                        pos,
                        Some(&assume),
                        None,
                        family,
                        &neighbor_tiles,
                        district_count,
                    );
                if yields.total() > bests[district_index].0 {
                    bests[district_index] = (yields.total(), yields);
                }
            }
        }
        let mut summary = Yields::default();
        for (_, best_yields) in bests {
            summary.add(best_yields);
        }
        summary
    }

    /// The settle-scoring look-ahead: given the district **families** a new
    /// city would most likely build, in the order it would build them, the
    /// plot each one would actually get and the adjacency it would pay
    /// there.
    ///
    /// This differs from [`Self::settlement_adjacency_summary`] in the two
    /// ways that matter to a settler choosing between sites:
    ///
    /// - It prices only the districts the planner is going to ask for, not
    ///   every adjacency-bearing family the ruleset knows. A Science seat's
    ///   site is its Campus plot; a Holy Site's +4 at the same site is
    ///   nothing to it, and the summary counted both.
    /// - Plots are assigned **once**, first district first. The summary let
    ///   every family claim the same best plot, so one river-mountain hex
    ///   was paid for as a Campus, a Holy Site and a Commercial Hub at the
    ///   same time.
    ///
    /// Families are resolved to the civilization's own variant
    /// (`seowon` for Korea's `campus`). Same placement and adjacency rules,
    /// same assumptions (`center` as the city, foundations as districts) as
    /// the summary; tech and ownership gates are ignored — the future city
    /// will own the ground and research the unlocks.
    pub fn settlement_district_lookahead_from_positions(
        &self,
        pid: usize,
        center: Pos,
        positions: &[Pos],
        families: &[Name],
    ) -> Vec<LookaheadSite> {
        let assume = PlanAssumption {
            city_center: Some(center),
            foundations: true,
        };
        let districts: Vec<(Name, Name)> = families
            .iter()
            .map(|family| (self.civ_district_variant(pid, family), *family))
            .collect();
        // Every plot's adjacency for every wanted district, computed once;
        // the greedy pass below only reads it.
        let mut table: Vec<(Pos, Vec<Option<Yields>>)> = Vec::new();
        for pos in positions
            .iter()
            .copied()
            .filter(|pos| *pos != center && self.plot_could_hold_a_district(*pos))
        {
            let candidate_neighbors = self.nbrs(pos);
            let mut neighbor_tiles = [None; 6];
            for (index, neighbor) in candidate_neighbors.iter().copied().enumerate() {
                neighbor_tiles[index] = self.map.get(neighbor);
            }
            let district_count =
                self.settlement_neighbor_district_count(pos, &neighbor_tiles, &assume);
            let row = districts
                .iter()
                .copied()
                .map(|(district, family)| {
                    if !self.rules.districts.contains_name(district)
                        || !self.plot_fits_placement_with_neighbors(
                            pid,
                            district,
                            pos,
                            center,
                            &candidate_neighbors,
                        )
                    {
                        return None;
                    }
                    Some(
                        self.district_adjacency_assuming_with_family_and_neighbors_and_district_count(
                            district,
                            pos,
                            Some(&assume),
                            None,
                            family,
                            &neighbor_tiles,
                            district_count,
                        ),
                    )
                })
                .collect();
            table.push((pos, row));
        }
        let mut taken: Vec<Pos> = Vec::with_capacity(districts.len());
        let mut sites = Vec::with_capacity(districts.len());
        for district_index in 0..districts.len() {
            let mut best: LookaheadSite = None;
            for (pos, row) in &table {
                if taken.contains(pos) {
                    continue;
                }
                let Some(yields) = row[district_index] else {
                    continue;
                };
                // Strictly better wins; a tie keeps the earlier plot so the
                // answer does not depend on how `positions` was ordered
                // beyond its own determinism.
                if best.is_none_or(|(_, held)| yields.total() > held.total()) {
                    best = Some((*pos, yields));
                }
            }
            if let Some((pos, _)) = best {
                taken.push(pos);
            }
            sites.push(best);
        }
        sites
    }

    /// How many standing or assumed districts neighbour `pos` — the count
    /// the `district` adjacency key pays on. Shared by the summary and the
    /// look-ahead so the two cannot disagree about what a neighbour is.
    fn settlement_neighbor_district_count(
        &self,
        pos: Pos,
        neighbor_tiles: &[Option<&crate::world::Tile>; 6],
        assume: &PlanAssumption,
    ) -> usize {
        let gaul = self
            .map
            .get(pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|city_id| self.cities.get(&city_id))
            .is_some_and(|city| self.players[city.owner].civ == "Gaul");
        neighbor_tiles
            .iter()
            .flatten()
            .filter(|tile| {
                !gaul
                    && ((tile.district.is_some() && !tile.pillaged)
                        || self.city_at(tile.pos).is_some()
                        || assume.treats_as_city_center(tile.pos)
                        || (assume.foundations && tile.district_foundation.is_some()))
            })
            .count()
    }

    /// Unclaimed, physically buildable plots within `radius` of `center`.
    fn settlement_candidates(&self, center: Pos, radius: i32) -> Vec<Pos> {
        self.wdisk(center, radius)
            .into_iter()
            .filter(|pos| *pos != center && self.plot_could_hold_a_district(*pos))
            .collect()
    }

    fn site_forecast(&self, district: Name, pos: Pos, assume: &PlanAssumption) -> SiteForecast {
        let mut sources = Vec::new();
        let yields =
            self.district_adjacency_assuming(district, pos, Some(assume), Some(&mut sources));
        SiteForecast {
            pos,
            yields,
            sources,
        }
    }

    /// The physical half of `district_sites`'s fresh-plot filter, for a plot
    /// no city owns yet: nothing standing on it, nothing irreplaceable under
    /// it.  Tech-gated feature removal is deliberately not checked.
    fn plot_could_hold_a_district(&self, pos: Pos) -> bool {
        let Some(tile) = self.map.get(pos) else {
            return false;
        };
        if tile.flooded
            || tile.district.is_some()
            || tile.district_foundation.is_some()
            || tile.wonder.is_some()
            || tile.improvement.as_deref() == Some("national_park")
            || tile.owner_city.is_some()
            || !self.rules.is_passable(tile)
        {
            return false;
        }
        if tile.feature.as_ref().is_some_and(|feature| {
            self.rules
                .features
                .get(feature.as_str())
                .is_some_and(|spec| spec.natural_wonder)
        }) {
            return false;
        }
        // Deposits reserve their tile exactly as in `district_sites`; buried
        // antiquity is destroyed by construction, not reserved.
        !tile.resource.as_ref().is_some_and(|resource| {
            !matches!(
                self.rules.resources[resource].class.as_str(),
                "bonus" | "artifact"
            )
        })
    }

    /// The placement-rule half, mirroring `district_sites` with the assumed
    /// `center` standing in for the city.  Aqueducts, Dams and Canals never
    /// reach this (no adjacency rules), so their branches answer `false`
    /// rather than approximating hydrology the settler cannot see anyway.
    pub(crate) fn plot_fits_placement(
        &self,
        pid: usize,
        district: Name,
        pos: Pos,
        center: Pos,
    ) -> bool {
        let neighbors = self.nbrs(pos);
        self.plot_fits_placement_with_neighbors(pid, district, pos, center, &neighbors)
    }

    fn plot_fits_placement_with_neighbors(
        &self,
        pid: usize,
        district: Name,
        pos: Pos,
        center: Pos,
        neighbors: &crate::hex::Neighbors,
    ) -> bool {
        let spec = &self.rules.districts[district];
        let tile = &self.map.tiles[&pos];
        let is_water = self.rules.is_water(tile);
        if self.players[pid].civ == "Vietnam"
            && spec.specialty
            && !matches!(tile.feature.as_deref(), Some("forest" | "jungle" | "marsh"))
        {
            return false;
        }
        let adjacent_center = neighbors.contains(&center);
        match spec.placement.as_str() {
            // `adjacent_land` is asked by this arm alone and costs six map
            // lookups plus six water tests; every other placement rule threw
            // it away. A settlement scan asks this question once per district
            // at every plot of the disk, so computing it here rather than
            // above is most of the six-neighbour work in the function.
            "coast" | "water_park" => {
                is_water
                    && matches!(tile.terrain.as_str(), "coast" | "lake")
                    && tile.feature.as_deref() != Some("reef")
                    && neighbors.iter().any(|neighbor| {
                        self.map
                            .get(*neighbor)
                            .is_some_and(|tile| !self.rules.is_water(tile))
                    })
            }
            "flat_land" => !is_water && !tile.hills,
            "hills" => !is_water && tile.hills,
            "hills_adjacent_city" => !is_water && tile.hills && adjacent_center,
            "not_adjacent_city" => {
                !is_water
                    && !adjacent_center
                    && !neighbors
                        .iter()
                        .any(|neighbor| self.city_at(*neighbor).is_some())
            }
            "forest" => !is_water && matches!(tile.feature.as_deref(), Some("forest" | "jungle")),
            "vietnam_feature" => {
                !is_water && matches!(tile.feature.as_deref(), Some("forest" | "jungle" | "marsh"))
            }
            "aqueduct" | "dam" | "canal" => false,
            "land" | "" => !is_water && !spec.water,
            _ => is_water == spec.water,
        }
    }
}

/// Best site first: adjacency total, then position for determinism.
fn rank(mut sites: Vec<SiteForecast>) -> Vec<SiteForecast> {
    sites.sort_by(|a, b| {
        b.yields
            .total()
            .partial_cmp(&a.yields.total())
            .unwrap()
            .then(a.pos.cmp(&b.pos))
    });
    sites
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flattened_game() -> (Game, u32, Pos) {
        let mut game = Game::new_full(2, 28, 18, 91_779, 200, 0, false);
        let capital = found_capital(&mut game, 0);
        let center = game.cities[&capital].pos;
        (game, capital, center)
    }

    fn found_capital(game: &mut Game, pid: usize) -> u32 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("seat starts with a settler");
        let city = game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
        city
    }

    fn flatten(game: &mut Game, positions: &[Pos]) {
        for position in positions {
            let Some(tile) = game.map.tiles.get_mut(position) else {
                continue;
            };
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
        }
    }

    /// Every adjacency key any district ships must be one the engine's
    /// `count` recognises — a typo'd key would otherwise score zero forever
    /// and no live path would ever notice.
    #[test]
    fn every_shipped_adjacency_key_is_recognised() {
        let (game, _, _) = flattened_game();
        let known = [
            "self",
            "river",
            "mountain",
            "forest",
            "woods",
            "rainforest",
            "jungle",
            "natural_wonder",
            "reef",
            "great_barrier_reef",
            "geothermal_fissure",
            "pamukkale",
            "district",
            "city_center",
            "wonder",
            "coast_resource",
            "sea_resource",
            "strategic_resource",
            "luxury_resource",
            "resource",
            "mine",
            "quarry",
            "lumber_mill",
            "plantation",
            "farm",
        ];
        for (district, spec) in &game.rules.districts {
            for key in spec.adjacency.keys() {
                assert!(
                    known.contains(&key.as_str())
                        || game.rules.districts.contains_key(key.as_str()),
                    "{district} adjacency key {key} is neither a counted kind nor a district"
                );
            }
        }
    }

    /// The calculator covers every buildable district for the seat's civ and
    /// its ledgers add up to the yields it forecasts.
    #[test]
    fn calculator_covers_every_buildable_district_and_ledgers_add_up() {
        let (game, capital, _) = flattened_game();
        let owner = game.cities[&capital].owner;
        let forecasts = game.district_adjacency_calculator(capital);
        let expected = game.plannable_districts(owner, false);
        assert_eq!(
            forecasts.len(),
            expected.len(),
            "one forecast per plannable district"
        );
        for forecast in &forecasts {
            for site in &forecast.sites {
                let mut ledger = Yields::default();
                for source in &site.sources {
                    ledger.add(source.yields);
                }
                assert_eq!(
                    ledger, site.yields,
                    "{} at {:?}: ledger must add up to the forecast",
                    forecast.district, site.pos
                );
            }
        }
    }

    /// The look-ahead hands each wanted district its own plot, first
    /// district first: a Campus and a Holy Site both want the one plot
    /// that touches three mountains, and only the family named first gets
    /// it. The summary, by contrast, pays both for the same hex.
    #[test]
    fn lookahead_assigns_distinct_plots_first_district_first() {
        let mut game = Game::new_full(2, 28, 18, 91_779, 200, 0, false);
        let mut land: Vec<Pos> = game.map.tiles.keys().copied().collect();
        land.sort();
        let center =
            land.into_iter()
                .find(|pos| {
                    game.map.get(*pos).is_some_and(|tile| {
                        !game.rules.is_water(tile) && game.rules.is_passable(tile)
                    }) && game
                        .wdisk(*pos, 3)
                        .iter()
                        .all(|ring| game.map.get(*ring).is_some())
                })
                .expect("an interior site");
        for pos in game.wdisk(center, 3) {
            let tile = game.map.tiles.get_mut(&pos).unwrap();
            tile.terrain = Name::new("plains");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
            tile.river_edges = [false; 6];
            tile.owner_city = None;
        }
        let anchor = game.nbrs(center).into_iter().next().unwrap();
        let peaks: Vec<Pos> = game
            .nbrs(anchor)
            .into_iter()
            .filter(|pos| game.wdist(*pos, center) == 2)
            .collect();
        assert_eq!(peaks.len(), 3);
        for pos in &peaks {
            game.map.tiles.get_mut(pos).unwrap().terrain = Name::new("mountain");
        }
        let positions = game.wdisk(center, 2);
        let campus_first = game.settlement_district_lookahead_from_positions(
            0,
            center,
            &positions,
            &[Name::new("campus"), Name::new("holy_site")],
        );
        let (campus_plot, campus_adjacency) = campus_first[0].expect("a Campus plot");
        let (holy_plot, holy_adjacency) = campus_first[1].expect("a Holy Site plot");
        assert_eq!(
            campus_plot, anchor,
            "the first family takes the three-peak plot"
        );
        assert_ne!(
            holy_plot, anchor,
            "the second family is handed a different plot"
        );
        assert!(campus_adjacency.science >= 3.0);
        assert!(holy_adjacency.faith < 3.0);

        let holy_first = game.settlement_district_lookahead_from_positions(
            0,
            center,
            &positions,
            &[Name::new("holy_site"), Name::new("campus")],
        );
        assert_eq!(
            holy_first[0].unwrap().0,
            anchor,
            "order decides who gets the plot"
        );
        assert_ne!(holy_first[1].unwrap().0, anchor);

        // A district no unclaimed plot can hold comes back as `None` rather
        // than as a zero that looks like a placed district.
        let harbor_inland = game.settlement_district_lookahead_from_positions(
            0,
            center,
            &positions,
            &[Name::new("harbor")],
        );
        assert!(harbor_inland[0].is_none(), "no coast, no Harbor plot");
    }

    /// A foundation under construction already pays neighbouring plans: the
    /// calculator's whole point is seeing the cluster before it stands.
    #[test]
    fn foundations_count_toward_planned_neighbors() {
        let (mut game, _capital, center) = flattened_game();
        let site = game
            .nbrs(center)
            .into_iter()
            .find(|position| game.map.get(*position).is_some())
            .unwrap();
        let ring: Vec<Pos> = game.wdisk(site, 1);
        flatten(&mut game, &ring);
        let neighbor = game
            .nbrs(site)
            .into_iter()
            .find(|position| *position != center && game.map.get(*position).is_some())
            .unwrap();
        game.map
            .tiles
            .get_mut(&neighbor)
            .unwrap()
            .district_foundation = Some(crate::world::DistrictFoundation {
            district: crate::name!("campus"),
            cost: 0.0,
        });

        let live = game.district_adjacency_sources(crate::name!("holy_site"), site);
        assert!(
            !live
                .iter()
                .any(|source| source.source == "district" && source.count > 1),
            "live yields must not pay unfinished districts"
        );
        let assume = PlanAssumption {
            city_center: None,
            foundations: true,
        };
        let mut sources = Vec::new();
        game.district_adjacency_assuming(
            crate::name!("holy_site"),
            site,
            Some(&assume),
            Some(&mut sources),
        );
        let district_line = sources
            .iter()
            .find(|source| source.source == "district")
            .expect("the planned campus and the city center are neighbours");
        // City center plus the foundation: two planned district neighbours.
        assert_eq!(district_line.count, 2);
        assert_eq!(district_line.yields.faith, 1.0);
    }

    /// Settlement planning sees the city that is not there yet: a Harbor
    /// forecast beside the assumed center pays its `city_center` key, and a
    /// mountain ridge pays a Campus, with no city founded anywhere near.
    /// A deterministic patch of open ground away from both starting settlers.
    fn open_center(game: &Game) -> Pos {
        let mut land: Vec<Pos> = game.map.tiles.keys().copied().collect();
        land.sort();
        land.into_iter()
            .find(|pos| {
                game.map
                    .get(*pos)
                    .is_some_and(|tile| !game.rules.is_water(tile) && game.rules.is_passable(tile))
                    && game
                        .units
                        .values()
                        .all(|unit| game.wdist(unit.pos, *pos) > 4)
                    && game
                        .wdisk(*pos, 1)
                        .iter()
                        .all(|ring| game.map.get(*ring).is_some())
            })
            .expect("map has open ground")
    }

    #[test]
    fn settlement_potential_pays_the_assumed_center_and_terrain() {
        let mut game = Game::new_full(2, 28, 18, 91_779, 200, 0, false);
        let center = open_center(&game);
        let disk = game.wdisk(center, 2);
        flatten(&mut game, &disk);
        let plot = game
            .nbrs(center)
            .into_iter()
            .find(|pos| game.map.get(*pos).is_some())
            .unwrap();
        for pos in game.nbrs(plot) {
            if pos != center && game.map.get(pos).is_some() {
                game.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
            }
        }

        let forecasts = game.settlement_adjacency_potential(0, center, 2);
        let campus = forecasts
            .iter()
            .find(|forecast| forecast.family == crate::name!("campus"))
            .expect("campus is plannable for every stock civ");
        let best = campus.sites.first().expect("open plots exist");
        assert_eq!(best.pos, plot, "the ridge plot is the best campus site");
        // Four mountains (six neighbours minus the center and the off-map or
        // flattened remainder — at least the ring we raised) plus the
        // assumed center as half a district point.
        assert!(
            best.yields.science >= 4.0,
            "a mountain ring is a real campus: got {}",
            best.yields.science
        );
        assert!(
            best.sources
                .iter()
                .any(|source| source.source == "district"),
            "the assumed city center pays the district key"
        );
    }

    /// The settlement path only proposes plots that could really hold the
    /// district: water districts on coast, land districts on land, nothing
    /// on reserved deposits or natural wonders.
    #[test]
    fn settlement_plots_respect_physical_placement() {
        let game = Game::new_full(2, 28, 18, 91_779, 200, 0, false);
        let center = open_center(&game);
        for forecast in game.settlement_adjacency_potential(0, center, 2) {
            let spec = &game.rules.districts[forecast.district];
            for site in &forecast.sites {
                let tile = game
                    .map
                    .get(site.pos)
                    .expect("forecast sites are on the map");
                assert_eq!(
                    game.rules.is_water(tile),
                    spec.water,
                    "{} forecast at {:?} is in the wrong domain",
                    forecast.district,
                    site.pos
                );
                assert!(
                    tile.owner_city.is_none(),
                    "claimed ground is not settleable"
                );
            }
        }
    }
}
