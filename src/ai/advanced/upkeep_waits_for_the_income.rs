//! `upkeep-waits-for-the-income`: a Domination seat starts no recurring
//! upkeep its income cannot carry.
//!
//! **The finding (live Immortal 2026-10-27 16:30Z to 10-29 03:00Z).**
//! - Of the 30 games that reached turn 160, 11 went bankrupt after turn 120
//!   (12 of the 45 that reached turn 120, mostly between turns 155 and 195).
//!   Income below zero held on 30% of the turns from 150 to 174, 39% from 175
//!   to 199 and 52% after 200; the treasury stood empty on 11% and 26%.
//! - Those were the big games: 9-10 cities against 8-9 in the ones that stayed
//!   solvent. Gold infrastructure did not separate them (income over upkeep
//!   0.19 against 0.21 at turn 120, 0.12 against 0.14 at 140; the bankrupt
//!   ones held more Commercial Hubs). The late war army did: the split opened
//!   only at turn 160 (0.06 against 0.25).
//! - civvis-20261028T162814Z, the best Production game of the day (386 to
//!   Australia's 389 at t173): the unit bill rose 46 → 96 and the building
//!   bill 49 → 90 from t160 to t200, against 90-176 of gross city Gold.
//!   Income went +25 → −37 in nine turns (t190 → t199) as seven modern units
//!   came out of the queues, then the host's bankruptcy charge (10 amenities
//!   in every city) put all twelve cities at −40% yields and disbanded the
//!   costliest units, military 1070 → 886, three turns before Australia won.
//!
//! **What already existed, and the gap.** Recovery (`live_war_economy_requires_recovery`:
//! income below −0.5 and the treasury under 100 + 25 a city) sends an
//! empty queue to income or upkeep-free work, but a Domination seat keeps
//! every queued unit and building: the committed-item preemption reads
//! `science_recovery_preempts`, which answers only for a Science seat. And
//! nothing looks ahead: in 162814Z the seven units were started while income
//! was still positive, each priced as if it alone would come due.
//!
//! **What the gene does (Domination seat, on).**
//! - An item that adds recurring upkeep (a unit's maintenance, a building's
//!   maintenance net of its own Gold, a district other than a Commercial Hub,
//!   Harbor or Spaceport) is priced out of `production_value` when income,
//!   less the upkeep every other city's queue head will add on completion,
//!   less this item's, falls below zero and the treasury cannot carry that
//!   deficit for [`UPKEEP_RUNWAY_TURNS`].
//! - The same test lets the governor switch a city off a queued upkeep item
//!   under half paid ([`UPKEEP_PREEMPT_PAID_SHARE`]), so routine rescoring
//!   (or Recovery, when it is on) takes the queue. An item paid past half
//!   is never priced out: it finishes.
//! - A land combat unit in a threatened city is never held: the defence of a
//!   city under attack comes first, as in Science's preemption.
//!
//! Off, or for any other victory target, nothing changes.

use super::*;

/// Turns of the projected deficit the treasury must cover for an upkeep item
/// to start anyway. Fifteen turns is the decisive window of a war (the host's
/// target mobilises ×1.76 within twenty), so a bank that outlasts it may
/// still pay for the army that wins it.
pub(crate) const UPKEEP_RUNWAY_TURNS: f64 = 15.0;

/// A queued upkeep item paid at least this share of its cost finishes: its
/// hammers are nearly spent and the unit is nearly in the field.
pub(crate) const UPKEEP_PREEMPT_PAID_SHARE: f64 = 0.5;

/// Formation sizes in units: a Corps/Fleet costs two units' upkeep, an
/// Army/Armada three.
fn formation_units(formation: u8) -> f64 {
    match formation {
        0 => 1.0,
        1 => 2.0,
        _ => 3.0,
    }
}

impl AdvancedAi {
    /// The recurring Gold a turn `item` adds once it completes: a unit's
    /// maintenance (times its formation size), a building's maintenance net
    /// of its own Gold yield, a district's maintenance unless it is an income
    /// district (Commercial Hub, Harbor, Spaceport). Zero for anything else.
    pub(super) fn item_added_upkeep(g: &Game, item: &Item) -> f64 {
        match item {
            Item::Unit { unit } => g
                .rules
                .units
                .get(unit)
                .map_or(0.0, |spec| spec.maintenance.max(0.0)),
            Item::Formation { unit, formation } => g.rules.units.get(unit).map_or(0.0, |spec| {
                spec.maintenance.max(0.0) * formation_units(*formation)
            }),
            Item::Building { building } => g
                .rules
                .buildings
                .get(building)
                .map_or(0.0, |spec| (spec.maintenance - spec.yields.gold).max(0.0)),
            Item::District { district, .. } => {
                if matches!(
                    g.district_family(*district).as_str(),
                    "commercial_hub" | "harbor" | "spaceport"
                ) {
                    0.0
                } else {
                    g.rules
                        .districts
                        .get(district)
                        .map_or(0.0, |spec| spec.maintenance.max(0.0))
                }
            }
            _ => 0.0,
        }
    }

    /// The upkeep our cities' queue heads will add as they complete, `except`
    /// the city whose queue is being chosen.
    pub(super) fn committed_queue_upkeep(g: &Game, pid: usize, except: Option<u32>) -> f64 {
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| Some(*cid) != except)
            .filter_map(|cid| g.cities.get(&cid))
            .filter_map(|city| city.queue.first())
            .map(|item| Self::item_added_upkeep(g, item))
            .sum()
    }

    /// Whether `item` is a land combat unit, the defence a threatened city
    /// keeps.
    fn upkeep_item_is_land_defender(g: &Game, item: &Item) -> bool {
        match item {
            Item::Unit { unit } | Item::Formation { unit, .. } => {
                g.rules.units.get(unit).is_some_and(|spec| {
                    spec.class == "military"
                        && !spec.siege
                        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                })
            }
            _ => false,
        }
    }

    /// `upkeep-waits-for-the-income`: whether `item` in `cid` must wait. Its
    /// added upkeep, on top of every other city's committed queue head, takes
    /// income below zero, and the treasury holds less than
    /// [`UPKEEP_RUNWAY_TURNS`] of the resulting deficit. A land defender in a
    /// `threatened` city never waits. False with the gene off or for a seat
    /// not aiming at Domination.
    pub(super) fn upkeep_waits_for(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        threatened: bool,
    ) -> bool {
        if !self.upkeep_waits_for_the_income
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return false;
        }
        let added = Self::item_added_upkeep(g, item);
        if added <= 0.0 || (threatened && Self::upkeep_item_is_land_defender(g, item)) {
            return false;
        }
        let player = &g.players[pid];
        let projected =
            player.gold_per_turn - Self::committed_queue_upkeep(g, pid, Some(cid)) - added;
        projected < 0.0 && player.gold < UPKEEP_RUNWAY_TURNS * -projected
    }

    /// The threat reading [`Self::upkeep_waits_for`] takes for a city's own
    /// queue head in the governor: the plan's threatened city, or a city
    /// attacked within four turns.
    fn upkeep_city_threatened(g: &Game, cid: u32, plan: &StrategicPlan) -> bool {
        plan.threatened_city == Some(cid)
            || g.cities.get(&cid).is_some_and(|city| {
                city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4
            })
    }

    /// `upkeep-waits-for-the-income`: whether the governor takes `cid` off its
    /// queued `item`: the item must wait (see [`Self::upkeep_waits_for`]) and
    /// has paid under [`UPKEEP_PREEMPT_PAID_SHARE`] of its cost.
    pub(super) fn upkeep_wait_preempts(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        plan: &StrategicPlan,
    ) -> bool {
        self.upkeep_wait_holds(g, pid, cid, item, plan)
            && !Self::upkeep_item_mostly_paid(g, pid, cid, item)
    }

    /// Whether `cid` has paid at least [`UPKEEP_PREEMPT_PAID_SHARE`] of
    /// `item`'s cost; such an item finishes whatever its upkeep.
    pub(super) fn upkeep_item_mostly_paid(g: &Game, pid: usize, cid: u32, item: &Item) -> bool {
        let cost = g.item_cost_for_city(pid, cid, item).max(1.0);
        g.item_invested_production(cid, item) >= UPKEEP_PREEMPT_PAID_SHARE * cost
    }

    fn upkeep_wait_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        plan: &StrategicPlan,
    ) -> bool {
        self.upkeep_waits_for(
            g,
            pid,
            cid,
            item,
            Self::upkeep_city_threatened(g, cid, plan),
        )
    }

    /// Journal the preemption the governor is about to make.
    pub(super) fn note_upkeep_wait_preemption(&self, g: &Game, pid: usize, cid: u32, item: &Item) {
        if !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        let player = &g.players[pid];
        let added = Self::item_added_upkeep(g, item);
        let committed = Self::committed_queue_upkeep(g, pid, Some(cid));
        let name = g
            .cities
            .get(&cid)
            .map(|city| city.name.clone())
            .unwrap_or_default();
        think!(self.journal(), Economy, Decision,
            "{} sets aside {} until the income carries it", name, Self::plain_item(item);
            "upkeep-waits-for-the-income: {added:.0} Gold a turn of upkeep on {:.1} income with \
             {committed:.0} more committed in other queues, and {:.0} Gold banked under {:.0} turns \
             of that deficit",
            player.gold_per_turn, player.gold, UPKEEP_RUNWAY_TURNS);
    }
}

#[cfg(test)]
mod tests;
