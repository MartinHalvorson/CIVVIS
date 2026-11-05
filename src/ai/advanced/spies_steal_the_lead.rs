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
//! Postings are unchanged: the racer's cities are usually the richest in
//! technologies we lack. A running steal is not a "safe" operation, so
//! `racer-keeps-the-spies` lets it finish before moving the Spy on.
//!
//! Off, every entry point returns its pre-gene answer.

use super::*;

/// The operation this gene runs.
pub(crate) const STEAL_OPERATION: &str = "steal_tech_boost";

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
        Self::racer_spy_disrupt_offered(g, spy, legal)
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
}

#[cfg(test)]
mod tests;
