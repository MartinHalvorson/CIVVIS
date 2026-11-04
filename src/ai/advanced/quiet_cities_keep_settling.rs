//! `quiet-cities-keep-settling`: a threatened Recovery holds the Settler
//! only where the war can reach.
//!
//! **Why (live Lv 7 Immortal, Gran Colombia at Standard speed, 41 games
//! 2026-11-04).** Rivals grew from a median 5 to 8 cities between turns 100
//! and 200, and the rival cities we could see were only 7-9% captures, so the
//! growth came from Settlers. We stayed at 4-5 cities. The governor's own
//! Settler journal (`settler_admission_journal.rs`) names one gate above all
//! others from turn 80 on: "the recovery plan holds Settlers while a city is
//! threatened". It accounts for 40% of the logged holds at turns 80-119, 63%
//! at 120-199 and 80% after 200. That gate is
//! `threatened_recovery_holds_settlers`: while a major war has one city
//! threatened, a Settler scores −10 000 in every city. Yet in those Recovery
//! turns 52% of our city-turns had no visible soldier of an enemy at war
//! within 7 tiles. In 74% of them at least one city was that quiet.
//!
//! The hold was written for a city that is itself in danger. Live run
//! `civvis-20260815T064852Z` had an empty Ostia start a Settler while the
//! empire was losing cities. This gene keeps the hold for that city and for
//! the cities around it. It lifts the hold for one Settler at a time from a
//! city the war cannot reach this turn. Such a city must stand more than
//! [`THREAT_RELIEF_RADIUS`] tiles from the threatened city. No hostile
//! military unit we can see, barbarians included, may stand within
//! [`QUIET_CITY_HOSTILE_RADIUS`] tiles. The city must have no remembered
//! pressure and no hit in the last [`QUIET_CITY_ATTACK_MEMORY`] turns. The
//! empire's land army must also outnumber its cities, so the Settler's
//! escort cannot be a city's last defender. -af found 3 of 4 early capital
//! losses came right after a settler expedition drew the capital's last
//! soldier.
//!
//! The site the Settler would walk to is held to the same test, so the new
//! city is not founded into the war. Every other Settler gate (pipeline,
//! population, window, site gate, loyalty) applies unchanged after this one.
//! Off, nothing here is read.

use super::*;

/// No hostile military unit we can see may stand within this many tiles of
/// a quiet city, or of the site its Settler would found. A land unit reaches
/// about two to three tiles a turn, so eight leaves three turns or more.
pub(crate) const QUIET_CITY_HOSTILE_RADIUS: i32 = 8;

/// Turns after a hit on the city during which it is not quiet. Matches the
/// four-turn `threatened` window of the production scorer.
pub(crate) const QUIET_CITY_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// Whether `cid` may train a Settler while a threatened Recovery holds
    /// Settlers everywhere else. `false` with the gene off. See the module
    /// documentation.
    pub(super) fn quiet_city_keeps_settling(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
    ) -> bool {
        if !self.quiet_cities_keep_settling || counts.settlers > 0 {
            return false;
        }
        let Some(threatened) = plan.threatened_city else {
            return false;
        };
        if threatened == cid {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.last_attacked > 0
            && g.turn.saturating_sub(city.last_attacked) <= QUIET_CITY_ATTACK_MEMORY
        {
            return false;
        }
        if g
            .cities
            .get(&threatened)
            .is_some_and(|front| g.wdist(city.pos, front.pos) <= THREAT_RELIEF_RADIUS)
        {
            return false;
        }
        let city_count = g.player_city_ids(pid).len();
        let land_military = counts
            .military
            .saturating_sub(counts.naval + counts.aircraft);
        if land_military <= city_count {
            return false;
        }
        if self.remembered_city_pressure(g, pid, cid) > 0.0 {
            return false;
        }
        Self::quiet_ground(g, pid, city.pos)
    }

    /// Whether a quiet city's Settler may found at `site`: no hostile military
    /// unit we can see within [`QUIET_CITY_HOSTILE_RADIUS`] and the site
    /// outside the threatened city's relief ring.
    pub(super) fn quiet_settle_site(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        site: Pos,
    ) -> bool {
        if plan
            .threatened_city
            .and_then(|id| g.cities.get(&id))
            .is_some_and(|front| g.wdist(site, front.pos) <= THREAT_RELIEF_RADIUS)
        {
            return false;
        }
        Self::quiet_ground(g, pid, site)
    }

    /// No hostile military unit `pid` can see stands within
    /// [`QUIET_CITY_HOSTILE_RADIUS`] of `pos`.
    fn quiet_ground(g: &Game, pid: usize, pos: Pos) -> bool {
        let visible = g.player_vision_frame(pid);
        Self::visible_hostile_strengths(g, pid, &visible)
            .iter()
            .all(|(hostile, _)| g.wdist(pos, *hostile) > QUIET_CITY_HOSTILE_RADIUS)
    }
}

#[cfg(test)]
mod tests;
