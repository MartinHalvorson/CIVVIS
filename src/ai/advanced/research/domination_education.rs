//! `education-follows-the-library`: a Domination seat with a Campus and a
//! Library standing in one of its cities researches Education, so its
//! Campuses can take Universities.
//!
//! **Why (live Deity Gran Colombia, 4-player Tiny Pangaea, Online, the 26
//! games of 2026-11-11 19:57Z-23:50Z).** At t100 the seat made 41.5 Science
//! against the best rival's 154 (24 technologies against 41.5). Taking out
//! the Deity AI's +32% Science, the rival makes 1.74 Science a citizen and we
//! make 1.0. Campus coverage is not the gap by then: we stand 0.66 Campuses a
//! city against 0.61 in the rival cities we see. What stands in them is:
//! our Education landed at a median t92 (never, by the game's end, in 6 of
//! 26 games) against the first rival's median t55, so at t100 our median
//! Campus count was 4 with 2 Libraries and **no University**. The research
//! chooser never forces Education on this lane: from t60 to t100 its 256
//! decisions were the standing army's modernization (74), the conquest's own
//! breakthrough (49), barbarian catch-up (25) and production unlocks (8), and
//! the remaining 93 went to the generic argmax ("worth N to the conquest
//! plan"), which reaches Education late because its value is the University
//! the lane does not weigh. A University is 125 Production at Online speed
//! for +4 Science and +1 Housing; Education cost 156-195 Science.
//!
//! **The gene.** While the active victory target is Domination, Education is
//! not known, and at least [`EDUCATION_READY_CAMPUS_CITIES`] owned city holds a
//! Campus with a Library (everything a University needs but the technology),
//! Education is the research goal in the Campus unlock's slot: behind every
//! military, barbarian, wall, siege, modernization (but see below),
//! production and luxury-connection goal, ahead of the bargains and the
//! lane's argmax. The chooser walks its prerequisites (Mathematics,
//! Apprenticeship) the usual way. The Culture lane has its own Education goal
//! (`culture_education.rs`); this one is the same contract for the conquest
//! seat. Off, nothing changes. Grep the journal for
//! `education-follows-the-library:`.
//!
//! **Ahead of the wartime modernization chain.** That slot starved under a
//! long war. Live 20261112T013737Z claimed its Library at t58, took
//! Mathematics for the gene at t61, then spent t65-t95 on the standing army's
//! modernization (Iron Working through Metal Casting) at 15-20 Science a
//! turn, and Education came at t98. Over the 29 Gran Colombia games since
//! 2026-11-11 19:57Z that stood a Campus with a Library before Education (a
//! median 32 turns apart), the modernization held research for 238 of the
//! 298 turns Education was itself researchable (decisive window 21,
//! barbarians 14, the argmax the gene already outranks 23), and for 228 more
//! turns while it was one (184), two (28) or three (16) technologies short.
//! Production unlocks (28 turns), the conquest's siege research (11) and
//! fuel (0) barely touched it. So while a modernization goal is live and
//! Education is at most [`EDUCATION_AHEAD_OF_MODERNIZATION_SHORT`]
//! technologies short (Mathematics and Apprenticeship), the gene's goal
//! takes the modernization's slot: still behind the decisive window,
//! barbarian, wall, stone and first-capital goals, a war plan's rush,
//! launches, fuel and production unlocks. The journal adds `ahead of the
//! wartime modernization chain toward <tech>`.

use super::*;

/// How many technologies Education may still wait on (Mathematics and
/// Apprenticeship) for its path to go ahead of the wartime modernization
/// chain. Three short is a Horseback Riding detour first; the gene's own slot
/// keeps it then.
pub(crate) const EDUCATION_AHEAD_OF_MODERNIZATION_SHORT: usize = 2;

/// The owned cities with a Campus and a Library that open the goal. One is
/// enough: Education also leads the lane to its later technologies, and the
/// first University repays its Production in about 30 turns.
pub(crate) const EDUCATION_READY_CAMPUS_CITIES: usize = 1;

impl AdvancedAi {
    /// The cities of `pid` with a Campus and a Library standing: the
    /// Universities Education would open.
    pub(super) fn education_ready_campus_cities(g: &Game, pid: usize) -> usize {
        let campus = crate::name!("campus");
        let library = crate::name!("library");
        g.cities
            .values()
            .filter(|city| city.owner == pid)
            .filter(|city| {
                city.districts
                    .keys()
                    .any(|district| g.district_family(*district) == campus)
            })
            .filter(|city| city.buildings.contains(&library))
            .count()
    }

    /// `education-follows-the-library`: Education while a Domination seat
    /// holds a Campus with a Library and does not know it. `None` with the
    /// gene off, on any other victory target, once Education is known, and
    /// before a Library stands beside a Campus.
    pub(super) fn domination_education_goal(&self, g: &Game, pid: usize) -> Option<&'static str> {
        if !self.education_follows_the_library
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || g.players[pid].techs.contains(&crate::name!("education"))
        {
            return None;
        }
        (Self::education_ready_campus_cities(g, pid) >= EDUCATION_READY_CAMPUS_CITIES)
            .then_some("education")
    }

    /// The technologies `pid` still lacks on the way to Education, not
    /// counting Education itself.
    pub(super) fn education_missing_prerequisites(g: &Game, pid: usize) -> usize {
        let held = &g.players[pid].techs;
        g.rules
            .tech_ancestors
            .get("education")
            .map_or(0, |ancestors| {
                ancestors
                    .iter()
                    .filter(|tech| !held.contains(&Name::new(tech)))
                    .count()
            })
    }

    /// `education-follows-the-library`, ahead of the wartime modernization
    /// chain: the gene's `education` goal while a `modernization` goal is
    /// live and Education is at most [`EDUCATION_AHEAD_OF_MODERNIZATION_SHORT`]
    /// technologies short. `None` otherwise, which leaves the gene in its own
    /// slot.
    pub(super) fn domination_education_ahead_of_modernization(
        g: &Game,
        pid: usize,
        education: Option<&'static str>,
        modernization: Option<&str>,
    ) -> Option<&'static str> {
        education.filter(|_| {
            modernization.is_some()
                && Self::education_missing_prerequisites(g, pid)
                    <= EDUCATION_AHEAD_OF_MODERNIZATION_SHORT
        })
    }
}

#[cfg(test)]
mod tests;
