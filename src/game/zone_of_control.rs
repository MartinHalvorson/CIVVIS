//! Zones of control: which units exert one, which ignore them, and how they
//! stop a move.
//!
//! `exerts_zoc`, `unit_ignores_zoc`, `in_enemy_zoc`, `enters_enemy_zoc` and
//! `stop_unit_by_zoc` for single units. The `formation_*` checks do the
//! same for an escorted formation (`unit_shares_escort_movement`).
//! `noncombat_action_blocked_by_zoc` covers civilian actions.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

#[derive(Default)]
pub(super) struct ZocMemo {
    owners: BTreeMap<usize, OwnerZocMemo>,
}

#[derive(Default)]
struct OwnerZocMemo {
    ordinary: Option<Vec<Option<bool>>>,
    religious: BTreeMap<String, Vec<Option<bool>>>,
}

impl ZocMemo {
    fn answer(&self, owner: usize, religion: Option<&str>, slot: usize) -> Option<bool> {
        let owner = self.owners.get(&owner)?;
        let answers = match religion {
            Some(religion) => owner.religious.get(religion)?,
            None => owner.ordinary.as_ref()?,
        };
        answers.get(slot).copied().flatten()
    }

    fn remember(
        &mut self,
        owner: usize,
        religion: Option<&str>,
        slot: usize,
        tile_count: usize,
        answer: bool,
    ) {
        let owner = self.owners.entry(owner).or_default();
        let answers = match religion {
            Some(religion) => {
                if !owner.religious.contains_key(religion) {
                    owner
                        .religious
                        .insert(religion.to_owned(), vec![None; tile_count]);
                }
                owner.religious.get_mut(religion).unwrap()
            }
            None => owner.ordinary.get_or_insert_with(|| vec![None; tile_count]),
        };
        if let Some(slot) = answers.get_mut(slot) {
            *slot = Some(answer);
        }
    }
}

/// Share exact destination answers while several units read one fixed board.
/// A nested guard reuses the outer batch; the last owning guard clears it.
pub(crate) struct ZocBatchMemo<'a> {
    game: &'a Game,
    outermost: bool,
}

impl Drop for ZocBatchMemo<'_> {
    fn drop(&mut self) {
        if self.outermost {
            *self.game.query_memo.zoc_batch.borrow_mut() = None;
        }
    }
}

#[cfg(test)]
thread_local! {
    static ZOC_CALCULATIONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

impl Game {
    pub(crate) fn zoc_batch_memo(&self) -> ZocBatchMemo<'_> {
        let outermost = self.query_memo.zoc_batch.borrow().is_none();
        if outermost {
            *self.query_memo.zoc_batch.borrow_mut() = Some(ZocMemo::default());
        }
        ZocBatchMemo {
            game: self,
            outermost,
        }
    }

    pub(super) fn unit_shares_escort_movement(&self, unit: &Unit) -> bool {
        self.promotion_effect(unit, "escort_mobility") > 0.0
            || self.rules.units[unit.kind]
                .effects
                .get("escort_mobility")
                .copied()
                .unwrap_or(0.0)
                > 0.0
    }

    pub(crate) fn exerts_zoc(&self, u: &Unit) -> bool {
        let spec = &self.rules.units[u.kind];
        // Embarkation does not remove a land unit's ZOC. Its native domain
        // still limits projection to land tiles, which is handled by the
        // caller when comparing the target tile's domain.
        spec.zone_of_control || self.promotion_effect(u, "zone_of_control") > 0.0
    }

    /// Cavalry, Naval Raiders, the Viking Longship, and air units ignore
    /// incoming ZOC. Civilian/support passengers inherit that ability from
    /// an escort, matching linked-formation behavior in Civ VI.
    pub(crate) fn unit_ignores_zoc(&self, uid: u32) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        let spec = &self.rules.units[unit.kind];
        let innate = spec.cavalry
            || spec.promotion_class == "naval_raider"
            || spec.domain.as_deref() == Some("air")
            || unit.kind == "viking_longship";
        if innate {
            return true;
        }
        if !matches!(spec.class.as_str(), "civilian" | "support") {
            return false;
        }
        unit.linked_to
            .and_then(|peer| self.units.get(&peer))
            .is_some_and(|peer| {
                let peer_spec = &self.rules.units[peer.kind];
                peer_spec.cavalry
                    || peer_spec.promotion_class == "naval_raider"
                    || peer_spec.domain.as_deref() == Some("air")
                    || peer.kind == "viking_longship"
            })
    }

    fn formation_zoc_stopped(&self, uid: u32) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return true;
        };
        unit.zoc_stopped
            || (self.is_linked_leader(uid)
                && unit
                    .linked_to
                    .and_then(|peer| self.units.get(&peer))
                    .is_some_and(|peer| peer.zoc_stopped))
    }

    pub(super) fn formation_movement_locked_by_zoc(&self, uid: u32) -> bool {
        if self.formation_zoc_stopped(uid) {
            return true;
        }
        self.action_locked_by_zoc(uid)
            || (self.is_linked_leader(uid)
                && self.units[&uid]
                    .linked_to
                    .is_some_and(|peer| self.action_locked_by_zoc(peer)))
    }

    /// The half of [`Self::formation_movement_locked_by_zoc`] one unit can
    /// answer for itself: it began the turn in a zone of control, has acted
    /// there without moving, and is still inside one.
    fn action_locked_by_zoc(&self, id: u32) -> bool {
        self.units.get(&id).is_some_and(|unit| {
            unit.started_turn_in_zoc
                && unit.acted
                && !unit.moved
                && !self.unit_ignores_zoc(id)
                && self.in_enemy_zoc_for(id, unit.pos)
        })
    }

    /// [`Self::formation_movement_locked_by_zoc`] as it will read when this
    /// unit's own next turn begins: with `uid`'s own `zoc_stopped`,
    /// `started_turn_in_zoc`, `acted` and `moved` all cleared, and nothing
    /// else changed.
    ///
    /// Those four are everything a turn's start resets that the lock reads
    /// off the unit itself, so with them cleared the unit's own two terms are
    /// false and what is left is its linked peer's, read as the peer stands.
    /// This is the lock a caller used to learn by writing those four fields
    /// on a copy of the game and asking the ordinary question; see
    /// [`Game::approach_arrivals_on_a_fresh_turn`].
    pub(super) fn formation_movement_locked_on_a_fresh_turn(&self, uid: u32) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return true;
        };
        self.is_linked_leader(uid)
            && unit.linked_to.is_some_and(|peer| {
                self.units.get(&peer).is_some_and(|peer| peer.zoc_stopped)
                    || self.action_locked_by_zoc(peer)
            })
    }

    /// Which units of a formation a tile has to be tested against, decided
    /// without naming one. See [`ZocTest`].
    pub(super) fn formation_zoc_test(&self, uid: u32) -> ZocTest {
        ZocTest {
            mover: (!self.unit_ignores_zoc(uid)).then_some(uid),
            peer: self
                .units
                .get(&uid)
                .filter(|_| self.is_linked_leader(uid))
                .and_then(|unit| unit.linked_to)
                .filter(|peer| !self.unit_ignores_zoc(*peer)),
        }
    }

    /// The tile half of [`Game::formation_enters_enemy_zoc`], once the
    /// formation half is settled.
    pub(super) fn enters_enemy_zoc(&self, test: &ZocTest, pos: Pos) -> bool {
        test.mover
            .is_some_and(|uid| self.in_enemy_zoc_for(uid, pos))
            || test
                .peer
                .is_some_and(|peer| self.in_enemy_zoc_for(peer, pos))
    }

    pub(super) fn formation_enters_enemy_zoc(&self, uid: u32, pos: Pos) -> bool {
        self.enters_enemy_zoc(&self.formation_zoc_test(uid), pos)
    }

    pub(super) fn stop_unit_by_zoc(&mut self, uid: u32) {
        let Some(unit) = self.units.get(&uid) else {
            return;
        };
        let loses_all_movement = matches!(
            self.rules.units[unit.kind].class.as_str(),
            "civilian" | "support"
        );
        let unit = self.units.get_mut(&uid).unwrap();
        unit.zoc_stopped = true;
        if loses_all_movement {
            unit.moves_left = 0.0;
        }
    }

    pub(super) fn noncombat_action_blocked_by_zoc(&self, uid: u32) -> bool {
        self.units.get(&uid).is_some_and(|unit| {
            unit.zoc_stopped
                && matches!(
                    self.rules.units[unit.kind].class.as_str(),
                    "civilian" | "support"
                )
        })
    }

    /// City Centers and every district whose rules data grants ZOC (the
    /// Encampment family and Oppidum in stock Civ VI) project across both
    /// domains and across rivers until pillaged.
    fn defensible_district_owner_at(&self, pos: Pos) -> Option<usize> {
        if let Some(cid) = self.city_at(pos) {
            return self.cities.get(&cid).map(|city| city.owner);
        }
        let tile = self.map.get(pos)?;
        let district = tile.district?;
        let has_zoc = self
            .rules
            .districts
            .get(district.as_str())
            .is_some_and(|spec| spec.effects.get("zone_of_control").copied().unwrap_or(0.0) > 0.0);
        if !has_zoc {
            return None;
        }
        let cid = tile.owner_city?;
        let city = self.cities.get(&cid)?;
        let pillaged = if self.district_is_family(district, crate::name!("encampment")) {
            city.encampment_pillaged
        } else {
            tile.pillaged
        };
        (!pillaged).then_some(city.owner)
    }

    /// Is `pos` inside a military enemy zone of control for player `pid`?
    /// ZOC exists from turn one: it is not unlocked by a technology or civic.
    /// Units only project into their native domain and rivers block projection;
    /// defensible districts project into every adjacent land or water tile.
    pub fn in_enemy_zoc(&self, pid: usize, pos: Pos) -> bool {
        let t = match self.map.get(pos) {
            Some(t) => t,
            None => return false,
        };
        let water = self.rules.is_water(t);
        for n in self.nbrs(pos) {
            if self.map.get(n).is_none() {
                continue;
            }
            for oid in self.unit_ids_at(n) {
                let o = &self.units[oid];
                if o.owner == pid || !self.is_at_war(pid, o.owner) || !self.exerts_zoc(o) {
                    continue;
                }
                let o_water = self.rules.units[o.kind].domain.as_deref() == Some("sea");
                if (o.kind != "giant_death_robot" && o_water != water) || self.crosses_river(n, pos)
                {
                    continue;
                }
                return true;
            }
            if self
                .defensible_district_owner_at(n)
                .is_some_and(|owner| owner != pid && self.is_at_war(pid, owner))
            {
                return true;
            }
        }
        false
    }

    pub(super) fn in_enemy_zoc_for(&self, uid: u32, pos: Pos) -> bool {
        // A flood already keeps its own per-destination answer. A batch also
        // shares it across different units: the mover contributes only its
        // owner and, when religious, its religion to the unchanged predicate.
        let active = self.query_memo.zoc_batch.borrow().is_some();
        if !active || self.query_memo.policy_reads_on.get() {
            return self.in_enemy_zoc_for_uncached(uid, pos);
        }
        let mover = &self.units[&uid];
        let spec = &self.rules.units[mover.kind];
        let Some(slot) = self.map.tiles.index_of(pos) else {
            return self.in_enemy_zoc_for_uncached(uid, pos);
        };
        let religion = mover
            .religion
            .as_deref()
            .filter(|_| spec.class == "religious");
        let cached = self
            .query_memo
            .zoc_batch
            .borrow()
            .as_ref()
            .and_then(|memo| memo.answer(mover.owner, religion, slot));
        if let Some(answer) = cached {
            return answer;
        }
        let answer = self.in_enemy_zoc_for_uncached(uid, pos);
        if let Some(memo) = self.query_memo.zoc_batch.borrow_mut().as_mut() {
            memo.remember(mover.owner, religion, slot, self.map.tiles.len(), answer);
        }
        answer
    }

    fn in_enemy_zoc_for_uncached(&self, uid: u32, pos: Pos) -> bool {
        #[cfg(test)]
        ZOC_CALCULATIONS.with(|count| count.set(count.get() + 1));
        let mover = &self.units[&uid];
        let mover_spec = &self.rules.units[mover.kind];
        let Some(t) = self.map.get(pos) else {
            return false;
        };
        let water = self.rules.is_water(t);
        // The mover's half of the religious test is the same at all six
        // neighbours and for every unit standing on them, and the occupancy
        // list is read, not kept — `units_at` would copy it into a fresh
        // `Vec` once per neighbour of every tile a flood expands.
        let mover_religious = mover_spec.class == "religious" && mover.religion.is_some();
        for n in self.nbrs(pos) {
            for oid in self.unit_ids_at(n) {
                let other = &self.units[oid];
                if other.owner == mover.owner {
                    continue;
                }
                let other_spec = &self.rules.units[other.kind];
                let religious_zoc = mover_religious
                    && other_spec.class == "religious"
                    && other.religion.is_some()
                    && mover.religion != other.religion;
                let military_zoc =
                    self.is_at_war(mover.owner, other.owner) && self.exerts_zoc(other);
                if !religious_zoc && !military_zoc {
                    continue;
                }
                let other_water = other_spec.domain.as_deref() == Some("sea");
                if (other.kind == "giant_death_robot" || other_water == water)
                    && !self.crosses_river(n, pos)
                {
                    return true;
                }
            }
            if self
                .defensible_district_owner_at(n)
                .map(|owner| owner != mover.owner && self.is_at_war(mover.owner, owner))
                .unwrap_or(false)
            {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod batch_memo_tests;
