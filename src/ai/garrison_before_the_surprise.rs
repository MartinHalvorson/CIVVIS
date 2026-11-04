//! `garrison-before-the-surprise`: when a stronger major's soldiers stand near
//! one of our cities that holds no soldier, the nearest idle soldier walks
//! home and garrisons it, before it explores, patrols or collects villages.
//!
//! ## Why
//!
//! Live Lv 7 on Standard speed, Gran Colombia, 2026-11-04T015124Z: eliminated
//! at turn 18. Vietnam (4.6x our power) declared at turn 17 and took Bogotá
//! at turn 18. The threat was read three turns out; the turn-15 journal says
//! "unlock Ancient Walls for the capital; Vietnam fields 4.6x our power and
//! its soldiers stood within 8 tiles". But the answer was Masonry research,
//! and the empire's only Warrior was exploring ten tiles away. The capital
//! held no garrison (defence 13), and the first Slinger completed on the turn
//! of the declaration. The Standard field's early eliminations all have that
//! shape: 3 of the first 8 Gran Colombia seats died at turns 18, 37 and 132,
//! each to a neighbour that declared while our soldiers stood elsewhere.
//!
//! Movement does not scale with game speed and production does, so at
//! Standard a neighbour's starting army reaches us in half the economy time
//! it takes at Online. A Wall is a twenty-turn answer and a walk home is a
//! three-turn one.
//!
//! ## What the gene does
//!
//! Each turn the seat lists its cities that a met major's land soldier stands
//! within [`GARRISON_CALL_TILES`] of, as the seat sees that soldier this turn,
//! where the major fields at least [`GARRISON_CALL_RATIO`] times our military
//! power. Each such city that holds no soldier of ours is matched, nearest
//! first, to the nearest of our idle land soldiers within
//! [`GARRISON_CALL_RANGE`] tiles. Recon units, embarked units, Settler
//! escorts, and a soldier that is already some city's only garrison are not
//! drafted. A drafted soldier walks to the city ahead of every other
//! peacetime task, and once there it holds while the call stands. Off,
//! nothing here runs and the controller is byte-identical.
use super::BasicAi;
use crate::game::{Game, Unit};
use crate::think;
use crate::Pos;
use std::collections::BTreeSet;

/// How near a stronger major's soldier must stand to call a garrison: the
/// distance `capital-walls-before-the-surprise` watches the capital at.
pub(crate) const GARRISON_CALL_TILES: i32 = 8;

/// How much stronger than us the major must be. A walk home is cheap, so the
/// bar is below the Walls' 2x.
pub(crate) const GARRISON_CALL_RATIO: f64 = 1.5;

/// The farthest a soldier is drafted from.
pub(crate) const GARRISON_CALL_RANGE: i32 = 12;

impl BasicAi {
    /// A land soldier that can fight and is not recon.
    fn garrison_call_soldier(g: &Game, unit: &Unit) -> bool {
        let spec = &g.rules.units[unit.kind];
        spec.class == "military"
            && spec.domain.as_deref().is_none_or(|domain| domain == "land")
            && spec.promotion_class != "recon"
            && (spec.is_melee_capable() || spec.has_ranged_attack())
            && !g.is_embarked(unit)
    }

    /// Our soldiers standing on `pos`.
    fn garrison_call_soldiers_at(g: &Game, pid: usize, pos: Pos) -> Vec<u32> {
        g.unit_ids_at(pos)
            .iter()
            .copied()
            .filter(|uid| {
                g.units
                    .get(uid)
                    .is_some_and(|unit| unit.owner == pid && Self::garrison_call_soldier(g, unit))
            })
            .collect()
    }

    /// Our cities a stronger major's visible soldier stands near this turn.
    /// Cached per turn: the reading is the turn-start frame.
    pub(crate) fn garrison_called_cities(&mut self, g: &Game, pid: usize) -> Vec<Pos> {
        if let Some((turn, cities)) = &self.garrison_call_cache {
            if *turn == g.turn {
                return cities.clone();
            }
        }
        let ours = g.military_power(pid).max(1.0);
        let strong: BTreeSet<usize> = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && !player.is_free_city
                    && g.has_met(pid, player.id)
                    && g.military_power(player.id) >= GARRISON_CALL_RATIO * ours
            })
            .map(|player| player.id)
            .collect();
        let mut cities = Vec::new();
        if !strong.is_empty() {
            let visible = g.player_vision_frame(pid);
            let soldiers: Vec<Pos> = g
                .units
                .values()
                .filter(|unit| {
                    strong.contains(&unit.owner)
                        && Self::garrison_call_soldier(g, unit)
                        && g.sees(&visible, unit.pos)
                        && g.unit_visible_to(unit.id, pid)
                })
                .map(|unit| unit.pos)
                .collect();
            cities = g
                .player_city_ids(pid)
                .into_iter()
                .map(|cid| g.cities[&cid].pos)
                .filter(|pos| {
                    soldiers
                        .iter()
                        .any(|at| g.wdist(*at, *pos) <= GARRISON_CALL_TILES)
                })
                .collect();
        }
        self.garrison_call_cache = Some((g.turn, cities.clone()));
        cities
    }

    /// Which called city, if any, `uid` is drafted to: the called cities
    /// with no soldier, nearest-soldier first, each matched to the nearest
    /// soldier not yet drafted.
    pub(crate) fn garrison_call_draft(
        g: &Game,
        pid: usize,
        called: &[Pos],
        uid: u32,
    ) -> Option<Pos> {
        let garrisons: BTreeSet<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let soldiers = Self::garrison_call_soldiers_at(g, pid, g.cities[&cid].pos);
                (soldiers.len() == 1).then(|| soldiers[0])
            })
            .collect();
        let mut free: Vec<(u32, Pos)> = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && Self::garrison_call_soldier(g, unit)
                    && unit.linked_to.is_none()
                    && !garrisons.contains(&unit.id)
            })
            .map(|unit| (unit.id, unit.pos))
            .collect();
        let mut empty: Vec<Pos> = called
            .iter()
            .copied()
            .filter(|pos| Self::garrison_call_soldiers_at(g, pid, *pos).is_empty())
            .collect();
        // The city with the nearest available soldier is served first, so a
        // soldier between two calls goes to the one it can reach.
        let reach = |pos: Pos, free: &[(u32, Pos)]| {
            free.iter()
                .map(|(_, at)| g.wdist(*at, pos))
                .filter(|distance| *distance <= GARRISON_CALL_RANGE)
                .min()
        };
        while !empty.is_empty() {
            let Some((index, _)) = empty
                .iter()
                .enumerate()
                .filter_map(|(index, pos)| reach(*pos, &free).map(|distance| (index, distance)))
                .min_by_key(|(index, distance)| (*distance, *index))
            else {
                break;
            };
            let city = empty.remove(index);
            let Some(slot) = free
                .iter()
                .enumerate()
                .filter(|(_, (_, at))| g.wdist(*at, city) <= GARRISON_CALL_RANGE)
                .min_by_key(|(_, (id, at))| (g.wdist(*at, city), *id))
                .map(|(slot, _)| slot)
            else {
                continue;
            };
            let (drafted, _) = free.remove(slot);
            if drafted == uid {
                return Some(city);
            }
        }
        None
    }

    /// See the module. `Some(acted)` when the step took this unit, `None` to
    /// leave it to the rest of `peacetime_step`.
    pub(super) fn garrison_before_the_surprise_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.garrison_before_the_surprise || self.minor || self.barb {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if !Self::garrison_call_soldier(g, unit) || unit.linked_to.is_some() {
            return None;
        }
        let upos = unit.pos;
        let called = self.garrison_called_cities(g, pid);
        if called.is_empty() {
            return None;
        }
        // Holding: the only soldier in a called city stays in it.
        if called.contains(&upos) && Self::garrison_call_soldiers_at(g, pid, upos) == [uid] {
            return Some(self.fortify_or_stop(g, pid, uid));
        }
        let city = Self::garrison_call_draft(g, pid, &called, uid)?;
        if self.step_toward(g, pid, uid, city) {
            think!(self.journal, Military, Detail,
                   "A soldier walks home to garrison the city at ({}, {})", city.0, city.1;
                   "garrison-before-the-surprise: a major fielding {GARRISON_CALL_RATIO}x our \
                    power has a soldier within {GARRISON_CALL_TILES} tiles and the city holds none");
            return Some(true);
        }
        None
    }
}

#[cfg(test)]
mod tests;
