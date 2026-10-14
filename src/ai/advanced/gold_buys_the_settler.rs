//! `gold-buys-the-settler`: in the expansion window, at peace and short of
//! [`SETTLER_FUND_CITIES`] cities and walkers, the treasury saves for a
//! Settler and buys it the turn it can, instead of idling as a reserve or
//! leaking into Warriors and Scouts.
//!
//! **Why (10-09 eco4 census, 144 live Emperor GC runs since 10-07).** The
//! city count at turn 60 decides the operator's production gate: of the 132
//! gated runs, 4 or fewer cities at t60 passed 4 of 51 (8%), five passed 11
//! of 49 (22%), six passed 14 of 28 (50%). Our empire matches the rivals'
//! city count all opening (median 3 vs 3 at t30, 4 vs 4-5 at t50) while their
//! cities are a third bigger, so the cities we found are the lever we hold.
//! Meanwhile the treasury sits at a median 121 Gold at t20, 145-169 at t30,
//! 158-170 at t40-t60 on +7 to +12 a turn: `treasury-at-work-2`'s working
//! reserve is one emergency defender (80-120 Gold), and what clears it goes
//! to the argmax's Warriors (160 buys before t60 across the 144 runs) and
//! Scouts (80). One Settler was bought with Gold in 144 runs. A Settler
//! costs four times its Production (160 Gold for the first at Online speed,
//! 220-280 for the next), so the bank is always one Settler short and never
//! catches up.
//!
//! **What it does.** While the window is open — the gene is on, before
//! [`SETTLER_FUND_END_STANDARD`], at peace with every major, no city of ours
//! threatened or attacked within [`SETTLER_FUND_ATTACK_MEMORY`] turns, at
//! most [`SETTLER_FUND_MAX_WALKERS`] Settler already walking, and founded
//! cities plus walkers short of both [`SETTLER_FUND_CITIES`] and the plan's
//! own `desired_cities` (which already caps itself at the practical sites the
//! map holds) — the fund looks at every city of ours with the population to
//! give and no raider in its home ring, the biggest first:
//!
//! 1. **Buy.** If one can buy a Settler now, out of the bank above the
//!    threatened-city floor (zero at peace), it does, and the turn's other
//!    discretionary spending stops.
//! 2. **Save.** Otherwise, if the cheapest Settler is within
//!    [`SETTLER_FUND_SAVE_STANDARD`] standard turns of income (twenty live
//!    turns), the reserve
//!    every discretionary purchase must leave behind rises to that price, so
//!    the Gold accumulates instead of leaking. Emergency, border and siege
//!    buys run before the fund and are untouched.
//!
//! The host prices a Settler only when the treasury can pay for it (its
//! purchase menu is affordability-filtered), so while saving the price is
//! the model's four Gold per Production on the city's own Settler cost.
//!
//! ⚠ Off by default; with the flag clear [`AdvancedAi::settler_fund`]
//! returns [`SettlerFund::Off`] before reading anything and
//! `advanced_gold_spending` plays byte-identically.

use super::*;

/// Founded cities plus walking Settlers the fund fills to. Six cities at t60
/// passed the gate 50% of the time against 22% at five; the seventh is the
/// t75 band the early census named (57%).
pub(crate) const SETTLER_FUND_CITIES: usize = 7;

/// The window, in standard turns: 105 standard turns are live turn 70 at the
/// seat's Online speed, late enough for the last bought Settler to found by
/// the t75 band.
pub(crate) const SETTLER_FUND_END_STANDARD: u32 = 105;

/// The fund saves only while the cheapest Settler is this many standard
/// turns of income away (twenty live turns at Online speed); a Settler
/// further off leaves the treasury to its ordinary work. The second to
/// fourth Settlers cost 220-340 Gold against a bank the census holds at
/// 150-230 on +7 to +12 a turn, so a shorter horizon never starts saving.
pub(crate) const SETTLER_FUND_SAVE_STANDARD: u32 = 30;

/// More Settlers walking than this and the fund waits for them to found.
pub(crate) const SETTLER_FUND_MAX_WALKERS: usize = 1;

/// A city attacked within this many turns does not spend its population.
const SETTLER_FUND_ATTACK_MEMORY: u32 = 4;

/// The shipped purchase rate on a unit's Production cost, for the price the
/// host will not quote until the treasury can pay it.
const SETTLER_FUND_GOLD_PER_PRODUCTION: f64 = 4.0;

/// What the fund did this turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum SettlerFund {
    /// The window is shut, or no Settler is within reach of the income.
    Off,
    /// Saving: discretionary purchases must leave at least this much behind.
    Saving { reserve: f64 },
    /// A Settler was bought this turn.
    Bought,
}

impl AdvancedAi {
    fn settler_fund_item() -> Item {
        Item::Unit {
            unit: crate::name!("settler"),
        }
    }

    fn settler_fund_walkers(g: &Game, pid: usize) -> usize {
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.units[uid].kind == "settler")
            .count()
    }

    /// Whether the window is open for `pid` this turn. See the module.
    pub(super) fn settler_fund_open(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> bool {
        if !self.gold_buys_the_settler
            || g.turn >= g.standard_duration(SETTLER_FUND_END_STANDARD)
            || plan.threatened_city.is_some()
        {
            return false;
        }
        let at_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && !other.is_free_city
                && g.is_at_war(pid, other.id)
        });
        if at_war {
            return false;
        }
        let cities = g.player_city_ids(pid);
        if cities.is_empty()
            || cities.iter().any(|cid| {
                let attacked = g.cities[cid].last_attacked;
                attacked > 0 && g.turn.saturating_sub(attacked) <= SETTLER_FUND_ATTACK_MEMORY
            })
        {
            return false;
        }
        let walkers = Self::settler_fund_walkers(g, pid);
        let target = SETTLER_FUND_CITIES.min(plan.desired_cities);
        walkers <= SETTLER_FUND_MAX_WALKERS && cities.len() + walkers < target
    }

    /// Cities that may give a Settler: the host's population floor, no
    /// barbarian raider in the home ring, the biggest first.
    fn settler_fund_cities(g: &Game, pid: usize) -> Vec<u32> {
        let mut cities: Vec<(i32, u32)> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                let city = &g.cities[cid];
                city.pop >= 2
                    && !g.units.values().any(|unit| {
                        crate::ai::BasicAi::is_barbarian_raider(g, unit)
                            && g.wdist(unit.pos, city.pos) <= crate::ai::HOME_THREAT_RADIUS
                    })
            })
            .map(|cid| (g.cities[&cid].pop, cid))
            .collect();
        cities.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
        cities.into_iter().map(|(_, cid)| cid).collect()
    }

    /// See the module: buy the Settler the treasury can pay for, else say how
    /// much the discretionary purchases must leave behind while it saves.
    /// [`SettlerFund::Off`] with the gene off.
    pub(super) fn settler_fund(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> SettlerFund {
        if !self.settler_fund_open(g, pid, plan) {
            return SettlerFund::Off;
        }
        let floor = self.threatened_city_gold_floor(g, pid, plan);
        let bank = g.players[pid].gold;
        let settler = Self::settler_fund_item();
        let mut cheapest: Option<f64> = None;
        for cid in Self::settler_fund_cities(g, pid) {
            if g.purchase_is_blocked(cid, &settler) {
                continue;
            }
            let quoted = g.unit_purchase_cost(pid, cid, "settler", "gold");
            if let Some(price) = quoted {
                if bank + f64::EPSILON >= floor + price {
                    let action = Action::Buy {
                        city: cid,
                        unit: crate::name!("settler"),
                        formation: 0,
                        currency: "gold".to_string(),
                    };
                    if g.apply(pid, &action).is_ok() {
                        let walkers = Self::settler_fund_walkers(g, pid);
                        let cities = g.player_city_ids(pid).len();
                        let name = g.cities[&cid].name.clone();
                        think!(self.journal(), Expansion, Decision,
                            "Buying a Settler for {name} with Gold";
                            "gold-buys-the-settler: {price:.0} Gold from a bank of {bank:.0} \
                             above a floor of {floor:.0}; {cities} cities and {walkers} walking \
                             against a fund target of {}",
                            SETTLER_FUND_CITIES.min(plan.desired_cities));
                        return SettlerFund::Bought;
                    }
                }
            }
            if !g.can_produce(pid, cid, &settler) {
                continue;
            }
            let estimate = quoted.unwrap_or_else(|| {
                g.item_cost_for_city(pid, cid, &settler) * SETTLER_FUND_GOLD_PER_PRODUCTION
            });
            cheapest = Some(cheapest.map_or(estimate, |best| best.min(estimate)));
        }
        let Some(price) = cheapest else {
            return SettlerFund::Off;
        };
        let income = g.players[pid].gold_per_turn.max(0.0);
        let horizon = g.standard_duration(SETTLER_FUND_SAVE_STANDARD) as f64;
        if bank + income * horizon + f64::EPSILON >= floor + price {
            think!(self.journal(), Expansion, Detail,
                "Saving the treasury for a Settler";
                "gold-buys-the-settler: {bank:.0} of {price:.0} Gold on {income:.1} a turn; \
                 discretionary purchases keep {:.0} behind", floor + price);
            SettlerFund::Saving {
                reserve: floor + price,
            }
        } else {
            SettlerFund::Off
        }
    }
}

#[cfg(test)]
mod tests;
