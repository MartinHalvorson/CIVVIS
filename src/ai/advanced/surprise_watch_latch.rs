//! `surprise-watch-holds-its-ratio`: a stronger neighbour seen at our door
//! keeps the walls armed through a dip in the ratio.
//!
//! `capital-walls-before-the-surprise` and `frontier-walls-before-the-surprise`
//! remember when a watched major's soldier was seen near a city. Each turn
//! they re-read that major's power against ours. The walls stand down the
//! turn the live ratio falls under 2.0, and so do Mining and Masonry in
//! research. The ratio falls exactly when we arm.
//!
//! Live Lv 7/8, 2026-10-30/31: 8 of 63 seats were declared on by t40, and 7
//! of them lost a city by t41. Five of the seven had walls part-built. At
//! the declaration the attacker had 1.7-2.3x our military in six of the
//! seven.
//! - 001453Z (Mali, war t23): the warriors we bought lifted us to 1.7-1.8x
//!   at t18-21, and the walls stood down. Athens built a Monument when
//!   Masonry landed at t20, started its walls at t22, and fell at t25 with
//!   31 of 40.
//! - 173341Z (Georgia, war t33): Georgia dipped to 1.6x at t30-33, so
//!   Ephesus built a Granary and a Settler first. It fell at t41 with 36 of
//!   40.
//! - 005343Z (Persia, Deity, war t33): Persia stood within 10 tiles of
//!   Bogotá at t5 but within 8 only at t20. Mining then Masonry ran
//!   t22-t33. The capital fell at t36 with 22 of 40.
//!
//! A 2x major with a soldier or city within 10 tiles of our capital by t25
//! went to war by t40 in 26% of seats (5/19), against 7% (3/44) without one.
//! The signal arrives a median 13 turns before the declaration. Walls
//! finished by the declaration hold every melee rush in the repo's own
//! combat model.
//!
//! With the gene on, a met major at peace that fields
//! [`CAPITAL_WALLS_RATIO`](super::capital_walls_before_the_surprise::CAPITAL_WALLS_RATIO)
//! times our power when its soldier is seen within [`LATCH_TILES`] of our
//! original capital, or of an unwalled frontier city, is latched for that
//! city. The latch lasts the walls' own ten-turn window from the last such
//! sighting, whatever the live ratio does. Both wall policies and their
//! research legs read it beside their own live reading. With the gene off,
//! nothing is recorded and nothing changes.

use super::*;

/// Tiles from the watched city within which a 2x major's soldier latches
/// the walls: two more than the walls' own watch (eight).
pub(crate) const LATCH_TILES: i32 = 10;

impl AdvancedAi {
    /// Record this turn's latches: each watched city, under its own policy,
    /// that sees a met peaceful major's soldier within [`LATCH_TILES`] while
    /// that major fields twice our power. The key is `None` for the original
    /// capital and `Some(city)` for a frontier city. Expired latches are
    /// dropped. Nothing with the gene off.
    pub(super) fn remember_surprise_watch_latches(&mut self, g: &Game, pid: usize) {
        if !self.surprise_watch_holds_its_ratio {
            return;
        }
        let window =
            g.standard_duration(super::capital_walls_before_the_surprise::CAPITAL_WATCH_STANDARD);
        self.surprise_watch_latches
            .retain(|_, seen| *seen <= g.turn && g.turn - *seen <= window);
        let ours = g.military_power(pid).max(1.0);
        let strong = |owner: usize| {
            g.military_power(owner) / ours
                >= super::capital_walls_before_the_surprise::CAPITAL_WALLS_RATIO
        };
        let mut watched: Vec<(Option<u32>, Pos)> = Vec::new();
        if self.capital_walls_before_the_surprise {
            if let Some(capital) = Self::capital_watch_city(g, pid) {
                watched.push((None, g.cities[&capital].pos));
            }
        }
        if self.frontier_walls_before_the_surprise {
            for cid in g.player_city_ids(pid) {
                if Self::frontier_watch_city(g, pid, cid) {
                    watched.push((Some(cid), g.cities[&cid].pos));
                }
            }
        }
        let mut latched: Vec<((Option<u32>, usize), u32)> = Vec::new();
        for (key, at) in watched {
            for unit in Self::capital_watch_seen(g, pid, at, LATCH_TILES) {
                if strong(unit.owner) {
                    latched.push(((key, unit.owner), g.turn));
                }
            }
        }
        for (key, turn) in latched {
            self.surprise_watch_latches.insert(key, turn);
        }
    }

    /// The latched major for `key`'s city that we are still at peace with,
    /// and its live ratio, the strongest first. `None` with the gene off.
    fn surprise_watch_latched(
        &self,
        g: &Game,
        pid: usize,
        key: Option<u32>,
    ) -> Option<(usize, f64)> {
        if !self.surprise_watch_holds_its_ratio {
            return None;
        }
        let window =
            g.standard_duration(super::capital_walls_before_the_surprise::CAPITAL_WATCH_STANDARD);
        let ours = g.military_power(pid).max(1.0);
        self.surprise_watch_latches
            .iter()
            .filter(|((city, owner), seen)| {
                *city == key
                    && **seen <= g.turn
                    && g.turn - **seen <= window
                    && g.players.get(*owner).is_some_and(|player| {
                        player.alive
                            && g.has_met(pid, *owner)
                            && !g.is_at_war(pid, *owner)
                            // A friend or an ally cannot declare by surprise.
                            && !g.are_friends(pid, *owner)
                            && g.alliance_with(pid, *owner).is_none()
                    })
            })
            .map(|((_, owner), _)| (*owner, g.military_power(*owner) / ours))
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    /// The capital's latch. Read by `capital_walls_threat` after its live
    /// reading.
    pub(super) fn capital_surprise_latched(&self, g: &Game, pid: usize) -> Option<(usize, f64)> {
        self.surprise_watch_latched(g, pid, None)
    }

    /// A frontier city's latch. Read by `frontier_walls_threat` after its
    /// live reading.
    pub(super) fn frontier_surprise_latched(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<(usize, f64)> {
        self.surprise_watch_latched(g, pid, Some(cid))
    }
}

#[cfg(test)]
mod tests;
