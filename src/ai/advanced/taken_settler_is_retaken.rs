//! `taken-settler-is-retaken`: a hostile unit standing on a civilian it took
//! from us is worth striking for the civilian, not only for itself.
//!
//! **The finding (hotseat `hotseat-20261128T031456Z`, seat p1, Greece, the
//! religion seat, 2026-11-27).** The person playing Sumeria walked a Scout
//! beside p1's first Settler, declared a surprise war and took the Settler on
//! turn 12. The p1 board of turn 12 (`events.jsonl` line 290) shows the
//! person's Scout (strength 10, full health) and the captured Settler stacked
//! on (60, 29), one tile from p1's Warrior on (61, 29) with both moves. The
//! decider asked the host for the price of that blow (`preview`, line 294:
//! 32 damage dealt, 28 taken) and fortified instead (`decisions.jsonl`, turn
//! 12: `fortify` for the Warrior; line 331). Two turns later the stack had
//! walked to (60, 27) and the Settler was gone for good.
//!
//! The objective board did raise a row ("Destroy 1 of scout (value 30, need
//! 15, force #3 of 1)", `why.log` turn 12) and sent the Warrior to engage it.
//! The blow itself was declined by the battle planner (`battle-planner-3`),
//! which prices a strike as the hostile's own worth: a 32-point wound on a
//! Scout earns about 41 on its scale against about 49 for the 28 points the
//! Warrior takes back and the Scout's reply next turn. Nothing anywhere
//! counted the Settler standing on the Scout's tile, though a kill would have
//! freed it. The capture lessons did not see the loss either: our capital
//! stood two tiles from where the Settler vanished, so
//! `resolve_vanished_settlers` read it as a founding and scarred nothing.
//!
//! **The gene.** Inert with the flag off.
//! - Each turn ([`AdvancedAi::observe_taken_civilians`]) the seat notes its
//!   Settlers and Builders. One that is gone at the next turn, where no city
//!   of ours now stands, was taken when a civilian of a rival at war with us
//!   (or of the barbarians) that was not in sight before stands within
//!   [`TAKEN_REACH`] of where ours was last seen; one the board still carries
//!   under another owner (a native board keeps the id) was taken too. The
//!   captive is remembered by its captor and its host identity, so it is
//!   followed through every rebuilt board, for [`TAKEN_MEMORY_TURNS`]
//!   standard turns or until its captor is at peace with us.
//! - The battle planner adds the captive's worth ([`captive_value`]: a
//!   Settler's cost plus `SETTLER_PREMIUM`, the city it would have been; a
//!   Builder's cost) to the hostile defending its tile: all of it for a
//!   kill, its share for a wound (damage over the defender's hit points).
//!   Every veto stands: a blow the host says kills the attacker is still
//!   refused, and a wounded attacker still strikes only to finish.
//! - The objective board adds the same worth to the Destroy row of the force
//!   standing on it, a nearby Defend row no longer covers that force, and a
//!   barbarian holding a captive is on the board wherever it stands.
//!
//! On the live board the blow reads about 131 against 49 and is taken. It
//! does not touch the per-unit ladder, which plays only the units the
//! battle planner leaves to it.

use super::*;

/// Standard-speed turns a captive stays worth retaking: long enough for a
/// captor walking a Settler home to stay a target, short enough that a
/// captive gone into the fog is not hunted for the rest of the game.
pub(super) const TAKEN_MEMORY_TURNS: u32 = 20;

/// Tiles from where our civilian was last seen within which a newly seen
/// foreign civilian is taken to be ours. A captured unit appears on the tile
/// it was taken on and has no moves that turn; two allows for the host
/// spending the captor's remaining moves on the stack.
pub(super) const TAKEN_REACH: i32 = 2;

/// Whether `kind` is a civilian a hostile can take on the open map.
fn capturable(kind: Name) -> bool {
    kind == "settler" || kind == "builder"
}

/// The identity a captive keeps across rebuilt boards: its owner, and its
/// host id on a mirrored board or its board id on a native one.
pub(super) fn civilian_identity(g: &Game, uid: u32) -> Option<(usize, i64)> {
    let unit = g.units.get(&uid)?;
    let native = g
        .host_unit_facts
        .get(&uid)
        .and_then(|facts| facts.civ6_id)
        .unwrap_or_else(|| i64::from(uid));
    Some((unit.owner, native))
}

/// What freeing one captive of `kind` is worth, in hammers: a Settler's cost
/// plus the city it would have been (`SETTLER_PREMIUM`, its own Escort row's
/// worth), a Builder's cost.
pub(super) fn captive_value(g: &Game, kind: Name) -> f64 {
    let cost = g.rules.units.get(kind).map_or(0.0, |spec| spec.cost);
    if kind == "settler" {
        cost + objective_board::SETTLER_PREMIUM
    } else {
        cost
    }
}

impl AdvancedAi {
    /// Which of our civilians a hostile took since the last turn. Reads the
    /// civilians noted at the last call, records each one taken in
    /// `taken_civilians`, forgets the captives past their time, and notes
    /// this turn's civilians and the foreign civilians in sight for the next
    /// call. Exact no-op with the gene off.
    pub(super) fn observe_taken_civilians(&mut self, g: &Game, pid: usize) {
        if !self.taken_settler_is_retaken {
            return;
        }
        let turn = g.turn;
        let horizon = g.standard_duration(TAKEN_MEMORY_TURNS).max(1);
        let mut vanished = std::mem::take(&mut self.taken_civilian_vanished);
        let mut taken: Vec<(usize, i64, Pos, Name)> = Vec::new();
        for (uid, (pos, kind)) in std::mem::take(&mut self.taken_civilian_watch) {
            match g.units.get(&uid) {
                None => vanished.push((pos, kind)),
                // A native board keeps the unit and hands it to its captor.
                Some(unit) if unit.owner != pid => {
                    if let Some((owner, native)) = civilian_identity(g, uid) {
                        taken.push((owner, native, unit.pos, unit.kind));
                    }
                }
                Some(_) => {}
            }
        }
        let foreign: Vec<(u32, (usize, i64))> = g
            .units
            .values()
            .filter(|unit| {
                unit.owner != pid && capturable(unit.kind) && g.unit_visible_to(unit.id, pid)
            })
            .filter_map(|unit| civilian_identity(g, unit.id).map(|identity| (unit.id, identity)))
            .collect();
        for (pos, kind) in vanished {
            // A Settler that founded where it stood left our city there.
            if g.city_at(pos)
                .is_some_and(|cid| g.cities[&cid].owner == pid)
            {
                continue;
            }
            let captor = foreign
                .iter()
                .copied()
                .filter(|&(uid, identity)| {
                    let unit = &g.units[&uid];
                    !self.taken_civilian_foreign.contains(&identity)
                        && g.is_at_war(pid, unit.owner)
                        && g.wdist(unit.pos, pos) <= TAKEN_REACH
                })
                .min_by_key(|&(uid, identity)| (g.wdist(g.units[&uid].pos, pos), identity));
            if let Some((uid, (owner, native))) = captor {
                taken.push((owner, native, g.units[&uid].pos, kind));
            }
        }
        for (owner, native, at, kind) in taken {
            if self.taken_civilians.insert((owner, native), turn).is_none() {
                think!(self.journal(), Military, Decision,
                    "Our {} was taken at {:?}", plain(&kind), at;
                    "taken-settler-is-retaken: the hostile standing on it is worth the {} \
                     ({:.0}) for {} standard turns",
                    plain(&kind), captive_value(g, kind), TAKEN_MEMORY_TURNS; at);
            }
        }
        self.taken_civilians.retain(|(owner, _), since| {
            *since <= turn && turn < since.saturating_add(horizon) && g.is_at_war(pid, *owner)
        });
        self.taken_civilian_foreign = foreign.into_iter().map(|(_, identity)| identity).collect();
        self.taken_civilian_watch = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                capturable(unit.kind).then_some((uid, (unit.pos, unit.kind)))
            })
            .collect();
    }

    /// Carry the watched civilians through a rebuilt board: one the board
    /// still carries is re-keyed, one it dropped waits, by its last tile, for
    /// the next [`Self::observe_taken_civilians`]. Empty with the gene off.
    pub(super) fn remap_taken_civilian_watch(&mut self, map: &BTreeMap<u32, u32>) {
        for (uid, seen) in std::mem::take(&mut self.taken_civilian_watch) {
            match map.get(&uid) {
                Some(next) => {
                    self.taken_civilian_watch.insert(*next, seen);
                }
                None => self.taken_civilian_vanished.push(seen),
            }
        }
    }

    /// A new game, or a board with no predecessor: nothing is remembered.
    pub(super) fn forget_taken_civilians(&mut self) {
        self.taken_civilians.clear();
        self.taken_civilian_watch.clear();
        self.taken_civilian_vanished.clear();
        self.taken_civilian_foreign.clear();
    }

    /// What freeing the civilians on `pos` is worth: each one taken from us
    /// at [`captive_value`]. Zero with the gene off.
    pub(super) fn taken_civilian_value_at(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        if !self.taken_settler_is_retaken || self.taken_civilians.is_empty() {
            return 0.0;
        }
        g.unit_ids_at(pos)
            .iter()
            .filter_map(|uid| {
                let unit = g.units.get(uid)?;
                if unit.owner == pid || !capturable(unit.kind) {
                    return None;
                }
                let identity = civilian_identity(g, *uid)?;
                self.taken_civilians
                    .contains_key(&identity)
                    .then(|| captive_value(g, unit.kind))
            })
            .sum()
    }

    /// The same over every tile a body of a hostile force stands on.
    pub(super) fn taken_civilian_value_in(&self, g: &Game, pid: usize, force: &[u32]) -> f64 {
        if !self.taken_settler_is_retaken || self.taken_civilians.is_empty() {
            return 0.0;
        }
        let mut tiles: Vec<Pos> = force
            .iter()
            .filter_map(|uid| g.units.get(uid))
            .map(|unit| unit.pos)
            .collect();
        tiles.sort_unstable();
        tiles.dedup();
        tiles
            .into_iter()
            .map(|pos| self.taken_civilian_value_at(g, pid, pos))
            .sum()
    }

    /// Whether a civilian taken from us stands on `pos`. False with the gene
    /// off.
    pub(super) fn holds_a_taken_civilian(&self, g: &Game, pid: usize, pos: Pos) -> bool {
        self.taken_civilian_value_at(g, pid, pos) > 0.0
    }
}

#[cfg(test)]
mod tests;
