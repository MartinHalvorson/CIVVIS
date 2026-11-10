//! `chop-feeds-the-long-build`: a Builder clears a Forest into a district, a
//! wonder or a space-race project.
//!
//! ## What the live seat does without it
//!
//! [`super::chop_for_expansion`] is the only caller of a clearing, and it runs
//! one errand: a Settler under production. In 30 live Deity games measured on
//! 2026-11-09 that came to a median of 2.5 accepted clearings a game, 89% of
//! them before turn 100 and every one for a Settler. No district, no wonder
//! and no space-race project has ever been paid by a clearing.
//!
//! ## What a clearing is worth on the host
//!
//! Measured either side of 48 accepted Forest clearings in those games, the
//! owning city receives `20 × game speed × (1 + 9 × progress)` Production,
//! where `progress` is the share of every technology and civic the player has
//! completed. District costs grow on almost the same curve, so one Forest is
//! worth about a third of a fresh district at any point in the game, and by
//! the space race it is worth more than a turn of a strong city's output.
//!
//! An improvement on the same tile pays one or two Production a turn for as
//! long as the tile is worked. So a clearing wins exactly where the build is
//! long and the time matters:
//!
//! - **A district or wonder in a city that needs many turns for it.** A new
//!   city making five Production a turn waits twenty turns for its first
//!   district. Two Forests bring that district online more than ten turns
//!   earlier, and the district then pays for the rest of the game.
//! - **A space-race project, and the Spaceport before it.** These are the last
//!   builds of a Science game. Nothing the tile could yield afterwards
//!   matters, and every turn saved is a turn off the victory clock. Rival
//!   science lands at a median of turn 188 against our turn 186 to 205 wins.
//!
//! ## What the gene does
//!
//! The same two legs as the Settler errand, for a different deadline. A city
//! of ours has a district, a wonder or a space-race project at the head of
//! its queue.
//!
//! 1. **Act.** Standing on a qualifying tile, the Builder clears it.
//! 2. **Walk.** Otherwise the best qualifying tile within reach becomes its
//!    destination, through the same barbarian-safe stepper.
//!
//! A tile qualifies when all of these hold:
//!
//! - [`Game::builder_operations`] offers a feature removal or a harvest there
//!   and it pays Production;
//! - the tile's owning city has a long build at the head of its queue that
//!   would not finish this turn anyway;
//! - for a district or wonder, the city needs at least [`LONG_BUILD_TURNS`]
//!   more turns of its own Production for it, and the clearing saves at least
//!   [`LONG_BUILD_TURNS_SAVED`] of them. A strong city finishes its district
//!   in a few turns and keeps its Forests; the Builder's charge is worth more
//!   as an improvement there;
//! - for the space race neither floor applies: any Production clearing in a
//!   city running a project or building its Spaceport qualifies.
//!
//! Space-race tiles rank first, then the most turns saved, then the tile whose
//! ordinary improvement value is lowest, then the nearest, then the lowest
//! position for determinism.
//!
//! ## What it deliberately does not do
//!
//! - **It never invents legality or price.** Candidates come from
//!   `Game::builder_operations` and are priced by
//!   `Game::builder_operation_payout`, as in the Settler errand.
//! - **It never clears for a unit or a building.** Those are short builds.
//! - **It does not buy or train a Builder for the purpose.** It spends charges
//!   the empire already has. Training Builders to clear a spaceport city's
//!   land is the next errand, not this one.
//! - **It does not touch the Settler errand.** `chop-for-expansion` runs first
//!   and is unchanged; either gene works without the other.
//!
//! ## What the first paired read said, and the second gene
//!
//! Twenty Deity seeds against the live genome, with and without the gene
//! (2026-11-10): no win in either arm, score share flat, and science 8% lower
//! at turn 100 (z -2.3), fading by turn 150. Every game ended near turn 163 on
//! a rival's victory, so no seat reached a space project and the space-race
//! leg was never exercised. The district and wonder leg therefore reads as no
//! help at best, and the two legs need separate prices.
//!
//! `chop-feeds-the-space-race` is the space-race leg alone: with only that
//! gene on, a district or wonder is never a long build and nothing is cleared
//! for one. Either gene works without the other.
//!
//! ⚠ The gene is off by default and inert while off:
//! [`AdvancedAi::chop_feeds_the_long_build_step`] returns `None` before
//! reading anything, so a game plays byte-identically.

use super::chop_for_expansion::CHOP_OPERATIONS;
use super::{Action, AdvancedAi, Game, GrandStrategy, Item, Pos};
use crate::name::Name;
use crate::reasoning::plain;
use crate::think;

/// A district or wonder is a long build when its city needs at least this
/// many more turns of its own Production to finish it.
pub(super) const LONG_BUILD_TURNS: f64 = 8.0;

/// A clearing for a district or wonder must save at least this many turns of
/// the city's own Production.
pub(super) const LONG_BUILD_TURNS_SAVED: f64 = 2.0;

/// How far a Builder walks to clear for a district or wonder, in tiles. The
/// Settler errand's radius.
pub(super) const LONG_BUILD_RADIUS: i32 = 3;

/// How far a Builder walks to clear for the space race, in tiles. The whole
/// workable radius of the project city and a turn of walking beyond it.
pub(super) const SPACE_RACE_RADIUS: i32 = 6;

/// The long build at the head of one city's queue.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct LongBuild {
    /// Production the item still needs.
    pub(super) remaining: f64,
    /// The city's own Production toward it each turn.
    pub(super) per_turn: f64,
    /// A space-race project, or the Spaceport itself.
    pub(super) space_race: bool,
}

/// One qualifying tile, ranked.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct LongBuildChop {
    pub(super) pos: Pos,
    pub(super) payout: f64,
    /// Turns of the owning city's Production the clearing replaces.
    pub(super) turns_saved: f64,
    pub(super) space_race: bool,
    /// What the ordinary Builder ranking thinks this tile is worth improved.
    pub(super) improvement_value: f64,
    pub(super) distance: i32,
}

impl AdvancedAi {
    /// The long build `cid` is paying for, if a clearing would serve it.
    ///
    /// `None` for a unit, a building, a repair, a district project or a
    /// product, for an item that finishes this turn anyway, and for a
    /// district or wonder the city finishes inside [`LONG_BUILD_TURNS`].
    pub(super) fn chop_long_build(&self, g: &Game, pid: usize, cid: u32) -> Option<LongBuild> {
        let city = g.cities.get(&cid)?;
        if city.owner != pid {
            return None;
        }
        let item = city.queue.first()?;
        let space_race = match item {
            Item::Project { project } if Game::is_space_project(project.as_str()) => true,
            Item::District { district, .. } => district.as_str() == "spaceport",
            Item::Wonder { .. } => false,
            _ => return None,
        };
        // `chop-feeds-the-space-race` runs the space-race leg alone.
        if !space_race && !self.chop_feeds_the_long_build {
            return None;
        }
        let remaining = g.item_cost_for_city(pid, cid, item) - city.production;
        let per_turn = g.city_yields(cid).production;
        if remaining <= per_turn {
            return None;
        }
        if !space_race && remaining < LONG_BUILD_TURNS * per_turn.max(1.0) {
            return None;
        }
        Some(LongBuild {
            remaining,
            per_turn,
            space_race,
        })
    }

    /// Every owned tile within reach of `from` whose clearing would pay
    /// Production into a long build.
    pub(super) fn long_build_chop_candidates(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        strategy: GrandStrategy,
    ) -> Vec<LongBuildChop> {
        let mut found: Vec<LongBuildChop> = Vec::new();
        for cid in g.player_city_ids(pid) {
            let Some(build) = self.chop_long_build(g, pid, cid) else {
                continue;
            };
            let radius = if build.space_race {
                SPACE_RACE_RADIUS
            } else {
                LONG_BUILD_RADIUS
            };
            for pos in &g.cities[&cid].owned_tiles {
                let distance = g.wdist(from, *pos);
                if distance > radius || self.conversion_preserves_woods(g, *pos) {
                    continue;
                }
                let payout = g
                    .builder_operations(pid, *pos)
                    .into_iter()
                    .filter(|operation| CHOP_OPERATIONS.contains(&operation.as_str()))
                    .map(|operation| self.chop_production_payout(g, pid, *pos, &operation))
                    .fold(0.0_f64, f64::max);
                if payout <= 0.0 {
                    continue;
                }
                // A clearing cannot save more turns than the item has left.
                let turns_saved = payout.min(build.remaining) / build.per_turn.max(1.0);
                if !build.space_race && turns_saved < LONG_BUILD_TURNS_SAVED {
                    continue;
                }
                let improvement_value = self
                    .worthwhile_improvements(g, pid, *pos, strategy)
                    .into_iter()
                    .map(|improvement| {
                        self.production_foundation_improvement_value(
                            g,
                            pid,
                            *pos,
                            &improvement,
                            strategy,
                            0.0,
                        )
                    })
                    .fold(0.0_f64, f64::max);
                found.push(LongBuildChop {
                    pos: *pos,
                    payout,
                    turns_saved,
                    space_race: build.space_race,
                    improvement_value,
                    distance,
                });
            }
        }
        found
    }

    /// The tile this errand would clear next.
    pub(super) fn best_long_build_chop(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        strategy: GrandStrategy,
    ) -> Option<LongBuildChop> {
        self.long_build_chop_candidates(g, pid, from, strategy)
            .into_iter()
            .min_by(|left, right| {
                right
                    .space_race
                    .cmp(&left.space_race)
                    .then(right.turns_saved.total_cmp(&left.turns_saved))
                    .then(left.improvement_value.total_cmp(&right.improvement_value))
                    .then(left.distance.cmp(&right.distance))
                    .then(left.pos.cmp(&right.pos))
            })
    }

    /// One Builder's turn on the errand.
    ///
    /// `None` when the gene is off or no tile qualifies, so the ordinary
    /// Builder decision runs. `Some(acted)` when the errand took the turn.
    pub(super) fn chop_feeds_the_long_build_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
    ) -> Option<bool> {
        if !self.chop_feeds_the_long_build && !self.chop_feeds_the_space_race {
            return None;
        }
        let from = g.units.get(&uid)?.pos;
        let best = self.best_long_build_chop(g, pid, from, strategy)?;
        if best.pos == from {
            let operation = g
                .builder_operations(pid, from)
                .into_iter()
                .filter(|operation| CHOP_OPERATIONS.contains(&operation.as_str()))
                .max_by(|left, right| {
                    self.chop_production_payout(g, pid, from, left)
                        .total_cmp(&self.chop_production_payout(g, pid, from, right))
                        .then(right.cmp(left))
                })?;
            let purpose = if best.space_race {
                "the space race"
            } else {
                "the long build"
            };
            think!(self.journal(), Economy, Decision,
                   "Clearing {} at {from:?} for {purpose}", plain(&operation);
                   "it pays {:.0} production, {:.1} turns of that city's own output",
                   best.payout, best.turns_saved; from);
            // ⚠ The engine retires the Builder when this spends its last
            // charge, so nothing may touch `uid` after the operation lands.
            let acted = g
                .apply(
                    pid,
                    &Action::Improve {
                        unit: uid,
                        improvement: Name::new(&operation),
                    },
                )
                .is_ok();
            if acted {
                self.builder_targets.remove(&uid);
            }
            return Some(acted);
        }
        self.builder_targets.insert(uid, best.pos);
        let stepped = if self.builder_reach_safety_on() {
            self.builder_step_out_of_reach(g, pid, uid, best.pos)
        } else {
            self.builder_step_toward_barbarian_safe(g, pid, uid, best.pos)
        };
        // A destination the Builder cannot step toward must not eat the turn.
        stepped.then_some(true)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::*;
    use crate::ai::VictoryTarget;

    /// A capital at (5,5), Mining in hand, a Forest on an owned tile and a
    /// Builder standing on it.
    fn fixture() -> (Game, AdvancedAi, u32, u32, Pos) {
        let mut g = Game::new_full(2, 24, 16, 91_516, 150, 0, false);
        for uid in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(uid);
        }
        for tile in g.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.river_edges = [false; 6];
            tile.resource = None;
            tile.improvement = None;
        }
        g.current = 0;
        let founder = g.spawn_test_unit("settler", 0, (5, 5));
        g.apply(0, &Action::FoundCity { unit: founder }).unwrap();
        let city = g.player_city_ids(0)[0];
        g.cities.get_mut(&city).unwrap().pop = 2;
        g.players[0].techs.insert(crate::name!("mining"));
        let wood = (6, 5);
        assert!(g.cities[&city].owned_tiles.contains(&wood));
        g.map.tiles.get_mut(&wood).unwrap().feature = Some(crate::name!("forest"));
        let builder = g.spawn_test_unit("builder", 0, wood);
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.enable_chop_feeds_the_long_build();
        (g, ai, city, builder, wood)
    }

    /// Put `item` at the head of the queue with `owed` Production still to pay.
    fn build(g: &mut Game, city: u32, item: Item, owed: f64) {
        g.cities.get_mut(&city).unwrap().queue = vec![item];
        owe(g, city, owed);
    }

    /// Leave the head of the queue with `owed` Production still to pay.
    fn owe(g: &mut Game, city: u32, owed: f64) {
        let item = g.cities[&city].queue[0].clone();
        let cost = g.item_cost_for_city(0, city, &item);
        g.cities.get_mut(&city).unwrap().production = cost - owed;
    }

    fn satellite() -> Item {
        Item::Project {
            project: crate::name!("launch_earth_satellite"),
        }
    }

    fn campus() -> Item {
        Item::District {
            district: crate::name!("campus"),
            pos: (4, 5),
        }
    }

    #[test]
    fn chop_feeds_the_long_build_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("chop-feeds-the-long-build", |ai| {
            ai.chop_feeds_the_long_build
        });
    }

    #[test]
    fn chop_feeds_the_space_race_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("chop-feeds-the-space-race", |ai| {
            ai.chop_feeds_the_space_race
        });
    }

    /// The space-race gene alone clears for a project and for nothing else.
    #[test]
    fn the_space_race_gene_alone_ignores_a_district() {
        let (mut g, mut ai, city, builder, wood) = fixture();
        ai.disable_chop_feeds_the_long_build();
        ai.enable_chop_feeds_the_space_race();

        build(&mut g, city, campus(), 10_000.0);
        assert_eq!(ai.chop_long_build(&g, 0, city), None);
        assert_eq!(
            ai.chop_feeds_the_long_build_step(&mut g, 0, builder, GrandStrategy::Science),
            None
        );
        assert_eq!(g.map.tiles[&wood].feature, Some(crate::name!("forest")));

        build(&mut g, city, satellite(), 10_000.0);
        assert_eq!(
            ai.chop_feeds_the_long_build_step(&mut g, 0, builder, GrandStrategy::Science),
            Some(true)
        );
        assert_eq!(g.map.tiles[&wood].feature, None, "the Forest is cleared");
    }

    #[test]
    fn a_builder_on_a_forest_clears_it_into_a_space_project() {
        let (mut g, mut ai, city, builder, wood) = fixture();
        build(&mut g, city, satellite(), 10_000.0);
        let before = g.cities[&city].production;
        let payout = ai.chop_production_payout(&g, 0, wood, "chop_woods");
        assert!(payout > 0.0);
        assert_eq!(
            ai.chop_feeds_the_long_build_step(&mut g, 0, builder, GrandStrategy::Science),
            Some(true)
        );
        assert_eq!(g.map.tiles[&wood].feature, None, "the Forest is cleared");
        assert_eq!(
            g.cities[&city].production,
            before + payout,
            "and the project is paid what the engine priced the Forest at"
        );
    }

    #[test]
    fn a_builder_walks_six_tiles_for_the_space_race_and_three_for_a_district() {
        let (mut g, mut ai, city, _, wood) = fixture();
        build(&mut g, city, satellite(), 10_000.0);
        let away = (10, 5);
        assert!(g.wdist(away, wood) > LONG_BUILD_RADIUS);
        assert!(g.wdist(away, wood) <= SPACE_RACE_RADIUS);
        let builder = g.spawn_test_unit("builder", 0, away);
        assert_eq!(
            ai.chop_feeds_the_long_build_step(&mut g, 0, builder, GrandStrategy::Science),
            Some(true)
        );
        assert_ne!(g.units[&builder].pos, away, "it set off for the Forest");
        assert_eq!(ai.builder_targets[&builder], wood);

        build(&mut g, city, campus(), 10_000.0);
        assert_eq!(
            ai.best_long_build_chop(&g, 0, away, GrandStrategy::Science),
            None,
            "a district is not worth a walk across the empire"
        );
    }

    /// The two floors, read off the fixture's own numbers so the test holds at
    /// any game speed and any payout rule.
    #[test]
    fn a_district_is_cleared_for_only_when_it_is_long_and_the_clearing_saves_turns() {
        let (mut g, ai, city, builder, wood) = fixture();
        let from = g.units[&builder].pos;
        let payout = ai.chop_production_payout(&g, 0, wood, "chop_woods");

        // A long district: the clearing qualifies exactly when it saves
        // enough turns of this city's own output.
        build(&mut g, city, campus(), 10_000.0);
        let per_turn = g.city_yields(city).production.max(1.0);
        owe(&mut g, city, LONG_BUILD_TURNS * per_turn + 1.0);
        let long = ai.chop_long_build(&g, 0, city).expect("a long build");
        assert!(!long.space_race);
        assert_eq!(
            ai.best_long_build_chop(&g, 0, from, GrandStrategy::Science)
                .is_some(),
            payout / per_turn >= LONG_BUILD_TURNS_SAVED
        );

        // A district this city finishes in two turns keeps its Forest.
        owe(&mut g, city, 2.0 * per_turn + 0.5);
        assert_eq!(ai.chop_long_build(&g, 0, city), None);
        assert_eq!(
            ai.best_long_build_chop(&g, 0, from, GrandStrategy::Science),
            None
        );

        // The same two turns left on a space project still qualify.
        build(&mut g, city, satellite(), 10_000.0);
        let per_turn = g.city_yields(city).production.max(1.0);
        owe(&mut g, city, 2.0 * per_turn + 0.5);
        assert!(ai.chop_long_build(&g, 0, city).unwrap().space_race);
        assert!(ai
            .best_long_build_chop(&g, 0, from, GrandStrategy::Science)
            .is_some());
    }

    #[test]
    fn a_build_that_finishes_this_turn_is_not_cleared_for() {
        let (mut g, mut ai, city, builder, wood) = fixture();
        build(&mut g, city, satellite(), 10_000.0);
        let per_turn = g.city_yields(city).production;
        owe(&mut g, city, per_turn * 0.5);
        assert_eq!(ai.chop_long_build(&g, 0, city), None);
        assert_eq!(
            ai.chop_feeds_the_long_build_step(&mut g, 0, builder, GrandStrategy::Science),
            None
        );
        assert_eq!(g.map.tiles[&wood].feature, Some(crate::name!("forest")));
    }

    #[test]
    fn units_and_buildings_are_not_long_builds() {
        let (mut g, ai, city, _, _) = fixture();
        for item in [
            Item::Unit {
                unit: crate::name!("settler"),
            },
            Item::Building {
                building: crate::name!("monument"),
            },
        ] {
            build(&mut g, city, item, 10_000.0);
            assert_eq!(ai.chop_long_build(&g, 0, city), None);
        }
    }

    #[test]
    fn a_food_only_clearance_does_not_serve_a_long_build() {
        let (mut g, mut ai, city, builder, wood) = fixture();
        build(&mut g, city, satellite(), 10_000.0);
        g.players[0].techs.insert(crate::name!("irrigation"));
        g.map.tiles.get_mut(&wood).unwrap().feature = Some(crate::name!("marsh"));
        assert_eq!(
            ai.chop_feeds_the_long_build_step(&mut g, 0, builder, GrandStrategy::Science),
            None
        );
        assert_eq!(g.map.tiles[&wood].feature, Some(crate::name!("marsh")));
    }

    #[test]
    fn the_gene_off_is_byte_identical() {
        let (mut g, mut ai, city, builder, wood) = fixture();
        build(&mut g, city, satellite(), 10_000.0);
        ai.disable_chop_feeds_the_long_build();
        assert_eq!(
            ai.chop_feeds_the_long_build_step(&mut g, 0, builder, GrandStrategy::Science),
            None
        );
        assert_eq!(
            g.map.tiles[&wood].feature,
            Some(crate::name!("forest")),
            "nothing was cleared"
        );
    }
}
