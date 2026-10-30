//! `launched-racer-draws-every-spy`: once a rival has launched its Exoplanet
//! Expedition, the denial spies answer only the launched racers.
//!
//! The launch is not the end of the pads. Orbital and Terrestrial Laser
//! Stations are Spaceport projects with no instance cap (Expansion2_Projects:
//! 600 Production, Offworld Mission, the launched Expedition as prerequisite),
//! and each one completed adds a light-year a turn. A pillaged Spaceport
//! builds none of them, so Disrupt Rocketry still slows a launched racer.
//!
//! Live Immortal culture seats, 2026-10-30 (Pericles, six science losses):
//!
//! | game    | launch | win | gap | VP a turn after launch | our Disrupts after |
//! |---------|--------|-----|-----|------------------------|--------------------|
//! | 123717Z | 180    | 184 |  4  | 1 → 9                  | 0                  |
//! | 132518Z | 186    | 210 | 24  | 1 throughout           | 6                  |
//! | 141223Z | 188    | 200 | 12  | 1 → 5                  | 1                  |
//! | 153459Z | 214    | 219 |  5  | 4 → 6                  | 1                  |
//! | 160045Z | 220    | 226 |  6  | 1 → 6                  | 0                  |
//! | 162440Z | 196    | 205 |  9  | 1 → 3                  | 2                  |
//!
//! Japan (132518Z) held Offworld Mission from turn 188 and stayed at one a
//! turn while six Disrupts landed on its three pads. Elsewhere the spies were
//! split over every rival holding Rocketry (`space-denial-spies-from-rocketry`
//! widens [`AdvancedAi::denial_spy_racers`] that way): 162440Z had Babylon
//! launched at 196 with our agents in Australian cities and on the road.
//!
//! With the gene on, while any rival the network would answer has launched,
//! [`AdvancedAi::denial_spy_racers`] keeps only those that have. That one set
//! feeds the spy training target, `science-denial-every-pad`'s postings and
//! reposts, and `racer-keeps-the-spies`' leader and stays, so every spy goes
//! to the launcher's standing pads, and a spy whose pad it just pillaged
//! (`every_pad_cities` skips a pillaged pad) moves to the next one. Wars and
//! the other science counters keep reading
//! [`AdvancedAi::decisive_space_racers`] unchanged.
use std::collections::BTreeSet;

use super::AdvancedAi;
use crate::game::Game;

/// The project a launched Expedition leaves in `Player::science_projects`.
const EXOPLANET_EXPEDITION: &str = "exoplanet_expedition";

impl AdvancedAi {
    /// `racers` less every rival that has not launched, when at least one has
    /// and the gene is on; otherwise `racers` unchanged.
    pub(crate) fn launched_racers_only(&self, g: &Game, racers: BTreeSet<usize>) -> BTreeSet<usize> {
        if !self.launched_racer_draws_every_spy {
            return racers;
        }
        let launched: BTreeSet<usize> = racers
            .iter()
            .copied()
            .filter(|rival| {
                g.players
                    .get(*rival)
                    .is_some_and(|player| player.science_projects.contains(EXOPLANET_EXPEDITION))
            })
            .collect();
        if launched.is_empty() {
            racers
        } else {
            launched
        }
    }
}

#[cfg(test)]
mod tests;
