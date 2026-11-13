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
    pub(super) fn early_settler_item() -> Item {
        Item::Unit {
            unit: crate::name!("settler"),
        }
    }

    /// Walking Settlers, and cities whose queue head is a Settler.
    pub(super) fn early_settler_census(g: &Game, pid: usize) -> (usize, usize) {
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
        // `score-seat-expands-hard`: the floor stays open past the band turn
        // for as long as a new city still pays. See
        // `advanced/score_expands_hard.rs`.
        if !(self.early_settler_floor || self.no_city_limit)
            || (g.turn >= g.standard_duration(EARLY_SETTLER_FLOOR_STANDARD_TURN)
                && !self.score_expands_hard_now(g))
        {
            return 0;
        }
        // `no-city-limit`: the opening fills to every seat that pays, not to
        // six. See `advanced/no_city_limit.rs`.
        if self.no_city_limit {
            return self.settlement_target(plan).max(1);
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
        // `holdable-land-keeps-settling`: the mid-game floor's Settler is
        // kept the same way, by every claim that reads this.
        if self.holdable_land_settler_holds(g, pid, cid, plan) {
            return true;
        }
        // `score-seat-expands-hard`: the Ancestral Hall its claim raised is
        // kept the same way. See `advanced/score_expands_hard.rs`.
        if self.score_expands_hard_now(g) && Self::score_expands_keeps_the_hall(g, cid) {
            return true;
        }
        let floor = self.early_settler_floor(g, plan);
        if floor == 0
            || !matches!(
                g.cities[&cid].queue.first(),
                Some(Item::Unit { unit }) if *unit == "settler"
            )
        {
            return false;
        }
        let (walkers, queued) = Self::early_settler_census(g, pid);
        // `no-city-limit`: the other Settlers in production count too.
        let others = if self.no_city_limit {
            queued.saturating_sub(1)
        } else {
            0
        };
        g.player_city_ids(pid).len() + walkers + others < floor
    }

    /// A barbarian raider in the city's home ring. A camp merely in reach is
    /// not enough: the controller's alarm reads one for as long as the camp
    /// stands, and in civvis-20261008T131421Z it held both cities from t24
    /// to t45 while no raider came, the Settler gate open in each.
    pub(super) fn early_settler_raider_near(g: &Game, home: Pos) -> bool {
        g.units.values().any(|unit| {
            crate::ai::BasicAi::is_barbarian_raider(g, unit)
                && g.wdist(unit.pos, home) <= crate::ai::HOME_THREAT_RADIUS
        })
    }

    /// Whether `cid`'s current queue head may give way to the floor's Settler.
    pub(super) fn early_settler_may_displace(g: &Game, pid: usize, cid: u32) -> bool {
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
        // `no-city-limit`: several Settlers at once. `None` with the gene
        // off. See `advanced/no_city_limit.rs`.
        let wide = self.no_city_limit_width(g, pid);
        let full = match wide {
            Some(width) => walkers + queued >= width || n_cities + walkers + queued >= floor,
            None => queued > 0 || walkers > 1 || n_cities + walkers >= floor,
        };
        if n_cities == 0 || full {
            return;
        }
        // `score-seat-expands-hard`: the capital queues its next Settler while
        // the first still walks.
        let hard = self.score_expands_hard_now(g);
        if walkers > 0 && n_cities < 2 && !hard {
            return;
        }
        // A home city under threat answers first; so does an army short of a
        // body per city in a war with a major. `no-city-limit` passes over
        // only the cities at the threatened one.
        if plan.threatened_city.is_some() && wide.is_none() {
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
        // `no-city-limit`: at war the cities near a visible enemy soldier
        // train no Settler; it would only shelter there.
        let war_soldiers = if wide.is_some() && at_major_war {
            Self::no_city_limit_war_soldiers(g, pid)
        } else {
            Vec::new()
        };
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
        // `score-seat-expands-hard`: a soldier's queue gives way only while
        // no major is at war with us and the army stands at a body a city.
        let army_stands = hard && !at_major_war && self.counts(g, pid).military >= n_cities;
        // ... and never in a city on a rival's doorstep.
        let rival_cities = if army_stands {
            Self::holdable_land_threats(g, pid).rival_cities
        } else {
            Vec::new()
        };
        let mut best: Option<(f64, u32)> = None;
        for &cid in &city_ids {
            let city = &g.cities[&cid];
            if Some(cid) == opening_capital
                || (wide.is_some() && !Self::no_city_limit_city_is_clear(g, plan, cid))
                || Self::no_city_limit_war_at_the_gates(g, &war_soldiers, city.pos)
                || (city.pop as f64) < crate::ai::HOST_SETTLER_MIN_POP
                || (city.last_attacked > 0
                    && g.turn.saturating_sub(city.last_attacked) <= EARLY_SETTLER_ATTACK_MEMORY)
                || match wide {
                    // `no-city-limit`: a garrisoned city trains through
                    // raiders.
                    Some(_) => Self::no_city_limit_raider_holds(g, pid, city.pos),
                    None => Self::early_settler_raider_near(g, city.pos),
                }
                || self.short_seowon_holds_settler_floor(g, pid, cid)
                || self.science_campus_holds_repeat_settler(g, pid, cid, plan)
                || city.queue.first().is_some_and(|head| {
                    self.first_capital_land_recon_holds(g, pid, cid, plan, head)
                })
                || (hard && Self::score_expands_keeps_the_hall(g, cid))
                || !(Self::early_settler_may_displace(g, pid, cid)
                    || (hard
                        && Self::score_expands_may_displace(
                            g,
                            pid,
                            cid,
                            army_stands
                                && !Self::score_expands_on_the_frontier(g, &rival_cities, city.pos),
                        )))
                || !g.can_produce(pid, cid, &item)
            {
                continue;
            }
            let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                g.item_cost_for_city(pid, cid, &item) / g.city_yields(cid).production.max(0.5)
                    // `no-city-limit`: the city's Settler modifiers count, so
                    // the Ancestral Hall's city and a Colonization empire
                    // train where the Settler is really cheapest.
                    / wide.map_or(1.0, |_| g.item_prod_mult(pid, cid, Some(&item)).max(0.1))
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
            if hard {
                think!(self.journal(), Expansion, Decision,
                    "{} starts a Settler for the land still free", g.cities[&cid].name;
                    "score-seat-expands-hard: {n_cities} cities, {walkers} walking and {queued} \
                     in production against {floor} seats that pay, {turns:.1} turns{}",
                    displaced.as_ref().map_or(String::new(), |item| format!(
                        "; {} keeps its progress", Self::plain_item(item))));
                return;
            }
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
