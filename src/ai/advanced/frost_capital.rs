//! `capital-leaves-the-frost`: the first Settler does not found the capital
//! on frozen ground when warmer land is a short walk away.
//!
//! ★ Live Immortal Standard `civvis-20261104T054226Z` (Gran Colombia): the
//! capital was founded on turn 1 where the Settler stood — a SNOW tile on a
//! one-hex strip between a lake (Crabs) and the polar sea (two Fish). Its
//! first ring was snow, snow, tundra and three water tiles; its second ring
//! held three ice tiles. Bogotá made 3 Production at turn 10, 4 at turn 30,
//! 5 at turn 70 (pop 4), while the median live capital makes 11 at turn 50.
//! The operator retired the game. `civvis-20261104T015124Z` (same lane) did
//! the same on a lakeside tundra forest with grassland two hexes south:
//! population 2 and 4 Production at turn 100.
//!
//! The site score could not see it. It forecasts only the first four
//! citizens over forty turns (`settlement_growth_forecast_from_positions`),
//! and a city center's yields are floored at 2 Food / 1 Production, so a
//! snow center costs nothing and three seafood tiles fill the first four
//! jobs. Fresh water paid +12 through Housing and the early-city water rule
//! takes 20 points off any dry site, so the lakeside snow tile scored 76.8
//! and nothing within the opening's two-hex search beat it by the walk's
//! price. Frozen ground is what the capital works for the rest of the game:
//! snow yields nothing, tundra one Food, and ice nothing at all.
//!
//! Under the gene, while the empire has no city every site's value is
//! reduced by `FROST_CAPITAL_TILE_PRICE` per frozen tile in its footprint:
//! snow and ice count one, tundra one half, ring two at half weight. Unseen
//! tiles count nothing, so fog never reads as frost. The price enters
//! `settle_value_visible`, so the ranking, the opening walk's price
//! (`opening_walk_pays`) and the stand-still value all see it. Later cities
//! are untouched: the 2026-10-15 census found junk polar cities still make
//! ~14 Production by turn 150, so a polar EXPANSION is not this gene's
//! business — only the capital, which is the empire's core all game.

use super::{AdvancedAi, SETTLER_STEP_RISK_LIMIT};
use crate::game::Game;
use crate::think;
use crate::world::Tile;
use crate::Pos;

/// Site points one fully frozen tile costs a prospective capital.
///
/// On the snow board (see the tests) the start carries 6.5 frozen tiles and
/// the lakeside tundra two hexes north 3.25, so this moves the stand value
/// by 26 points and the gap by 13. That alone does not lift the dry plains
/// beyond them (the early-city water rule holds them 20 points down), which
/// is why the gene also LOOKS before founding — see `frost_capital_look`.
pub(super) const FROST_CAPITAL_TILE_PRICE: f64 = 4.0;
/// The second ring is worked later and partly by districts; half weight.
const FROST_RING_TWO_WEIGHT: f64 = 0.5;
/// A stand whose own tile is warm still counts as frozen ground when its
/// footprint holds this many frozen tiles (a plains tile in a snow field).
const FROST_LOOK_FOOTPRINT: f64 = 4.0;
/// A look step must cut the footprint's frost by at least this much, so
/// successive looks strictly warm and can never cycle.
const FROST_LOOK_MIN_GAIN: f64 = 1.0;
/// How far a look step may go: the opening's own local radius.
const FROST_LOOK_RADIUS: i32 = 2;

impl AdvancedAi {
    /// Whether the gene prices this player's site: on, and still cityless.
    /// Russia and Canada are left alone — their tundra start bias and
    /// abilities are built to work frozen land.
    fn frost_capital_applies(&self, g: &Game, pid: usize) -> bool {
        self.capital_leaves_the_frost
            && g.player_city_ids(pid).is_empty()
            && !matches!(g.players[pid].civ.as_str(), "Russia" | "Canada")
    }

    /// What frozen ground costs `pos` as the empire's FIRST city. Zero with
    /// the gene off and once the player holds any city.
    pub(super) fn frost_capital_penalty(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        if !self.frost_capital_applies(g, pid) {
            return 0.0;
        }
        Self::frozen_footprint(g, pos) * FROST_CAPITAL_TILE_PRICE
    }

    /// Frozen tiles in `pos`'s radius-two footprint, center and first ring at
    /// full weight, second ring at `FROST_RING_TWO_WEIGHT`.
    pub(super) fn frozen_footprint(g: &Game, pos: Pos) -> f64 {
        g.wdisk(pos, 2)
            .into_iter()
            .map(|at| {
                let ring = if g.wdist(pos, at) <= 1 {
                    1.0
                } else {
                    FROST_RING_TWO_WEIGHT
                };
                ring * g
                    .map
                    .get(at)
                    .map_or(0.0, |tile| Self::frost_weight(g, tile))
            })
            .sum()
    }

    /// One tile's frost: ice (land or sea) and snow count one, tundra one
    /// half. Open water, warm land and unseen (`unknown`) tiles count zero,
    /// so fog never reads as frost.
    fn frost_weight(g: &Game, tile: &Tile) -> f64 {
        // Unseen first: `unknown` is a mirror terrain a native ruleset lacks.
        if tile.terrain == "unknown" {
            0.0
        } else if tile
            .feature
            .as_ref()
            .is_some_and(|feature| *feature == "ice")
        {
            1.0
        } else if g.rules.is_water(tile) {
            0.0
        } else if tile.terrain == "snow" {
            1.0
        } else if tile.terrain == "tundra" {
            0.5
        } else {
            0.0
        }
    }

    /// A capital stand is frozen ground when its own tile is snow or tundra,
    /// or its footprint holds `FROST_LOOK_FOOTPRINT` frozen tiles.
    pub(super) fn frozen_stand(g: &Game, pos: Pos) -> bool {
        g.map
            .get(pos)
            .is_some_and(|tile| tile.terrain == "snow" || tile.terrain == "tundra")
            || Self::frozen_footprint(g, pos) >= FROST_LOOK_FOOTPRINT
    }

    /// Opening turns on which the first Settler may still look rather than
    /// found: one fewer than the speed's opening walk budget, so the walk
    /// that follows a look stays inside it (Online and Quick look on turn 1
    /// only; Standard on turns 1–2; Epic 1–3; Marathon 1–4).
    fn frost_look_turns(g: &Game) -> u32 {
        Self::opening_walk_budget(g.game_speed)
            .0
            .saturating_sub(1)
            .max(1)
    }

    /// Unseen tiles within three hexes of `pos`: what a step there reveals.
    fn frost_look_unseen(g: &Game, pos: Pos) -> usize {
        g.wdisk(pos, 3)
            .into_iter()
            .filter(|at| g.map.get(*at).is_none_or(|tile| tile.terrain == "unknown"))
            .count()
    }

    /// The first Settler is about to found the capital where it stands.
    /// When that tile is frozen ground and the opening budget allows, step
    /// it instead to the reachable tile within two hexes whose footprint is
    /// least frozen (ties: the most fog revealed, then the shortest walk),
    /// and let the next decision found with the land it then sees.
    ///
    /// Returns `true` when the Settler moved; `false` leaves the founding to
    /// the caller — with the gene off, once the budget is spent, on warm
    /// ground, or when no warmer tile is reachable and safe.
    pub(super) fn frost_capital_look(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        current: Pos,
    ) -> bool {
        if !self.frost_capital_applies(g, pid)
            || g.turn > Self::frost_look_turns(g)
            || !Self::frozen_stand(g, current)
            || self.capital_frost_looks.contains(&current)
        {
            return false;
        }
        let stand_frost = Self::frozen_footprint(g, current);
        let visible = self.battlefront_visibility(g, pid);
        let costs = Self::settle_sooner_walk_costs(g, uid, FROST_LOOK_RADIUS);
        let mut steps = costs
            .iter()
            .filter(|(pos, _)| **pos != current && !self.capital_frost_looks.contains(pos))
            .filter_map(|(pos, cost)| {
                let tile = g.map.get(*pos)?;
                if tile.terrain == "unknown"
                    || g.rules.is_water(tile)
                    || !g.rules.is_passable(tile)
                    || tile
                        .owner_city
                        .is_some_and(|cid| g.cities[&cid].owner != pid)
                    || g.cities.values().any(|city| g.wdist(city.pos, *pos) < 4)
                {
                    return None;
                }
                let frost = Self::frozen_footprint(g, *pos);
                if frost > stand_frost - FROST_LOOK_MIN_GAIN {
                    return None;
                }
                if self.settlement_safety
                    && self.settlement_tile_risk(g, pid, Some(uid), *pos, &visible)
                        > SETTLER_STEP_RISK_LIMIT
                {
                    return None;
                }
                Some((*pos, frost, Self::frost_look_unseen(g, *pos), *cost))
            })
            .collect::<Vec<_>>();
        steps.sort_by(|a, b| {
            a.1.total_cmp(&b.1)
                .then(b.2.cmp(&a.2))
                .then(a.3.total_cmp(&b.3))
                .then(a.0.cmp(&b.0))
        });
        let Some(&(step, frost, unseen, _)) = steps.first() else {
            return false;
        };
        self.capital_frost_looks.push(current);
        self.settler_targets.remove(&uid);
        think!(self.journal(), Expansion, Detail,
               "The first Settler looks past the frost";
               "{current:?} is frozen ground (frost {stand_frost:.2}); it steps toward {step:?} \
                (frost {frost:.2}, {unseen} unseen tiles within three) and chooses again on the land \
                it then sees; looks end after turn {}", Self::frost_look_turns(g); step);
        self.settler_step_toward_safe(g, pid, uid, step)
    }
}

#[cfg(test)]
mod tests {
    use super::super::AdvancedAi;
    use super::FROST_CAPITAL_TILE_PRICE;
    use crate::game::Game;
    use crate::name::Name;
    use crate::Pos;

    /// A grassland map with an interior site.
    fn warm_board() -> (Game, Pos) {
        let mut game = Game::new_full(1, 20, 14, 91_041, 120, 0, false);
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
        }
        let center = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|position| game.wdisk(*position, 3).len() == 37)
            .expect("an interior tile");
        (game, center)
    }

    #[test]
    fn the_footprint_counts_snow_ice_and_half_tundra_by_ring() {
        let (mut game, center) = warm_board();
        assert_eq!(AdvancedAi::frozen_footprint(&game, center), 0.0);
        assert!(!AdvancedAi::frozen_stand(&game, center));
        game.map.tiles.get_mut(&center).unwrap().terrain = Name::new("snow");
        let ring_one = game.nbrs(center);
        game.map.tiles.get_mut(&ring_one[0]).unwrap().terrain = Name::new("tundra");
        let ring_two = game
            .wdisk(center, 2)
            .into_iter()
            .find(|at| game.wdist(center, *at) == 2)
            .unwrap();
        game.map.tiles.get_mut(&ring_two).unwrap().terrain = Name::new("coast");
        game.map.tiles.get_mut(&ring_two).unwrap().feature = Some(Name::new("ice"));
        // center 1 + tundra 0.5 + ice at half weight 0.5
        assert_eq!(AdvancedAi::frozen_footprint(&game, center), 2.0);
        assert!(AdvancedAi::frozen_stand(&game, center));
        // Fog is not frost.
        game.map.tiles.get_mut(&ring_one[1]).unwrap().terrain = Name::new("unknown");
        assert_eq!(AdvancedAi::frozen_footprint(&game, center), 2.0);
    }

    #[test]
    fn the_price_is_zero_off_the_gene_with_a_city_and_for_tundra_civs() {
        let (mut game, center) = warm_board();
        game.map.tiles.get_mut(&center).unwrap().terrain = Name::new("snow");
        let mut ai = AdvancedAi::new();
        assert_eq!(ai.frost_capital_penalty(&game, 0, center), 0.0);
        ai.enable_capital_leaves_the_frost();
        assert_eq!(
            ai.frost_capital_penalty(&game, 0, center),
            FROST_CAPITAL_TILE_PRICE
        );
        let civ = game.players[0].civ.clone();
        game.players[0].civ = "Russia".to_string();
        assert_eq!(ai.frost_capital_penalty(&game, 0, center), 0.0);
        game.players[0].civ = civ;
        let elsewhere = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|at| game.wdist(*at, center) > 6 && game.wdisk(*at, 2).len() == 19)
            .unwrap();
        game.found_city_for(0, elsewhere, None);
        assert_eq!(ai.frost_capital_penalty(&game, 0, center), 0.0);
    }

    /// The turn-1 board of a live run, trimmed to its seat/state/tiles lines.
    fn live_turn_one(fixture: &str) -> crate::mirror::LiveMirror {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("data/fixtures/capital_frost")
            .join(fixture);
        let snapshot = crate::mirror::snapshot_from_events_at(&path, Some(1)).unwrap();
        let state = crate::mirror::state_from_events(&path, Some(1)).unwrap();
        crate::mirror::LiveMirror::new(&snapshot, &state, 6, 1, 500, 6)
    }

    /// Run the first Settler's turn-1 decision the way the live seat does:
    /// the step is re-asked while it keeps acting and has moves.
    fn first_settler_turn(g: &mut Game, ai: &mut AdvancedAi) -> (u32, Pos) {
        let uid = g
            .units
            .values()
            .find(|unit| unit.owner == 0 && unit.kind == "settler")
            .map(|unit| unit.id)
            .expect("a settler");
        for _ in 0..4 {
            if !g.units.contains_key(&uid)
                || g.units[&uid].moves_left <= 0.0
                || !ai.advanced_settler_step(g, 0, uid)
            {
                break;
            }
        }
        let pos = g
            .units
            .get(&uid)
            .map_or((i32::MIN, i32::MIN), |unit| unit.pos);
        (uid, pos)
    }

    /// ★ `civvis-20261104T054226Z`: the Settler stood on snow at axial
    /// (23, 6), lake (Crabs) north, polar sea (two Fish) south, ice beyond.
    /// The deployment genome founds Bogota there on turn 1 (worth 76.8);
    /// under the gene it walks north off the snow and founds nothing yet.
    #[test]
    fn the_snow_start_walks_north_instead_of_founding_on_the_snow() {
        let snow = (23, 6);
        let mut mirror = live_turn_one("snow_start_20261104T054226Z.jsonl");
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge();
        let mut g = mirror.game.clone();
        first_settler_turn(&mut g, &mut ai);
        let founded = g
            .player_city_ids(0)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect::<Vec<_>>();
        assert_eq!(
            founded,
            vec![snow],
            "the deployment genome founds on the snow"
        );

        let g = &mut mirror.game;
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge();
        ai.enable_capital_leaves_the_frost();
        let (_, at) = first_settler_turn(g, &mut ai);
        assert!(
            g.player_city_ids(0).is_empty(),
            "no capital on frozen ground"
        );
        assert_ne!(at, snow);
        assert!(at.1 > snow.1, "it walks away from the pole: {at:?}");
        assert!(
            AdvancedAi::frozen_footprint(g, at) <= AdvancedAi::frozen_footprint(g, snow) - 1.0,
            "{at:?} is warmer ground"
        );
    }

    /// ★ `civvis-20261104T015124Z`: the Settler stood on coastal tundra and
    /// walked one hex to found on lakeside tundra (population 2 and 4
    /// Production at turn 100) with grassland two hexes south, unseen
    /// beyond it the lake shore and a river. Under the gene it walks south
    /// onto the grass instead.
    #[test]
    fn the_tundra_start_walks_onto_the_grass_instead_of_founding_on_tundra() {
        let lakeside_tundra = (42, 37);
        let mut mirror = live_turn_one("tundra_start_20261104T015124Z.jsonl");
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge();
        let mut g = mirror.game.clone();
        first_settler_turn(&mut g, &mut ai);
        let founded = g
            .player_city_ids(0)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect::<Vec<_>>();
        assert_eq!(founded, vec![lakeside_tundra]);

        let g = &mut mirror.game;
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge();
        ai.enable_capital_leaves_the_frost();
        let (_, at) = first_settler_turn(g, &mut ai);
        assert!(g.player_city_ids(0).is_empty(), "no capital on tundra");
        assert_eq!(
            g.map.get(at).map(|tile| tile.terrain.to_string()),
            Some("grassland".to_string()),
            "{at:?}"
        );
    }

    /// Replay probe: the turn-1 board of a live run, every site within three
    /// hexes of the first Settler, priced with the live genome.
    ///
    /// `CIVVIS_CAPITAL_REPLAY=<events.jsonl> CIVVIS_FORCED=<a,b,...>
    ///  cargo test --profile ci --lib frost_capital::tests::probe -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn probe_capital_sites_on_a_live_board() {
        let path = std::path::PathBuf::from(std::env::var("CIVVIS_CAPITAL_REPLAY").unwrap());
        let turn = std::env::var("CIVVIS_CAPITAL_TURN")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(1);
        let snapshot = crate::mirror::snapshot_from_events_at(&path, Some(turn)).unwrap();
        let state = crate::mirror::state_from_events(&path, Some(turn)).unwrap();
        let mirror = crate::mirror::LiveMirror::new(&snapshot, &state, 6, 1, 500, 6);
        let g = &mirror.game;
        let forced = std::env::var("CIVVIS_FORCED").unwrap_or_default();
        let forced = forced
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .collect::<Vec<_>>();
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge_universe();
        ai.apply_gene_ledger_with_forced_live(&forced);
        let pid = 0;
        let settler = g
            .units
            .values()
            .find(|unit| unit.owner == pid && unit.kind == "settler")
            .expect("a settler");
        let (uid, from) = (settler.id, settler.pos);
        eprintln!(
            "settler {uid} at {from:?} speed {:?} safety {}",
            g.game_speed, ai.settlement_safety
        );
        let mut rows = g
            .wdisk(from, 3)
            .into_iter()
            .filter_map(|pos| {
                let tile = g.map.get(pos)?;
                if g.rules.is_water(tile) || !g.rules.is_passable(tile) {
                    return None;
                }
                let positions = g.wdisk(pos, 2);
                let forecast =
                    ai.settlement_growth_forecast_from_positions(g, pid, pos, &positions);
                let (fresh, coastal) = g.city_site_water(pid, pos);
                Some((
                    pos,
                    ai.settle_value(g, pid, pos),
                    format!(
                        "{}{} d{} fresh={fresh} coast={coastal} forecast={:.1} pop={} \
                         four_at={:?} water_adj={:.1} coastal_bonus={:.1} worked={:?}",
                        tile.terrain,
                        if tile.hills { "+hills" } else { "" },
                        g.wdist(from, pos),
                        forecast.score,
                        forecast.reached_population,
                        forecast.turns_to_four.map(|t| t.round()),
                        AdvancedAi::early_city_water_adjustment(g, pid, pos),
                        ai.coastal_city_site_bonus(g, pos, &positions),
                        forecast.worked.iter().map(|w| w.pos).collect::<Vec<_>>(),
                    ),
                ))
            })
            .collect::<Vec<_>>();
        rows.sort_by(|a, b| b.1.total_cmp(&a.1));
        for (pos, value, detail) in rows {
            eprintln!("{pos:?} {value:7.1} {detail}");
        }
        eprintln!(
            "local radius-2 pick: {:?}",
            ai.best_reachable_settle_site_except(g, pid, uid, 2, None)
        );
    }
}
