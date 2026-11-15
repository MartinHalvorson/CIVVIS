//! `one-upgrade-lifts-every-city`: the first unit into a new tier upgrades
//! ahead of the treasury floor, because in Civilization VI that one upgrade
//! lifts the defence or the ranged strike of every city the seat owns.
//!
//! ## Why
//!
//! A city's ranged strike is the best ranged strength its owner has ever
//! owned, and its defence is built from the strongest unit it has ever owned
//! less 10 (`Game::city_ranged_strength` and `Game::city_strength`, through
//! the `strongest_ranged_built` and `strongest_unit_built` counters, which an
//! upgrade raises exactly as a new unit does). The live logs of 2026-11-12/13
//! (115 games) agree: a city's strike equals the best ranged strength the seat
//! ever owned in 93% of 12,374 state rows, upgrades included. So the first
//! unit into a tier lifts every city at once; the second adds nothing to them.
//!
//! The ordinary upgrade pass (`BasicAi::modernize_army_with_filter`) ranks by
//! strength gained per Gold and never spends below 120 Gold at peace (30 at
//! war), raised further by `upgrade_treasury_floor`. The live treasury's
//! median is 79 / 111 / 144 Gold at turns 50 / 75 / 100, so that one upgrade
//! waits for a war. Live, from turn 30, the best melee strength the seat ever
//! owned sits 10 or more below what it has unlocked with the material in
//! stock on 38.6% of turns (stretches of 21 turns, median), and the best
//! ranged on 19.5%; when a war is declared on us, city defence is 10 or more
//! short in 42% of those wars and strike in 25%.
//!
//! ## What the gene does
//!
//! Immediately before the ordinary pass in `upgrade_units_preserving_air_wing`
//! (never for the barbarians or a seat without a city):
//!
//! 1. The basis the cities read: the best melee strength is the larger of the
//!    `strongest_unit_built` counter and the strongest military unit owned
//!    now; the best ranged strength the larger of `strongest_ranged_built`,
//!    the best ranged unit owned now and, where an own city carries an
//!    observed strike, that strike less the cards' `city_ranged`. The maximum
//!    guards a resumed live mirror whose counters start empty.
//! 2. Every Gold upgrade the board offers that keeps the air wing's material
//!    is priced by its lift: what the successor adds above the melee basis
//!    plus what it adds above the ranged basis. It qualifies when either half
//!    is at least [`CITY_LIFT_MIN`].
//! 3. The treasury after the bill stays at or above zero and the seat's Gold
//!    a turn is not negative. Nothing else holds it back: this pass runs
//!    ahead of the 120-Gold floor and the upkeep reserve by design.
//! 4. The best lift per Gold goes first (the cheaper, then the lower unit id,
//!    on a tie). The basis is read again and the pass repeats until nothing
//!    qualifies, which is at once after the first unit of a tier.
//!
//! Off, the pass returns before it reads anything.

use super::*;

/// Strength one upgrade must add to every city's defence or ranged strike
/// before it goes ahead of the treasury floor.
pub(crate) const CITY_LIFT_MIN: f64 = 5.0;

/// What one upgrade adds to every city of its owner: above the best melee
/// strength the seat has owned (defence) and above its best ranged strength
/// (strike).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct CityLift {
    pub(super) defence: f64,
    pub(super) strike: f64,
}

impl CityLift {
    fn qualifies(self) -> bool {
        self.defence >= CITY_LIFT_MIN || self.strike >= CITY_LIFT_MIN
    }

    fn total(self) -> f64 {
        self.defence + self.strike
    }
}

impl AdvancedAi {
    /// The best melee and the best ranged strength `pid` has owned, as its
    /// cities read them: the engine's counters, the military units it owns
    /// now, and (ranged) any own city's observed strike less the cards'
    /// `city_ranged`.
    pub(super) fn city_lift_basis(g: &Game, pid: usize) -> (f64, f64) {
        let counter = |name: &str| {
            g.players[pid]
                .counters
                .get(name)
                .map_or(0.0, |best| *best as f64)
        };
        let mut melee = counter("strongest_unit_built");
        let mut ranged = counter("strongest_ranged_built");
        for uid in g.player_unit_ids(pid) {
            let spec = &g.rules.units[g.units[&uid].kind];
            if spec.class == "military" {
                melee = melee.max(spec.strength);
                ranged = ranged.max(spec.ranged_strength);
            }
        }
        let card = g.policy_effect(pid, "city_ranged");
        for cid in g.player_city_ids(pid) {
            if let Some(strike) = g.observed_city_ranged_strength.get(&cid) {
                ranged = ranged.max(strike - card);
            }
        }
        (melee, ranged)
    }

    /// `one-upgrade-lifts-every-city`: take every Gold upgrade that lifts
    /// each city's defence or ranged strike by [`CITY_LIFT_MIN`] or more,
    /// down to an empty treasury, while the seat's income is not negative.
    /// Returns the upgrades taken; 0 with the gene off.
    pub(super) fn upgrades_that_lift_every_city(&self, g: &mut Game, pid: usize) -> usize {
        if !self.one_upgrade_lifts_every_city
            || g.players[pid].is_barbarian
            || g.player_city_ids(pid).is_empty()
        {
            return 0;
        }
        let income = g.players[pid].gold_per_turn;
        if !income.is_finite() || income < 0.0 {
            return 0;
        }
        let mut taken = 0;
        loop {
            let (melee, ranged) = Self::city_lift_basis(g, pid);
            let treasury = g.players[pid].gold;
            let mut best: Option<(f64, f64, u32, Name, CityLift)> = None;
            for uid in g.player_unit_ids(pid) {
                let Some((target, gold, resources)) = g.unit_gold_upgrade_offer(pid, uid) else {
                    continue;
                };
                if gold > treasury {
                    continue;
                }
                let to = &g.rules.units[target];
                if to.class != "military" {
                    continue;
                }
                let lift = CityLift {
                    defence: (to.strength - melee).max(0.0),
                    strike: (to.ranged_strength - ranged).max(0.0),
                };
                if !lift.qualifies()
                    || !self.air_resource_upgrade_preserves_wing(g, pid, uid, target, resources)
                {
                    continue;
                }
                let value = lift.total() / gold.max(1.0);
                let better = match &best {
                    None => true,
                    Some((top, top_gold, top_uid, _, _)) => {
                        value > *top + 1e-9
                            || ((value - *top).abs() <= 1e-9
                                && (gold < *top_gold - 1e-9
                                    || ((gold - *top_gold).abs() <= 1e-9 && uid < *top_uid)))
                    }
                };
                if better {
                    best = Some((value, gold, uid, target, lift));
                }
            }
            let Some((_, gold, uid, target, lift)) = best else {
                break;
            };
            let from = g.units[&uid].kind;
            if g.apply(pid, &Action::UpgradeUnit { unit: uid }).is_err() {
                break;
            }
            taken += 1;
            think!(self.journal(), Military, Decision,
                   "One upgrade lifts every city";
                   "one-upgrade-lifts-every-city: {} to {} for {gold:.0} Gold of {treasury:.0} \
                    ({income:+.0} a turn); every city's defence +{:.0}, ranged strike +{:.0}",
                   plain(from.as_str()), plain(target.as_str()), lift.defence, lift.strike);
        }
        taken
    }
}

#[cfg(test)]
mod tests;
