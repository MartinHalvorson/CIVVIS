//! `founder-spreads-only-its-faith`: a founder buys and spreads only its own
//! faith.
//!
//! A purchased religious unit takes the purchase city's majority, on the host
//! and on our board alike. Our board can run ahead of the host inside a turn:
//! `BasicAi`'s ancillary pass applies `FoundReligion` the moment a Prophet is
//! pending, and the engine then hands the Holy City our faith at once, so the
//! same turn's faith spending saw a city of ours that the host still counted
//! for the old faith. Live Emperor G186 (civvis-20261006T025742Z) did exactly
//! that at turn 48: the host refused the founding (`not_founded`) and bought
//! the Missionary in Hindu Bogotá, so it came out Hindu. And a founder spread
//! any faith a unit of its carried (`adopted_faith_spread_allowed` lets every
//! founder through): that Hindu Missionary spread Hinduism from Bogotá's tile
//! on turns 49, 50 and 51, the last two while Bogotá was our new Buddhist Holy
//! City. Indonesia's Hinduism won a Religious Victory at 84.
//!
//! Under the gene:
//! - no religious unit is bought on a turn whose board founded our religion
//!   ([`AdvancedAi::founding_pending`]): the founding is not yet the host's;
//! - a religious unit is bought only in a city that followed our faith when
//!   the turn began ([`AdvancedAi::founder_purchase_withheld`]), not one our
//!   own units converted on this turn's board;
//! - a founder's spreader of another faith holds its charges
//!   ([`AdvancedAi::founder_holds_a_foreign_spreader`]).
//!
//! A seat with no religion keeps its adopted-faith purchases and spreads
//! (`religious_defense`, `adopted_faith_spread_allowed`). Off: unchanged.

use super::*;

/// Our faith as the board stood when the turn began: the religion we had
/// founded and the cities that followed it. Recorded by
/// [`AdvancedAi::record_turn_start_faith`] under the gene.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct TurnStartFaith {
    pub(super) turn: u32,
    pub(super) faith: Option<String>,
    pub(super) cities: BTreeSet<u32>,
}

impl AdvancedAi {
    /// Record [`TurnStartFaith`] before anything this turn can found a
    /// religion or spread one. Nothing with the gene off.
    pub(super) fn record_turn_start_faith(&mut self, g: &Game, pid: usize) {
        self.turn_start_faith = self.founder_spreads_only_its_faith.then(|| {
            let faith = g.players[pid].religion.clone();
            let cities = faith
                .as_deref()
                .map(|faith| {
                    g.player_city_ids(pid)
                        .into_iter()
                        .filter(|cid| g.city_religion(&g.cities[cid]) == Some(faith))
                        .collect()
                })
                .unwrap_or_default();
            TurnStartFaith {
                turn: g.turn,
                faith,
                cities,
            }
        });
    }

    /// This turn's [`TurnStartFaith`], if the gene recorded one.
    fn turn_start_faith_now(&self, g: &Game) -> Option<&TurnStartFaith> {
        self.turn_start_faith
            .as_ref()
            .filter(|start| self.founder_spreads_only_its_faith && start.turn == g.turn)
    }

    /// Whether our religion was founded on this turn's board: the turn began
    /// without it. The host has not shown the founding yet, and may refuse it
    /// (G186 turn 48), so a purchase now takes the city's old majority.
    pub(super) fn founding_pending(&self, g: &Game, pid: usize) -> bool {
        g.players[pid].religion.is_some()
            && self
                .turn_start_faith_now(g)
                .is_some_and(|start| start.faith != g.players[pid].religion)
    }

    /// Whether a founder must not buy a religious unit in `city` this turn:
    /// the city did not follow our faith when the turn began, so the unit
    /// would take another faith (or none). False with the gene off and for a
    /// seat with no religion.
    pub(super) fn founder_purchase_withheld(&self, g: &Game, pid: usize, city: u32) -> bool {
        g.players[pid].religion.is_some()
            && self
                .turn_start_faith_now(g)
                .is_some_and(|start| !start.cities.contains(&city))
    }

    /// Whether a founder holds a spreader of `faith` instead of spreading it:
    /// any faith but our own. False with the gene off and for a seat with no
    /// religion, whose adopted-faith spreaders answer to
    /// `adopted_faith_spread_allowed`.
    pub(super) fn founder_holds_a_foreign_spreader(
        &self,
        g: &Game,
        pid: usize,
        faith: &str,
    ) -> bool {
        self.founder_spreads_only_its_faith
            && g.players[pid]
                .religion
                .as_deref()
                .is_some_and(|ours| ours != faith)
    }
}

#[cfg(test)]
mod tests;
