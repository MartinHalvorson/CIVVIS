//! `rear-cities-grow-through-the-war-hold`: while
//! `threatened-war-holds-the-claims` holds the economy's idle-queue claims,
//! the growth claims still run for the cities away from the front.
//!
//! **Why (about 34 live Gran Colombia Deity Domination games, 2026-11-11/12).**
//! The hold fired on a median of about 36 turns a game (up to 87), mostly
//! from t50, because the seat was at war with a stronger Deity major and
//! `home_city_threatened` is true for the whole empire when any one city is
//! threatened or the plan is Recovery. While it holds, every city skips the
//! Granary, Aqueduct, Neighbourhood and backlog Builder claims. 635
//! city-turns were lost to it: 31% of all city-turns at the housing cap with
//! a Granary buildable, and 49% of those in t61-100. 51.6% of our worked
//! tiles were still unimproved at t100, and our cities averaged 5.6 citizens
//! at t100 against the rivals' 8.4. The threat is local, to one city near the
//! front, but the hold covers the whole empire.
//!
//! **The gene.** While the hold is on, these claims still run, but only for
//! rear cities: the Granary (`claim_bound_granaries`), the Aqueduct
//! (`claim_capped_aqueduct`), the Neighbourhood (`claim_capped_neighborhood`),
//! the backlog Builder (`claim_backlog_builder`), the Campus building
//! (`claim_campus_building`), a free slot's first Campus district
//! (`claim_free_slot_campus`, added after the live city-loss read: the
//! seat's first Campus came at a median t55 and its science ran 40 against
//! the best rival's 191 at t100), the parallel Library beside the backlog
//! Builder (`claim_parallel_library_backlog`), and the idle-route Trader
//! (`claim_trader_for_idle_route`). For the Trader, the rear city is the one
//! that would train it. A city is rear when the plan does not name it as the
//! threatened city, it was not attacked within
//! [`super::hint_runs_the_claims::HINT_ATTACK_MEMORY`] turns, and no visible
//! or remembered hostile presses it (the pressure test
//! `AdvancedAi::threatened_city` uses). Front cities stay under the hold, so
//! their idle queues still go to the war target. The Industrial Zone,
//! Factory, culture, Monument and repair claims stay held everywhere.
//!
//! Each claim keeps its own rules, its order and its caps. The only change is
//! that the claim's city loop skips cities that are not rear, so every city
//! a claim takes still passes every check it passes today. With the gene off,
//! or with no hold, nothing changes.

use super::*;

impl AdvancedAi {
    /// The cities of `pid` away from the front: not the plan's threatened
    /// city, not attacked within
    /// [`super::hint_runs_the_claims::HINT_ATTACK_MEMORY`] turns, and under no
    /// visible or remembered hostile pressure.
    pub(super) fn rear_cities_under_the_war_hold(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> BTreeSet<u32> {
        let memory = super::hint_runs_the_claims::HINT_ATTACK_MEMORY;
        let visible = g.player_vision_frame(pid);
        let hostiles = Self::visible_hostile_strengths(g, pid, &visible);
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                let city = &g.cities[cid];
                let hit =
                    city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= memory;
                plan.threatened_city != Some(*cid)
                    && !hit
                    && self
                        .city_pressure_with_belief_from_hostiles(g, pid, *cid, &visible, &hostiles)
                        <= 0.0
            })
            .collect()
    }

    /// Opens the growth claims to the rear cities for this idle-queue pass,
    /// when the war holds the claims and the gene is on. Returns whether any
    /// rear city is open. Otherwise it clears the set and returns false, and
    /// every claim reads every city as it does today.
    pub(super) fn open_rear_cities_under_the_war_hold(
        &mut self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        war_holds: bool,
    ) -> bool {
        self.rear_cities_open_under_the_hold = None;
        if !war_holds || !self.rear_cities_grow_through_the_war_hold {
            return false;
        }
        let rear = self.rear_cities_under_the_war_hold(g, pid, plan);
        if rear.is_empty() {
            return false;
        }
        think!(self.journal(), Cities, Detail,
               "Rear cities grow through the war hold";
               "{} of {} cities are away from the front; \
                Granary/Aqueduct/Neighbourhood/Builder/Campus/Trader claims stay open there, first Campus included",
               rear.len(), g.player_city_ids(pid).len());
        self.rear_cities_open_under_the_hold = Some(rear);
        true
    }

    /// Ends the pass [`Self::open_rear_cities_under_the_war_hold`] opened.
    pub(super) fn close_rear_cities_under_the_war_hold(&mut self) {
        self.rear_cities_open_under_the_hold = None;
    }

    /// Whether a growth claim may take `cid` in this pass: always, unless the
    /// war's hold is open only to the rear cities and `cid` is not one of
    /// them.
    pub(super) fn claim_open_behind_the_front(&self, cid: u32) -> bool {
        self.rear_cities_open_under_the_hold
            .as_ref()
            .is_none_or(|rear| rear.contains(&cid))
    }
}

#[cfg(test)]
mod tests;
