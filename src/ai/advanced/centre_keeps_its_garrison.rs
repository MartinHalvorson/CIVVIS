//! `centre-keeps-its-garrison`: the land soldier standing in the City Center
//! of a threatened city of ours keeps that post, and an empty threatened City
//! Center takes the first soldier of ours that can reach it this turn.
//!
//! A city is threatened while a visible land soldier of a civilization at war
//! with us stands within [`CENTRE_THREAT_REACH`] of its centre. The post is
//! the one tile of the defence no blow can reach: a blow aimed at a City
//! Center lands on the city (`combat_forecast`: "A garrison cannot be
//! targeted"), and the garrison adds its strength to the city's
//! (`Game::city_strength`).
//!
//! **Why (live Deity Gran Colombia, 62 games of 2026-11-12, 100 cities
//! lost).** Census over `events.jsonl` state frames and `combat` rows:
//! - Not one of 2,646 enemy blows on our units in 40 of those games landed on
//!   a unit standing in one of our City Centers.
//! - Our land soldiers started a turn in a City Center with an at-war soldier
//!   within three tiles 5,230 times. They stepped out on 1,507 of them (29%),
//!   left the centre empty on 1,062, and 104 of those who stepped out died in
//!   the enemy phase that followed. The 3,658 who stayed lost none except
//!   with the city itself (61).
//! - Of the 67 cities taken by force, 32 saw a soldier leave the centre in
//!   the last three turns, 10 fell with the centre empty at the end of our
//!   last turn, and 30 fell with a melee soldier rather than a shooter in it.
//!   civvis-20261112T010242Z: Bogotá's only garrison, a full-health
//!   Horseman, rode out at turn 87 to a tile two off; the capital fell that
//!   enemy phase with nobody in it.
//!
//! The Anvil (`anvil-spares-the-one-blow`) already fills the City Center of
//! the one city it defends. Nothing covers the other threatened cities, nor a
//! garrison that is not a member of the Anvil's force: the attack scan, the
//! kill plan's melee blows (which advance on a kill), the Defend force's
//! Engage march and the recovery all walk it out.
//!
//! **What it does,** for every threatened city of ours:
//! 1. **Keep.** A land soldier that starts its step on the City Center ends
//!    the turn there. A shooter fires from the centre at the best visible
//!    target in range (a kill first, then a soldier beside the walls, then the
//!    most damage); a melee soldier makes no blow, since a blow that kills
//!    carries it out. The battle planner offers it only ranged shots from its
//!    own tile. A Scout neither keeps nor fills.
//! 2. **Fill.** A City Center with no land soldier of ours takes the first
//!    soldier within [`CENTRE_FILL_REACH`] that can reach it this turn. The
//!    turn moves shooters before melee, so a shooter in reach fills first. A
//!    settler's guard and a siege member are left to their own work.
//!
//! Off, nothing changes: every reading returns before it looks at the board.

use super::*;
use crate::game::{effective_strength, expected_damage};
use std::cmp::Reverse;

/// A city is threatened while a visible land soldier of a civilization at war
/// with us stands this close to its centre.
pub(crate) const CENTRE_THREAT_REACH: i32 = 3;
/// An empty threatened City Center is filled only by a soldier of ours this
/// close to it, and only one that can step onto it this turn.
pub(crate) const CENTRE_FILL_REACH: i32 = 3;

impl AdvancedAi {
    /// A land soldier of ours that can hold a City Center: military, on land
    /// and not embarked, unlinked, able to fight, and not a Scout.
    pub(super) fn centre_soldier(g: &Game, pid: usize, uid: u32) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let spec = &g.rules.units[unit.kind];
        unit.owner == pid
            && unit.hp > 0
            && spec.class == "military"
            && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            && spec.promotion_class != "recon"
            && unit.linked_to.is_none()
            && !g.is_embarked(unit)
            && (spec.is_melee_capable() || spec.has_ranged_attack())
    }

    /// Whether a visible land soldier of a civilization at war with `pid`
    /// stands within [`CENTRE_THREAT_REACH`] of `at`.
    pub(super) fn centre_threatened(&self, g: &Game, pid: usize, at: Pos) -> bool {
        g.wdisk(at, CENTRE_THREAT_REACH).into_iter().any(|tile| {
            g.unit_ids_at(tile).iter().any(|id| {
                g.units.get(id).is_some_and(|enemy| {
                    let spec = &g.rules.units[enemy.kind];
                    enemy.owner != pid
                        && enemy.hp > 0
                        && spec.class == "military"
                        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                        && g.is_at_war(pid, enemy.owner)
                        && self.battlefront_unit_visible(g, pid, enemy.id)
                })
            })
        })
    }

    /// Whether `uid` keeps the City Center of ours it stands on: see the
    /// module. Always false with the gene off.
    pub(super) fn keeps_the_centre(&self, g: &Game, pid: usize, uid: u32) -> bool {
        if !self.centre_keeps_its_garrison {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        Self::centre_soldier(g, pid, uid)
            && g.city_at(unit.pos)
                .is_some_and(|cid| g.cities[&cid].owner == pid)
            && self.centre_threatened(g, pid, unit.pos)
    }

    /// The best ranged shot `uid` has from where it stands, at a soldier: a
    /// kill first, then one beside the City Center it keeps, then the most
    /// damage. `None` for a unit with no legal shot.
    pub(super) fn centre_keeper_shot(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let unit = g.units.get(&uid)?;
        let spec = &g.rules.units[unit.kind];
        if !spec.has_ranged_attack() || unit.attacks_left <= 0 || unit.moves_left <= 0.0 {
            return None;
        }
        let at = unit.pos;
        let frame = g.player_vision_frame(pid);
        let viewers = g.visibility_viewers(pid);
        let reading =
            |enemy: &crate::game::Unit| effective_strength(g.unit_strength(enemy, true), enemy.hp);
        g.wdisk(at, g.unit_attack_range(uid))
            .into_iter()
            .filter(|tile| g.city_at(*tile).is_none())
            .filter(|tile| g.ranged_order_is_legal(pid, uid, *tile, frame.as_ref(), &viewers))
            .filter_map(|tile| {
                let defender = g
                    .unit_ids_at(tile)
                    .iter()
                    .filter_map(|id| g.units.get(id))
                    .filter(|enemy| {
                        let spec = &g.rules.units[enemy.kind];
                        enemy.owner != pid
                            && g.is_at_war(pid, enemy.owner)
                            && spec.class == "military"
                            && spec.domain.as_deref() != Some("air")
                    })
                    .max_by(|a, b| reading(a).total_cmp(&reading(b)))?;
                let (att, def) = g.ranged_strike_strengths(uid, defender.id, tile)?;
                let damage = expected_damage(att, def);
                let kills = damage >= f64::from(defender.hp);
                let beside = g.wdist(tile, at) <= 1;
                Some((
                    (kills, beside, (damage * 100.0) as i64, Reverse(tile)),
                    tile,
                ))
            })
            .max_by(|a, b| a.0.cmp(&b.0))
            .map(|(_, tile)| tile)
    }

    /// The City Center of a threatened city of ours, within
    /// [`CENTRE_FILL_REACH`], with no land soldier of ours on it, that `uid`
    /// can step onto this turn: the nearest. `None` with the gene off.
    pub(super) fn centre_to_fill(&self, g: &Game, pid: usize, uid: u32) -> Option<(u32, Pos)> {
        if !self.centre_keeps_its_garrison || !Self::centre_soldier(g, pid, uid) {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.moves_left <= 0.0
            || g.city_at(unit.pos)
                .is_some_and(|cid| g.cities[&cid].owner == pid)
        {
            return None;
        }
        let mut open: Vec<(i32, u32, Pos)> = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let at = g.cities[&cid].pos;
                let distance = g.wdist(unit.pos, at);
                (distance <= CENTRE_FILL_REACH).then_some((distance, cid, at))
            })
            .filter(|(_, _, at)| {
                !g.unit_ids_at(*at).iter().any(|id| {
                    g.units.get(id).is_some_and(|other| {
                        let spec = &g.rules.units[other.kind];
                        other.owner == pid
                            && spec.class == "military"
                            && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    })
                })
            })
            .filter(|(_, _, at)| self.centre_threatened(g, pid, *at))
            .collect();
        if open.is_empty()
            || self.guard_is_bound_to_any_settler(uid)
            || self.guard_is_reserved_for_civilian(uid)
            || self.active_siege_member(g, pid, uid)
            || self.staging_siege_member(g, pid, uid)
        {
            return None;
        }
        open.sort();
        let reach = g.reachable(uid);
        open.into_iter()
            .find(|(_, _, at)| reach.contains(at))
            .map(|(_, cid, at)| (cid, at))
    }

    /// The unit's step under the gene: a keeper fires from the centre if it
    /// can (`Some(true)`, and its next pass fortifies) or else fortifies there
    /// (`Some(false)`, its turn is over); a soldier in reach of an empty
    /// threatened centre steps onto it (`Some(true)`). `None` with the gene
    /// off, or when neither applies, and the unit's ordinary step runs.
    pub(super) fn centre_garrison_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.centre_keeps_its_garrison {
            return None;
        }
        if self.keeps_the_centre(g, pid, uid) {
            let kind = g.units[&uid].kind;
            let at = g.units[&uid].pos;
            let city = g.city_at(at).map(|cid| g.cities[&cid].name.clone());
            let shot = self.centre_keeper_shot(g, pid, uid).filter(|target| {
                g.apply(
                    pid,
                    &Action::Ranged {
                        unit: uid,
                        target: *target,
                    },
                )
                .is_ok()
            });
            let city = city.unwrap_or_default();
            if let Some(target) = shot {
                // The next pass of the unit's turn fortifies it in place.
                self.force_groups_dirty = true;
                think!(self.journal(), Military, Decision,
                    "Centre keeps its garrison: the {} holds {} and fires", plain(kind.as_str()), city;
                    "centre-keeps-its-garrison: a soldier at war stands within {CENTRE_THREAT_REACH}; \
                     a blow at a City Center lands on the city, so the post costs nothing; it fires \
                     from the centre at {target:?}");
                return Some(true);
            }
            let fired = g.units.get(&uid).is_none_or(|unit| unit.attacks_left <= 0);
            if !fired {
                think!(self.journal(), Military, Decision,
                    "Centre keeps its garrison: the {} holds {}", plain(kind.as_str()), city;
                    "centre-keeps-its-garrison: a soldier at war stands within {CENTRE_THREAT_REACH}; \
                     a blow at a City Center lands on the city, so the post costs nothing; it makes \
                     no blow that would carry it out");
            }
            if g.units.contains_key(&uid) {
                self.base.fortify_or_stop(g, pid, uid);
            }
            return Some(false);
        }
        let (cid, at) = self.centre_to_fill(g, pid, uid)?;
        let from = g.units[&uid].pos;
        let moved = g.apply(pid, &Action::MoveTo { unit: uid, to: at }).is_ok()
            && g.units.get(&uid).is_some_and(|unit| unit.pos == at);
        if !moved {
            return None;
        }
        self.force_groups_dirty = true;
        think!(self.journal(), Military, Decision,
            "Centre keeps its garrison: the {} fills {}", plain(g.units[&uid].kind.as_str()), g.cities[&cid].name;
            "centre-keeps-its-garrison: nobody of ours stood in the threatened City Center; \
             {from:?} -> {at:?}");
        Some(true)
    }
}

#[cfg(test)]
mod tests;
