//! `denial-spies-cover-every-racer`: the denial's Spies cover every space
//! racer within one project of the lead before a second Spy goes to any.
//!
//! ## Why
//!
//! Live Immortal civvis-20261115T212021Z (Brazil, Culture seat) lost at turn
//! 212 to India's Science Victory with 304 of the 326 tourists it needed.
//! Three racers ran level: India landed its third project at 176 while Persia
//! and Scythia stood on two, all three stood on three from 190, and all three
//! launched the Exoplanet Expedition by 198 (Science Victory Points at 210:
//! India 19, Persia 13, Scythia 12, of 25). `racer-keeps-the-spies` named
//! Persia the leader on every post from 156 to 198 ("Persia leads the space
//! race"): a racer one project behind counts level when it makes
//! [`RACER_SCIENCE_EDGE`] times the Science of the racer ahead (Persia
//! 433-538 a turn against India's 323-370), and level racers are then told
//! apart by Production. Of our 13 Disrupt Rocketry, 9 went to Persia (7
//! landed, its pads pillaged), 1 to India (Delhi at 163, failed) and none to
//! Scythia before 205. India won. Disrupt Rocketry landed about half the time
//! at any turn across that night's games, so spreading the Spies costs no
//! success odds.
//!
//! ## What the gene does
//!
//! With `racer-keeps-the-spies` naming the racers:
//!
//! 1. the racers are ranked by space progress
//!    ([`AdvancedAi::racer_space_progress`]): the projects landed, plus the
//!    share of the expedition's trip covered once the Exoplanet Expedition is
//!    under way. Science a turn only breaks a tie, then the lower seat. No
//!    Science edge lifts a racer that is a project behind;
//! 2. every racer within [`RACER_COVER_STEP`] of the leader's progress (one
//!    project, or the whole trip once every one of them has launched) is a
//!    co-leader;
//! 3. each Spy answers one co-leader ([`AdvancedAi::racer_cover_target`]). A
//!    Spy standing in a co-leader's cities answers that co-leader unless it
//!    can leave for the best-ranked co-leader no Spy of ours covers: always
//!    when another Spy of ours shares its co-leader, and when it is alone
//!    there only for a co-leader ahead of it in progress (Science alone never
//!    moves it, so a tie that flips from turn to turn cannot walk it back and
//!    forth). Either way it stays while it can run the Disrupt Rocketry
//!    offered where it stands. Any other Spy answers the co-leader with the
//!    fewest Spies of ours, the best-ranked on a tie.
//!
//! The racer a Spy answers then takes the leader's part in every rule of
//! `racer-keeps-the-spies` and `racer-spies-search-for-the-pad` for that Spy:
//! its pad city seen or found first, else the capital and the cities nearest
//! it, the pad search's next city, when a Listening Post no longer holds it,
//! and when it disrupts. A racer more than one project ahead of every other
//! racer is the only co-leader and gets every Spy, as before. `Kind::OptIn`;
//! off, the co-leaders are empty and the leader every Spy answers is
//! `racer_spy_leader`'s.

use super::*;

/// `denial-spies-cover-every-racer`: how far behind the leader's space
/// progress a racer still counts as a co-leader: one project, or the whole
/// trip of a launched Exoplanet Expedition.
pub(crate) const RACER_COVER_STEP: f64 = 1.0;

impl AdvancedAi {
    /// A racer's space progress: its space projects landed, plus the share of
    /// the Exoplanet Expedition's trip covered once it is under way (its
    /// Science Victory Points against those it needs).
    pub(crate) fn racer_space_progress(g: &Game, rival: usize) -> f64 {
        let landed = Self::science_denial_stages(g, rival) as f64;
        if !g.players[rival]
            .science_projects
            .contains("exoplanet_expedition")
        {
            return landed;
        }
        let needed = g.science_victory_points_needed(rival);
        if needed <= 0.0 {
            return landed;
        }
        landed + (g.science_victory_points(rival) / needed).clamp(0.0, 1.0)
    }

    /// `racers` ranked by space progress, then Science a turn, then the lower
    /// seat.
    pub(crate) fn racer_cover_ranked(g: &Game, racers: &BTreeSet<usize>) -> Vec<usize> {
        let mut ranked: Vec<(usize, f64, f64)> = racers
            .iter()
            .map(|rival| {
                (
                    *rival,
                    Self::racer_space_progress(g, *rival),
                    Self::seat_science_per_turn(g, *rival),
                )
            })
            .collect();
        ranked.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| right.2.total_cmp(&left.2))
                .then_with(|| left.0.cmp(&right.0))
        });
        ranked.into_iter().map(|(rival, _, _)| rival).collect()
    }

    /// The co-leaders of the space race, best first: every racer within
    /// [`RACER_COVER_STEP`] of the leader's progress. Empty with the gene off
    /// or no racer.
    pub(crate) fn racer_cover_co_leaders(&self, g: &Game, racers: &BTreeSet<usize>) -> Vec<usize> {
        if !self.denial_spies_cover_every_racer {
            return Vec::new();
        }
        let ranked = Self::racer_cover_ranked(g, racers);
        let Some(top) = ranked
            .first()
            .map(|leader| Self::racer_space_progress(g, *leader))
        else {
            return Vec::new();
        };
        ranked
            .into_iter()
            .filter(|rival| Self::racer_space_progress(g, *rival) >= top - RACER_COVER_STEP - 1e-9)
            .collect()
    }

    /// Spies of `pid` other than `spy`, not captured, standing in (or bound
    /// for) a city of `racer`.
    pub(crate) fn racer_cover_spies(g: &Game, pid: usize, spy: u32, racer: usize) -> usize {
        g.spies
            .values()
            .filter(|other| {
                other.owner == pid
                    && other.id != spy
                    && other.captured_by.is_none()
                    && other
                        .city
                        .and_then(|city| g.cities.get(&city))
                        .is_some_and(|city| city.owner == racer)
            })
            .count()
    }

    /// The racer `spy` answers: `leader` (the stock pick) with the gene off
    /// (`co_leaders` empty); otherwise the co-leader it covers. See the module
    /// header for the rule; `legal` and `refile` say whether it can run the
    /// Disrupt Rocketry offered where it stands
    /// ([`Self::racer_spy_disrupt_runnable`]).
    pub(crate) fn racer_cover_target(
        g: &Game,
        spy: u32,
        leader: Option<usize>,
        co_leaders: &[usize],
        legal: &[Action],
        refile: &BTreeSet<usize>,
    ) -> Option<usize> {
        let Some(first) = co_leaders.first().copied() else {
            return leader;
        };
        if co_leaders.len() == 1 {
            return Some(first);
        }
        let Some(agent) = g.spies.get(&spy) else {
            return Some(first);
        };
        let pid = agent.owner;
        let covering = |racer: usize| Self::racer_cover_spies(g, pid, spy, racer);
        let here = agent
            .city
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner)
            .filter(|owner| co_leaders.contains(owner));
        match here {
            Some(here) => {
                // A Spy sharing its co-leader leaves for the best-ranked
                // uncovered one; a Spy alone there only for one ahead of it
                // in progress (never on Science alone, so a tie that flips
                // from turn to turn cannot walk it back and forth).
                let shared = covering(here) > 0;
                let standing = Self::racer_space_progress(g, here);
                let uncovered = co_leaders.iter().copied().find(|racer| {
                    *racer != here
                        && covering(*racer) == 0
                        && (shared || Self::racer_space_progress(g, *racer) > standing + 1e-9)
                });
                match uncovered {
                    Some(next) if !Self::racer_spy_disrupt_runnable(g, spy, legal, refile) => {
                        Some(next)
                    }
                    _ => Some(here),
                }
            }
            // `min_by_key` keeps the first of equals: the best-ranked.
            None => co_leaders
                .iter()
                .copied()
                .min_by_key(|racer| covering(*racer)),
        }
    }

    /// The racers by name and space projects landed, best first, with the
    /// Science Victory Points of any whose expedition is under way.
    pub(crate) fn racer_cover_roll(g: &Game, racers: &[usize]) -> String {
        racers
            .iter()
            .map(|rival| {
                let landed = Self::science_denial_stages(g, *rival);
                let trip = if g.players[*rival]
                    .science_projects
                    .contains("exoplanet_expedition")
                {
                    format!(
                        ", {:.0} of {:.0} points",
                        g.science_victory_points(*rival),
                        g.science_victory_points_needed(*rival)
                    )
                } else {
                    String::new()
                };
                format!(
                    "{} {landed} project{}{trip}",
                    g.players[*rival].civ,
                    if landed == 1 { "" } else { "s" }
                )
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// The journal reason of a post under the gene: the co-leaders and their
    /// projects, the racers further behind, and why the Spy goes to `racer`.
    pub(crate) fn racer_cover_reason(
        g: &Game,
        spy: u32,
        racer: usize,
        racers: &BTreeSet<usize>,
        co_leaders: &[usize],
    ) -> String {
        let behind: Vec<usize> = Self::racer_cover_ranked(g, racers)
            .into_iter()
            .filter(|rival| !co_leaders.contains(rival))
            .collect();
        let behind = if behind.is_empty() {
            String::new()
        } else {
            format!("; further back: {}", Self::racer_cover_roll(g, &behind))
        };
        let civ = &g.players[racer].civ;
        if co_leaders.len() <= 1 {
            return format!(
                "denial-spies-cover-every-racer: {} leads the space race by more than a \
                 project{behind}, so the Spies go to {civ}",
                Self::racer_cover_roll(g, co_leaders)
            );
        }
        let pid = g.spies.get(&spy).map_or(0, |agent| agent.owner);
        let cover = match Self::racer_cover_spies(g, pid, spy, racer) {
            0 => format!("no other Spy of ours covers {civ}"),
            others => format!("{civ} has the fewest other Spies of ours ({others})"),
        };
        format!(
            "denial-spies-cover-every-racer: {} race within a project of each \
             other{behind}; each gets a Spy of ours before any gets a second, and {cover}",
            Self::racer_cover_roll(g, co_leaders)
        )
    }
}

#[cfg(test)]
mod tests;
