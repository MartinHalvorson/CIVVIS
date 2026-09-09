//! Two Deity habits as opt-in genes (operator, 2026-08-24: *"study expert
//! level deity civ 6 tips and tricks and implement the best as heuristics"*).
//! A third, `chop-into-the-queue` (a Builder chops woods, rainforest or marsh
//! into the Settler, district or wonder at the front of the owning city's
//! queue), left the code on 2026-09-09 under the batch rule at -23/-22/-11;
//! its rows stay in the ranking's *Removed from the code* table.
//!
//! The expert play these encode, and why each is a gene rather than a fact
//! the agent already knew:
//!
//! - **`eureka-chasing-builder`.** Sixty-two technologies and fifty-three
//!   civics carry a boost worth 40% of their cost, and `tech_value` already
//!   pays +28 for a technology whose boost is *in hand* — but nothing ever
//!   went and *earned* one. A Deity player builds the quarry for Masonry, the
//!   mine on a resource for The Wheel, the pasture for Horseback Riding, the
//!   sixth farm for Feudalism. `improvement_value_with_appeal` now adds the
//!   research a boost would grant, spread over the steps the trigger still
//!   needs, wherever this improvement on this tile is one of them.
//! - **`eureka-chasing-production`.** The same table read by the production
//!   queue: two Galleys for Shipbuilding, three Archers for Machinery, Walls
//!   for Engineering, a Water Mill for Construction, an Aqueduct for Military
//!   Engineering, two Markets for Guilds. `production_value` adds the boost's
//!   research to the item's raw value, spread the same way.
//!
//! Both read the shipped `BoostSpec` rows (`data/techs.json`,
//! `data/civics.json`) through the same trigger vocabulary
//! `Game::boost_met` checks, so a trigger the engine cannot detect is never
//! chased. Off, every path is byte-identical to before.

use std::cell::RefCell;

use super::AdvancedAi;
use crate::game::{Game, Item};
use crate::name::Name;
use crate::Pos;

/// `eureka_chasing_*`: a boost on a node more than this many eras past the
/// world era is not chased — three Archers for Machinery are worth training
/// in the Ancient era, three Tanks for Composites are not.
pub(super) const EUREKA_CHASE_ERA_REACH: usize = 2;
/// `eureka_chasing_builder`: what one point of granted research is worth on
/// the improvement scale (a Mine is ~2–5, a luxury connection 14, a
/// strategic connection 30). Masonry's 32 points make a quarry on stone
/// worth +16; Feudalism's 120 over six farms adds +10 each.
pub(super) const EUREKA_BUILDER_VALUE_PER_POINT: f64 = 0.5;
/// `eureka_chasing_builder`: the ceiling on one improvement's premium.
pub(super) const EUREKA_BUILDER_VALUE_CAP: f64 = 40.0;
/// `eureka_chasing_builder_2`: even an immediate, final-step boost may only
/// break a close Builder tie. The improvement's own yields remain the reason
/// to spend a charge there; the boost is the tiebreaker that lands now.
pub(super) const EUREKA_BUILDER_V2_OWN_VALUE_FRACTION: f64 = 0.75;
/// `eureka_chasing_production`: what one point of granted research is worth
/// on `production_value`'s raw scale (a Settler or a district sits in the
/// hundreds to low thousands before the turns divisor). Machinery's 120
/// points over three Archers add +160 raw each.
pub(super) const EUREKA_PRODUCTION_VALUE_PER_POINT: f64 = 4.0;
/// `eureka_chasing_production`: the ceiling on one item's premium.
pub(super) const EUREKA_PRODUCTION_VALUE_CAP: f64 = 400.0;

/// One boost the empire can still earn by building something: the trigger as
/// the rules spell it, how many more of the thing it takes, and the research
/// the boost grants.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct EurekaChase {
    /// The node (a technology or a civic) whose boost this is.
    pub node: Name,
    /// The `BoostSpec` trigger verbatim: `improvement_on_resource:quarry`,
    /// `units_of:archer`, `building:walls`, `district:aqueduct`, …
    pub trigger: String,
    /// How many more matching things complete the trigger; at least one.
    pub remaining: i64,
    /// Research points the boost grants at this game speed.
    pub research: f64,
}

impl EurekaChase {
    /// The research one more matching thing earns: the whole boost spread
    /// evenly over the steps still needed.
    pub fn per_step(&self) -> f64 {
        self.research / self.remaining.max(1) as f64
    }
}

/// The per-turn memo behind both eureka genes: the table is the same for
/// every Builder job and every queue item priced this turn.
#[derive(Clone, Default)]
pub(super) struct EurekaChaseCache {
    entries: RefCell<Option<(u32, usize, Vec<EurekaChase>)>>,
}

impl AdvancedAi {
    /// Every unresearched, unboosted technology or civic within
    /// `EUREKA_CHASE_ERA_REACH` eras whose boost an improvement, a unit, a
    /// building or a district can still complete, with the steps left and the
    /// research at stake. Memoised per turn and player.
    pub(super) fn eureka_chases(&self, g: &Game, pid: usize) -> Vec<EurekaChase> {
        {
            let cached = self.eureka_chase_cache.entries.borrow();
            if let Some((turn, owner, entries)) = cached.as_ref() {
                if *turn == g.turn && *owner == pid {
                    return entries.clone();
                }
            }
        }
        let entries = self.eureka_chases_uncached(g, pid);
        *self.eureka_chase_cache.entries.borrow_mut() = Some((g.turn, pid, entries.clone()));
        entries
    }

    fn eureka_chases_uncached(&self, g: &Game, pid: usize) -> Vec<EurekaChase> {
        let player = &g.players[pid];
        let reach = g.world_era + EUREKA_CHASE_ERA_REACH;
        let mut chases = Vec::new();
        let trees = [
            (&g.rules.techs, &player.techs, &player.boosted_techs),
            (&g.rules.civics, &player.civics, &player.boosted_civics),
        ];
        for (specs, known, boosted) in trees {
            for (node, spec) in specs.iter() {
                let Some(boost) = spec.boost.as_ref() else {
                    continue;
                };
                if spec.era > reach || known.contains(node) || boosted.contains(node) {
                    continue;
                }
                // `chase-every-boost`: every trigger the engine can judge is a
                // chase, read from `Game::boost_progress` — the same arm that
                // decides "met". Off, only the six buildable prefixes below
                // are chased, exactly as the two Deity genes measured. A best-
                // single-value arm with nothing to measure reads `i64::MIN`;
                // clamped to zero it is "none yet". See
                // `advanced/chase_every_boost.rs`.
                let (have, need) = if self.chase_every_boost {
                    let (have, need) = g.boost_progress(pid, boost);
                    (have.max(0), need.max(1))
                } else {
                    let Some(have) = self.eureka_trigger_progress(g, pid, &boost.trigger) else {
                        continue;
                    };
                    (have, boost.count.max(1))
                };
                let remaining = need - have;
                if remaining <= 0 {
                    continue;
                }
                let research =
                    g.game_speed.scale(spec.cost) * boost.percent.unwrap_or(40.0) / 100.0;
                chases.push(EurekaChase {
                    node: *node,
                    trigger: boost.trigger.clone(),
                    remaining,
                    research,
                });
            }
        }
        chases
    }

    /// How far along a buildable trigger the empire is, in the same terms
    /// `Game::boost_met` counts it; `None` for a trigger nothing built can
    /// advance (kills, contacts, eras, …).
    fn eureka_trigger_progress(&self, g: &Game, pid: usize, trigger: &str) -> Option<i64> {
        let owned_tiles_where = |want: &dyn Fn(&crate::world::Tile) -> bool| -> i64 {
            g.cities
                .values()
                .filter(|city| city.owner == pid)
                .flat_map(|city| city.owned_tiles.iter())
                .filter(|pos| {
                    g.map
                        .get(**pos)
                        .is_some_and(|tile| !tile.pillaged && want(tile))
                })
                .count() as i64
        };
        if let Some(improvement) = trigger.strip_prefix("improvement:") {
            return Some(owned_tiles_where(&|tile| {
                tile.improvement.as_deref() == Some(improvement)
            }));
        }
        if let Some(improvement) = trigger.strip_prefix("improvement_on_resource:") {
            return Some(owned_tiles_where(&|tile| {
                tile.improvement.as_deref() == Some(improvement) && tile.resource.is_some()
            }));
        }
        if let Some(resource) = trigger.strip_prefix("improve_resource:") {
            return Some(owned_tiles_where(&|tile| {
                tile.resource.as_deref() == Some(resource)
                    && tile
                        .improvement
                        .as_deref()
                        .is_some_and(|imp| Self::improvement_connects(g, imp, resource))
            }));
        }
        if let Some(kind) = trigger.strip_prefix("units_of:") {
            return Some(
                g.units
                    .values()
                    .filter(|unit| unit.owner == pid && unit.kind == kind)
                    .count() as i64,
            );
        }
        if let Some(building) = trigger.strip_prefix("building:") {
            return Some(
                g.cities
                    .values()
                    .filter(|city| {
                        city.owner == pid && city.buildings.iter().any(|have| have == building)
                    })
                    .count() as i64,
            );
        }
        if let Some(district) = trigger.strip_prefix("district:") {
            return Some(
                g.cities
                    .values()
                    .filter(|city| city.owner == pid)
                    .flat_map(|city| city.districts.keys())
                    .filter(|have| g.district_family(**have) == district)
                    .count() as i64,
            );
        }
        None
    }

    /// Does `improvement` connect `resource` — the resource's own improvement
    /// or one whose resource list names it — as `Game::boost_met` reads it?
    pub(super) fn improvement_connects(g: &Game, improvement: &str, resource: &str) -> bool {
        g.rules
            .resources
            .get(resource)
            .is_some_and(|spec| spec.improvement == improvement)
            || g.rules
                .improvements
                .get(improvement)
                .is_some_and(|spec| spec.resources.iter().any(|named| named == resource))
    }

    /// `eureka_chasing_builder`: the research this improvement on this tile
    /// earns toward a boost, on the improvement-value scale. Zero when the
    /// gene is off or nothing is chased.
    pub(super) fn eureka_builder_premium(&self, g: &Game, pos: Pos, improvement: &str) -> f64 {
        // `chase-every-boost` carries this premium as part of its union; see
        // `advanced/chase_every_boost.rs`.
        if !(self.eureka_chasing_builder || self.chase_every_boost) {
            return 0.0;
        }
        let tile = &g.map.tiles[&pos];
        let Some(pid) = tile
            .owner_city
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.owner)
        else {
            return 0.0;
        };
        let premium: f64 = self
            .eureka_chases(g, pid)
            .iter()
            .filter(|chase| {
                if let Some(named) = chase.trigger.strip_prefix("improvement:") {
                    named == improvement
                } else if let Some(named) = chase.trigger.strip_prefix("improvement_on_resource:") {
                    named == improvement && tile.resource.is_some()
                } else if let Some(resource) = chase.trigger.strip_prefix("improve_resource:") {
                    tile.resource.as_deref() == Some(resource)
                        && Self::improvement_connects(g, improvement, resource)
                } else {
                    false
                }
            })
            .map(|chase| chase.per_step() * EUREKA_BUILDER_VALUE_PER_POINT)
            .sum();
        premium.min(EUREKA_BUILDER_VALUE_CAP)
    }

    /// `eureka_chasing_builder_2`: a Builder may break a close improvement
    /// tie only for the final step of the technology or civic the empire is
    /// researching *now*. V1 offered every future boost in both trees at
    /// once, so a farm could outrank a resource or a needed yield merely
    /// because it was one of six Feudalism farms or a future Irrigation step.
    ///
    /// The engine grants a boost's research immediately only when that node
    /// is active (`Game::check_boosts`), and a partial multi-step trigger has
    /// no value until the unknown later steps arrive. Restricting the premium
    /// to that concrete, final action makes the benefit real this turn. Its
    /// own-value cap keeps the ordinary improvement ranking in charge.
    pub(super) fn eureka_builder_v2_premium(
        &self,
        g: &Game,
        pos: Pos,
        improvement: &str,
        base_value: f64,
    ) -> f64 {
        if !self.eureka_chasing_builder_2 {
            return 0.0;
        }
        let tile = &g.map.tiles[&pos];
        let Some(pid) = tile
            .owner_city
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.owner)
        else {
            return 0.0;
        };
        let player = &g.players[pid];
        let active_tech = player.research.as_deref();
        let active_civic = player.civic.as_deref();
        let premium: f64 = self
            .eureka_chases(g, pid)
            .iter()
            .filter(|chase| {
                chase.remaining == 1
                    && (active_tech == Some(chase.node.as_str())
                        || active_civic == Some(chase.node.as_str()))
                    && if let Some(named) = chase.trigger.strip_prefix("improvement:") {
                        named == improvement
                    } else if let Some(named) =
                        chase.trigger.strip_prefix("improvement_on_resource:")
                    {
                        named == improvement && tile.resource.is_some()
                    } else if let Some(resource) = chase.trigger.strip_prefix("improve_resource:") {
                        tile.resource.as_deref() == Some(resource)
                            && Self::improvement_connects(g, improvement, resource)
                    } else {
                        false
                    }
            })
            .map(|chase| chase.per_step() * EUREKA_BUILDER_VALUE_PER_POINT)
            .sum();
        premium
            .min(EUREKA_BUILDER_VALUE_CAP)
            .min(base_value.max(0.0) * EUREKA_BUILDER_V2_OWN_VALUE_FRACTION)
    }

    /// `eureka_chasing_production`: the research this queue item earns toward
    /// a boost, on `production_value`'s raw scale. Zero when the gene is off
    /// or nothing is chased.
    pub(super) fn eureka_production_premium(&self, g: &Game, pid: usize, item: &Item) -> f64 {
        if !self.eureka_chasing_production {
            return 0.0;
        }
        let key = match item {
            Item::Unit { unit } => format!("units_of:{unit}"),
            Item::Building { building } => format!("building:{building}"),
            Item::District { district, .. } => {
                format!("district:{}", g.district_family(*district))
            }
            _ => return 0.0,
        };
        let premium: f64 = self
            .eureka_chases(g, pid)
            .iter()
            .filter(|chase| chase.trigger == key)
            .map(|chase| chase.per_step() * EUREKA_PRODUCTION_VALUE_PER_POINT)
            .sum();
        premium.min(EUREKA_PRODUCTION_VALUE_CAP)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::super::{AdvancedAi, GrandStrategy};
    use super::*;
    use crate::game::{Action, Game, Item};
    use crate::name;

    #[test]
    fn eureka_chasing_builder_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("eureka-chasing-builder", |ai| ai.eureka_chasing_builder);
    }

    #[test]
    fn eureka_chasing_builder_v2_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("eureka-chasing-builder-2", |ai| {
            ai.eureka_chasing_builder_2
        });
    }

    #[test]
    fn eureka_chasing_production_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("eureka-chasing-production", |ai| {
            ai.eureka_chasing_production
        });
    }

    /// One founded capital, every other unit gone, and the tile the test
    /// names beside it.
    fn capital_board(seed: u64) -> (Game, u32, Pos) {
        let mut game = Game::new_full(1, 20, 14, seed, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .expect("the player opens with a settler");
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        for uid in game.player_unit_ids(0) {
            game.remove_unit(uid);
        }
        let cid = game.player_city_ids(0)[0];
        let home = game.cities[&cid].pos;
        (game, cid, home)
    }

    /// An owned land tile one step from the capital, with no feature,
    /// resource or improvement on it.
    fn bare_ring_tile(game: &Game, cid: u32, home: Pos) -> Pos {
        let mut ring: Vec<Pos> = game.cities[&cid]
            .owned_tiles
            .iter()
            .copied()
            .filter(|pos| {
                game.wdist(*pos, home) == 1
                    && game.map.get(*pos).is_some_and(|tile| {
                        game.rules.is_passable(tile)
                            && !game.rules.is_water(tile)
                            && tile.district.is_none()
                    })
            })
            .collect();
        ring.sort_unstable();
        ring.into_iter()
            .next()
            .expect("an owned land tile beside the capital")
    }

    /// The table both eureka genes read: with Mining known and Masonry
    /// neither known nor boosted, a quarry on a resource is one step from
    /// 40% of Masonry; boosting Masonry drops the row.
    #[test]
    fn eureka_chases_list_the_boosts_something_built_can_still_earn() {
        let (mut game, _cid, _home) = capital_board(41_003);
        game.players[0].techs.insert(name!("mining"));
        let ai = AdvancedAi::new();
        let chases = ai.eureka_chases(&game, 0);
        let masonry = chases
            .iter()
            .find(|chase| chase.node == "masonry")
            .expect("Masonry's quarry boost is within reach");
        assert_eq!(masonry.trigger, "improvement_on_resource:quarry");
        assert_eq!(masonry.remaining, 1);
        let expected = game.game_speed.scale(game.rules.techs["masonry"].cost) * 0.4;
        assert!((masonry.research - expected).abs() < 1e-9);
        assert!(
            chases.iter().all(|chase| game
                .rules
                .techs
                .get(&chase.node)
                .is_none_or(|spec| spec.era <= game.world_era + EUREKA_CHASE_ERA_REACH)),
            "nothing past the era reach is chased"
        );
        game.players[0].boosted_techs.insert(name!("masonry"));
        game.turn += 1;
        assert!(ai
            .eureka_chases(&game, 0)
            .iter()
            .all(|chase| chase.node != "masonry"));
    }

    /// The Builder's half: a quarry on stone is worth Masonry's boost more
    /// under the gene, exactly the per-point rate, and nothing once the boost
    /// is in hand.
    #[test]
    fn eureka_chasing_builder_pays_the_quarry_that_earns_masonry() {
        let (mut game, cid, home) = capital_board(41_004);
        let pos = bare_ring_tile(&game, cid, home);
        {
            let tile = game.map.tiles.get_mut(&pos).unwrap();
            tile.feature = None;
            tile.improvement = None;
            tile.hills = true;
            tile.resource = Some(name!("stone"));
        }
        game.players[0].techs.insert(name!("mining"));
        let strategy = GrandStrategy::Expansion;
        let plain = AdvancedAi::new();
        let mut chasing = AdvancedAi::new();
        chasing.enable_eureka_chasing_builder();
        let off = plain.improvement_value(&game, pos, "quarry", strategy);
        let on = chasing.improvement_value(&game, pos, "quarry", strategy);
        let expected = game.game_speed.scale(game.rules.techs["masonry"].cost)
            * 0.4
            * EUREKA_BUILDER_VALUE_PER_POINT;
        assert!(
            (on - off - expected).abs() < 1e-9,
            "the quarry earns Masonry's boost: on {on} off {off} expected +{expected}"
        );
        // A farm on the stone is itself a step: Irrigation's farm-on-a-resource
        // and one of Feudalism's six farms — the gene prices both, nothing else.
        let farm = chasing.improvement_value(&game, pos, "farm", strategy)
            - plain.improvement_value(&game, pos, "farm", strategy);
        let farm_expected = (game.game_speed.scale(game.rules.techs["irrigation"].cost) * 0.4
            + game.game_speed.scale(game.rules.civics["feudalism"].cost) * 0.4 / 6.0)
            * EUREKA_BUILDER_VALUE_PER_POINT;
        assert!(
            (farm - farm_expected).abs() < 1e-9,
            "a farm on the stone is a step toward Irrigation and Feudalism: {farm} v {farm_expected}"
        );
        game.players[0].boosted_techs.insert(name!("masonry"));
        game.turn += 1;
        assert_eq!(
            chasing.improvement_value(&game, pos, "quarry", strategy),
            plain.improvement_value(&game, pos, "quarry", strategy),
            "a boost in hand is not chased"
        );
    }

    /// V2 only pays where the boost changes the active research immediately,
    /// and cannot turn a weak quarry into the entire reason to spend a Builder
    /// charge there. V1 remains the broad historical control.
    #[test]
    fn eureka_chasing_builder_v2_is_an_active_final_step_tiebreak() {
        let (mut game, cid, home) = capital_board(41_006);
        let pos = bare_ring_tile(&game, cid, home);
        {
            let tile = game.map.tiles.get_mut(&pos).unwrap();
            tile.feature = None;
            tile.improvement = None;
            tile.hills = true;
            tile.resource = Some(name!("stone"));
        }
        game.players[0].techs.insert(name!("mining"));
        let strategy = GrandStrategy::Expansion;
        let plain = AdvancedAi::new();
        let mut v1 = AdvancedAi::new();
        v1.enable_eureka_chasing_builder();
        let mut v2 = AdvancedAi::new();
        v2.enable_eureka_chasing_builder_2();

        let off = plain.improvement_value(&game, pos, "quarry", strategy);
        let v1_idle = v1.improvement_value(&game, pos, "quarry", strategy);
        let v2_idle = v2.improvement_value(&game, pos, "quarry", strategy);
        assert!(v1_idle > off, "v1 still prices the future Masonry boost");
        assert_eq!(
            v2_idle, off,
            "v2 ignores an inactive Masonry boost rather than steering every Builder toward it"
        );

        game.players[0].research = Some("masonry".to_string());
        let v2_active = v2.improvement_value(&game, pos, "quarry", strategy);
        let premium = v2_active - off;
        assert!(
            premium > 0.0,
            "the active Masonry quarry is still actionable"
        );
        assert!(
            premium <= off * EUREKA_BUILDER_V2_OWN_VALUE_FRACTION + 1e-9,
            "the immediate boost stays a tiebreak: +{premium} on a base {off}"
        );

        game.players[0].research = None;
        game.players[0].civic = Some("feudalism".to_string());
        let v2_partial = v2.improvement_value(&game, pos, "farm", strategy)
            - plain.improvement_value(&game, pos, "farm", strategy);
        assert_eq!(
            v2_partial, 0.0,
            "one of six Feudalism farms is not paid before it can actually trigger the boost"
        );
    }

    #[test]
    fn eureka_chasing_builder_versions_are_mutually_exclusive() {
        let mut ai = AdvancedAi::new();
        ai.enable_eureka_chasing_builder();
        assert!(ai.eureka_chasing_builder);
        assert!(!ai.eureka_chasing_builder_2);
        ai.enable_eureka_chasing_builder_2();
        assert!(!ai.eureka_chasing_builder);
        assert!(ai.eureka_chasing_builder_2);
        ai.enable_eureka_chasing_builder();
        assert!(ai.eureka_chasing_builder);
        assert!(!ai.eureka_chasing_builder_2);
    }

    /// The production half: with Archery known and no Archer alive, each of
    /// Machinery's three Archers is worth a third of its boost; a Warrior is
    /// worth nothing extra; a completed trigger pays nothing.
    #[test]
    fn eureka_chasing_production_pays_the_archers_that_earn_machinery() {
        let (mut game, _cid, home) = capital_board(41_005);
        game.players[0].techs.insert(name!("archery"));
        let mut ai = AdvancedAi::new();
        ai.enable_eureka_chasing_production();
        let archer = Item::Unit {
            unit: name!("archer"),
        };
        let warrior = Item::Unit {
            unit: name!("warrior"),
        };
        let expected = game.game_speed.scale(game.rules.techs["machinery"].cost) * 0.4 / 3.0
            * EUREKA_PRODUCTION_VALUE_PER_POINT;
        let premium = ai.eureka_production_premium(&game, 0, &archer);
        assert!(
            (premium - expected).abs() < 1e-9,
            "one of three Archers: {premium} v {expected}"
        );
        assert_eq!(ai.eureka_production_premium(&game, 0, &warrior), 0.0);
        assert_eq!(
            AdvancedAi::new().eureka_production_premium(&game, 0, &archer),
            0.0
        );
        for _ in 0..3 {
            game.spawn_unit("archer", 0, home);
        }
        game.turn += 1;
        assert_eq!(
            ai.eureka_production_premium(&game, 0, &archer),
            0.0,
            "three Archers alive complete the trigger; a fourth earns nothing"
        );
    }
}
