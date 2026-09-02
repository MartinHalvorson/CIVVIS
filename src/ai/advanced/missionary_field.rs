//! The Missionary in the field: three opt-in genes — two for what a spreader
//! does between its charges, and one for what it does about the raiders it
//! meets.
//!
//! Both answer an operator goal of 2026-08-24 — *"a heuristic for exploring
//! with missionaries with 1 charge remaining … missionaries should be smart
//! enough to evade barbarians using their fast movement"* — and both rest on
//! the same two engine facts. A Missionary moves **4** to a Scout's 3 and a
//! Warrior's 2, and it ignores closed borders (`Game::unit_ignores_closed_
//! borders`); and its last charge is its life: `Game::do_spread` removes the
//! unit the moment `charges` reaches zero, so the third charge is spent by a
//! unit that would otherwise have walked home to be nothing.
//!
//! ## `missionary-last-charge-explores`
//!
//! `advanced_missionary_step` walks a Missionary to the best-scored city and
//! spreads the moment it stands beside one, last charge included. That is
//! the right thing for the first two charges — they leave the unit standing
//! — and a waste of the third, which deletes a four-move unit standing in
//! foreign territory it is free to cross. With version one on, a Missionary
//! on its last charge explores the fog within [`MISSIONARY_EXPLORE_RADIUS`]
//! for up to [`MISSIONARY_EXPLORE_TURNS`] turns first, and spends the charge
//! when the fog is gone, the turns are up, or something better than fog
//! turns up: a city of ours slipping to a rival faith (the charge is owed to
//! the defence) or a city beside it that our faith has never touched (the
//! find exploring is for). The fog goal is scored the way a Scout's is,
//! [`crate::ai::BasicAi::frontier_reveal_value`] first and distance second,
//! and kept while it stays unexplored, so the unit does not oscillate
//! between two equally dark horizons.
//!
//! `missionary-last-charge-explores-2` preserves that obligation to cities,
//! but uses the last charge as a true expedition: it searches out to
//! [`MISSIONARY_EXPEDITION_RADIUS`], prefers a farther high-reveal shore, and
//! retains only a goal with a legal route. The route rule is deliberately
//! shared with movement, so religious units use their closed-border exception
//! without learning what lies beyond the fog.
//!
//! ## `missionary-evades-raiders`
//!
//! Since 2026-08-24 the barbarian seat hunts religious units
//! (`BasicAi::barbarian_heretic_hunt`): a raider walks onto one and spends
//! the movement it arrives with on `Action::CondemnHeretic`. A spreading
//! Missionary is the easiest prey on the map — it stands still beside its
//! city at zero movement for three turns — and nothing in the controller
//! read a raider before walking one there. With the gene on, a religious
//! unit (1) steps out of the exact set of tiles a visible raider can end
//! its next move on — `Game::threat_reach`, the same envelope the Builder's
//! `builder_barbarian_safety` reads — when it finds itself inside it, and
//! (2) never steps into that set on the way to anything, sidestepping when
//! a safe neighbour still makes progress and holding (which is also the
//! only turn it heals) when none does. The one exception is the last charge
//! beside a city it can convert: the spread consumes the unit, so nothing
//! is exposed by taking it. An own city and a tile under a friendly
//! military unit are refuges: a raider takes the first only by taking the
//! city and reaches the second only by attacking the guard. Sea raiders are
//! ignored — a galley cannot condemn.
//!
//! All three genes ship off and are priced by the standard screen like every
//! other opt-in; `docs/gene_screens/fires/` carries their fires probes.

use std::collections::BTreeMap;

use super::{AdvancedAi, BarbarianCaptureThreat};
use crate::ai::BasicAi;
use crate::game::Game;
use crate::Pos;

/// How far version one of the exploring gene looks for fog. Ten tiles is
/// two and a half turns of Missionary movement on open ground.
pub(super) const MISSIONARY_EXPLORE_RADIUS: i32 = 10;

/// How far version two of the exploring gene searches for fog. Thirty-six
/// tiles is nine turns of Missionary movement on open ground: enough to cross
/// a normal ocean gap or the far side of a continent while the last charge is
/// still expendable.
pub(super) const MISSIONARY_EXPEDITION_RADIUS: i32 = 36;

/// How many turns a last-charge Missionary may explore before the charge is
/// owed to a city whatever the horizon still hides.
pub(super) const MISSIONARY_EXPLORE_TURNS: u32 = 12;

/// The exploring gene's memory of one Missionary.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct MissionaryExplore {
    /// The fog tile the unit is walking toward.
    pub(super) goal: Option<Pos>,
    /// Turns spent exploring so far.
    pub(super) turns: u32,
    /// The game turn `turns` last advanced on, so several steps in one turn
    /// count once.
    pub(super) last_turn: u32,
}

impl AdvancedAi {
    /// The visible barbarian raiders that could stand on a land tile before
    /// this unit moves again. Sea threats are dropped: a galley cannot
    /// condemn.
    fn religious_raider_threats(&self, g: &Game, pid: usize) -> Vec<BarbarianCaptureThreat> {
        let visible = self.battlefront_visibility(g, pid);
        self.visible_barbarian_capture_threats(g, pid, &visible)
            .into_iter()
            .filter(|threat| !threat.sea)
            .collect()
    }

    /// Whether a religious unit standing on `pos` could be condemned before
    /// its next turn. An own city and a tile under a friendly military unit
    /// are refuges; see the module header.
    fn religious_tile_in_reach(
        g: &Game,
        pid: usize,
        pos: Pos,
        threats: &[BarbarianCaptureThreat],
    ) -> bool {
        if threats.is_empty() {
            return false;
        }
        if g.city_at(pos)
            .is_some_and(|city| g.cities[&city].owner == pid)
        {
            return false;
        }
        let guarded = g.unit_ids_at(pos).iter().any(|other| {
            let other = &g.units[other];
            other.owner == pid && g.rules.units[other.kind].class == "military"
        });
        !guarded && Self::barbarian_capture_reaches(g, pos, threats)
    }

    /// How many visible raiders reach `pos`: the tie-break when no neighbour
    /// is out of reach.
    fn raiders_reaching(pos: Pos, threats: &[BarbarianCaptureThreat]) -> usize {
        threats
            .iter()
            .filter(|threat| threat.capture_tiles.contains(&pos))
            .count()
    }

    /// Whether a spreader beside a city it could convert would spend its
    /// last charge on it: the exception to fleeing.
    fn last_charge_spends_here(g: &Game, pid: usize, uid: u32) -> bool {
        let unit = &g.units[&uid];
        if unit.charges != 1 || g.rules.units[unit.kind].religious_spread <= 0.0 {
            return false;
        }
        let Some(religion) = unit
            .religion
            .clone()
            .or_else(|| g.players[pid].religion.clone())
        else {
            return false;
        };
        std::iter::once(unit.pos)
            .chain(g.nbrs(unit.pos))
            .filter_map(|position| g.city_at(position))
            .map(|city| &g.cities[&city])
            .any(|city| {
                Self::city_needs_religious_support(g, pid, city, &religion)
                    || (city.owner != pid
                        && !g.is_at_war(pid, city.owner)
                        && g.city_religion(city) != Some(religion.as_str()))
            })
    }

    /// `missionary_evades_raiders`, the flee: a religious unit standing where
    /// a visible raider can reach it steps to the neighbour that is out of
    /// reach — nearest home first — or, when none is, to the one the fewest
    /// raiders reach. `None` when the gene is off or the tile is safe, so the
    /// caller goes on with its turn; `Some(false)` when nothing legal helps,
    /// which holds the unit. One step per call; the unit loop composes the
    /// rest of the turn, and the next call reads the new tile.
    pub(super) fn religious_unit_evades_raiders(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.missionary_evades_raiders {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if g.rules.units[unit.kind].class != "religious" {
            return None;
        }
        let current = unit.pos;
        let threats = self.religious_raider_threats(g, pid);
        if !Self::religious_tile_in_reach(g, pid, current, &threats) {
            return None;
        }
        if Self::last_charge_spends_here(g, pid, uid) {
            return None;
        }
        let home_distance = |position: Pos| {
            g.player_city_ids(pid)
                .into_iter()
                .map(|city| g.wdist(position, g.cities[&city].pos))
                .min()
                .unwrap_or(0)
        };
        let flight = g
            .nbrs(current)
            .into_iter()
            .filter(|position| g.can_move(uid, *position))
            .map(|position| {
                (
                    Self::religious_tile_in_reach(g, pid, position, &threats),
                    Self::raiders_reaching(position, &threats),
                    home_distance(position),
                    position,
                )
            })
            .min();
        let Some((still_in_reach, reaching, _, position)) = flight else {
            return Some(false);
        };
        let kind = g.units[&uid].kind.as_str();
        let outcome = if still_in_reach {
            format!("is still reached by {reaching} raiders")
        } else {
            "is out of reach".to_string()
        };
        crate::think!(self.journal(), Expansion, Detail,
               "{kind} {uid} steps out of a raider's reach";
               "a religious unit is condemned by the raider that reaches its tile, and \
                this one moves faster than any raider; the step {outcome}";
               position);
        Some(self.base.path_move(g, pid, uid, position))
    }

    /// `missionary_evades_raiders`, the march: `BasicAi::step_toward_range`
    /// for a religious unit that never steps into a visible raider's reach.
    /// The route's own next tile when it is safe; else a safe neighbour that
    /// still closes on the target; else a hold, which is the turn the unit
    /// heals. Exactly the shipped step when the gene is off or no raider is
    /// in sight.
    pub(super) fn religious_step_toward_range(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
        stop_range: i32,
    ) -> bool {
        if !self.missionary_evades_raiders {
            return self.base.step_toward_range(g, pid, uid, target, stop_range);
        }
        let threats = self.religious_raider_threats(g, pid);
        if threats.is_empty() {
            return self.base.step_toward_range(g, pid, uid, target, stop_range);
        }
        let current = g.units[&uid].pos;
        let here = g.wdist(current, target);
        if here <= stop_range {
            return false;
        }
        let route = g
            .route_step(uid, target, stop_range)
            .filter(|next| g.can_move(uid, *next))
            .filter(|next| !Self::religious_tile_in_reach(g, pid, *next, &threats));
        if let Some(next) = route {
            return self.base.path_move(g, pid, uid, next);
        }
        let sidestep = g
            .nbrs(current)
            .into_iter()
            .filter(|position| g.can_move(uid, *position))
            .filter(|position| !Self::religious_tile_in_reach(g, pid, *position, &threats))
            .map(|position| (g.wdist(position, target), position))
            .filter(|(distance, _)| *distance < here)
            .min();
        sidestep.is_some_and(|(_, position)| self.base.path_move(g, pid, uid, position))
    }

    /// The fog tile a version-one last-charge Missionary walks toward: the
    /// reachable dry unexplored tile within [`MISSIONARY_EXPLORE_RADIUS`] that
    /// would reveal the most, nearest first, outside every visible raider's
    /// reach.
    fn missionary_frontier_goal(
        g: &Game,
        pid: usize,
        uid: u32,
        threats: &[BarbarianCaptureThreat],
    ) -> Option<Pos> {
        let origin = g.units[&uid].pos;
        let explored = &g.players[pid].explored;
        g.wdisk(origin, MISSIONARY_EXPLORE_RADIUS)
            .into_iter()
            .filter(|position| *position != origin && !explored.contains(position))
            .filter(|position| {
                g.map
                    .get(*position)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .filter(|position| !Self::religious_tile_in_reach(g, pid, *position, threats))
            .max_by_key(|position| {
                (
                    BasicAi::frontier_reveal_value(g, pid, uid, *position),
                    std::cmp::Reverse(g.wdist(origin, *position)),
                    std::cmp::Reverse(*position),
                )
            })
    }

    /// The version-two expedition target: a reachable dry unexplored tile
    /// within [`MISSIONARY_EXPEDITION_RADIUS`] that reveals the most, then is
    /// farthest away, outside every visible raider's reach. The route check is
    /// intentional: a Missionary can cross foreign closed borders, unlike a
    /// Scout, but it still cannot cross an ocean before embarkation or a
    /// mountain wall at all.
    fn missionary_expedition_goal(
        g: &Game,
        pid: usize,
        uid: u32,
        threats: &[BarbarianCaptureThreat],
    ) -> Option<Pos> {
        let origin = g.units[&uid].pos;
        let explored = &g.players[pid].explored;
        let mut candidates: Vec<Pos> = g
            .wdisk(origin, MISSIONARY_EXPEDITION_RADIUS)
            .into_iter()
            .filter(|position| *position != origin && !explored.contains(position))
            .filter(|position| {
                g.map
                    .get(*position)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .filter(|position| !Self::religious_tile_in_reach(g, pid, *position, threats))
            .collect();
        candidates.sort_by_key(|position| {
            std::cmp::Reverse((
                BasicAi::frontier_reveal_value(g, pid, uid, *position),
                g.wdist(origin, *position),
                std::cmp::Reverse(*position),
            ))
        });
        candidates
            .into_iter()
            .find(|position| g.route_step(uid, *position, 0).is_some())
    }

    /// `missionary_last_charge_explores`: a Missionary on its last charge
    /// walks the fog before spending it. `targets` are the cities the
    /// spreader would otherwise choose between, best first. `None` hands
    /// the turn back to the ordinary spread: the gene is off, the unit is
    /// not a Missionary on its last charge, a city of ours is slipping, an
    /// untouched city stands beside it, the fog within reach is gone, the
    /// exploring turns are spent, or the way to the goal is blocked.
    pub(super) fn last_charge_missionary_explores(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        religion: &str,
        targets: &[Pos],
    ) -> Option<bool> {
        if !self.missionary_last_charge_explores {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.kind != "missionary" || unit.charges != 1 {
            return None;
        }
        let current = unit.pos;
        if g.cities.values().any(|city| {
            city.owner == pid && Self::city_needs_religious_support(g, pid, city, religion)
        }) {
            return None;
        }
        let untouched_beside = targets.iter().any(|target| {
            g.wdist(current, *target) <= 1
                && g.city_at(*target).is_some_and(|city| {
                    g.cities[&city]
                        .pressure
                        .get(religion)
                        .copied()
                        .unwrap_or(0.0)
                        <= 0.0
                })
        });
        if untouched_beside {
            return None;
        }
        let threats = self.religious_raider_threats(g, pid);
        let turn = g.turn;
        let goal = {
            let mut memory = self.missionary_explore.borrow_mut();
            memory.retain(|other, _| g.units.contains_key(other));
            let entry = memory.entry(uid).or_default();
            if entry.turns >= MISSIONARY_EXPLORE_TURNS {
                return None;
            }
            let explored = &g.players[pid].explored;
            let held = entry.goal.filter(|goal| {
                !explored.contains(goal)
                    && g.wdist(current, *goal) <= MISSIONARY_EXPLORE_RADIUS
                    && !Self::religious_tile_in_reach(g, pid, *goal, &threats)
            });
            let Some(goal) = held.or_else(|| Self::missionary_frontier_goal(g, pid, uid, &threats))
            else {
                entry.goal = None;
                return None;
            };
            entry.goal = Some(goal);
            if entry.last_turn != turn {
                entry.last_turn = turn;
                entry.turns += 1;
            }
            goal
        };
        if self.religious_step_toward_range(g, pid, uid, goal, 0) {
            return Some(true);
        }
        None
    }

    /// `missionary_last_charge_explores_2`: a last-charge Missionary makes a
    /// long, routeable expedition before spending the charge. Version one
    /// stays separate so its existing screen history remains a measurement of
    /// the original local policy.
    pub(super) fn last_charge_missionary_expedition(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        religion: &str,
        targets: &[Pos],
    ) -> Option<bool> {
        if !self.missionary_last_charge_explores_2 {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.kind != "missionary" || unit.charges != 1 {
            return None;
        }
        let current = unit.pos;
        if g.cities.values().any(|city| {
            city.owner == pid && Self::city_needs_religious_support(g, pid, city, religion)
        }) {
            return None;
        }
        let untouched_beside = targets.iter().any(|target| {
            g.wdist(current, *target) <= 1
                && g.city_at(*target).is_some_and(|city| {
                    g.cities[&city]
                        .pressure
                        .get(religion)
                        .copied()
                        .unwrap_or(0.0)
                        <= 0.0
                })
        });
        if untouched_beside {
            return None;
        }
        let threats = self.religious_raider_threats(g, pid);
        let turn = g.turn;
        let goal = {
            let mut memory = self.missionary_explore.borrow_mut();
            memory.retain(|other, _| g.units.contains_key(other));
            let entry = memory.entry(uid).or_default();
            if entry.turns >= MISSIONARY_EXPLORE_TURNS {
                return None;
            }
            let explored = &g.players[pid].explored;
            let held = entry.goal.filter(|goal| {
                !explored.contains(goal)
                    && g.wdist(current, *goal) <= MISSIONARY_EXPEDITION_RADIUS
                    && !Self::religious_tile_in_reach(g, pid, *goal, &threats)
                    && g.route_step(uid, *goal, 0).is_some()
            });
            let Some(goal) =
                held.or_else(|| Self::missionary_expedition_goal(g, pid, uid, &threats))
            else {
                entry.goal = None;
                return None;
            };
            entry.goal = Some(goal);
            if entry.last_turn != turn {
                entry.last_turn = turn;
                entry.turns += 1;
            }
            goal
        };
        if self.religious_step_toward_range(g, pid, uid, goal, 0) {
            return Some(true);
        }
        None
    }

    /// The exploring gene's memory for the tests: `(goal, turns)` of a unit.
    #[cfg(test)]
    pub(super) fn missionary_explore_memory(&self, uid: u32) -> Option<(Option<Pos>, u32)> {
        self.missionary_explore
            .borrow()
            .get(&uid)
            .map(|entry| (entry.goal, entry.turns))
    }

    /// The tests' way to spend a unit's exploring turns.
    #[cfg(test)]
    pub(super) fn set_missionary_explore_turns(&self, uid: u32, turns: u32) {
        self.missionary_explore
            .borrow_mut()
            .entry(uid)
            .or_default()
            .turns = turns;
    }

    /// Forget the exploring gene's memory of every unit, for a rebuilt board.
    pub(super) fn forget_missionary_explore(&mut self) {
        self.missionary_explore.get_mut().clear();
    }

    /// Carry the exploring gene's memory across reassigned unit ids.
    pub(super) fn remap_missionary_explore(&mut self, map: &BTreeMap<u32, u32>) {
        let memory = self.missionary_explore.get_mut();
        *memory = memory
            .iter()
            .filter_map(|(uid, entry)| map.get(uid).map(|new| (*new, entry.clone())))
            .collect();
    }
}
