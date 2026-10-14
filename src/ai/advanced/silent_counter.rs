//! Journal only: an urgent Culture or Science racer whose Conquest counter
//! cannot be executed.
//!
//! `actionable_victory_denial` falls through a rival whose counter fails
//! `conquest_denial_actionable`, and the "Countering X" line prints only for a
//! counter that was chosen, so an urgent racer with no executable answer left
//! no line at all. Live Emperor civvis-20261014T095222Z: the Ottomans read
//! Culture 87-97% and urgent from turn 143 (a traced replay at 1ddb91f4b), the
//! Conquest counter failed every turn behind America's closed borders, the
//! plan read "following the assigned victory lane", and the Ottomans won on
//! Culture at 148 with nothing in the journal. This says which feasibility
//! clause held the counter, once per rival per [`SILENT_COUNTER_NOTE_TURNS`]
//! standard turns or when the clause changes. It decides nothing.

use std::collections::{BTreeSet, VecDeque};

use super::{denial_nearest_finish, AdvancedAi, GrandStrategy};
use crate::game::Game;

/// Standard turns between repeats of one rival's unchanged silent counter.
pub(super) const SILENT_COUNTER_NOTE_TURNS: u32 = 5;

impl AdvancedAi {
    /// See the module docs.
    pub(crate) fn journal_silent_counter(&mut self, g: &Game, pid: usize) {
        if !self.journal().wants(crate::reasoning::Level::Strategy) || !self.deny_leaders {
            return;
        }
        let _memo = g.query_memo();
        let culture_pressures = self.rival_culture_pressures(g);
        let own_progress = self.victory_focus(g, pid).progress;
        let chosen = self
            .actionable_victory_denial_with_culture_pressures(g, pid, &culture_pressures)
            .map(|(rival, _)| rival);
        let mut notes: Vec<(usize, &'static str, String)> = Vec::new();
        for (rival, pressure) in self.ranked_rival_victory_pressures(g, pid, &culture_pressures) {
            if !matches!(
                pressure.strategy,
                GrandStrategy::Culture | GrandStrategy::Science
            ) || chosen == Some(rival)
                || !self.victory_pressure_is_urgent(g, rival, pressure)
                || self.denial_response_for_pressure(g, pid, own_progress, rival, pressure)
                    != Some(GrandStrategy::Conquest)
                || self.conquest_denial_actionable(g, pid, rival, GrandStrategy::Conquest)
            {
                continue;
            }
            let (kind, clause) = self.silent_counter_clause(g, pid, rival);
            let finish = denial_nearest_finish::countering_finish_note(
                pressure.strategy,
                (pressure.strategy == GrandStrategy::Culture)
                    .then(|| self.projected_culture_finish(g, rival))
                    .flatten(),
            );
            let reading = format!(
                "its {} race reads {}%{finish}, urgent; the Conquest counter {clause}",
                pressure.strategy.as_str(),
                pressure.progress
            );
            notes.push((rival, kind, reading));
        }
        let every = g.standard_duration(SILENT_COUNTER_NOTE_TURNS);
        for (rival, kind, reading) in notes {
            let due = self
                .silent_counter_noted
                .get(&rival)
                .is_none_or(|(noted, turn)| {
                    *noted != kind || g.turn.saturating_sub(*turn) >= every
                });
            if !due {
                continue;
            }
            self.silent_counter_noted.insert(rival, (kind, g.turn));
            crate::think!(self.journal(), Strategy, Strategy,
                   "No counter for {}", g.players[rival].civ;
                   "{reading}");
        }
    }

    /// The first clause of `conquest_denial_actionable` that fails for
    /// `rival`, in its order, as a repeat key and its reading.
    pub(super) fn silent_counter_clause(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> (&'static str, String) {
        if !self.campaign_target_legal(g, pid, rival) {
            if self.siege_road_cuts_off(g, pid, rival) {
                return (
                    "siege-road",
                    "is stood down: its siege lost its road".to_string(),
                );
            }
            if !g.has_met(pid, rival) {
                return ("unmet", "has no target: we have not met them".to_string());
            }
            if g.are_friends(pid, rival) {
                return ("friends", "is barred: we are declared friends".to_string());
            }
            if g.alliance_with(pid, rival).is_some() {
                return ("allied", "is barred: we are allied".to_string());
            }
            return ("legal", "has no legal target".to_string());
        }
        if g.player_city_ids(rival).is_empty() {
            return (
                "no-city",
                "has no target: no city of theirs is located".to_string(),
            );
        }
        if self.denial_needs_a_road && !self.rival_reachable_by_land(g, pid, rival) {
            return match land_road_blocker(g, pid, rival) {
                Some(blocker) => (
                    "closed-borders",
                    format!(
                        "has no land road (behind {}'s closed borders)",
                        g.players[blocker].civ
                    ),
                ),
                None if reachable_through(g, pid, rival, None, true) => (
                    "borders",
                    "has no land road (behind more than one closed border)".to_string(),
                ),
                None => ("water", "has no land road (across water)".to_string()),
            };
        }
        ("other", "is held by a feasibility check".to_string())
    }
}

/// The one civilization whose closed borders alone shut the land road from
/// our cities to `rival`'s: opening its ground opens the road. Majors before
/// city-states, then by id.
fn land_road_blocker(g: &Game, pid: usize, rival: usize) -> Option<usize> {
    let mut candidates: Vec<usize> = g
        .players
        .iter()
        .filter(|player| {
            player.alive
                && !player.is_barbarian
                && player.id != pid
                && player.id != rival
                && !g.is_at_war(pid, player.id)
                && !g.has_open_borders(pid, player.id)
        })
        .map(|player| player.id)
        .collect();
    candidates.sort_by_key(|id| (g.players[*id].is_minor, *id));
    candidates
        .into_iter()
        .find(|blocker| reachable_through(g, pid, rival, Some(*blocker), false))
}

/// `denial_needs_a_road`'s land search, with `opened`'s ground (or, with
/// `all_open`, every foreign ground) read as passable.
fn reachable_through(
    g: &Game,
    pid: usize,
    rival: usize,
    opened: Option<usize>,
    all_open: bool,
) -> bool {
    let passable = |pos: crate::Pos| {
        let Some(tile) = g.map.get(pos) else {
            return false;
        };
        if !g.rules.is_passable(tile) || g.rules.is_water(tile) {
            return false;
        }
        match tile
            .owner_city
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.owner)
        {
            None => true,
            Some(owner) => {
                all_open
                    || owner == pid
                    || owner == rival
                    || Some(owner) == opened
                    || g.is_at_war(pid, owner)
                    || g.has_open_borders(pid, owner)
            }
        }
    };
    let goals: BTreeSet<crate::Pos> = g
        .player_city_ids(rival)
        .into_iter()
        .flat_map(|cid| {
            let pos = g.cities[&cid].pos;
            std::iter::once(pos).chain(g.nbrs(pos))
        })
        .collect();
    if goals.is_empty() {
        return false;
    }
    let mut seen: BTreeSet<crate::Pos> = BTreeSet::new();
    let mut queue: VecDeque<crate::Pos> = VecDeque::new();
    for cid in g.player_city_ids(pid) {
        let pos = g.cities[&cid].pos;
        if seen.insert(pos) {
            queue.push_back(pos);
        }
    }
    while let Some(pos) = queue.pop_front() {
        if goals.contains(&pos) {
            return true;
        }
        for next in g.nbrs(pos) {
            if !seen.contains(&next) && passable(next) {
                seen.insert(next);
                queue.push_back(next);
            }
        }
    }
    false
}

#[cfg(test)]
#[path = "silent_counter/tests.rs"]
mod tests;
