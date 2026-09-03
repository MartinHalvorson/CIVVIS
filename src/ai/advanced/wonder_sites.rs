//! Settle beside a natural wonder, and see its far side first.
//!
//! Two opt-in genes from the live seat civvis-20260825T162542Z: Mount Roraima
//! stood three tiles east of Rome from turn 1, and the second, third and
//! fourth Settlers all walked west. Two mechanisms, neither of them a tuning:
//!
//! 1. **The site model could not see the wonder.** `Game::player_tile_yields`
//!    — what a Citizen is actually paid — adds every neighbouring feature's
//!    `adjacent_yields` (Roraima: +1 Science +1 Faith on each of its six
//!    neighbours; only natural wonders carry the field). Every settle-site
//!    valuation — `settlement_growth_forecast_from_positions`,
//!    `settlement_prefilter_score`, `legacy_settle_value` — reads the
//!    tile-local `Rules::tile_yields` instead, so the projection is invisible;
//!    and because the wonder's own tiles are impassable they are dropped from
//!    the work-tile list, so a footprint holding a wonder has FEWER jobs. The
//!    only credit the stock score pays is the district planner's Holy Site
//!    adjacency — +0.4 for Roraima beside an all-grassland site
//!    (`a_wonder_in_the_footprint_reads_no_better_without_the_gene_and_better_with_it`).
//!    Nothing else in the score knows a wonder is there: not the +1 Amenity
//!    the host pays a city holding one (328 city-turns at
//!    `amenities_natural_wonders = 1` across the 08-25 live runs), not the
//!    +2 Appeal on every neighbour, not the +2 Holy Site adjacency per wonder
//!    tile, not the era score. `wonder-adjacent-sites` pays the projection
//!    exactly as the engine does and nothing flat: only the tiles the
//!    forecast actually works earn it, which is the engine's own answer.
//!    Its version 2 adds a small flat footprint credit for the rest.
//!
//!    ⚠ This is not #1419's `wonder-ring-settle-value`, culled in #2464 at
//!    **−0.553 pp over ≥30k seats**. That gene never touched the yields a
//!    worked tile earns; it paid a flat credit per wonder FEATURE in the disk
//!    — appeal × a weight plus the ring yields × all six ring tiles, as if
//!    every neighbour were worked from turn one — on top of a forecast that
//!    still could not see the projection. A speculative flat credit lost;
//!    the engine-exact projection was never screened. Version 1 here is the
//!    projection alone; version 2 adds a credit sized at a third of a river,
//!    capped at one, so the batch prices the two apart.
//!
//! 2. **The ring was never seen.** The four tiles from which a city could
//!    stand beside Roraima — civ6 (15,8), (15,9), (15,11), (16,10) — were
//!    still unexplored at turn 41: the wonder blocks sight through itself, the
//!    Scout fanned west (`explore_commit` prefers the deepest fog farthest from
//!    home, and an eighteen-tile pocket three tiles from the capital is
//!    neither), and an unknown tile is never a site candidate nor a work tile.
//!    `wonder-ring-recon` makes an explorer walk the unseen ring of a natural
//!    wonder within settling range of an own city before it picks a frontier,
//!    so the site exists to be priced.

use std::collections::HashSet;

use super::AdvancedAi;
use crate::ai::BasicAi;
use crate::game::Game;
use crate::rules::Yields;
use crate::think;
use crate::world::Tile;
use crate::Pos;

/// Version 2 only: what one natural-wonder tile inside a site's radius-two
/// footprint is worth beyond the yields it projects — the +1 Amenity the host
/// pays the city that holds it, +2 Appeal on each neighbour, +2 Holy Site
/// adjacency, era score at discovery. Deliberately small: #1419's flat credit
/// (see the module doc) lost at scale. The base-housing term pays
/// `(5 - 2) * 4 = 12` for a river; one wonder tile is a third of that and the
/// credit is capped at a river.
const WONDER_FOOTPRINT_TILE_VALUE: f64 = 4.0;
/// The most the version-2 footprint credit can add: one river's worth.
const WONDER_FOOTPRINT_CAP: f64 = 12.0;

/// A natural wonder this far from an own city is one a Settler could stand
/// beside — the local settle scan's radius is eight, and a site beside the
/// wonder lies one tile past the wonder itself.
const WONDER_RING_RECON_CITY_RADIUS: i32 = 6;
/// The tiles from which a city can stand beside the wonder, and the tiles
/// such a city would work first: two rings.
const WONDER_RING_RADIUS: i32 = 2;
/// An explorer farther than this from the pocket is left on its frontier; the
/// pocket waits for the next one, or for this one on its way home.
const WONDER_RING_RECON_UNIT_RANGE: i32 = 10;

/// A V2 ring-recon destination, with the information used to rank it.
type WonderRingReconCandidate = (i32, Pos, usize, Pos, (Pos, i32), usize);

impl AdvancedAi {
    /// Either version of the family pays the projection; a seat plays one.
    pub(super) fn wonder_adjacent_sites_on(&self) -> bool {
        self.wonder_adjacent_sites || self.wonder_adjacent_sites_2
    }

    /// What a Citizen working `pos` is paid, as the site model sees it: the
    /// tile's own yields, plus — under `wonder-adjacent-sites` — the yields
    /// every neighbouring feature projects onto it, the rule
    /// `Game::player_tile_yields` pays with. Only natural wonders carry
    /// `adjacent_yields`, so off the gene and away from a wonder this is
    /// exactly `Rules::tile_yields`.
    pub(super) fn site_work_yields(&self, g: &Game, pos: Pos, tile: &Tile) -> Yields {
        let mut yields = g.rules.tile_yields(tile);
        if self.wonder_adjacent_sites_on() {
            yields.add(Self::projected_neighbour_yields(g, pos));
        }
        yields
    }

    /// The yields the features around `pos` project onto it.
    fn projected_neighbour_yields(g: &Game, pos: Pos) -> Yields {
        let mut yields = Yields::default();
        for neighbour in g.nbrs(pos) {
            if let Some(spec) = g
                .map
                .get(neighbour)
                .and_then(|tile| tile.feature.as_deref())
                .and_then(|feature| g.rules.features.get(feature))
            {
                yields.add(spec.adjacent_yields);
            }
        }
        yields
    }

    /// Version 2 only: the natural-wonder tiles among `positions` (a site's
    /// radius-two footprint), priced at `WONDER_FOOTPRINT_TILE_VALUE` each and
    /// capped. Zero under version 1 and off the family.
    pub(super) fn wonder_footprint_value(&self, g: &Game, positions: &[Pos]) -> f64 {
        if !self.wonder_adjacent_sites_2 {
            return 0.0;
        }
        let tiles = positions
            .iter()
            .filter(|pos| {
                g.map
                    .get(**pos)
                    .is_some_and(|tile| g.tile_is_natural_wonder(tile))
            })
            .count();
        (tiles as f64 * WONDER_FOOTPRINT_TILE_VALUE).min(WONDER_FOOTPRINT_CAP)
    }

    /// `settlement_prefilter_score` as the family sees it, so a ranking
    /// scan's top-`limit` cut cannot drop the site the full score would
    /// prefer: the projection onto the centre and its neighbours at the
    /// prefilter's own weights, and version 2's flat credit.
    pub(super) fn settlement_prefilter_score_for(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        let score = Self::settlement_prefilter_score(g, pid, pos)
            + self.coastal_city_site_prefilter_bonus(g, pid, pos)
            + Self::early_city_water_adjustment(g, pid, pos);
        if !self.wonder_adjacent_sites_on() {
            return score;
        }
        let projected = |at: Pos, weight: f64| {
            weight * Self::settlement_yield_value(Self::projected_neighbour_yields(g, at))
        };
        let mut projection = projected(pos, 1.5);
        for neighbour in g.nbrs(pos) {
            if g.map.get(neighbour).is_some() {
                projection += projected(neighbour, 1.0);
            }
        }
        score + projection + self.wonder_footprint_value(g, &g.wdisk(pos, 2))
    }
}

impl BasicAi {
    /// The nearest unseen tile within two of a natural wonder the player has
    /// seen within `WONDER_RING_RECON_CITY_RADIUS` of an own city, if this
    /// explorer is within `WONDER_RING_RECON_UNIT_RANGE` of it. Honours the
    /// same refusals as the frontier scan: retired ground, a visible hostile's
    /// reach, another explorer's held goal, water for a land unit kept ashore.
    /// See `wonder_ring_recon`.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::ai) fn wonder_ring_goal(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        dry_only: bool,
        dead: &HashSet<Pos>,
        threats: &[Pos],
        reserved: &[Pos],
    ) -> Option<Pos> {
        if g.players[pid].is_barbarian {
            return None;
        }
        let unit = g.units.get(&uid)?;
        let explored = &g.players[pid].explored;
        let cities = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid))
            .map(|city| city.pos)
            .collect::<Vec<_>>();
        if cities.is_empty() {
            return None;
        }
        // (distance from the explorer, goal, wonder tile, nearest own city).
        let mut best: Option<(i32, Pos, Pos, (Pos, i32))> = None;
        for (pos, tile) in &g.map.tiles {
            if !explored.contains(pos) || !g.tile_is_natural_wonder(tile) {
                continue;
            }
            let Some(city) = cities
                .iter()
                .map(|city| (*city, g.wdist(*city, *pos)))
                .min_by_key(|(city, distance)| (*distance, *city))
            else {
                continue;
            };
            if city.1 > WONDER_RING_RECON_CITY_RADIUS {
                continue;
            }
            for ring in g.wdisk(*pos, WONDER_RING_RADIUS) {
                if explored.contains(&ring) || dead.contains(&ring) {
                    continue;
                }
                let Some(ring_tile) = g.map.get(ring) else {
                    continue;
                };
                if g.tile_is_natural_wonder(ring_tile)
                    || !g.unit_can_traverse(uid, ring)
                    || (dry_only && g.rules.is_water(ring_tile))
                    || threats.iter().any(|threat| {
                        g.wdist(*threat, ring) <= crate::ai::EXPLORE_COMMIT_THREAT_RADIUS
                    })
                    || reserved
                        .iter()
                        .any(|held| g.wdist(*held, ring) <= crate::ai::EXPLORE_COMMIT_SEPARATION)
                {
                    continue;
                }
                let distance = g.wdist(unit.pos, ring);
                if distance > WONDER_RING_RECON_UNIT_RANGE {
                    continue;
                }
                if best.is_none_or(|(near, goal, _, _)| (distance, ring) < (near, goal)) {
                    best = Some((distance, ring, *pos, city));
                }
            }
        }
        let (distance, goal, wonder, (city, city_distance)) = best?;
        let unseen = g
            .wdisk(wonder, WONDER_RING_RADIUS)
            .into_iter()
            .filter(|ring| !explored.contains(ring))
            .count();
        let kind = unit.kind;
        let name = g.map.tiles[&wonder]
            .feature
            .as_deref()
            .unwrap_or("a natural wonder");
        think!(self.journal, Military, Detail,
               "{kind} {uid} scouts the far side of {name}";
               "{unseen} tiles within two of a natural wonder {city_distance} tiles from {city:?} are \
                still unseen, and a Settler cannot price a site it has not seen; the nearest is \
                {distance} tiles away";
               goal);
        Some(goal)
    }

    /// Version two of [`Self::wonder_ring_goal`]. It preserves every V1 trigger
    /// and refusal, then considers the nearest eligible goal and the goals one
    /// tile beyond it. The one whose sight disk exposes the most of its
    /// wonder's radius-two pocket wins, distance next. The detour is therefore
    /// capped at one tile while each trip is at least as informative as V1's.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::ai) fn wonder_ring_goal_2(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        dry_only: bool,
        dead: &HashSet<Pos>,
        threats: &[Pos],
        reserved: &[Pos],
    ) -> Option<Pos> {
        if g.players[pid].is_barbarian {
            return None;
        }
        let unit = g.units.get(&uid)?;
        let explored = &g.players[pid].explored;
        let cities = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| g.cities.get(&cid))
            .map(|city| city.pos)
            .collect::<Vec<_>>();
        if cities.is_empty() {
            return None;
        }

        // (distance, goal, pocket tiles exposed, wonder, nearest own city,
        // unseen pocket size). The candidate set deliberately matches V1's;
        // only the ordering below changes.
        let mut candidates: Vec<WonderRingReconCandidate> = Vec::new();
        for (wonder, tile) in &g.map.tiles {
            if !explored.contains(wonder) || !g.tile_is_natural_wonder(tile) {
                continue;
            }
            let Some(city) = cities
                .iter()
                .map(|city| (*city, g.wdist(*city, *wonder)))
                .min_by_key(|(city, distance)| (*distance, *city))
            else {
                continue;
            };
            if city.1 > WONDER_RING_RECON_CITY_RADIUS {
                continue;
            }
            let unseen = g
                .wdisk(*wonder, WONDER_RING_RADIUS)
                .into_iter()
                .filter(|position| !explored.contains(position))
                .count();
            for goal in g.wdisk(*wonder, WONDER_RING_RADIUS) {
                if explored.contains(&goal) || dead.contains(&goal) {
                    continue;
                }
                let Some(goal_tile) = g.map.get(goal) else {
                    continue;
                };
                if g.tile_is_natural_wonder(goal_tile)
                    || !g.unit_can_traverse(uid, goal)
                    || (dry_only && g.rules.is_water(goal_tile))
                    || threats.iter().any(|threat| {
                        g.wdist(*threat, goal) <= crate::ai::EXPLORE_COMMIT_THREAT_RADIUS
                    })
                    || reserved
                        .iter()
                        .any(|held| g.wdist(*held, goal) <= crate::ai::EXPLORE_COMMIT_SEPARATION)
                {
                    continue;
                }
                let distance = g.wdist(unit.pos, goal);
                if distance > WONDER_RING_RECON_UNIT_RANGE {
                    continue;
                }
                let revealed = g
                    .wdisk(*wonder, WONDER_RING_RADIUS)
                    .into_iter()
                    .filter(|site| {
                        !explored.contains(site) && g.wdist(*site, goal) <= g.unit_sight(uid)
                    })
                    .count();
                candidates.push((distance, goal, revealed, *wonder, city, unseen));
            }
        }
        let nearest = candidates.iter().map(|candidate| candidate.0).min()?;
        let (distance, goal, revealed, wonder, (city, city_distance), unseen) = candidates
            .into_iter()
            .filter(|candidate| candidate.0 <= nearest + 1)
            .max_by_key(|(distance, goal, revealed, _, _, _)| {
                (
                    *revealed,
                    std::cmp::Reverse(*distance),
                    std::cmp::Reverse(*goal),
                )
            })?;
        let kind = unit.kind;
        let name = g.map.tiles[&wonder]
            .feature
            .as_deref()
            .unwrap_or("a natural wonder");
        think!(self.journal, Military, Detail,
               "{kind} {uid} scouts the far side of {name}, most revealing first";
               "{unseen} tiles within two of a natural wonder {city_distance} tiles from \
                {city:?} are still unseen; {goal:?} is {distance} tiles away (the nearest goal \
                is {nearest}) and its sight disk covers {revealed} of them";
               goal);
        Some(goal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Action;
    use crate::name::Name;

    /// A one-player board flattened to grassland, with an interior tile whose
    /// radius-three disk is entirely on the map, and one of its neighbours.
    fn flat_board() -> (Game, Pos, Pos) {
        let mut game = Game::new_full(1, 20, 14, 91_003, 120, 0, false);
        for unit in game.player_unit_ids(0) {
            game.remove_unit(unit);
        }
        for tile in game.map.tiles.values_mut() {
            tile.terrain = Name::new("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
            tile.owner_city = None;
            tile.river_edges = [false; 6];
            tile.cliff_edges = [false; 6];
        }
        let center = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|position| game.wdisk(*position, 3).len() == 37)
            .expect("the test map has an interior tile");
        let beside = game
            .nbrs(center)
            .into_iter()
            .find(|position| game.map.get(*position).is_some())
            .expect("the center has a neighbour");
        (game, center, beside)
    }

    #[test]
    fn a_wonder_in_the_footprint_reads_no_better_without_the_gene_and_better_with_it() {
        let (plain, center, beside) = flat_board();
        let mut with_wonder = plain.clone();
        with_wonder.map.tiles.get_mut(&beside).unwrap().feature = Some(Name::new("mount_roraima"));
        assert!(with_wonder.tile_is_natural_wonder(&with_wonder.map.tiles[&beside]));

        let stock = AdvancedAi::new();
        let plain_stock = stock.settlement_static_value_uncached(&plain, 0, center);
        let wonder_stock = stock.settlement_static_value_uncached(&with_wonder, 0, center);
        assert!(
            wonder_stock < plain_stock + 1.0,
            "the mechanism: off the gene an impassable wonder tile is a lost job, and the only \
             credit is the district planner's Holy Site adjacency, under a point \
             ({wonder_stock:.1} vs {plain_stock:.1})"
        );

        let mut gene = AdvancedAi::new();
        gene.enable_wonder_adjacent_sites();
        let plain_gene = gene.settlement_static_value_uncached(&plain, 0, center);
        assert!(
            (plain_gene - plain_stock).abs() < 1e-9,
            "away from a wonder the gene changes nothing ({plain_gene:.1} vs {plain_stock:.1})"
        );
        let wonder_gene = gene.settlement_static_value_uncached(&with_wonder, 0, center);
        assert!(
            wonder_gene > plain_stock + 10.0,
            "beside Roraima the site must read materially better ({wonder_gene:.1} vs {plain_stock:.1})"
        );

        // Version 2 adds exactly the footprint credit on top of version 1.
        let mut second = AdvancedAi::new();
        second.enable_wonder_adjacent_sites_2();
        assert!(
            !second.wonder_adjacent_sites,
            "one version of a family plays"
        );
        let wonder_second = second.settlement_static_value_uncached(&with_wonder, 0, center);
        assert!(
            (wonder_second - wonder_gene - WONDER_FOOTPRINT_TILE_VALUE).abs() < 1e-9,
            "version 2 is version 1 plus one wonder tile's credit ({wonder_second:.1} vs {wonder_gene:.1})"
        );
        let plain_second = second.settlement_static_value_uncached(&plain, 0, center);
        assert!((plain_second - plain_stock).abs() < 1e-9);
    }

    #[test]
    fn the_projection_is_the_engines_rule_and_only_the_gene_pays_it() {
        let (mut game, center, beside) = flat_board();
        game.map.tiles.get_mut(&beside).unwrap().feature = Some(Name::new("mount_roraima"));
        let tile = &game.map.tiles[&center];
        let bare = game.rules.tile_yields(tile);
        let stock = AdvancedAi::new().site_work_yields(&game, center, tile);
        assert_eq!(
            stock, bare,
            "off the gene the site model reads the tile alone"
        );

        let mut gene = AdvancedAi::new();
        gene.enable_wonder_adjacent_sites();
        let paid = gene.site_work_yields(&game, center, tile);
        assert_eq!(
            paid.science,
            bare.science + 1.0,
            "Roraima projects +1 Science"
        );
        assert_eq!(paid.faith, bare.faith + 1.0, "Roraima projects +1 Faith");
        assert_eq!(paid.food, bare.food);
        assert_eq!(paid.production, bare.production);

        // A tile two away is not a neighbour and gets nothing.
        let far = game
            .wdisk(center, 2)
            .into_iter()
            .find(|position| {
                game.wdist(*position, beside) == 2 && game.wdist(*position, center) == 2
            })
            .expect("a ring-two tile away from the wonder");
        let far_tile = &game.map.tiles[&far];
        assert_eq!(
            gene.site_work_yields(&game, far, far_tile),
            game.rules.tile_yields(far_tile)
        );

        // Only version 2 pays the flat footprint credit, per wonder tile, capped.
        assert_eq!(
            gene.wonder_footprint_value(&game, &game.wdisk(center, 2)),
            0.0
        );
        let mut second = AdvancedAi::new();
        second.enable_wonder_adjacent_sites_2();
        assert_eq!(
            second.wonder_footprint_value(&game, &game.wdisk(center, 2)),
            WONDER_FOOTPRINT_TILE_VALUE
        );
        assert_eq!(
            AdvancedAi::new().wonder_footprint_value(&game, &game.wdisk(center, 2)),
            0.0
        );
        let mut four = game.clone();
        for position in four.wdisk(center, 2) {
            if position != center {
                four.map.tiles.get_mut(&position).unwrap().feature =
                    Some(Name::new("mount_roraima"));
            }
        }
        assert_eq!(
            second.wonder_footprint_value(&four, &four.wdisk(center, 2)),
            WONDER_FOOTPRINT_CAP
        );

        // Both versions order a ranking scan's prefilter the same way.
        let bare_order = AdvancedAi::settlement_prefilter_score(&game, 0, center)
            + AdvancedAi::early_city_water_adjustment(&game, 0, center);
        assert!(gene.settlement_prefilter_score_for(&game, 0, center) > bare_order);
        assert!(
            second.settlement_prefilter_score_for(&game, 0, center)
                > gene.settlement_prefilter_score_for(&game, 0, center)
        );
        assert_eq!(
            AdvancedAi::new().settlement_prefilter_score_for(&game, 0, center),
            bare_order
        );
    }

    /// A board with an own city, a wonder three tiles east of it whose far
    /// side is fog, and a Scout on the explored disk's west edge, where the
    /// nearest fog is one tile further west.
    fn wonder_pocket_board() -> (Game, Pos, Pos, u32) {
        let (mut game, city_pos, _) = flat_board();
        let settler = game.spawn_test_unit("settler", 0, city_pos);
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        // The wonder: three tiles east on the same row.
        let wonder = game
            .wdisk(city_pos, 3)
            .into_iter()
            .filter(|position| game.wdist(*position, city_pos) == 3 && position.1 == city_pos.1)
            .max()
            .expect("a tile three east of the city");
        game.map.tiles.get_mut(&wonder).unwrap().feature = Some(Name::new("mount_roraima"));
        // Explored: the city's radius-five disk, minus the wonder's far side.
        let explored = game
            .wdisk(city_pos, 5)
            .into_iter()
            .filter(|position| {
                !(game.wdist(*position, wonder) <= WONDER_RING_RADIUS
                    && game.wdist(*position, city_pos) > game.wdist(wonder, city_pos))
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert!(explored.contains(&wonder));
        game.players[0].explored = explored;
        // The Scout on the disk's west edge, five tiles west of the city.
        let edge = game
            .wdisk(city_pos, 5)
            .into_iter()
            .filter(|position| game.wdist(*position, city_pos) == 5 && position.1 == city_pos.1)
            .min()
            .expect("a tile five west of the city");
        let scout = game.spawn_test_unit("scout", 0, edge);
        (game, city_pos, wonder, scout)
    }

    #[test]
    fn the_scout_walks_the_wonders_unseen_ring_only_under_the_gene() {
        let (game, city_pos, wonder, scout) = wonder_pocket_board();
        let stock = BasicAi::new();
        let frontier = stock
            .exploration_goal(&game, 0, scout, false)
            .expect("fog remains west of the scout");
        assert!(
            game.wdist(frontier, wonder) > WONDER_RING_RADIUS,
            "stock walks to the nearest fog, west, not to the wonder's far side ({frontier:?})"
        );

        let mut gene = BasicAi::new();
        gene.wonder_ring_recon = true;
        let ring = gene
            .exploration_goal(&game, 0, scout, false)
            .expect("the wonder's far side is a goal");
        assert!(
            game.wdist(ring, wonder) <= WONDER_RING_RADIUS,
            "the gene sends the scout to the unseen ring ({ring:?} is not within two of {wonder:?})"
        );
        assert!(!game.players[0].explored.contains(&ring));
        assert!(
            game.wdist(ring, city_pos) > game.wdist(wonder, city_pos),
            "the goal is on the far side, the ground the capital's sight cannot reach"
        );

        // With the far side seen, the gene falls back to the ordinary frontier.
        let mut seen = game.clone();
        for position in seen.wdisk(wonder, WONDER_RING_RADIUS) {
            seen.players[0].explored.insert(position);
        }
        assert_eq!(
            gene.exploration_goal(&seen, 0, scout, false),
            Some(frontier)
        );
    }

    #[test]
    fn version_two_reveals_at_least_as_much_for_at_most_one_extra_tile() {
        let (game, _, wonder, scout) = wonder_pocket_board();
        let mut original = BasicAi::new();
        original.wonder_ring_recon = true;
        let v1_goal = original
            .exploration_goal(&game, 0, scout, false)
            .expect("V1 finds the pocket");
        let mut gene = BasicAi::new();
        gene.wonder_ring_recon_2 = true;
        let goal = gene
            .exploration_goal(&game, 0, scout, false)
            .expect("V2 preserves V1's unseen wonder pocket");
        assert!(game.wdist(goal, wonder) <= WONDER_RING_RADIUS);
        assert!(
            game.wdist(game.units[&scout].pos, goal)
                <= game.wdist(game.units[&scout].pos, v1_goal) + 1,
            "V2's information gain may cost at most one extra tile"
        );
        assert!(!game.players[0].explored.contains(&goal));
        let revealed = |at| {
            game.wdisk(wonder, WONDER_RING_RADIUS)
                .into_iter()
                .filter(|position| {
                    !game.players[0].explored.contains(position)
                        && game.wdist(*position, at) <= game.unit_sight(scout)
                })
                .count()
        };
        assert!(
            revealed(goal) >= revealed(v1_goal),
            "V2 never reveals less of the pocket than V1's nearest-tile choice"
        );
    }

    #[test]
    fn version_two_keeps_the_work_ring_because_the_site_model_needs_it() {
        let (game, _, wonder, scout) = wonder_pocket_board();
        let mut centres_known = game.clone();
        for position in centres_known.nbrs(wonder) {
            centres_known.players[0].explored.insert(position);
        }
        assert!(
            centres_known
                .wdisk(wonder, WONDER_RING_RADIUS)
                .into_iter()
                .any(|position| !centres_known.players[0].explored.contains(&position)),
            "the work ring deliberately retains fog"
        );

        let mut original = BasicAi::new();
        original.wonder_ring_recon = true;
        let v1_goal = original
            .exploration_goal(&centres_known, 0, scout, false)
            .expect("V1 still vacuums the work ring");
        assert!(centres_known.wdist(v1_goal, wonder) <= WONDER_RING_RADIUS);

        let mut second = BasicAi::new();
        second.wonder_ring_recon_2 = true;
        let v2_goal = second
            .exploration_goal(&centres_known, 0, scout, false)
            .expect("V2 preserves the work-tile information V1's city result depended on");
        assert!(
            centres_known.wdist(v2_goal, wonder) <= WONDER_RING_RADIUS,
            "the remaining work ring still wins over the generic frontier"
        );
    }

    #[test]
    fn version_two_preserves_v1_when_no_adjacent_site_is_legal() {
        let (game, _, wonder, scout) = wonder_pocket_board();
        let mut original = BasicAi::new();
        original.w.min_city_dist = 20.0;
        original.wonder_ring_recon = true;
        let v1_goal = original
            .exploration_goal(&game, 0, scout, false)
            .expect("V1 ignores whether the pocket can seat a city");
        assert!(game.wdist(v1_goal, wonder) <= WONDER_RING_RADIUS);

        let mut second = BasicAi::new();
        second.w.min_city_dist = 20.0;
        second.wonder_ring_recon_2 = true;
        let v2_goal = second
            .exploration_goal(&game, 0, scout, false)
            .expect("V2 preserves V1's trigger even without a legal city centre");
        assert!(
            game.wdist(v2_goal, wonder) <= WONDER_RING_RADIUS,
            "V2 preserves V1's pocket instead of adding a settlement gate"
        );
    }

    #[test]
    fn version_two_preserves_v1_after_the_expansion_window() {
        let (game, _, wonder, scout) = wonder_pocket_board();
        let mut original = BasicAi::new();
        original.w.city_target = 1.0;
        original.w.settler_stop_turn = 0.0;
        original.wonder_ring_recon = true;
        let v1_goal = original
            .exploration_goal(&game, 0, scout, false)
            .expect("V1 scouts independently of the expansion horizon");
        assert!(game.wdist(v1_goal, wonder) <= WONDER_RING_RADIUS);

        let mut second = BasicAi::new();
        second.w.city_target = 1.0;
        second.w.settler_stop_turn = 0.0;
        second.wonder_ring_recon_2 = true;
        let v2_goal = second
            .exploration_goal(&game, 0, scout, false)
            .expect("V2 preserves V1 after the expansion horizon");
        assert!(game.wdist(v2_goal, wonder) <= WONDER_RING_RADIUS);
    }

    #[test]
    fn wonder_ring_recon_versions_are_mutually_exclusive() {
        let mut ai = AdvancedAi::new();
        ai.enable_wonder_ring_recon();
        assert!(ai.base.wonder_ring_recon);
        assert!(!ai.base.wonder_ring_recon_2);

        ai.enable_wonder_ring_recon_2();
        assert!(!ai.base.wonder_ring_recon);
        assert!(ai.base.wonder_ring_recon_2);

        ai.enable_wonder_ring_recon();
        assert!(ai.base.wonder_ring_recon);
        assert!(!ai.base.wonder_ring_recon_2);
    }

    #[test]
    fn a_wonder_far_from_every_city_is_not_a_ring_to_scout() {
        let (mut game, city_pos, wonder, scout) = wonder_pocket_board();
        // Move the wonder out of settling range: seven tiles east.
        game.map.tiles.get_mut(&wonder).unwrap().feature = None;
        let far = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|position| position.1 == city_pos.1 && game.wdist(*position, city_pos) == 7)
            .max()
            .expect("a tile seven east of the city");
        game.map.tiles.get_mut(&far).unwrap().feature = Some(Name::new("mount_roraima"));
        game.players[0].explored.insert(far);
        let mut gene = BasicAi::new();
        gene.wonder_ring_recon = true;
        let goal = gene
            .exploration_goal(&game, 0, scout, false)
            .expect("fog remains");
        assert!(
            game.wdist(goal, far) > WONDER_RING_RADIUS,
            "a wonder seven tiles from home is a frontier like any other ({goal:?})"
        );
    }
}
