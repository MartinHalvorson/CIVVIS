//! `early-settler-floor`: before the band turn, while the empire holds fewer
//! than [`EARLY_SETTLER_FLOOR_CITIES`] cities and walkers and no Settler is in
//! production, the unthreatened city that trains one soonest puts a Settler at
//! the head of its queue, ahead of the Prophet race's Holy Site and Shrine,
//! the land-frontier Scout and the strategic scorer's routine builds. Local
//! defence, the conquest opening's capital and investments in progress keep
//! their queues.
//!
//! **Why (10-08 census, 54 gate-era live Emperor runs).** The city count at
//! turn 75 decides the operator's production gate: at four or fewer cities
//! 0 of 19 runs were top two by Production at turn 150, at five or six 2 of 21,
//! at seven or more 8 of 14. The gap opens after turn 20, not before: both
//! groups held a median 2 cities at t25 and 3-4 at t50. Across turns 20-50
//! the four-or-fewer runs started a median one Settler against 2.8 for the
//! seven-plus runs, and their capital spent 3.6-7.6 of those 31 turns on a
//! Settler against 9.9. The walk is not the gap (a median five turns from
//! first sight to founding in every group, and fewer guard holds in the small
//! empires), nor is the site search (two of 19 runs). The capital's turns went
//! elsewhere:
//!
//! - the early-conquest opening's strike force (7 of 19 runs, 13.4 turns of
//!   units in the capital) -- the operator's call, and this gene leaves the
//!   reserved capital alone;
//! - a war a rival declared (7 of 19, from turns 37-72);
//! - and in the 5 peaceful runs the Prophet race's Holy Site and Shrine (about
//!   ten turns), land-frontier Scouts (five to seven), Walls and a Granary,
//!   while the Settler gate itself was open: `--explain-settler` on
//!   civvis-20261008T131421Z t35 reads every gate true and `pick_item`
//!   answering Walls.
//!
//! `expansion-schedule` already measured the same thing from the other side:
//! widening the pipeline cannot fix a city that does not want the Settler it
//! may now build. This gene is the actuation half -- it puts the Settler in
//! the queue -- and is bounded so it cannot outrun the opening: one Settler in
//! production at a time, at most one walker already out, a city at the host's
//! population floor, a build no longer than [`EARLY_SETTLER_MAX_BUILD`]
//! standard turns, and nothing at all past the band turn.

use super::*;

/// Cities plus walkers the floor fills to before the band turn. Seven-city
/// empires at turn 75 passed the gate 57% of the time; a floor of six by the
/// band turn leaves the last city to the ordinary governors.
pub(crate) const EARLY_SETTLER_FLOOR_CITIES: usize = 6;

/// The longest Settler build, in standard turns, the floor will queue: a
/// city that needs longer is not where the next city comes from.
pub(crate) const EARLY_SETTLER_MAX_BUILD: u32 = 24;

/// The window, in standard turns: 90 standard turns are turn 60 at the live
/// seat's Quick speed. The census decides the gate at turn 75 and the gap
/// opens after turn 20; the floor works before the last walker must leave.
pub(crate) const EARLY_SETTLER_FLOOR_STANDARD_TURN: u32 = 90;

/// A city attacked within this many turns keeps its queue.
const EARLY_SETTLER_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    fn early_settler_item() -> Item {
        Item::Unit {
            unit: crate::name!("settler"),
        }
    }

    /// Walking Settlers, and cities whose queue head is a Settler.
    fn early_settler_census(g: &Game, pid: usize) -> (usize, usize) {
        let walkers = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.units[uid].kind == "settler")
            .count();
        let queued = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                matches!(
                    g.cities[cid].queue.first(),
                    Some(Item::Unit { unit }) if *unit == "settler"
                )
            })
            .count();
        (walkers, queued)
    }

    /// The cities-plus-walkers floor in force now: the band floor, never
    /// above the plan's own target. Zero with the gene off and past the band
    /// turn.
    pub(super) fn early_settler_floor(&self, g: &Game, plan: &StrategicPlan) -> usize {
        if !self.early_settler_floor
            || g.turn >= g.standard_duration(EARLY_SETTLER_FLOOR_STANDARD_TURN)
        {
            return 0;
        }
        EARLY_SETTLER_FLOOR_CITIES.min(plan.desired_cities.max(1))
    }

    /// Whether the floor still wants the Settler queued in `cid`: the gene is
    /// on, it is the band window, and the founded cities plus the walkers
    /// already out (this queue excluded) are short of the floor.
    pub(super) fn early_settler_floor_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        let floor = self.early_settler_floor(g, plan);
        if floor == 0
            || !matches!(
                g.cities[&cid].queue.first(),
                Some(Item::Unit { unit }) if *unit == "settler"
            )
        {
            return false;
        }
        let (walkers, _) = Self::early_settler_census(g, pid);
        g.player_city_ids(pid).len() + walkers < floor
    }

    /// A barbarian raider in the city's home ring. A camp merely in reach is
    /// not enough: the controller's alarm reads one for as long as the camp
    /// stands, and in civvis-20261008T131421Z it held both cities from t24
    /// to t45 while no raider came, the Settler gate open in each.
    fn early_settler_raider_near(g: &Game, home: Pos) -> bool {
        g.units.values().any(|unit| {
            crate::ai::BasicAi::is_barbarian_raider(g, unit)
                && g.wdist(unit.pos, home) <= crate::ai::HOME_THREAT_RADIUS
        })
    }

    /// Whether `cid`'s current queue head may give way to the floor's Settler.
    fn early_settler_may_displace(g: &Game, pid: usize, cid: u32) -> bool {
        let Some(item) = g.cities[&cid].queue.first() else {
            return true;
        };
        let remaining =
            g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item);
        if remaining <= g.city_yields(cid).production {
            return false;
        }
        match item {
            Item::Building { building } => g.rules.buildings.get(building).is_some_and(|spec| {
                !spec.wonder
                    && !matches!(
                        building.as_str(),
                        "walls" | "medieval_walls" | "renaissance_walls"
                    )
            }),
            Item::District { .. } => g.item_invested_production(cid, item) <= 0.0,
            Item::Unit { unit } => matches!(unit.as_str(), "scout" | "trader"),
            _ => false,
        }
    }

    /// See the module: queue one Settler in the unthreatened city that trains
    /// it soonest while the empire is short of the band floor. Exact no-op
    /// with the gene off.
    pub(super) fn claim_early_settler_floor(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let floor = self.early_settler_floor(g, plan);
        if floor == 0 {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        let n_cities = city_ids.len();
        let (walkers, queued) = Self::early_settler_census(g, pid);
        if n_cities == 0 || queued > 0 || walkers > 1 || n_cities + walkers >= floor {
            return;
        }
        if walkers > 0 && n_cities < 2 {
            return;
        }
        // A home city under threat answers first; so does an army short of a
        // body per city in a war with a major.
        if plan.threatened_city.is_some() {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.is_at_war(pid, other.id)
        });
        if at_major_war && self.counts(g, pid).military < n_cities {
            return;
        }
        if !self.base.has_practical_settle_site(g, pid) {
            return;
        }
        let opening_capital = self
            .conquest_reservation_open(g)
            .then(|| {
                city_ids
                    .iter()
                    .copied()
                    .find(|cid| g.cities[cid].is_capital)
            })
            .flatten();
        let item = Self::early_settler_item();
        let max_turns = g.standard_duration(EARLY_SETTLER_MAX_BUILD) as f64;
        let mut best: Option<(f64, u32)> = None;
        for &cid in &city_ids {
            let city = &g.cities[&cid];
            if Some(cid) == opening_capital
                || (city.pop as f64) < crate::ai::HOST_SETTLER_MIN_POP
                || (city.last_attacked > 0
                    && g.turn.saturating_sub(city.last_attacked) <= EARLY_SETTLER_ATTACK_MEMORY)
                || Self::early_settler_raider_near(g, city.pos)
                || !Self::early_settler_may_displace(g, pid, cid)
                || !g.can_produce(pid, cid, &item)
            {
                continue;
            }
            let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                g.item_cost_for_city(pid, cid, &item) / g.city_yields(cid).production.max(0.5)
            });
            if turns > max_turns {
                continue;
            }
            if best.is_none_or(|(old, old_city)| {
                turns + 1e-9 < old || ((turns - old).abs() <= 1e-9 && cid < old_city)
            }) {
                best = Some((turns, cid));
            }
        }
        let Some((turns, cid)) = best else {
            return;
        };
        let displaced = g.cities[&cid].queue.first().cloned();
        if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
            think!(self.journal(), Expansion, Decision,
                "{} starts a Settler for the early city floor", g.cities[&cid].name;
                "early-settler-floor: {n_cities} cities and {walkers} walking against a floor of \
                 {floor} before turn {}, {turns:.1} turns{}",
                g.standard_duration(EARLY_SETTLER_FLOOR_STANDARD_TURN),
                displaced.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
