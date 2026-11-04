//! `gold-buys-the-production-tile`: in the opening, idle Gold buys a
//! Production tile next to a city that has a citizen on a tile without one.
//!
//! **Why (live Lv 7 Immortal Standard, Gran Colombia, 11-03/04).**
//! - At Standard speed every build costs twice the turns of Online, while Gold
//!   income per turn does not change, and the treasury sat at 100-190 Gold
//!   through t20-t45 in most games. What that bank can buy at Standard prices
//!   is little: a Warrior is 160, a Builder about 200, a Settler more.
//! - A tile is cheap: 50 Gold in ring two, 75 in ring three, before the speed
//!   scaling. The purchase phase never bought one early: a plot must leave
//!   `reserve + 200 + cost` behind, and with the reserve near one emergency
//!   defender that bar sits above 400 Gold. Twelve recent Standard games
//!   bought no tile before t80, and at most one in the whole game.
//! - Meanwhile 17 of 26 Standard capitals at t30 worked a tile with at most
//!   one Production while an unowned Hill or Forest of two or more lay on the
//!   border, buyable. In `civvis-20261104T061724Z` Bogotá sat at population
//!   five and four Production from t25 to t40 on jungle and floodplain, with
//!   two Plains Hills (2/2) two tiles away.
//!
//! **What it does.** Once a turn in the opening, at peace with every major
//! and with no city threatened, the treasury buys one border tile above the
//! purchase upkeep floor when the tile pays at least
//! [`PRODUCTION_TILE_MIN_PRODUCTION`] Production and one Food, and the owning
//! city's citizen plan works a tile at least one Production poorer that the
//! new tile would replace. Highest Production first, then Food, then the
//! cheaper tile. Called right after `gold-buys-the-opening-builder`, so the
//! opening Builder keeps its claim on the same Gold. Off, nothing runs.

use super::*;

/// The window, in standard turns (live turn 100 at Online, 150 at Standard).
pub(crate) const PRODUCTION_TILE_END_STANDARD: u32 = 150;

/// The least Production a bought tile pays.
pub(crate) const PRODUCTION_TILE_MIN_PRODUCTION: f64 = 2.0;

/// A city attacked within this many turns is not where the tile is bought.
const PRODUCTION_TILE_ATTACK_MEMORY: u32 = 4;

/// One candidate: (production, food, cheaper-first price, city, tile).
type Candidate = (i64, i64, i64, u32, Pos);

impl AdvancedAi {
    /// See the module. True when a tile was bought. Exact no-op with the gene
    /// off.
    pub(in crate::ai::advanced) fn production_tile_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some((cost, city, pos)) = self.production_tile_choice(g, pid, plan) else {
            return false;
        };
        let before = g.players[pid].gold;
        if g.apply(pid, &Action::BuyPlot { city, pos, cost }).is_err() {
            return false;
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let yields = g.modeled_tile_yields(pos);
            think!(self.journal(), Economy, Decision,
                "{} buys the tile at {:?}", g.cities[&city].name, pos;
                "gold-buys-the-production-tile: {:.0} Gold for a tile of {:.0} Production and {:.0} Food beside a citizen working less; {:.0} Gold left",
                before - g.players[pid].gold, yields.production, yields.food, g.players[pid].gold);
        }
        true
    }

    /// The tile the gene would buy this turn, and its price: `None` with the
    /// gene off, outside the window, at war with a major, with a city
    /// threatened, or when no tile qualifies within the budget.
    pub(in crate::ai::advanced) fn production_tile_choice(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(f64, u32, Pos)> {
        if !self.gold_buys_the_production_tile
            || self.base.minor
            || self.base.barb
            || plan.threatened_city.is_some()
            || g.turn >= g.standard_duration(PRODUCTION_TILE_END_STANDARD)
        {
            return None;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && !other.is_free_city
                && g.is_at_war(pid, other.id)
        });
        if at_major_war {
            return None;
        }
        let budget = g.players[pid].gold - self.purchase_upkeep_floor(g, pid, 0.0);
        if budget <= 0.0 {
            return None;
        }
        let mut best: Option<(Candidate, f64)> = None;
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            if city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= PRODUCTION_TILE_ATTACK_MEMORY
            {
                continue;
            }
            // The poorest Production among the tiles the city works now.
            let Some(poorest) = g
                .city_citizen_plan(cid)
                .worked_tiles
                .iter()
                .map(|pos| g.modeled_tile_yields(*pos).production)
                .min_by(f64::total_cmp)
            else {
                continue;
            };
            for pos in g.wdisk(city.pos, 3) {
                let Some(cost) = g.plot_purchase_cost(pid, cid, pos) else {
                    continue;
                };
                if cost > budget + 1e-9 {
                    continue;
                }
                let yields = g.modeled_tile_yields(pos);
                if yields.production < PRODUCTION_TILE_MIN_PRODUCTION
                    || yields.food < 1.0
                    || yields.production < poorest + 1.0
                {
                    continue;
                }
                let key: Candidate = (
                    (yields.production * 100.0).round() as i64,
                    (yields.food * 100.0).round() as i64,
                    -((cost * 100.0).round() as i64),
                    u32::MAX - cid,
                    pos,
                );
                if best.as_ref().is_none_or(|(old, _)| key > *old) {
                    best = Some((key, cost));
                }
            }
        }
        best.map(|((_, _, _, inverse_city, pos), cost)| (cost, u32::MAX - inverse_city, pos))
    }
}

#[cfg(test)]
mod tests;
