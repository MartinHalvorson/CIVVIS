//! `gold-buys-the-chopping-builder`: the treasury buys a Builder for a city
//! that is building a Settler and has Forests to clear into it.
//!
//! ## Why the Settler errand needs its own Builder
//!
//! [`super::chop_for_expansion`] clears a feature into a Settler, but only
//! with a Builder that already stands within three tiles of the city. The
//! live seat holds one to two Builders across four to six cities before turn
//! 80, so the errand rarely finds one: 30 Deity games measured on 2026-11-09
//! made a median of 2.5 clearings each.
//!
//! The clearing itself is the best trade the opening offers. Measured on the
//! host, a Forest cleared into a Settler under Colonization pays 27 to 30
//! Production before turn 60 on Online speed, so three charges return about
//! 85 Production, a whole Settler, for a Builder that costs about 30. Bought
//! with Gold, the same Builder turns each Gold piece into roughly three times
//! the Production that buying the Settler outright would.
//!
//! A Builder diverted from improving tiles does not capture that: the
//! clearing replaces an improvement. A Builder bought for the purpose adds
//! its charges on top, which is what this gene does.
//!
//! ## What the gene does
//!
//! Once a turn, at peace and with no city threatened, it looks for a city of
//! ours that:
//!
//! - has a Settler at the head of its queue that will not finish this turn;
//! - owns at least [`CHOPPING_BUILDER_MIN_TILES`] tiles within the errand's
//!   radius whose clearing pays Production and clears the errand's own share
//!   floor;
//! - has no Builder with charges within reach and none in its queue;
//! - would get back, from its three best clearings, at least the Production
//!   a Builder costs there.
//!
//! It buys a Builder in the city with the most to clear, from the same budget
//! and above the same upkeep floor as the other Builder purchases. The Settler
//! errand then spends the charges, so this gene does nothing without
//! `chop-for-expansion`.
//!
//! ⚠ Off by default and inert while off:
//! [`AdvancedAi::chopping_builder_purchase`] returns before reading anything.

use super::chop_for_expansion::{CHOP_ERRAND_RADIUS, CHOP_MIN_SHARE, CHOP_OPERATIONS};
use super::*;

/// The purchase stops at this turn of a Standard-speed game. Settlers after
/// it are few, and a Forest is worth more as a Lumber Mill by then.
pub(crate) const CHOPPING_BUILDER_END_STANDARD: u32 = 200;

/// Fewest qualifying clearings a Settler city must hold. A Builder carries
/// three charges, and one clearing does not pay for it.
pub(super) const CHOPPING_BUILDER_MIN_TILES: usize = 2;

/// A Builder's charges, the most clearings one purchase can make.
const CHOPPING_BUILDER_CHARGES: usize = 3;

const CHOPPING_BUILDER_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// The Production the three best clearings of `cid` would pay into the
    /// Settler it is building, or `None` when the city has no Settler under
    /// way or too few tiles to clear.
    pub(super) fn chopping_builder_value(&self, g: &Game, pid: usize, cid: u32) -> Option<f64> {
        let remaining = self.chop_settler_shortfall(g, pid, cid)?;
        let city = &g.cities[&cid];
        let mut payouts: Vec<f64> = city
            .owned_tiles
            .iter()
            .filter(|pos| {
                g.wdist(city.pos, **pos) <= CHOP_ERRAND_RADIUS
                    && !self.conversion_preserves_woods(g, **pos)
            })
            .map(|pos| {
                g.builder_operations(pid, *pos)
                    .into_iter()
                    .filter(|operation| CHOP_OPERATIONS.contains(&operation.as_str()))
                    .map(|operation| self.chop_production_payout(g, pid, *pos, &operation))
                    .fold(0.0_f64, f64::max)
            })
            .filter(|payout| *payout > 0.0 && *payout >= remaining * CHOP_MIN_SHARE)
            .collect();
        if payouts.len() < CHOPPING_BUILDER_MIN_TILES {
            return None;
        }
        payouts.sort_by(|left, right| right.total_cmp(left));
        Some(payouts.into_iter().take(CHOPPING_BUILDER_CHARGES).sum())
    }

    /// Whether a Builder with charges is already in reach of `cid`'s land, or
    /// one is in its queue.
    pub(super) fn chopping_builder_at_hand(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let city = &g.cities[&cid];
        city.queue
            .iter()
            .any(|item| matches!(item, Item::Unit { unit } if *unit == "builder"))
            || g.player_unit_ids(pid).into_iter().any(|uid| {
                let unit = &g.units[&uid];
                unit.kind == "builder"
                    && unit.charges > 0
                    && g.wdist(unit.pos, city.pos) <= 2 * CHOP_ERRAND_RADIUS
            })
    }

    pub(super) fn chopping_builder_purchase_open(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.gold_buys_the_chopping_builder
            || !self.chop_for_expansion
            || self.base.minor
            || self.base.barb
            || plan.threatened_city.is_some()
            || g.turn >= g.standard_duration(CHOPPING_BUILDER_END_STANDARD)
        {
            return false;
        }
        !g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && !other.is_free_city
                && g.is_at_war(pid, other.id)
        })
    }

    /// Buy one Builder for the Settler city with the most to clear. Returns
    /// whether a purchase was made.
    pub(in crate::ai::advanced) fn chopping_builder_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.chopping_builder_purchase_open(g, pid, plan) {
            return false;
        }
        let budget = g.players[pid].gold - self.purchase_upkeep_floor(g, pid, 0.0);
        if budget <= 0.0 {
            return false;
        }
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        // Most Production to clear first, then the lower city id.
        let mut best: Option<(i64, u32, f64, f64)> = None;
        for cid in g.player_city_ids(pid) {
            let Some(value) = self.chopping_builder_value(g, pid, cid) else {
                continue;
            };
            let city = &g.cities[&cid];
            let attacked = city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= CHOPPING_BUILDER_ATTACK_MEMORY;
            let raided = g.units.values().any(|unit| {
                BasicAi::is_barbarian_raider(g, unit)
                    && g.wdist(unit.pos, city.pos) <= crate::ai::HOME_THREAT_RADIUS
            });
            if attacked
                || raided
                || self.chopping_builder_at_hand(g, pid, cid)
                || !g.can_produce(pid, cid, &builder)
                || g.purchase_is_blocked(cid, &builder)
                || value < g.item_cost_for_city(pid, cid, &builder)
            {
                continue;
            }
            let Some(price) = g.unit_purchase_cost(pid, cid, "builder", "gold") else {
                continue;
            };
            if price > budget + 1e-9 {
                continue;
            }
            let key = (-((value * 100.0).round() as i64), cid, price, value);
            if best
                .as_ref()
                .is_none_or(|old| (key.0, key.1) < (old.0, old.1))
            {
                best = Some(key);
            }
        }
        let Some((_, cid, price, value)) = best else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Buy {
                city: cid,
                unit: crate::name!("builder"),
                formation: 0,
                currency: "gold".into(),
            },
        )
        .is_err()
        {
            return false;
        }
        think!(self.journal(), Economy, Decision,
            "{} buys a Builder to clear land into its Settler", g.cities[&cid].name;
            "gold-buys-the-chopping-builder: {:.0} Production to clear for {:.0} Gold of a {:.0} budget",
            value, price, budget);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One major at peace, a capital at population 3 building a Settler with
    /// nothing paid, Mining in hand, `forests` unimproved Forests beside the
    /// city, every unit removed and `gold` in the bank.
    fn settler_city(gold: f64, forests: usize) -> (Game, u32) {
        let mut game = Game::new_full(1, 32, 20, 936_221, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .expect("fixture: a starting Settler");
        game.apply(0, &Action::FoundCity { unit: settler })
            .expect("fixture: the capital is founded");
        for uid in game.player_unit_ids(0) {
            game.remove_unit(uid);
        }
        let capital = game.player_city_ids(0)[0];
        game.turn = 20;
        game.players[0].gold = gold;
        game.players[0].gold_per_turn = 0.0;
        game.players[0].techs.insert(crate::name!("mining"));
        let center = game.cities[&capital].pos;
        let ring: Vec<Pos> = game.cities[&capital]
            .owned_tiles
            .iter()
            .copied()
            .filter(|pos| *pos != center && game.wdist(center, *pos) == 1)
            .collect();
        assert!(ring.len() >= 3, "fixture: the capital owns its first ring");
        for (index, pos) in ring.iter().enumerate() {
            let tile = game.map.tiles.get_mut(pos).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.district_foundation = None;
            tile.wonder = None;
            tile.feature = (index < forests).then(|| crate::name!("forest"));
        }
        let city = game.cities.get_mut(&capital).unwrap();
        city.pop = 3;
        city.queue = vec![Item::Unit {
            unit: crate::name!("settler"),
        }];
        city.production = 0.0;
        (game, capital)
    }

    fn armed() -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.enable_chop_for_expansion();
        ai.enable_gold_buys_the_chopping_builder();
        ai
    }

    fn builders(game: &Game) -> usize {
        game.player_unit_ids(0)
            .into_iter()
            .filter(|uid| game.units[uid].kind == "builder")
            .count()
    }

    fn buys(ai: &AdvancedAi, game: &Game) -> (bool, Game) {
        let mut played = game.clone();
        let plan = ai.assess(&played, 0);
        let bought = ai.chopping_builder_purchase(&mut played, 0, &plan);
        (bought, played)
    }

    #[test]
    fn gold_buys_the_chopping_builder_is_a_native_opt_in_off_in_both_controllers() {
        super::super::test_support::opt_in_off_in_both_controllers(
            "gold-buys-the-chopping-builder",
            |ai| ai.gold_buys_the_chopping_builder,
        );
    }

    #[test]
    fn a_settler_city_with_forests_buys_its_builder() {
        let (game, capital) = settler_city(2_000.0, 3);
        let ai = armed();
        let value = ai
            .chopping_builder_value(&game, 0, capital)
            .expect("fixture: three Forests qualify");
        let builder = Item::Unit {
            unit: crate::name!("builder"),
        };
        assert!(
            value >= game.item_cost_for_city(0, capital, &builder),
            "fixture: three clearings return more than a Builder costs"
        );
        let (bought, played) = buys(&ai, &game);
        assert!(bought);
        assert_eq!(builders(&played), 1, "one Builder bought");
        assert!(played.players[0].gold < game.players[0].gold, "Gold spent");

        // With that Builder at hand, the next turn buys no second one.
        let (again, after) = buys(&ai, &played);
        assert!(!again);
        assert_eq!(builders(&after), 1);
    }

    #[test]
    fn the_gene_off_or_the_errand_off_buys_nothing() {
        let (game, _) = settler_city(2_000.0, 3);
        let stock = AdvancedAi::new();
        let (bought, played) = buys(&stock, &game);
        assert!(!bought);
        assert_eq!(played.players[0].gold, game.players[0].gold);

        let mut no_errand = AdvancedAi::new();
        no_errand.enable_gold_buys_the_chopping_builder();
        assert!(
            !buys(&no_errand, &game).0,
            "nothing would spend the charges"
        );
    }

    #[test]
    fn one_forest_or_no_settler_or_no_gold_buys_nothing() {
        let ai = armed();

        let (one_forest, capital) = settler_city(2_000.0, 1);
        assert_eq!(ai.chopping_builder_value(&one_forest, 0, capital), None);
        assert!(!buys(&ai, &one_forest).0);

        let (mut no_settler, capital) = settler_city(2_000.0, 3);
        no_settler.cities.get_mut(&capital).unwrap().queue = vec![Item::Unit {
            unit: crate::name!("warrior"),
        }];
        assert!(!buys(&ai, &no_settler).0);

        let (poor, _) = settler_city(1.0, 3);
        assert!(!buys(&ai, &poor).0);
    }

    #[test]
    fn a_builder_already_at_hand_buys_nothing() {
        let (mut game, capital) = settler_city(2_000.0, 3);
        let center = game.cities[&capital].pos;
        game.spawn_test_unit("builder", 0, center);
        let ai = armed();
        assert!(ai.chopping_builder_at_hand(&game, 0, capital));
        let (bought, played) = buys(&ai, &game);
        assert!(!bought);
        assert_eq!(builders(&played), 1);
    }
}
