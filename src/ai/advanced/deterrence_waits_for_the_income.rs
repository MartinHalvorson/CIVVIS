//! `deterrence-waits-for-the-income`: at peace, `near-rival-deterrence` (its
//! opening, `-opens-early` and `-holds-late` phases alike) trains no unit the
//! income cannot pay for.
//!
//! **The finding (live Deity Kongo, Science lane, 6 players, Continents,
//! Online; `civvis-20261118T045850Z`).** Canada fielded 4-6 times our power
//! within ten tiles from turn 13, and the deterrent trained a Warrior, an
//! Archer or a Horseman 26 times on 22 turns from t39 to t78 (military units
//! 8 at t50, 14 at t65, 23 at t74, 27 at t80). An Archer is 1 Gold a turn
//! and a Warrior none: the unit bill went 3 → 16 Gold while gross income
//! stood at 5-15, so income read +1 at t50, 0 at t65, −1 at t68, −3 at
//! t74-80 and −10 at t84 on a treasury of 45-90 Gold. Sixteen of the 26 were
//! queued on a turn that opened at an income of zero or less. At t84-85
//! `bankruptcy-exits-the-drain-2` sold the Horses and disbanded three Archers
//! and the Galley. Science stood at 21 a turn at t80, and "held by near-rival
//! deterrence" postponed a city's first Campus on 38 city-turns from t35 to
//! t78.
//!
//! **What already existed, and the gap.** The deterrent is a claim: it runs
//! ahead of the governor and queues its unit with `Action::Produce`, and the
//! governor then keeps that head as a commitment (`deterrence_unit_holds`).
//! - `upkeep-waits-for-the-income` prices upkeep out of the governor's
//!   `production_value`, which the claim never asks, and only on a Domination
//!   seat from Online turn 120.
//! - Recovery (`live_war_economy_requires_recovery`: income below −0.5 and
//!   the treasury under 100 + 25 a city) sends the other queues to income
//!   work, but its preemption spares every land soldier as a defender
//!   (`science_recovery_preempts`).
//! - `upkeep-reserve` and `purchases-keep-the-upkeep-reserve` are treasury
//!   floors for upgrades and Gold purchases. `solvent-or-hold` holds the
//!   Domination modernizer and the amenity and upkeep cards.
//! - `bankruptcy-exits-the-drain-2` acts afterwards: it sells and disbands
//!   once the treasury is under five turns of the deficit.
//!
//! **What the gene does.** A deterrent candidate is passed over when its
//! upkeep would put the empire on Recovery's own line: income, less the
//! upkeep our cities' queue heads add when they complete, less the unit's,
//! is below [`DETERRENT_INCOME_FLOOR`] and the treasury is under
//! [`DETERRENT_RESERVE_BASE`] plus [`DETERRENT_RESERVE_PER_CITY`] a city.
//! The head the deterrent would displace counts with the others: it keeps
//! its progress and comes back. A unit that costs nothing waits too once
//! income is already under that line: those queues are the ones Recovery
//! needs. A unit's upkeep is the host's effective figure, the type's
//! maintenance less the slotted per-unit discount.
//!
//! **Why Recovery's reserve and not a runway.** `upkeep-waits-for-the-income`
//! lets an item start when the treasury covers fifteen turns of the deficit,
//! the decisive window of a war. A deterrent has no such window: it stands
//! until the neighbour leaves, so its upkeep is paid from income or not at
//! all. Through t73 this game's treasury of 65-90 Gold covered fifteen turns
//! of the projected deficit (2 to 5 Gold), so that runway would have passed
//! every deterrent queued in the six turns after income turned negative.
//!
//! **What it never touches.** The gate stands down while we are at war with
//! any major or city-state, while the plan reads a threatened city or plays
//! Recovery, and in a city attacked within the last four turns
//! (`upkeep_city_threatened`). It reads only the deterrent's own claim: the
//! governor, the emergency lanes, the capital's shooters
//! (`capital-archers-before-the-surprise`, two at most) and Walls are not
//! asked. A deterrent already at the head of a queue keeps its hold, at most
//! `DETERRENCE_MAX_QUEUED` of them.
//!
//! Off: nothing changes. Grep the journal for
//! `deterrence-waits-for-the-income:`.

use super::*;

/// Projected income below this is a deficit: Recovery's line
/// (`live_war_economy_requires_recovery`).
pub(crate) const DETERRENT_INCOME_FLOOR: f64 = -0.5;

/// The treasury that carries a standing deficit, Recovery's reserve: this
/// much Gold and [`DETERRENT_RESERVE_PER_CITY`] more for each city.
pub(crate) const DETERRENT_RESERVE_BASE: f64 = 100.0;

/// See [`DETERRENT_RESERVE_BASE`].
pub(crate) const DETERRENT_RESERVE_PER_CITY: f64 = 25.0;

/// Why a deterrent waits: the figures the journal prints.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct DeterrentShortfall {
    /// The unit's own Gold a turn.
    pub(super) upkeep: f64,
    /// The upkeep our cities' queue heads add when they complete.
    pub(super) committed: f64,
    /// Income this turn.
    pub(super) income: f64,
    /// The treasury.
    pub(super) gold: f64,
    /// The treasury that would have carried it.
    pub(super) reserve: f64,
}

impl AdvancedAi {
    /// The Gold a turn one more unit of `kind` costs `pid`: the type's
    /// maintenance less the slotted per-unit discount, never below zero,
    /// plus the military surcharge (`Game::unit_gold_maintenance_cost`).
    fn deterrent_unit_upkeep(g: &Game, pid: usize, kind: &Name) -> f64 {
        let Some(spec) = g.rules.units.get(kind) else {
            return 0.0;
        };
        let surcharge = if spec.class == "military" {
            g.policy_effect(pid, "unit_maintenance_surcharge")
        } else {
            0.0
        };
        (spec.maintenance - g.policy_effect(pid, "unit_maintenance_discount")).max(0.0) + surcharge
    }

    /// The upkeep `item` adds once it completes: a unit at its effective
    /// cost, anything else as `upkeep-waits-for-the-income` prices it.
    fn deterrent_item_upkeep(g: &Game, pid: usize, item: &Item) -> f64 {
        match item {
            Item::Unit { unit } => Self::deterrent_unit_upkeep(g, pid, unit),
            other => Self::item_added_upkeep(g, other),
        }
    }

    /// Whether `pid` is at war with a living major or city-state.
    fn deterrent_seat_at_war(g: &Game, pid: usize) -> bool {
        g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_free_city
                && g.is_at_war(pid, other.id)
        })
    }

    /// `deterrence-waits-for-the-income`: why the deterrent `item` waits in
    /// `cid`, or `None` when it may be trained. See the module: at peace,
    /// with no city threatened, income after every queue head's upkeep and
    /// this unit's is below [`DETERRENT_INCOME_FLOOR`] and the treasury is
    /// under Recovery's reserve. `None` with the gene off.
    pub(super) fn deterrent_income_shortfall(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        plan: &StrategicPlan,
    ) -> Option<DeterrentShortfall> {
        if !self.deterrence_waits_for_the_income
            || plan.threatened_city.is_some()
            || plan.strategy == GrandStrategy::Recovery
            || Self::upkeep_city_threatened(g, cid, plan)
            || Self::deterrent_seat_at_war(g, pid)
        {
            return None;
        }
        let city_ids = g.player_city_ids(pid);
        let committed: f64 = city_ids
            .iter()
            .filter_map(|city| g.cities.get(city)?.queue.first())
            .map(|head| Self::deterrent_item_upkeep(g, pid, head))
            .sum();
        let upkeep = Self::deterrent_item_upkeep(g, pid, item);
        let player = &g.players[pid];
        let reserve = DETERRENT_RESERVE_BASE + DETERRENT_RESERVE_PER_CITY * city_ids.len() as f64;
        (player.gold_per_turn - committed - upkeep < DETERRENT_INCOME_FLOOR
            && player.gold < reserve)
            .then_some(DeterrentShortfall {
                upkeep,
                committed,
                income: player.gold_per_turn,
                gold: player.gold,
                reserve,
            })
    }

    /// Journal that the deterrent against `rival` was not trained this turn
    /// because every candidate waits; `cid` and `unit` are the first one.
    pub(super) fn note_deterrent_waits(
        &self,
        g: &Game,
        rival: usize,
        cid: u32,
        unit: &str,
        shortfall: DeterrentShortfall,
    ) {
        think!(self.journal(), Military, Decision,
            "No deterrent against {}: the income cannot pay for it", g.players[rival].civ;
            "deterrence-waits-for-the-income: {} in {} adds {:.0} Gold a turn to {:+.1} income \
             with {:.0} more committed in the queues; {:.0} Gold banked, under the {:.0} that \
             would carry a standing deficit",
            unit.replace('_', " "),
            g.cities.get(&cid).map(|city| city.name.as_str()).unwrap_or_default(),
            shortfall.upkeep, shortfall.income, shortfall.committed, shortfall.gold,
            shortfall.reserve);
    }
}

#[cfg(test)]
mod tests;
