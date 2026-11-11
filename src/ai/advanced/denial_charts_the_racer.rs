//! `denial-charts-the-racer`: a Culture or Domination seat charts every city
//! of the leading Science rival, so its Spies have the Spaceport city to go to.
//!
//! **Why (live Deity Kristina, Online, Small Continents; the ten Science
//! losses of 2026-11-10).** In five of ten the seat ran no Disrupt Rocketry on
//! the eventual winner: it knew 0-4 of the winner's 9-15 cities, so the city
//! with the Spaceport was never a target a Spy could travel to, and its Spies
//! sat 40-60 turns on Listening Post. In civvis-20261110T205256Z the seat held
//! Open Borders with Babylon from turn 110 to 160 and saw none of its cities
//! until turn 154; Babylon won on Science at turn 170. Nothing sends a unit
//! into a met rival's ground once one of its cities is known: `contact-fleet`
//! closes its gap on the first city, and `find-the-capital` hunts only the
//! Domination lane's unseen capitals.
//!
//! **What the gene does**, for a seat on the Culture or Domination lane once a
//! met rival is on the Science clock (`science_race_pressure` above zero) or
//! from [`RACER_CHARTING_FROM_STANDARD`] standard turns (turn 100 Online),
//! while the Science Victory is on:
//! - **Racer** ([`AdvancedAi::charting_racer`]): the met living major with the
//!   highest science race pressure, then the most technologies. It is charted
//!   while the seat knows fewer of its cities than the host's public count
//!   ([`AdvancedAi::racer_under_seen`]).
//! - **Gap.** The contact gap stays open and the Religion seat's charting parts
//!   act (`lane_charts_the_water`, as for `domination-charts-every-capital`):
//!   the hull claim and hold, the ocean-crossing landfall routing, and one
//!   warship kept off the objective board to chart.
//! - **Land.** `find-the-capital`'s hunt row takes the racer as well: once its
//!   ground is open to us (Open Borders, or war) one Recon row aims a Scout or
//!   a fast land unit at the fog nearest the middle of its known ground, and
//!   the border-buy lane may pay `FIND_CAPITAL_PASSAGE_GOLD` for the passage.
//!
//! One hull and one land unit at most. Off, or on any other lane, nothing
//! changes.

use super::*;

/// Standard turns from which the leading Science rival is charted even before
/// it is on the Science clock: turn 100 Online.
pub(super) const RACER_CHARTING_FROM_STANDARD: u32 = 150;

impl AdvancedAi {
    /// The leading Science rival among met living majors and its science race
    /// pressure: the highest pressure, then the most technologies (the host's
    /// public count), then the lower id.
    pub(super) fn charting_racer(&self, g: &Game, pid: usize) -> Option<(usize, i32)> {
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !rival.is_free_city
                    && !g.same_team(pid, rival.id)
                    && g.has_met(pid, rival.id)
            })
            .map(|rival| {
                // The host's public technology count: a live board redacts a
                // rival's technology identities.
                let techs = g
                    .observed_public_empire_stats
                    .get(&rival.id)
                    .and_then(|stats| stats.techs)
                    .unwrap_or(rival.techs.len());
                (
                    self.science_race_pressure(g, rival.id),
                    techs,
                    std::cmp::Reverse(rival.id),
                )
            })
            .max()
            .map(|(pressure, _, std::cmp::Reverse(rival))| (rival, pressure))
    }

    /// Whether `pid` knows fewer of `rival`'s cities than the host's public
    /// count (the board's own count off the live seat).
    pub(super) fn racer_under_seen(g: &Game, pid: usize, rival: usize) -> bool {
        let count = g
            .observed_public_empire_stats
            .get(&rival)
            .and_then(|stats| stats.city_count)
            .unwrap_or_else(|| g.player_city_ids(rival).len());
        BasicAi::rival_cities_known(g, pid, rival) < count
    }

    /// The racer the gene charts this turn, if it acts. See the module.
    pub(super) fn denial_racer(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.denial_charts_the_racer
            || !g.victory_conditions.science
            || !matches!(
                self.active_victory_target(g),
                Some(VictoryTarget::Culture | VictoryTarget::Domination)
            )
        {
            return None;
        }
        let (racer, pressure) = self.charting_racer(g, pid)?;
        if pressure <= 0 && g.turn < g.standard_duration(RACER_CHARTING_FROM_STANDARD) {
            return None;
        }
        Self::racer_under_seen(g, pid, racer).then_some(racer)
    }
}

#[cfg(test)]
mod tests;
