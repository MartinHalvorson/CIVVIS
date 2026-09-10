//! `chop-for-expansion`: a Builder turns a forest into a Settler.
//!
//! ## The premise this gene corrects
//!
//! [`super::expansion_scales_with_difficulty`] raises the empire's city target
//! with the difficulty rung, because from Emperor upward every rival opens
//! with free Settlers and every rival yield is scaled per city. Its own note
//! records the human answer to that field as *"eight to twelve cities by turn
//! 100, funded by the expansion policy cards and by Builder chops"*, and then
//! records the chop leg as unavailable:
//!
//! > **Chops: unavailable, and not invented here.** … **This simulator has no
//! > harvest.** `grep -n 'harvest\|chop' src/game.rs` returns three lines and
//! > all three are the reasoning-log sense of the word.
//!
//! ⚠⚠ **That grep was reading the wrong file, and the conclusion is wrong.**
//! The action layer had been carved out of `src/game.rs` into
//! `src/game/actions.rs` by `#2668` before that note was written, so the grep
//! found only the leftovers. This engine has modelled feature removal and
//! resource harvesting the whole time:
//! [`Game::builder_operations`] offers `chop_woods`, `chop_rainforest`,
//! `clear_marsh`, `harvest_resource` and `plant_woods`;
//! `Game::do_builder_operation` executes them, pays the shipped
//! `Feature_Removes` and `Resource_Harvests` bases scaled by the world era
//! and by Magnus' `harvest_pct`, honours the Deforestation Treaty, spends the
//! charge and retires the Builder — and `Action::Improve` already carries
//! them, so `legal_actions_within` enumerates every one.
//!
//! ⚠⚠ **What was actually missing is a caller.** Before this gene, `chop_woods`
//! and `harvest_resource` appeared **nowhere** in the 69,000 lines of
//! `src/ai.rs` and `src/ai/advanced/`, and `builder_operations` had no caller
//! outside the engine's own enumeration, its executor and two tests. The
//! empire has never chopped a single tile in any recorded game — not because
//! the rule is missing, but because no controller ever asks for it.
//!
//! ## Why a Builder never stumbles onto one either
//!
//! [`AdvancedAi::advanced_builder_step`] decides from
//! `worthwhile_improvements`, which ranks *improvements* — the rows of
//! `data/improvements.json`. A builder operation is not an improvement and
//! never appears in that list, at the tile the Builder stands on or anywhere
//! in the empire-wide sweep that chooses where it walks next. So a chop could
//! only ever happen by accident, and the sweep is exactly where this
//! controller has been caught before: the note above the same sweep records
//! that a pillaged tile was invisible to it, so *"the Builder had to wander
//! onto it by accident"* while `has_builder_work` counted the very same tile.
//!
//! This gene therefore has two legs, and it needs both. Seeing a chop is
//! worth nothing if the Builder is never sent to one.
//!
//! ## What the gene does
//!
//! One errand, with a deadline it did not invent: a city of ours is building
//! a Settler right now.
//!
//! 1. **Act.** Standing on a qualifying tile, the Builder chops it.
//! 2. **Walk.** Otherwise the best qualifying tile within
//!    [`CHOP_ERRAND_RADIUS`] becomes the destination, through the same
//!    barbarian-safe stepper every other Builder errand uses.
//!
//! A tile qualifies when all of these hold:
//!
//! - [`Game::builder_operations`] offers a **feature removal or a harvest**
//!   there for us (`plant_woods` is excluded: it is the opposite errand);
//! - the operation pays **Production** — a Settler is bought with hammers, so
//!   a Marsh's Food and a Copper's Gold do not serve this errand;
//! - the tile's owning city is **producing a Settler**;
//! - the payout covers at least [`CHOP_MIN_SHARE`] of what that Settler still
//!   has left to pay, so the charge is spent on a real acceleration rather
//!   than on a rounding error;
//! - the Settler would **not finish this turn anyway**, which is the one case
//!   where the whole payout is wasted.
//!
//! Among qualifying tiles the errand takes the largest payout, then the tile
//! whose ordinary improvement value is **lowest** — a Builder charge spent
//! here is a charge not spent improving, so of two equal chops it takes the
//! one that costs the empire least — then the nearest, then the lowest
//! position for determinism.
//!
//! ## What it deliberately does not do
//!
//! - **It never invents legality.** Every candidate comes from
//!   `Game::builder_operations`, so district sites, wonder tiles, foreign and
//!   unowned land, and a Deforestation Treaty in force are all excluded by the
//!   engine, not by a second opinion here.
//! - **It never chops for anything but a Settler.** Districts, wonders and
//!   buildings are the obvious next errands and every one of them widens the
//!   set of tiles the empire will destroy. Widening is a versioned sibling
//!   (`chop-for-expansion-2`), not an edit to this one.
//! - **It never prices the payout itself.** `Game::builder_operation_payout`
//!   is the function the executor pays from, so the number this gene chooses
//!   on is the number the engine hands over.
//!
//! ⚠ The gene is off by default and every leg is inert while it is off: with
//! the flag clear [`AdvancedAi::chop_for_expansion_step`] returns `None`
//! before reading anything, so a game plays byte-identically.

use super::{Action, AdvancedAi, Game, GrandStrategy, Item, Pos};
use crate::name::Name;
use crate::reasoning::plain;
use crate::think;

/// A chop must cover at least this share of what the Settler still owes.
///
/// An Ancient-era Forest pays 20 Production and a Settler's shipped cost is
/// 80, so the first chop of a game clears this bar with room to spare while a
/// Jungle's 10 Production against a nearly-finished Settler does not.
pub(super) const CHOP_MIN_SHARE: f64 = 0.20;

/// How far a Builder will walk to run this errand, in tiles.
///
/// A Settler is a short deadline, and a Builder that crosses the empire for a
/// chop arrives after the Settler has already been paid for. Three rings is
/// about two turns for a Builder with two movement.
pub(super) const CHOP_ERRAND_RADIUS: i32 = 3;

/// The builder operations this errand will run: feature removal and harvest.
///
/// `plant_woods` is the opposite errand and is never one of these.
pub(super) const CHOP_OPERATIONS: [&str; 4] = [
    "chop_woods",
    "chop_rainforest",
    "clear_marsh",
    "harvest_resource",
];

/// One qualifying tile, ranked.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) struct ChopCandidate {
    pub(super) pos: Pos,
    pub(super) payout: f64,
    /// What the ordinary Builder ranking thinks this tile is worth improved.
    /// Lower is a cheaper tile to spend, and breaks a tie on payout.
    pub(super) improvement_value: f64,
    pub(super) distance: i32,
}

impl AdvancedAi {
    /// The Production a builder operation on `pos` would pay, or zero.
    ///
    /// Reads `Game::builder_operation_payout`, the same function the executor
    /// pays from, and keeps only the Production rows: this errand exists to
    /// buy a Settler, and a Settler is bought with hammers.
    pub(super) fn chop_production_payout(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        operation: &str,
    ) -> f64 {
        g.builder_operation_payout(pid, pos, operation)
            .into_iter()
            .filter(|(yield_type, _)| yield_type == "production")
            .map(|(_, amount)| amount)
            .sum()
    }

    /// What the Settler in this city still owes, or `None` when the city is
    /// not building one, or would finish it this turn regardless.
    ///
    /// The whole payout of a chop is wasted on a Settler that completes
    /// anyway, so that case is not a weak candidate — it is not a candidate.
    pub(super) fn chop_settler_shortfall(&self, g: &Game, pid: usize, cid: u32) -> Option<f64> {
        let city = g.cities.get(&cid)?;
        if city.owner != pid {
            return None;
        }
        let item = city.queue.first()?;
        match item {
            Item::Unit { unit } if unit == "settler" => {}
            _ => return None,
        }
        let cost = g.item_cost_for_city(pid, cid, item);
        let remaining = cost - city.production;
        (remaining > g.city_yields(cid).production).then_some(remaining)
    }

    /// Every tile this errand would accept, unranked.
    ///
    /// Legality is never decided here: each candidate is an operation
    /// `Game::builder_operations` already offered for this seat on that tile.
    pub(super) fn chop_candidates(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        strategy: GrandStrategy,
    ) -> Vec<ChopCandidate> {
        let mut found: Vec<ChopCandidate> = Vec::new();
        for cid in g.player_city_ids(pid) {
            let Some(remaining) = self.chop_settler_shortfall(g, pid, cid) else {
                continue;
            };
            for pos in &g.cities[&cid].owned_tiles {
                let distance = g.wdist(from, *pos);
                if distance > CHOP_ERRAND_RADIUS {
                    continue;
                }
                let payout = g
                    .builder_operations(pid, *pos)
                    .into_iter()
                    .filter(|operation| CHOP_OPERATIONS.contains(&operation.as_str()))
                    .map(|operation| self.chop_production_payout(g, pid, *pos, &operation))
                    .fold(0.0_f64, f64::max);
                if payout <= 0.0 || payout < remaining * CHOP_MIN_SHARE {
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
                found.push(ChopCandidate {
                    pos: *pos,
                    payout,
                    improvement_value,
                    distance,
                });
            }
        }
        found
    }

    /// The candidate this errand takes: biggest payout, then the tile the
    /// empire can most afford to spend, then the nearest, then the lowest
    /// position so two identical boards choose identically.
    pub(super) fn best_chop_candidate(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        strategy: GrandStrategy,
    ) -> Option<ChopCandidate> {
        self.chop_candidates(g, pid, from, strategy)
            .into_iter()
            .min_by(|left, right| {
                right
                    .payout
                    .total_cmp(&left.payout)
                    .then(left.improvement_value.total_cmp(&right.improvement_value))
                    .then(left.distance.cmp(&right.distance))
                    .then(left.pos.cmp(&right.pos))
            })
    }

    /// The errand. `None` means it did not apply and the ordinary Builder
    /// decision runs untouched; `Some(acted)` is this Builder's whole turn.
    pub(super) fn chop_for_expansion_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
    ) -> Option<bool> {
        if !self.chop_for_expansion {
            return None;
        }
        let from = g.units.get(&uid)?.pos;
        let best = self.best_chop_candidate(g, pid, from, strategy)?;
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
            think!(self.journal(), Expansion, Detail,
                   "Clearing {} at {from:?} for the Settler", plain(&operation);
                   "it pays {:.0} production, and the Settler still owes more than {:.0}",
                   best.payout, best.payout / CHOP_MIN_SHARE; from);
            // ⚠ The Builder is retired by the engine when this spends its last
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
        let stepped = if self.civilian_reach_safety_on() {
            self.builder_step_out_of_reach(g, pid, uid, best.pos)
        } else {
            self.builder_step_toward_barbarian_safe(g, pid, uid, best.pos)
        };
        // A destination we cannot step toward must not eat the turn: falling
        // through lets the ordinary Builder decision run, exactly as the
        // shipped sweep does when its own target is unreachable.
        stepped.then_some(true)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::*;
    use crate::ai::VictoryTarget;

    /// A capital at (5,5), a Builder beside it, Mining in hand so `chop_woods`
    /// is offered, and a Forest on an owned tile.
    fn fixture() -> (Game, AdvancedAi, u32, u32, Pos) {
        let mut g = Game::new_full(2, 24, 16, 91_515, 150, 0, false);
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
        ai.enable_chop_for_expansion();
        (g, ai, city, builder, wood)
    }

    /// Put a Settler at the head of the city's queue with nothing paid in.
    fn build_a_settler(g: &mut Game, city: u32) {
        let settler = Item::Unit {
            unit: crate::name!("settler"),
        };
        let city = g.cities.get_mut(&city).unwrap();
        city.queue = vec![settler];
        city.production = 0.0;
    }

    #[test]
    fn chop_for_expansion_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("chop-for-expansion", |ai| ai.chop_for_expansion);
    }

    /// The premise the gene exists to correct: the engine has always offered
    /// the operation and always paid for it.
    #[test]
    fn the_engine_offers_and_pays_for_a_chop() {
        let (g, _, _, _, wood) = fixture();
        assert!(
            g.builder_operations(0, wood)
                .iter()
                .any(|operation| operation == "chop_woods"),
            "the engine offers chop_woods on an owned Forest with Mining in hand"
        );
        let payout = g.builder_operation_payout(0, wood, "chop_woods");
        assert_eq!(
            payout,
            vec![("production".to_string(), 20.0)],
            "the shipped Feature_Removes base for Forest, Ancient era"
        );
    }

    /// The gap the gene closes: the ordinary Builder ranking cannot see one.
    #[test]
    fn the_ordinary_builder_ranking_cannot_see_a_chop() {
        let (g, ai, _, _, wood) = fixture();
        let improvements = ai.worthwhile_improvements(&g, 0, wood, GrandStrategy::Expansion);
        assert!(
            !improvements
                .iter()
                .any(|improvement| CHOP_OPERATIONS.contains(&improvement.as_str())),
            "a builder operation is not an improvement, so it is never ranked"
        );
    }

    #[test]
    fn a_builder_on_a_forest_chops_it_for_the_settler() {
        let (mut g, mut ai, city, builder, wood) = fixture();
        build_a_settler(&mut g, city);
        assert_eq!(
            ai.chop_for_expansion_step(&mut g, 0, builder, GrandStrategy::Expansion),
            Some(true)
        );
        assert_eq!(g.map.tiles[&wood].feature, None, "the Forest is cleared");
        assert_eq!(
            g.cities[&city].production, 20.0,
            "and the Settler is paid the shipped Forest base"
        );
    }

    #[test]
    fn a_builder_walks_to_the_chop_it_is_not_standing_on() {
        let (mut g, mut ai, city, _, _) = fixture();
        build_a_settler(&mut g, city);
        let away = (4, 4);
        let builder = g.spawn_test_unit("builder", 0, away);
        assert_eq!(
            ai.chop_for_expansion_step(&mut g, 0, builder, GrandStrategy::Expansion),
            Some(true)
        );
        assert_ne!(g.units[&builder].pos, away, "it set off for the Forest");
        assert_eq!(ai.builder_targets[&builder], (6, 5));
    }

    #[test]
    fn no_settler_in_production_is_no_errand() {
        let (mut g, mut ai, city, builder, _) = fixture();
        g.cities.get_mut(&city).unwrap().queue = vec![Item::Unit {
            unit: crate::name!("warrior"),
        }];
        assert_eq!(
            ai.chop_for_expansion_step(&mut g, 0, builder, GrandStrategy::Expansion),
            None,
            "the errand has no deadline, so the ordinary decision runs"
        );
    }

    /// The whole payout is wasted on a Settler that completes anyway.
    #[test]
    fn a_settler_finishing_this_turn_is_not_chopped_for() {
        let (mut g, mut ai, city, builder, _) = fixture();
        build_a_settler(&mut g, city);
        let cost = g.item_cost_for_city(
            0,
            city,
            &Item::Unit {
                unit: crate::name!("settler"),
            },
        );
        g.cities.get_mut(&city).unwrap().production = cost - 1.0;
        assert_eq!(
            ai.chop_for_expansion_step(&mut g, 0, builder, GrandStrategy::Expansion),
            None
        );
    }

    /// A payout too small to matter is not worth a Builder charge.
    #[test]
    fn a_chop_below_the_share_floor_is_refused() {
        let (mut g, ai, city, builder, _) = fixture();
        build_a_settler(&mut g, city);
        // 20 Production against a 400-cost item is 5%, under the 20% floor.
        g.cities.get_mut(&city).unwrap().queue = vec![Item::Unit {
            unit: crate::name!("settler"),
        }];
        g.cities.get_mut(&city).unwrap().production = 0.0;
        let remaining = 20.0 / CHOP_MIN_SHARE + 1.0;
        assert!(
            ai.chop_settler_shortfall(&g, 0, city).unwrap() < remaining,
            "the fixture's Settler is inside the floor, so the gene fires"
        );
        assert!(ai
            .best_chop_candidate(&g, 0, g.units[&builder].pos, GrandStrategy::Expansion)
            .is_some());
        // Now make the same Settler far too expensive for one Forest.
        g.cities.get_mut(&city).unwrap().production = -1_000.0;
        assert_eq!(
            ai.best_chop_candidate(&g, 0, g.units[&builder].pos, GrandStrategy::Expansion),
            None,
            "20 Production is a rounding error against that shortfall"
        );
    }

    #[test]
    fn a_food_only_clearance_does_not_serve_a_settler() {
        let (mut g, mut ai, city, builder, wood) = fixture();
        build_a_settler(&mut g, city);
        g.players[0].techs.insert(crate::name!("irrigation"));
        g.map.tiles.get_mut(&wood).unwrap().feature = Some(crate::name!("marsh"));
        assert!(
            g.builder_operations(0, wood)
                .iter()
                .any(|operation| operation == "clear_marsh"),
            "the engine still offers the operation"
        );
        assert_eq!(
            ai.chop_for_expansion_step(&mut g, 0, builder, GrandStrategy::Expansion),
            None,
            "but Marsh pays Food and a Settler is bought with hammers"
        );
    }

    #[test]
    fn the_gene_off_is_byte_identical() {
        let (mut g, mut ai, city, builder, wood) = fixture();
        build_a_settler(&mut g, city);
        ai.disable_chop_for_expansion();
        assert_eq!(
            ai.chop_for_expansion_step(&mut g, 0, builder, GrandStrategy::Expansion),
            None
        );
        assert_eq!(
            g.map.tiles[&wood].feature,
            Some(crate::name!("forest")),
            "nothing was cleared"
        );
    }
}
