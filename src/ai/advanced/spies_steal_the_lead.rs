//! `spies-steal-the-lead`: a Science seat's Spies steal Eurekas from the
//! rivals ahead of it instead of idling on Listening Posts.
//!
//! ## Why (live Deity Korea, Science seat, Online, 2026-11-04 21Z - 11-05 04Z)
//!
//! Every game that lived past turn 150 lost a science race it trailed by 11 to
//! 17 technologies (rival Technology wins at 174-196). The Deity rivals know
//! many technologies we do not, which is exactly what Steal Tech Boost needs
//! (`UNITOPERATION_SPY_STEAL_TECH_BOOST`, a Campus target): one success grants
//! the Eureka of a technology the target knows and we lack.
//!
//! Across the 11 Deity games that reached turn 130, our Spies were ordered on
//! Disrupt Rocketry 60 times, Gain Sources 36, Listening Post 35 and Steal Tech
//! Boost 8 (`orders.sqlite`). Those 8 steals all ran at turns 113-124, before
//! any rival owned a pad, and 6 of them landed a Eureka within the mission
//! (Steel, Refining twice, Combustion twice, Nanotechnology); 2 Spies were
//! lost. A Combustion Eureka at Online speed is 40% of 1370 x 0.5 = 274
//! Science, about two turns of our whole empire's Science at turn 117.
//!
//! The steals stop once a racer exists because `racer-keeps-the-spies` posts
//! every Spy to the leading racer and, in its cities, allows only Listening
//! Post or Gain Sources until Disrupt Rocketry is offered; it also lists Steal
//! Tech Boost among the capture-risk operations a Spy outside the racer's
//! cities may not start. Live 034948Z from turn 132: "A Spy goes to
//! Phoenicia's Tyre | racer-keeps-the-spies: ... the Spy waits there for
//! Disrupt Rocketry instead of risking its slot elsewhere". Meanwhile the
//! training step sends every Spy below level 2 on Gain Sources first, but the
//! live Spies never level: every agent of these games read level 1 with no
//! promotion until it was lost (11-21 lost a game), so the training step bought
//! four turns of nothing per Spy.
//!
//! ## What it does
//!
//! On a seat aiming at Science (its victory target, or the plan's strategy),
//! while Steal Tech Boost is offered where the Spy stands (by the board's menu
//! and, when the host sent one, by the host's):
//!
//! 1. in a space racer's city the Spy steals instead of running Listening
//!    Post or Gain Sources;
//! 2. a Spy below level 2 steals instead of running its Gain Sources training
//!    step;
//! 3. outside the racers' cities Steal Tech Boost is no longer one of
//!    `racer-keeps-the-spies`' withheld capture-risk operations, so the
//!    ordinary operation table (which ranks it first on a Science plan) may
//!    pick it.
//!
//! Wherever Disrupt Rocketry is offered (by the board or the host) the pass
//! keeps its pre-gene path, so the denial genes still decide there.
//!
//! 4. Spies to steal with. Spy capacity arrived at turns 99-146 and we held
//!    about half of it (0 of 1 at turn 110, 1 of 3 at 150): before a space
//!    racer exists only the stock valuation trains Spies, and
//!    `science-denial-trains-spies` starts once one does. While a met rival
//!    with a Campus we have seen knows [`STEAL_MIN_LACKED`] technologies we
//!    lack, and no denial claim is training, one Spy at a time trains in the
//!    top-Production city whose queue holds routine work (the denial claim's
//!    displacement rule), until we hold our capacity. A pause from
//!    `denial-spies-count-their-losses` holds it. At Online speed a stolen
//!    Eureka was worth 250-431 Science, a steal took about four turns and 6
//!    of 8 landed, so one Spy is worth roughly +50 Science a turn against our
//!    130-200 at turns 120-150, for 150-260 Production.
//!
//! Postings are unchanged: the racer's cities are usually the richest in
//! technologies we lack. A running steal is not a "safe" operation, so
//! `racer-keeps-the-spies` lets it finish before moving the Spy on.
//!
//! Off, every entry point returns its pre-gene answer.

use super::*;

/// The operation this gene runs.
pub(crate) const STEAL_OPERATION: &str = "steal_tech_boost";

/// Technologies a rival must know that we lack before a Spy is trained to
/// steal from it.
pub(crate) const STEAL_MIN_LACKED: usize = 3;

/// Turns since a city was last attacked under which it keeps its queue (the
/// denial claim's rule).
const STEAL_CLAIM_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// Whether the gene runs for this seat under `plan`: on, and aiming at
    /// Science, by the seat's victory target or the plan's strategy.
    pub(super) fn spies_steal_the_lead_on(&self, plan: &StrategicPlan) -> bool {
        self.spies_steal_the_lead
            && (self.victory_target == Some(VictoryTarget::Science)
                || plan.strategy == GrandStrategy::Science)
    }

    /// The Steal Tech Boost offered to `spy` where it stands: on the board's
    /// menu, and on the host's when the host sent one (a host menu without it
    /// means the host would refuse the order).
    pub(super) fn steal_the_lead_offered<'a>(
        g: &Game,
        spy: u32,
        legal: &'a [Action],
    ) -> Option<&'a Action> {
        let host_refuses = g
            .host_unit_facts
            .get(&spy)
            .and_then(|facts| facts.spy_missions.as_ref())
            .is_some_and(|menu| !menu.contains(STEAL_OPERATION));
        if host_refuses {
            return None;
        }
        legal.iter().find(
            |action| matches!(action, Action::SpyMission { mission, .. } if mission == STEAL_OPERATION),
        )
    }

    /// Under the gene, the steal `spy` runs before `instead` (the operation
    /// the pass would otherwise reach), or `None`. Never with the gene off or
    /// off a Science aim.
    pub(super) fn steal_the_lead_action(
        &self,
        g: &Game,
        plan: &StrategicPlan,
        spy: u32,
        legal: &[Action],
    ) -> Option<Action> {
        if !self.spies_steal_the_lead_on(plan) {
            return None;
        }
        if self.steal_yields_to_the_disrupt(g, spy, legal) {
            return None;
        }
        Self::steal_the_lead_offered(g, spy, legal).cloned()
    }

    /// Whether the steal stands aside for Disrupt Rocketry where `spy`
    /// stands: whenever the disruption is offered (by the board or the
    /// host), so the denial genes keep their pre-gene path there. A later
    /// gene that disrupts only at the racer's match point narrows this
    /// predicate, and the steal then fills the turns it withholds.
    pub(super) fn steal_yields_to_the_disrupt(&self, g: &Game, spy: u32, legal: &[Action]) -> bool {
        // `disrupt-waits-for-the-match-point`: where the gene withholds the
        // Disrupt (a racer's city before its last projects) the steal stands
        // aside too, so the Spy runs the pass's safe operation (Listening
        // Post or Gain Sources, 6-7% deaths against 50-64% for Disrupt and
        // steal there) and lives to the match point.
        Self::racer_spy_disrupt_offered(g, spy, legal) || self.disrupt_withheld_here(g, spy)
    }

    /// Whether `mission` stays withheld outside the racers' cities by
    /// `racer-keeps-the-spies`: its capture-risk list, less Steal Tech Boost
    /// under this gene.
    pub(super) fn racer_spy_withholds(&self, plan: &StrategicPlan, mission: &str) -> bool {
        Self::racer_spy_risky(mission)
            && !(mission == STEAL_OPERATION && self.spies_steal_the_lead_on(plan))
    }

    /// Count and journal a steal the pass just started. `instead` names what
    /// the Spy would have run without the gene.
    pub(super) fn steal_the_lead_note(&self, g: &mut Game, pid: usize, spy: u32, instead: &str) {
        *g.players[pid]
            .counters
            .entry("spy_steals".to_string())
            .or_insert(0) += 1;
        let Some((city_name, owner)) = g
            .spies
            .get(&spy)
            .and_then(|agent| agent.city)
            .and_then(|city| g.cities.get(&city))
            .map(|city| (city.name.clone(), city.owner))
        else {
            return;
        };
        let rival = g.players[owner].civ.to_string();
        let lacked = g.players[owner]
            .techs
            .iter()
            .filter(|tech| !g.players[pid].techs.contains(*tech))
            .count();
        think!(self.journal(), Military, Decision,
               "A Spy steals a Eureka in {rival}'s {city_name}";
               "spies-steal-the-lead: {rival} knows {lacked} technologies we lack; \
                Steal Tech Boost instead of {instead}");
    }

    /// The rivals worth a steal: met, living majors we are not allied with,
    /// a Campus of theirs on a tile we have seen, and at least
    /// [`STEAL_MIN_LACKED`] technologies we lack.
    pub(super) fn steal_targets(g: &Game, pid: usize) -> Vec<usize> {
        let ours = &g.players[pid].techs;
        let explored = &g.players[pid].explored;
        (0..g.players.len())
            .filter(|other| *other != pid)
            .filter(|other| {
                let player = &g.players[*other];
                player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && g.has_met(pid, *other)
                    && !g.are_allied(pid, *other)
                    && player
                        .techs
                        .iter()
                        .filter(|tech| !ours.contains(*tech))
                        .count()
                        >= STEAL_MIN_LACKED
                    && g.player_city_ids(*other).iter().any(|cid| {
                        g.cities[cid].districts.iter().any(|(district, tile)| {
                            g.district_family(*district) == "campus" && explored.contains(tile)
                        })
                    })
            })
            .collect()
    }

    /// Whether `item`, at the head of a queue, is routine work a Spy may
    /// displace: the denial claim's rule (no defensive building, military
    /// unit, Settler, Spy, Wonder, project or repair).
    fn steal_spy_displaces(g: &Game, item: &Item) -> bool {
        match item {
            Item::Building { building } => g
                .rules
                .buildings
                .get(building)
                .is_some_and(|spec| spec.outer_defense <= 0),
            Item::District { .. } => true,
            Item::Unit { unit } => g.rules.units.get(unit).is_some_and(|spec| {
                spec.class != "military" && *unit != "settler" && *unit != "spy"
            }),
            Item::Formation { .. }
            | Item::Wonder { .. }
            | Item::Repair { .. }
            | Item::Project { .. }
            | Item::Product { .. } => false,
        }
    }

    /// See the module, point 4: train the Spies the steals need. Exact no-op
    /// with the gene off or off a Science aim.
    pub(super) fn claim_queues_for_steal_spies(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.spies_steal_the_lead_on(plan) || self.base.spy_training_paused {
            return;
        }
        // Once a racer exists the denial claim trains to our capacity.
        if self.science_denial_trains_spies && !self.denial_spy_racers(g, pid).is_empty() {
            return;
        }
        let targets = Self::steal_targets(g, pid);
        let Some(&target) = targets.first() else {
            return;
        };
        let spy = Item::Unit {
            unit: crate::name!("spy"),
        };
        let city_ids = g.player_city_ids(pid);
        let queued = city_ids
            .iter()
            .filter(|cid| g.cities[cid].queue.contains(&spy))
            .count();
        let held = g.spy_agents(pid);
        let capacity = g.spy_capacity(pid).max(0) as usize;
        // One Spy in training at a time: the host trains them one by one.
        if queued > 0 || held >= capacity {
            return;
        }
        let mut candidates: Vec<(f64, u32)> = city_ids
            .iter()
            .copied()
            .filter(|cid| {
                let city = &g.cities[cid];
                let attacked = city.last_attacked > 0
                    && g.turn.saturating_sub(city.last_attacked) <= STEAL_CLAIM_ATTACK_MEMORY;
                plan.threatened_city != Some(*cid)
                    && !attacked
                    && !self.launch_pad_holds_city(g, pid, *cid)
                    && city
                        .queue
                        .first()
                        .is_none_or(|head| Self::steal_spy_displaces(g, head))
                    && g.can_produce(pid, *cid, &spy)
            })
            .map(|cid| (g.city_yields(cid).production, cid))
            .collect();
        candidates.sort_by(|(left, left_city), (right, right_city)| {
            right.total_cmp(left).then(left_city.cmp(right_city))
        });
        let Some((production, city)) = candidates.first().copied() else {
            return;
        };
        let city_name = g.cities[&city].name.clone();
        let rival = g.players[target].civ.to_string();
        let others = targets.len() - 1;
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: spy.clone(),
            },
        )
        .is_ok()
        {
            *g.players[pid]
                .counters
                .entry("steal_spies_trained".to_string())
                .or_insert(0) += 1;
            think!(self.journal(), Military, Decision,
                   "{city_name} trains a Spy to steal Eurekas";
                   "spies-steal-the-lead: {held} held of {capacity}; {rival} and {others} \
                    other rival(s) know technologies we lack; {production:.0} Production a \
                    turn here");
        }
    }
}

#[cfg(test)]
mod tests;
