//! Gold and the production cards: four opt-in genes that decide WHICH of the
//! two currencies pays for an item. From the operator's heuristic
//! (2026-08-25):
//!
//! > production is most efficiently spent in combination with boosted
//! > production policy cards. money is flexible — a new city will always
//! > have low production but has access to the same money as anywhere else.
//! > somewhat optimal to spend money on things that can't be boosted by
//! > production policy cards and to spend money on new cities (that take a
//! > while to scale production) and on emergencies. money is also immediate
//! > — whereas production takes turns to build.
//!
//! ## What the controller did before
//!
//! The engine knows exactly what a slotted card is worth to a build:
//! [`Game::item_prod_mult`] returns the +% Production a city's queue head
//! earns from Agoge, Colonization, Ilkum, Maritime Industries, Maneuver,
//! Limes, Feudal Contract and the rest, and `process_city` applies it every
//! turn. **No controller ever read it** — `grep item_prod_mult src/ai` is
//! empty before this module. Both places that price an item against a city's
//! Production, `gold_purchase_score` and `production_value`, divide the
//! remaining cost by the RAW city yield. A Settler under Colonization
//! therefore looks half again slower to build than it is, which makes it a
//! BETTER Gold purchase in the scorer's eyes: the shipped purchaser is biased
//! toward buying exactly the items a card is already discounting, and the
//! governor is indifferent to the discount.
//!
//! The emergency purchase, `emergency_city_defense_purchase`, is gated on
//! `garrison_under_fire`, a live-bridge-only flag no native controller sets
//! (`BasicAi::besieged_city_item` says so in its own comment), so on a native
//! board a city can bleed to nothing beside a full treasury while the
//! purchaser runs its ordinary reserve arithmetic.
//!
//! ## The four genes
//!
//! 1. **`buy-what-cards-cannot-boost`** — the Gold purchase scorer prices the
//!    turns a build would take at the card-boosted rate, and scales the Gold
//!    price by the same multiplier: a Gold spent on a boosted item replaces
//!    less Production than the same Gold spent on an item no card touches.
//!    Boosted items lose purchase priority to unboosted ones; an item at
//!    multiplier 1 scores exactly as it did.
//! 2. **`build-what-cards-boost`** — the city production governor scales a
//!    positive item value by [`BUILD_BOOST_SHARE`] of the card's bonus (a +50%
//!    card is +25% value), capped at [`BUILD_BOOST_CAP`], so the queue leans
//!    toward what the slotted deck makes cheap. It only reorders: a value at
//!    or below zero is never raised.
//! 3. **`gold-for-the-young-city`** — a Gold purchase in a city producing
//!    less than the empire's best city earns a premium proportional to the
//!    deficit, [`YOUNG_CITY_PREMIUM`] at zero output. The same money buys the
//!    same item anywhere, and the city that cannot yet build is where it buys
//!    the most turns.
//! 4. **`native-emergency-purchase`** — the emergency defence purchase fires
//!    on a native signal: a city that has lost health, was struck within the
//!    last [`EMERGENCY_RECENT_TURNS`] turns, and has a hostile military unit
//!    within [`EMERGENCY_RADIUS`] tiles. It buys Walls if the city can raise
//!    them, otherwise the best land defender — the live doctrine's own choice
//!    — and it spends through the reserve exactly as the live path does.
//!
//! ⚠ The trigger asks for DAMAGE, not for a hostile in sight.
//! `besieged_city_item` records why: reacting to a single raider in range
//! "bought city count while COSTING score: walls and defenders displace the
//! buildings and districts score is actually made of".
//!
//! Each gene is byte-identical when off: every multiplier reads exactly 1.0
//! (and `x * 1.0`, `x / (p * 1.0)` are exact in IEEE arithmetic), and the
//! emergency branch is not consulted.

use super::{AdvancedAi, CITY_MAX_HP};
use crate::game::{Game, Item};
use crate::name::Name;

/// Share of a card's Production bonus the governor turns into item value: a
/// +50% card raises a positive value by 25%.
pub const BUILD_BOOST_SHARE: f64 = 0.5;
/// The most card bonus the governor prices. Limes and a Maritime Industries
/// Galley reach +100%; nothing is priced past a doubling.
pub const BUILD_BOOST_CAP: f64 = 1.0;
/// The card multiplier is clamped into this range when pricing a purchase, so
/// an item a Congress treaty forbids building (multiplier 0) reads as a slow
/// build rather than an infinite one.
pub const PURCHASE_CARD_MULT_RANGE: (f64, f64) = (0.25, 4.0);
/// Purchase premium for a city producing nothing; a city at half the best
/// city's output earns half of it.
pub const YOUNG_CITY_PREMIUM: f64 = 0.5;
/// How recently a city must have been struck for the native emergency.
pub const EMERGENCY_RECENT_TURNS: u32 = 4;
/// How near a hostile military unit must stand for the native emergency.
pub const EMERGENCY_RADIUS: i32 = 3;

/// The premium a purchase earns in a city producing `here` when the empire's
/// best city produces `best`. Exactly 1.0 at or above the best city.
pub fn young_city_premium_from(here: f64, best: f64) -> f64 {
    if best <= 0.0 {
        return 1.0;
    }
    let deficit = (1.0 - here / best).clamp(0.0, 1.0);
    1.0 + YOUNG_CITY_PREMIUM * deficit
}

impl AdvancedAi {
    /// `buy-what-cards-cannot-boost`: the card multiplier the purchase scorer
    /// prices this item's build at. Exactly 1.0 while the gene is off.
    pub(super) fn purchase_card_multiplier(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> f64 {
        if !self.buy_what_cards_cannot_boost {
            return 1.0;
        }
        let (floor, ceiling) = PURCHASE_CARD_MULT_RANGE;
        g.item_prod_mult(pid, cid, Some(item)).clamp(floor, ceiling)
    }

    /// `build-what-cards-boost`: a positive governor value leans toward what
    /// the slotted deck makes cheap. Unchanged while the gene is off, and for
    /// any value at or below zero.
    pub(super) fn card_boosted_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        value: f64,
    ) -> f64 {
        if !self.build_what_cards_boost || value <= 0.0 {
            return value;
        }
        let bonus = (g.item_prod_mult(pid, cid, Some(item)) - 1.0).clamp(0.0, BUILD_BOOST_CAP);
        if bonus <= 0.0 {
            return value;
        }
        value * (1.0 + BUILD_BOOST_SHARE * bonus)
    }

    /// `gold-for-the-young-city`: the purchase premium for buying in this
    /// city rather than in the empire's best producer. Exactly 1.0 while the
    /// gene is off.
    pub(super) fn young_city_premium(&self, g: &Game, pid: usize, cid: u32) -> f64 {
        if !self.gold_for_the_young_city {
            return 1.0;
        }
        let here = g.city_yields(cid).production.max(0.0);
        let best = g
            .player_city_ids(pid)
            .into_iter()
            .map(|city| g.city_yields(city).production)
            .fold(0.0_f64, f64::max);
        young_city_premium_from(here, best)
    }

    /// `native-emergency-purchase`: whether this city is bleeding under an
    /// attack a native controller can see. False while the gene is off.
    pub(super) fn native_city_emergency(&self, g: &Game, pid: usize, cid: u32) -> bool {
        if !self.native_emergency_purchase {
            return false;
        }
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return false;
        };
        if city.hp >= CITY_MAX_HP {
            return false;
        }
        if city.last_attacked == 0
            || g.turn.saturating_sub(city.last_attacked) > EMERGENCY_RECENT_TURNS
        {
            return false;
        }
        g.units.values().any(|unit| {
            unit.owner != pid
                && g.rules.units[unit.kind].class == "military"
                && (g.players[unit.owner].is_barbarian || g.is_at_war(pid, unit.owner))
                && g.wdist(unit.pos, city.pos) <= EMERGENCY_RADIUS
        })
    }

    /// The emergency's answer for a bleeding city: Walls if the city can raise
    /// them, otherwise the best land defender. This is
    /// `BasicAi::besieged_city_item`'s own choice without its live gate.
    pub(super) fn native_emergency_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if !self.native_city_emergency(g, pid, cid) {
            return None;
        }
        for building in ["walls", "medieval_walls", "renaissance_walls"] {
            let wall = Item::Building {
                building: Name::new(building),
            };
            if g.can_produce(pid, cid, &wall) {
                return Some(wall);
            }
        }
        self.base
            .best_military(g, pid, cid, Some(false))
            .map(|unit| Item::Unit {
                unit: Name::new(&unit),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Action, Game};

    fn board() -> (Game, u32) {
        let mut g = Game::new(2, 24, 16, 71, 250, 0);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.current = 0;
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = g.player_city_ids(0)[0];
        (g, city)
    }

    fn settler() -> Item {
        Item::Unit {
            unit: crate::name!("settler"),
        }
    }

    fn monument() -> Item {
        Item::Building {
            building: crate::name!("monument"),
        }
    }

    #[test]
    fn off_by_default_and_toggles() {
        let ai = AdvancedAi::new();
        assert!(!ai.buy_what_cards_cannot_boost, "an opt-in ships off");
        assert!(!ai.build_what_cards_boost, "an opt-in ships off");
        assert!(!ai.gold_for_the_young_city, "an opt-in ships off");
        assert!(!ai.native_emergency_purchase, "an opt-in ships off");
        let legacy = AdvancedAi::legacy();
        assert!(!legacy.buy_what_cards_cannot_boost);
        assert!(!legacy.build_what_cards_boost);
        assert!(!legacy.gold_for_the_young_city);
        assert!(!legacy.native_emergency_purchase);

        let mut ai = AdvancedAi::new();
        ai.enable_buy_what_cards_cannot_boost();
        ai.enable_build_what_cards_boost();
        ai.enable_gold_for_the_young_city();
        ai.enable_native_emergency_purchase();
        assert!(ai.buy_what_cards_cannot_boost);
        assert!(ai.build_what_cards_boost);
        assert!(ai.gold_for_the_young_city);
        assert!(ai.native_emergency_purchase);
        ai.disable_buy_what_cards_cannot_boost();
        ai.disable_build_what_cards_boost();
        ai.disable_gold_for_the_young_city();
        ai.disable_native_emergency_purchase();
        assert!(!ai.buy_what_cards_cannot_boost);
        assert!(!ai.build_what_cards_boost);
        assert!(!ai.gold_for_the_young_city);
        assert!(!ai.native_emergency_purchase);
    }

    #[test]
    fn the_engine_prices_colonization_and_the_controller_reads_it() {
        let (mut g, city) = board();
        assert_eq!(g.item_prod_mult(0, city, Some(&settler())), 1.0);
        g.players[0].policies.insert(crate::name!("colonization"));
        assert!(
            (g.item_prod_mult(0, city, Some(&settler())) - 1.5).abs() < 1e-9,
            "Colonization is +50% toward Settlers"
        );
        assert_eq!(g.item_prod_mult(0, city, Some(&monument())), 1.0);

        let off = AdvancedAi::new();
        assert_eq!(off.purchase_card_multiplier(&g, 0, city, &settler()), 1.0);
        let mut on = AdvancedAi::new();
        on.enable_buy_what_cards_cannot_boost();
        assert!((on.purchase_card_multiplier(&g, 0, city, &settler()) - 1.5).abs() < 1e-9);
        assert_eq!(on.purchase_card_multiplier(&g, 0, city, &monument()), 1.0);
    }

    #[test]
    fn the_governor_leans_toward_the_boosted_item_and_only_upward() {
        let (mut g, city) = board();
        g.players[0].policies.insert(crate::name!("colonization"));
        let off = AdvancedAi::new();
        assert_eq!(off.card_boosted_value(&g, 0, city, &settler(), 100.0), 100.0);
        let mut on = AdvancedAi::new();
        on.enable_build_what_cards_boost();
        assert!(
            (on.card_boosted_value(&g, 0, city, &settler(), 100.0) - 125.0).abs() < 1e-9,
            "half of a +50% card"
        );
        assert_eq!(on.card_boosted_value(&g, 0, city, &monument(), 100.0), 100.0);
        assert_eq!(on.card_boosted_value(&g, 0, city, &settler(), -100.0), -100.0);
        assert_eq!(on.card_boosted_value(&g, 0, city, &settler(), 0.0), 0.0);
    }

    #[test]
    fn the_young_city_premium_scales_with_the_deficit() {
        assert_eq!(young_city_premium_from(10.0, 10.0), 1.0);
        assert_eq!(young_city_premium_from(12.0, 10.0), 1.0, "never below one");
        assert!((young_city_premium_from(5.0, 10.0) - 1.25).abs() < 1e-9);
        assert!((young_city_premium_from(0.0, 10.0) - 1.5).abs() < 1e-9);
        assert_eq!(young_city_premium_from(0.0, 0.0), 1.0, "no producer, no premium");
        let (g, city) = board();
        let off = AdvancedAi::new();
        assert_eq!(off.young_city_premium(&g, 0, city), 1.0);
        let mut on = AdvancedAi::new();
        on.enable_gold_for_the_young_city();
        assert_eq!(on.young_city_premium(&g, 0, city), 1.0, "the only city is the best city");
    }

    #[test]
    fn the_native_emergency_needs_damage_a_recent_strike_and_a_hostile() {
        let (mut g, city) = board();
        let home = g.cities[&city].pos;
        let raider_at = g
            .wdisk(home, 2)
            .into_iter()
            .find(|position| {
                *position != home
                    && g.map.get(*position).is_some_and(|tile| {
                        g.rules.is_passable(tile) && !g.rules.is_water(tile)
                    })
                    && g.units_at(*position).is_empty()
            })
            .expect("open ground beside the capital");
        let barbarians = g
            .players
            .iter()
            .position(|player| player.is_barbarian)
            .expect("a barbarian seat");
        g.spawn_test_unit("warrior", barbarians, raider_at);
        g.turn = 30;

        let mut on = AdvancedAi::new();
        on.enable_native_emergency_purchase();
        assert!(!on.native_city_emergency(&g, 0, city), "an unhurt city is not an emergency");

        let state = g.cities.get_mut(&city).unwrap();
        state.hp = 120;
        state.last_attacked = 29;
        assert!(on.native_city_emergency(&g, 0, city));
        let item = on
            .native_emergency_item(&g, 0, city)
            .expect("a bleeding city under a raider has an answer");
        assert!(
            matches!(item, Item::Building { .. } | Item::Unit { .. }),
            "walls or a land defender: {item:?}"
        );

        let off = AdvancedAi::new();
        assert!(!off.native_city_emergency(&g, 0, city), "off is off");
        assert!(off.native_emergency_item(&g, 0, city).is_none());

        g.cities.get_mut(&city).unwrap().last_attacked = 20;
        assert!(!on.native_city_emergency(&g, 0, city), "the strike is stale");
        g.cities.get_mut(&city).unwrap().last_attacked = 29;
        for unit in g.player_unit_ids(barbarians) {
            g.remove_unit(unit);
        }
        assert!(!on.native_city_emergency(&g, 0, city), "no hostile near, no emergency");
    }
}
