//! `defenders-hold-the-walls`: a healthy land soldier within
//! [`WALL_DEFENDER_REACH`] of one of our cities while a hostile land soldier
//! stands within [`WALL_THREAT_REACH`] of it is that city's defender. The
//! battle planner treats it the way it treats an active siege member: the
//! danger at its stand dooms none of its blows, the kill search and the
//! sequence replay keep its strike, and the heal rotation leaves it on its
//! post unless it is wounded (under `battle_planner::ROTATE_HP`).
//!
//! **Why (live Emperor civvis-20261025T070148Z).** Spain declared at turn 84
//! at our power (367 against 344) and took Bogotá, the capital, at 93; we
//! held 5 cities at 84, 2 at 100 and 1 at the end. Through turns 87-92 nine
//! or ten of ours (Archers, Crossbowmen, Pikemen, Warriors) stood within
//! three tiles of the capital against two or three Knights, two or three
//! Trebuchets, a Conquistador and a Great General.
//! - We struck one to three times a turn, spread over different Trebuchets
//!   for 18-38 damage, and killed none; the walls fell 100 → 9 in four turns.
//! - The plan rotated a 100-hp Crossbowman "out to heal" at danger 88, and
//!   "N unit(s) have no blow they would survive" took the rest off the
//!   ladder: every stand in reach of the Knights read lethal.
//! - A shot from beside the walls costs nothing if the city holds and
//!   everything if it falls.
//!
//! Off, nothing changes.

use super::battle_planner::ROTATE_HP;
use super::*;

/// A city's defender stands this close to it.
pub(super) const WALL_DEFENDER_REACH: i32 = 2;
/// A city is threatened while a hostile land soldier stands this close.
pub(super) const WALL_THREAT_REACH: i32 = 2;

impl AdvancedAi {
    /// Whether `uid` holds the walls of a threatened city of ours: see the
    /// module. Always false with the gene off.
    pub(super) fn holds_the_walls(&self, g: &Game, pid: usize, uid: u32) -> bool {
        if !self.defenders_hold_the_walls {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let spec = &g.rules.units[unit.kind];
        if unit.owner != pid
            || unit.hp < ROTATE_HP
            || spec.class != "military"
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
            || g.is_embarked(unit)
        {
            return false;
        }
        // `walls-release-the-one-blow`: a ring defender one blow removes is
        // released to the rotation; see `walls_one_blow_units`.
        if self.walls_release_the_one_blow && self.walls_one_blow.contains(&uid) {
            return false;
        }
        g.player_city_ids(pid).into_iter().any(|cid| {
            let at = g.cities[&cid].pos;
            g.wdist(at, unit.pos) <= WALL_DEFENDER_REACH && Self::walls_threatened(g, pid, at)
        })
    }

    /// `walls-release-the-one-blow`: our land soldiers whose own tile is
    /// one-blow lethal this frame — a visible hostile unit's blow there, at
    /// the top of its roll, meets the unit's health. City and Encampment
    /// strikes are left out (the walls answer those), and a unit standing on
    /// a City Center or a defending district is never listed: the district
    /// takes the blow. Empty with the gene off.
    pub(super) fn walls_one_blow_units(
        &self,
        g: &Game,
        pid: usize,
        field: &mut super::battle_planner::DangerField,
    ) -> BTreeSet<u32> {
        if !self.walls_release_the_one_blow {
            return BTreeSet::new();
        }
        let mut out = BTreeSet::new();
        for uid in g.player_unit_ids(pid) {
            let Some(unit) = g.units.get(&uid) else {
                continue;
            };
            let spec = &g.rules.units[unit.kind];
            if spec.class != "military"
                || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                || g.is_embarked(unit)
                || g.city_at(unit.pos).is_some()
                || g.defending_district_at(unit.pos).is_some()
                || !field.reaches_tile(unit.pos)
            {
                continue;
            }
            let hp = f64::from(unit.hp);
            let lethal = field
                .contributions(unit.pos, uid)
                .iter()
                .any(|(source, blow)| {
                    source.is_some_and(|id| id & super::battle_planner::STRUCTURE_SOURCE == 0)
                        && (blow * crate::ai::COMBAT_ROLL_MAX).ceil().min(100.0) >= hp
                });
            if lethal {
                out.insert(uid);
            }
        }
        out
    }

    /// Whether a hostile land soldier at war with us stands within
    /// [`WALL_THREAT_REACH`] of `at`.
    fn walls_threatened(g: &Game, pid: usize, at: Pos) -> bool {
        g.wdisk(at, WALL_THREAT_REACH).into_iter().any(|tile| {
            g.unit_ids_at(tile).iter().any(|id| {
                g.units.get(id).is_some_and(|enemy| {
                    let spec = &g.rules.units[enemy.kind];
                    enemy.owner != pid
                        && g.is_at_war(pid, enemy.owner)
                        && spec.class == "military"
                        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                })
            })
        })
    }
}

#[cfg(test)]
mod tests;
