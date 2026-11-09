//! `religion-seat-marches-in-a-pack`: a Religion-target founder's spreaders
//! gather at home before a long walk through rival Apostles.
//!
//! **Why (live Khmer Deity civvis-20261109T070122Z).** The seat bought 56
//! religious units and earned 11,900 Faith, and 27 of them died with charges
//! left, 14 with a rival Apostle or Inquisitor in sight at last report, 7 of
//! those adjacent. Rival Apostles carry promotions and arrive in numbers. Ours
//! left the Holy Site one at a time. A single Missionary cannot fight, and the
//! only unit that can, the Apostle, takes a winnable fight only when it stands
//! beside the rival (`religion-seat-presses-the-campaign`).
//!
//! **What it does.** A spreader of a Religion-target founder near home, about
//! to walk more than [`PACK_TRAVEL`] tiles to a foreign target while a rival
//! Apostle or Inquisitor is in sight, waits until [`PACK_SIZE`] units of its
//! faith stand within [`PACK_RADIUS`] tiles, an Apostle among them. It waits
//! at most [`PACK_WAIT_TURNS`] turns, once, and it never waits with no such
//! rival in sight, so a safe road stays open. A spread on or beside a city and
//! every home target are untouched.
//!
//! Off, or for a seat aiming elsewhere, nothing changes.

use super::*;

/// Units of our faith within this many tiles count as one pack.
pub(crate) const PACK_RADIUS: i32 = 2;

/// Units a pack needs before it leaves, the waiting unit included.
pub(crate) const PACK_SIZE: usize = 3;

/// A foreign target at or inside this distance is walked to at once.
pub(crate) const PACK_TRAVEL: i32 = 4;

/// A unit farther than this from every city of ours is already on the road.
pub(crate) const PACK_HOME_RADIUS: i32 = 3;

/// The longest one unit waits for its pack.
pub(crate) const PACK_WAIT_TURNS: u32 = 6;

impl AdvancedAi {
    /// Whether `uid` stands in a full pack: enough units of its faith close
    /// by, at least one an Apostle.
    fn religion_seat_pack_ready(g: &Game, pid: usize, uid: u32, faith: &str) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let pack: Vec<&crate::game::Unit> = g
            .units
            .values()
            .filter(|other| {
                other.owner == pid
                    && g.rules.units[other.kind].class == "religious"
                    && other.religion.as_deref() == Some(faith)
                    && g.wdist(other.pos, unit.pos) <= PACK_RADIUS
            })
            .collect();
        pack.len() >= PACK_SIZE && pack.iter().any(|other| other.kind == "apostle")
    }

    /// See the module. `true` holds this unit for the turn.
    pub(super) fn religion_seat_holds_for_the_pack(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        if !self.religion_seat_marches_in_a_pack
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
        {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let Some(faith) = unit.religion.as_deref() else {
            return false;
        };
        if g.players[pid].religion.as_deref() != Some(faith)
            || g.wdist(unit.pos, target) <= PACK_TRAVEL
            || g.city_at(target)
                .is_none_or(|cid| g.cities[&cid].owner == pid)
            || !g
                .player_city_ids(pid)
                .iter()
                .any(|cid| g.wdist(unit.pos, g.cities[cid].pos) <= PACK_HOME_RADIUS)
            || self.theological_threats(g, pid, uid).is_empty()
            || Self::religion_seat_pack_ready(g, pid, uid, faith)
        {
            return false;
        }
        let first = *self
            .pack_wait_started
            .borrow_mut()
            .entry(uid)
            .or_insert(g.turn);
        if g.turn >= first + PACK_WAIT_TURNS {
            return false;
        }
        think!(self.journal(), Faith, Detail,
            "{} {uid} waits at home for its pack", unit.kind;
            "religion-seat-marches-in-a-pack: a rival Apostle or Inquisitor is in sight and {} tiles lie ahead; turn {} of {} waiting",
            g.wdist(unit.pos, target), g.turn - first + 1, PACK_WAIT_TURNS);
        true
    }
}

#[cfg(test)]
mod tests;
