//! `seen-pad-starts-the-denial`: the space race starts at the first Spaceport
//! we see standing, not at the first launch, and the Spaceport takes the Spy.
//!
//! ## Why
//!
//! Lv 8 Deity, Diplomatic Victory off (Tournament 7, 2026-11-23..26): 59
//! reviewed games, 2 won; 40 of the 57 losses were a rival's Science Victory,
//! t172-t203. The reviews read "the first denial came at t180". The logs say
//! otherwise: in the 11 of those losses whose runs are still on disk, the
//! denial network trained and posted Spies from t110-t154 and ordered its
//! first Disrupt Rocketry at t130-t156. (The reviewer reads only a game's last
//! 25 decisions, and of civvis-t7-20261125T065313Z only its `-cont1` segment,
//! which starts at t179.)
//!
//! What came late was the first Disrupt Rocketry against the rival that won.
//! The host offers the operation only against a completed Spaceport, and from
//! the turn we first saw the winner's completed Spaceport to our first Disrupt
//! Rocketry against it took 9 turns (Kongo, 065313Z), 22 (Macedon, 094034Z),
//! 9 (Germany, 095636Z), 18 (Russia, 122122Z), 5 (Canada, 152046Z) and 1
//! (Scotland, 092547Z). Macedon's first pad was pillaged on the turn it
//! launched the Exoplanet Expedition (t168), Germany's two turns after
//! (t156); nothing at a pad stops a launched expedition. Meanwhile, between
//! the winner's Earth Satellite and its expedition, 319 of 496 Spy-turns (64%)
//! were spent outside the winner's cities: 191 with another racer within a
//! project of the lead (which did not win), 88 with racers further back, 40 at
//! home. In Macedon's game two Spies stood in Cahokia and Alexandria Troas,
//! cities with no Spaceport, stealing Eurekas six times from t133 to t158
//! while Pella's pad stood from t142; in Germany's, a Spy ran Listening Posts
//! in Ulm from t124 to t146 while Dortmund's pad stood from t143. The pad
//! search (`racer-spies-search-for-the-pad`) and the green Spy's disruption
//! (`disrupt-before-the-sources`) wait for the racer's first project, and the
//! cover (`denial-spies-cover-every-racer`) gives every racer within a project
//! one Spy before any gets a second.
//!
//! Pillaging the winner's pads does slow it: from Earth Satellite to
//! Exoplanet Expedition took 28-40 turns in the four games where we pillaged
//! its pads two to six times before the launch, against 16-36 (mean 26) in
//! the six where we pillaged them once or not at all.
//!
//! ## What the gene does
//!
//! The *racers* this gene answers are the met, living, non-team majors that
//! stand a completed Spaceport we have seen (the board carries a foreign
//! district only once it is complete) and
//!
//! - have not launched the Exoplanet Expedition (nothing at a pad stops a
//!   launched one),
//! - are within one project of the racer that has landed the most and has
//!   not launched, and
//! - lead us in the space race: more space progress than ours, or as much and
//!   more Science a turn. Racers we lead are not answered.
//!
//! They are ranked by projects landed, a racer one project behind counting
//! level when it makes [`RACER_SCIENCE_EDGE`] times the Science a turn of the
//! best racer ahead, and then by Science a turn. Over the 228 race-turns of
//! the 10 losses above with two or more such racers, that ranking named the
//! eventual winner first on 179 (79%): projects then Science, the cover's
//! order, 161 (71%); the edge then Production, `racer-keeps-the-spies`'
//! leader, 154 (68%); Production alone 108 (47%). Russia (122122Z) landed
//! its Satellite at t149, a project behind Vietnam and the Inca, at 377
//! Science a turn against Vietnam's 276, and won; our Spies searched the
//! Inca's cities and stood in Vietnam's until the first Disrupt Rocketry on
//! St. Petersburg at t163.
//!
//! Every rule reads nothing when the Science Victory is off. Against those
//! racers, in that order:
//!
//! 1. **Post.** A Spy standing outside every racer's pad cities (at home or
//!    abroad, but not in a culture racer's city, where the heist keeps it)
//!    goes to the best-ranked racer's pad city that no Spy of ours holds, the
//!    nearest first. One Spy to a pad city.
//! 2. **Release.** A Spy whose order is still running (`spy-mission-patience`)
//!    gives way to the post when that order is only a Listening Post or Gain
//!    Sources, the operations `racer-keeps-the-spies` already lets go.
//! 3. **Disrupt.** A Spy standing in a racer's pad city runs Disrupt Rocketry
//!    when the board or the host offers it, before the green Spy's Gain Sources
//!    training and the Eureka steal, from the first seen pad on, not the first
//!    launch. A long-odds disruption still waits for its sources under
//!    `disrupt-waits-for-the-sources`, which runs first.
//!
//! The trigger is a rival with a seen Spaceport or its first launch: a
//! completed pad is the first thing a Spy can act on. It reads the same in
//! every lane; Religious and Culture seats lose to the same launches.
//!
//! `Kind::OptIn`. With the gene off [`AdvancedAi::seen_pad_racers`] is empty
//! and every rule below returns before it reads the board.

use std::collections::BTreeSet;

use super::racer_keeps_the_spies::RACER_SCIENCE_EDGE;
use super::AdvancedAi;
use crate::game::{Action, Game};
use crate::shared_sort::SharedSort;
use crate::think;

/// Operations a running order may be that the post interrupts: the ones that
/// hold a Spy waiting rather than doing something it cannot take back.
const RELEASED_OPERATIONS: [&str; 2] = ["listening_post", "gain_sources"];

/// A racer this gene answers and its pad cities: the cities a Spy can disrupt
/// one of its standing, seen Spaceports from.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SeenPadRacer {
    /// The racing major.
    pub(crate) racer: usize,
    /// Its pad cities (`every_pad_cities`), less those where the host refused
    /// us the disruption with the pad standing.
    pub(crate) pads: BTreeSet<u32>,
}

impl AdvancedAi {
    /// Whether this seat leads the space race against `racer`: more space
    /// progress (`racer_space_progress`: projects landed plus the share of a
    /// launched expedition's trip), or as much and at least its Science a turn.
    pub(crate) fn seen_pad_we_lead(g: &Game, pid: usize, racer: usize) -> bool {
        let ours = Self::racer_space_progress(g, pid);
        let theirs = Self::racer_space_progress(g, racer);
        ours > theirs + 1e-9
            || ((ours - theirs).abs() <= 1e-9
                && Self::seat_science_per_turn(g, pid) >= Self::seat_science_per_turn(g, racer))
    }

    /// The racers this gene answers, best first; see the module header. Empty
    /// with the gene off or the Science Victory off.
    pub(crate) fn seen_pad_racers(&self, g: &Game, pid: usize) -> Vec<SeenPadRacer> {
        if !self.seen_pad_starts_the_denial || !g.victory_conditions.science {
            return Vec::new();
        }
        let pads_of = |racer: usize| {
            let mut pads = Self::every_pad_cities(g, pid, &BTreeSet::from([racer]));
            pads.retain(|cid| !self.science_denial_refused_pads.contains_key(cid));
            pads
        };
        let racers: Vec<usize> = g
            .players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !g.same_team(pid, rival.id)
                    && g.has_met(pid, rival.id)
                    && !rival.science_projects.contains("exoplanet_expedition")
            })
            .map(|rival| rival.id)
            .filter(|rival| {
                Self::science_denial_stages(g, *rival) >= 1 || !pads_of(*rival).is_empty()
            })
            .collect();
        let stages = |racer: usize| Self::science_denial_stages(g, racer);
        let science = |racer: usize| Self::seat_science_per_turn(g, racer);
        let Some(top) = racers.iter().map(|racer| stages(*racer)).max() else {
            return Vec::new();
        };
        let top_science = racers
            .iter()
            .filter(|racer| stages(**racer) == top)
            .map(|racer| science(*racer))
            .fold(0.0_f64, f64::max);
        // A racer one project behind with the Science edge counts level.
        let level = |racer: usize| {
            let landed = stages(racer);
            if landed + 1 == top
                && top_science > 0.0
                && science(racer) >= RACER_SCIENCE_EDGE * top_science
            {
                top
            } else {
                landed
            }
        };
        let mut ranked: Vec<(usize, usize, f64)> = racers
            .into_iter()
            .filter(|racer| stages(*racer) + 1 >= top && !Self::seen_pad_we_lead(g, pid, *racer))
            .map(|racer| (racer, level(racer), science(racer)))
            .collect();
        ranked.sort_by_shared(|left, right| {
            right
                .1
                .cmp(&left.1)
                .then_with(|| right.2.total_cmp(&left.2))
                .then_with(|| left.0.cmp(&right.0))
        });
        ranked
            .into_iter()
            .filter_map(|(racer, _, _)| {
                let pads = pads_of(racer);
                (!pads.is_empty()).then_some(SeenPadRacer { racer, pads })
            })
            .collect()
    }

    /// The racer whose pad city `cid` is, if any.
    pub(crate) fn seen_pad_racer_at(racers: &[SeenPadRacer], cid: Option<u32>) -> Option<usize> {
        let cid = cid?;
        racers
            .iter()
            .find(|entry| entry.pads.contains(&cid))
            .map(|entry| entry.racer)
    }

    /// Rule 1: the pad city `spy`, standing at `current_city`, goes to, and
    /// the racer it belongs to. `None` when the Spy stands in a racer's pad
    /// city already or in one of `keep`'s cities, when every pad city is
    /// held, or with the gene off (`racers` empty). Only cities the host lets
    /// us post to (`legal`) count.
    pub(crate) fn seen_pad_post(
        g: &Game,
        pid: usize,
        spy: u32,
        current_city: Option<u32>,
        racers: &[SeenPadRacer],
        keep: &BTreeSet<usize>,
        legal: &[Action],
    ) -> Option<(u32, usize)> {
        if racers.is_empty() || Self::seen_pad_racer_at(racers, current_city).is_some() {
            return None;
        }
        let here = current_city.and_then(|city| g.cities.get(&city));
        if here.is_some_and(|city| keep.contains(&city.owner)) {
            return None;
        }
        let from = here.map(|city| city.pos);
        racers.iter().find_map(|entry| {
            entry
                .pads
                .iter()
                .copied()
                .filter(|cid| {
                    Self::spy_holding(g, pid, *cid).is_none()
                        && legal.iter().any(|action| {
                            matches!(action, Action::AssignSpy { spy: id, city } if *id == spy && city == cid)
                        })
                })
                .min_by_key(|cid| {
                    (
                        from.map_or(0, |from| g.wdist(from, g.cities[cid].pos)),
                        *cid,
                    )
                })
                .map(|cid| (cid, entry.racer))
        })
    }

    /// Rule 2: whether `spy`, whose order is still running, gives way to the
    /// post: the order is only a Listening Post or Gain Sources and
    /// [`Self::seen_pad_post`] has a pad city for it. Never with the gene off.
    pub(crate) fn seen_pad_releases(
        g: &Game,
        pid: usize,
        spy: u32,
        racers: &[SeenPadRacer],
        keep: &BTreeSet<usize>,
    ) -> bool {
        if racers.is_empty() {
            return false;
        }
        let Some(agent) = g.spies.get(&spy) else {
            return false;
        };
        let running = g
            .host_unit_facts
            .get(&spy)
            .and_then(|facts| facts.spy_operation.clone())
            .or_else(|| agent.mission.as_ref().map(|mission| mission.kind.clone()));
        running.is_some_and(|operation| RELEASED_OPERATIONS.contains(&operation.as_str()))
            && Self::seen_pad_post(
                g,
                pid,
                spy,
                agent.city,
                racers,
                keep,
                &g.legal_spy_actions(pid, spy),
            )
            .is_some()
    }

    /// Rule 3: the Disrupt Rocketry `spy` runs where it stands, in a racer's
    /// pad city; `every_pad_disrupt` files a pad the host names under this
    /// city first. `None` elsewhere or with the gene off.
    pub(crate) fn seen_pad_disrupt(
        g: &mut Game,
        pid: usize,
        spy: u32,
        racers: &[SeenPadRacer],
    ) -> Option<Action> {
        let here = g.spies.get(&spy)?.city;
        let racer = Self::seen_pad_racer_at(racers, here)?;
        Self::every_pad_disrupt(g, pid, spy, &BTreeSet::from([racer]))
    }

    /// The journal line of a post or a disruption under the gene.
    pub(crate) fn seen_pad_note(&self, g: &Game, cid: u32, racer: usize, disrupting: bool) {
        let Some(city) = g.cities.get(&cid) else {
            return;
        };
        let civ = &g.players[racer].civ;
        let landed = Self::science_denial_stages(g, racer);
        let science = Self::seat_science_per_turn(g, racer);
        if disrupting {
            think!(self.journal(), Military, Decision,
                   "Disrupting the Spaceport at {}", city.name;
                   "seen-pad-starts-the-denial: {civ} has landed {landed} space project(s) at \
                    {science:.0} Science a turn and stands this pad; it is disrupted before the \
                    Spy trains or steals";
                   city.pos);
        } else {
            think!(self.journal(), Military, Decision,
                   "A Spy goes to {civ}'s {}, a Spaceport we have seen", city.name;
                   "seen-pad-starts-the-denial: {civ} has landed {landed} space project(s) at \
                    {science:.0} Science a turn, leads us in the space race and stands a pad \
                    there no Spy of ours holds";
                   city.pos);
        }
    }
}

#[cfg(test)]
mod tests;
