//! `watched-capital-keeps-its-defender-gold`: while the capital watch of
//! `capital-walls-before-the-surprise` reads (a met major at
//! [`capital_walls_before_the_surprise::CAPITAL_WALLS_RATIO`] times our power,
//! one of its soldiers seen within
//! [`capital_walls_before_the_surprise::CAPITAL_WATCH_TILES`] of our original
//! capital in the watch window), the capital stands unwalled and fewer than
//! [`capital_walls_before_the_surprise::CAPITAL_GARRISON`] of our soldiers
//! stand beside it, the peacetime Gold buyers keep the price of the missing
//! defenders in the bank. The production tile (`gold-buys-the-production-tile`),
//! the idle route's Trader (`gold-buys-the-idle-trader`) and the backlog
//! Builder (`gold-buys-the-backlog-builder`) spend only what lies above the
//! upkeep floor plus that reserve.
//!
//! **Why (Tournament 7 / 8 review, 2026-11-25/26, 9 of 40 Deity games).**
//! The capital or the first cities fell to an early war at t28-t75. In the
//! games' own logs the capital watch read a 2.6-5.3x neighbour at the door 3
//! to 10 turns before the declaration, and its door leg
//! (`capital_garrison_purchase`) is armed to buy a defender then. It bought
//! one in 2 of 8 games: the treasury was empty. `gold-buys-the-production-tile`
//! had spent it on border tiles after the watch first read, 40-275 Gold a
//! game (`civvis-t8-20261126T071255Z`: China at 4.7x from t21; tiles at t12,
//! 16, 17, 18 and 22 for 170 Gold; war t24; Constantinople fell t29 with 0-5
//! Gold in the bank and no unit bought; `T085118Z`: Hungary at 3.2x from t14,
//! tiles at t19 and t24, war t26, no unit bought). Where Gold was there, the
//! door and emergency lanes did buy (`T073755Z` Slingers at t27 and t31).
//!
//! **What it does.** [`AdvancedAi::watched_capital_defender_reserve`] is the
//! cheapest land defender the capital can buy for Gold, once per missing
//! garrison soldier, while the watch reads; zero otherwise. The three buyers
//! subtract it from their budget. It holds Gold, nothing more: no queue, no
//! research, no unit moves, and the city-saving lanes (which read the plain
//! upkeep floor) can spend all of it. The reserve lapses with the watch, when
//! the Walls stand, or when the garrison is full. Off, it is zero.

use super::*;

/// Gold per Production of a unit's purchase price, the model's base rate,
/// when the host quotes none because a soldier holds the City Center.
const DEFENDER_GOLD_PER_PRODUCTION: f64 = 4.0;

impl AdvancedAi {
    /// The Gold the peacetime buyers leave for the capital's missing
    /// defenders: the cheapest land defender's price per missing garrison
    /// soldier while the capital watch reads at an unwalled original capital.
    /// Zero with the gene off.
    pub(in crate::ai::advanced) fn watched_capital_defender_reserve(
        &self,
        g: &Game,
        pid: usize,
    ) -> f64 {
        self.watched_capital_reserve_detail(g, pid)
            .map_or(0.0, |(_, _, _, reserve)| reserve)
    }

    /// (rival, ratio, missing soldiers, reserve) while the reserve holds.
    fn watched_capital_reserve_detail(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<(usize, f64, usize, f64)> {
        if !self.watched_capital_keeps_its_defender_gold || self.base.minor || self.base.barb {
            return None;
        }
        let capital = Self::capital_watch_city(g, pid)?;
        if g.city_max_wall_hp(&g.cities[&capital]) > 0 {
            return None;
        }
        let (rival, ratio) = self.capital_watched_major_power(g, pid)?;
        let missing = capital_walls_before_the_surprise::CAPITAL_GARRISON
            .saturating_sub(Self::capital_garrison(g, pid, capital));
        if missing == 0 {
            return None;
        }
        // A soldier of ours on the City Center takes the purchase slot, and
        // the host then quotes no land soldier; the model's own base price
        // (four Gold per Production) stands in until it steps out.
        let cheapest = g
            .rules
            .units
            .keys()
            .filter(|name| Self::capital_defender_kind(g, pid, capital, **name))
            .map(|name| {
                g.unit_purchase_cost(pid, capital, name.as_str(), "gold")
                    .unwrap_or_else(|| {
                        DEFENDER_GOLD_PER_PRODUCTION
                            * g.item_cost_for(pid, &Item::Unit { unit: *name })
                    })
            })
            .filter(|price| price.is_finite() && *price > 0.0)
            .min_by(f64::total_cmp)?;
        Some((rival, ratio, missing, cheapest * missing as f64))
    }

    /// Journal the reserve once a purchase phase. No-op with the gene off or
    /// the reserve at zero.
    pub(in crate::ai::advanced) fn note_watched_capital_defender_reserve(
        &self,
        g: &Game,
        pid: usize,
    ) {
        let Some((rival, ratio, missing, reserve)) = self.watched_capital_reserve_detail(g, pid)
        else {
            return;
        };
        think!(self.journal(), Economy, Detail,
            "The treasury keeps the capital's defender Gold";
            "watched-capital-keeps-its-defender-gold: {} fields {ratio:.1}x our power near the \
             unwalled capital and {missing} defender(s) are missing; {reserve:.0} of {:.0} Gold \
             stays for them, the tile, Trader and Builder buys spend the rest",
            g.players[rival].civ, g.players[pid].gold);
    }
}

#[cfg(test)]
mod tests;
