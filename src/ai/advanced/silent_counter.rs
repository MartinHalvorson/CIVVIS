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

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{denial_nearest_finish, AdvancedAi, GrandStrategy, VictoryFocus, VictoryTarget};
use crate::game::Game;

/// Standard turns between repeats of one rival's unchanged silent counter.
pub(super) const SILENT_COUNTER_NOTE_TURNS: u32 = 5;

/// One silent counter's journal entry: the racer, its repeat key, its
/// reading, and under `racer-blocker-declares` the blocker and its reading.
type SilentNote = (usize, &'static str, String, Option<(usize, String)>);

/// An urgent racer whose Conquest counter cannot be executed, and why.
pub(super) struct SilentCounter {
    pub(super) rival: usize,
    pub(super) pressure: VictoryFocus,
    pub(super) clause: SilentClause,
}

/// The first failing clause of `conquest_denial_actionable`: its repeat key,
/// its reading, and the one civilization whose closed borders alone shut the
/// land road when that is the clause.
pub(super) struct SilentClause {
    pub(super) kind: &'static str,
    pub(super) text: String,
    pub(super) blocker: Option<usize>,
}

impl AdvancedAi {
    /// The urgent Culture and Science racers whose denial answer is
    /// Conquest, which `conquest_denial_actionable` refuses and the denial
    /// layer did not choose, with the clause that refuses it. Highest
    /// pressure first, as `ranked_rival_victory_pressures` orders them.
    pub(super) fn silent_counters(&self, g: &Game, pid: usize) -> Vec<SilentCounter> {
        if !self.deny_leaders {
            return Vec::new();
        }
        let _memo = g.query_memo();
        let culture_pressures = self.rival_culture_pressures(g);
        let own_progress = self.victory_focus(g, pid).progress;
        let chosen = self
            .actionable_victory_denial_with_culture_pressures(g, pid, &culture_pressures)
            .map(|(rival, _)| rival);
        self.ranked_rival_victory_pressures(g, pid, &culture_pressures)
            .into_iter()
            .filter(|(rival, pressure)| {
                matches!(
                    pressure.strategy,
                    GrandStrategy::Culture | GrandStrategy::Science
                ) && chosen != Some(*rival)
                    && self.victory_pressure_is_urgent(g, *rival, *pressure)
                    && self.denial_response_for_pressure(g, pid, own_progress, *rival, *pressure)
                        == Some(GrandStrategy::Conquest)
                    && !self.conquest_denial_actionable(g, pid, *rival, GrandStrategy::Conquest)
            })
            .map(|(rival, pressure)| SilentCounter {
                rival,
                pressure,
                clause: self.silent_counter_clause(g, pid, rival),
            })
            .collect()
    }

    /// See the module docs. Under `racer-blocker-declares` a racer held
    /// behind one civilization's closed borders also reads that blocker's
    /// weakness: "Racer's road blocker is the front" when it passes,
    /// "Racer's road blocker held" when it does not.
    pub(crate) fn journal_silent_counter(&mut self, g: &Game, pid: usize) {
        if !self.journal().wants(crate::reasoning::Level::Strategy) {
            return;
        }
        let front = self
            .racer_road_blocker(g, pid)
            .map(|(blocker, _, _)| blocker);
        let mut notes: Vec<SilentNote> = Vec::new();
        for counter in self.silent_counters(g, pid) {
            let SilentCounter {
                rival,
                pressure,
                clause,
            } = counter;
            let finish = denial_nearest_finish::countering_finish_note(
                pressure.strategy,
                (pressure.strategy == GrandStrategy::Culture)
                    .then(|| self.projected_culture_finish(g, rival))
                    .flatten(),
            );
            let reading = format!(
                "its {} race reads {}%{finish}, urgent; the Conquest counter {}",
                pressure.strategy.as_str(),
                pressure.progress,
                clause.text
            );
            let (kind, blocker_note) = match clause.blocker {
                Some(blocker) if self.racer_blocker_declares => {
                    let weak = front == Some(blocker);
                    let ours = g.military_power(pid);
                    let note = format!(
                        "racer-blocker-declares: {} shut the only land road to {}; our {ours:.0} power against their steady {:.0} and 30-turn peak {:.0} (needs {:.1} and {:.1} times){}",
                        g.players[blocker].civ,
                        g.players[rival].civ,
                        self.steady_rival_power(g, blocker),
                        self.peak_rival_power(g, blocker),
                        super::siege_road::BLOCKER_POWER_RATIO,
                        super::one_war::CRUSHING_PEAK_FLOOR,
                        if weak {
                            "; the war that opens the road is the front"
                        } else {
                            "; not weak enough, or not a legal target"
                        }
                    );
                    (
                        if weak {
                            "closed-borders-weak"
                        } else {
                            "closed-borders-held"
                        },
                        Some((blocker, note)),
                    )
                }
                _ => (clause.kind, None),
            };
            notes.push((rival, kind, reading, blocker_note));
        }
        let every = g.standard_duration(SILENT_COUNTER_NOTE_TURNS);
        for (rival, kind, reading, blocker_note) in notes {
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
            if let Some((blocker, note)) = blocker_note {
                let headline = if kind == "closed-borders-weak" {
                    "Racer's road blocker is the front"
                } else {
                    "Racer's road blocker held"
                };
                crate::think!(self.journal(), Military, Strategy,
                       "{headline}: {}", g.players[blocker].civ;
                       "{note}");
            }
        }
    }

    /// `racer-blocker-declares`: the weak civilization whose closed borders
    /// alone shut the land road to an urgent racer our Conquest counter
    /// cannot reach (`silent_counters`, clause "closed-borders"), with that
    /// racer and its pressure. Weak is `closed-border-blocker-declares`'s
    /// reading (`closed_border_blocker_is_weak`), re-read every turn with no
    /// held siege march asked of it. Not while the last plan was Recovery or
    /// defended a threatened city of ours. `None` with the gene off or under
    /// another victory target; the highest-pressure racer first.
    pub(super) fn racer_road_blocker(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<(usize, usize, VictoryFocus)> {
        if !self.racer_blocker_declares
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || self.plan.as_ref().is_some_and(|plan| {
                plan.strategy == GrandStrategy::Recovery || plan.threatened_city.is_some()
            })
        {
            return None;
        }
        self.silent_counters(g, pid)
            .into_iter()
            .filter_map(|counter| {
                counter
                    .clause
                    .blocker
                    .map(|blocker| (blocker, counter.rival, counter.pressure))
            })
            .find(|(blocker, _, _)| self.closed_border_blocker_is_weak(g, pid, *blocker))
    }

    /// `racer-blocker-declares`: the front `road_blocker_front` names.
    pub(super) fn racer_blocker_front(&self, g: &Game, pid: usize) -> Option<usize> {
        self.racer_road_blocker(g, pid)
            .map(|(blocker, _, _)| blocker)
    }

    /// `racer-blocker-declares`: the blocker and its city whose ground the
    /// land road to the racer crosses first -- the war's objective, not the
    /// blocker's far capital.
    pub(super) fn racer_blocker_objective(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        let (blocker, racer, _) = self.racer_road_blocker(g, pid)?;
        road_path_city(g, pid, racer, blocker).map(|city| (blocker, city))
    }

    /// The first clause of `conquest_denial_actionable` that fails for
    /// `rival`, in its order: a repeat key, its reading, and the one
    /// civilization whose closed borders alone shut the road, when that is
    /// the clause.
    pub(super) fn silent_counter_clause(&self, g: &Game, pid: usize, rival: usize) -> SilentClause {
        let clause = |kind: &'static str, text: &str| SilentClause {
            kind,
            text: text.to_string(),
            blocker: None,
        };
        if !self.campaign_target_legal(g, pid, rival) {
            if self.siege_road_cuts_off(g, pid, rival) {
                return clause("siege-road", "is stood down: its siege lost its road");
            }
            if !g.has_met(pid, rival) {
                return clause("unmet", "has no target: we have not met them");
            }
            if g.are_friends(pid, rival) {
                return clause("friends", "is barred: we are declared friends");
            }
            if g.alliance_with(pid, rival).is_some() {
                return clause("allied", "is barred: we are allied");
            }
            return clause("legal", "has no legal target");
        }
        if g.player_city_ids(rival).is_empty() {
            return clause("no-city", "has no target: no city of theirs is located");
        }
        if self.denial_needs_a_road && !self.rival_reachable_by_land(g, pid, rival) {
            return match land_road_blocker(g, pid, rival) {
                Some(blocker) => SilentClause {
                    kind: "closed-borders",
                    text: format!(
                        "has no land road (behind {}'s closed borders)",
                        g.players[blocker].civ
                    ),
                    blocker: Some(blocker),
                },
                None if reachable_through(g, pid, rival, None, true) => clause(
                    "borders",
                    "has no land road (behind more than one closed border)",
                ),
                None => clause("water", "has no land road (across water)"),
            };
        }
        clause("other", "is held by a feasibility check")
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

/// The `blocker` city whose ground the shortest land road from our cities to
/// `racer`'s, read with `blocker`'s ground open, enters first.
fn road_path_city(g: &Game, pid: usize, racer: usize, blocker: usize) -> Option<u32> {
    let owner_of = |pos: crate::Pos| {
        g.map
            .get(pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|cid| g.cities.get(&cid).map(|city| (cid, city.owner)))
    };
    let passable = |pos: crate::Pos| {
        let Some(tile) = g.map.get(pos) else {
            return false;
        };
        if !g.rules.is_passable(tile) || g.rules.is_water(tile) {
            return false;
        }
        match owner_of(pos).map(|(_, owner)| owner) {
            None => true,
            Some(owner) => {
                owner == pid
                    || owner == racer
                    || owner == blocker
                    || g.is_at_war(pid, owner)
                    || g.has_open_borders(pid, owner)
            }
        }
    };
    let goals: BTreeSet<crate::Pos> = g
        .player_city_ids(racer)
        .into_iter()
        .flat_map(|cid| {
            let pos = g.cities[&cid].pos;
            std::iter::once(pos).chain(g.nbrs(pos))
        })
        .collect();
    let mut parent: BTreeMap<crate::Pos, Option<crate::Pos>> = BTreeMap::new();
    let mut queue: VecDeque<crate::Pos> = VecDeque::new();
    for cid in g.player_city_ids(pid) {
        let pos = g.cities[&cid].pos;
        if parent.insert(pos, None).is_none() {
            queue.push_back(pos);
        }
    }
    while let Some(pos) = queue.pop_front() {
        if goals.contains(&pos) {
            let mut path = vec![pos];
            let mut step = pos;
            while let Some(Some(previous)) = parent.get(&step) {
                path.push(*previous);
                step = *previous;
            }
            return path
                .into_iter()
                .rev()
                .find_map(|tile| owner_of(tile).filter(|(_, owner)| *owner == blocker))
                .map(|(cid, _)| cid);
        }
        for next in g.nbrs(pos) {
            if !parent.contains_key(&next) && passable(next) {
                parent.insert(next, Some(pos));
                queue.push_back(next);
            }
        }
    }
    None
}

#[cfg(test)]
#[path = "silent_counter/tests.rs"]
mod tests;
