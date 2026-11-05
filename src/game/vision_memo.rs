//! Vision memo: the stamps and frames that let a player's vision be reused
//! instead of recomputed.
//!
//! Stamps say what a vision result depends on: the world roster
//! (`world_stamp`), the map's vision geometry, each unit's vision inputs,
//! spies, the diplomacy epoch, and the suzerain map
//! (`vision_input_stamp`, `vision_input_stamp_with_suzerains`). The memo
//! keys them (`vision_input_memo_key`), matches and stores them, and keeps
//! the vision frames built from them (`vision_frame`, `player_vision_frame`,
//! `store_vision_frame`, `matching_vision_frame`). The suzerain read memo
//! lives here too. `player_vision` and `sees`, which read the frames, stay
//! with the rest of the vision API.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! and `pub(crate)` methods are unchanged; the code is otherwise unchanged.

use super::*;

impl Game {
    /// A number that changes whenever anything a sweep reads could have
    /// changed: the map itself, and the cities whose centers and Encampments
    /// look out over it.
    pub(super) fn world_stamp(&self) -> u64 {
        // Same roster walk as [`Self::vision_geometry_stamp`], and on the
        // hotter path of the two: `height_field()` asks for this, and
        // `player_can_see` builds a height field per question.  Every ask used
        // to rewalk every city and every district — allocating a `Vec` per
        // city on the way, because `&Districts` collects before it iterates —
        // so it reads the roster's own fold instead.
        vision_key(&[
            self.map.tiles.epoch(),
            self.turn as u64,
            self.cities.vision_stamps().geometry,
        ])
    }

    /// Fold only map fields that a sight ray reads.  Roads, improvements,
    /// resources, and ownership can all open the map for writing without
    /// changing a ray; the tile epoch is therefore just the memo's invalidator
    /// rather than part of the resulting geometry stamp.
    fn map_vision_geometry_stamp(&self) -> u64 {
        let epoch = self.map.tiles.epoch();
        if let Some((cached_epoch, stamp)) = *self.vision_frames.map_geometry.borrow() {
            if cached_epoch == epoch {
                return stamp;
            }
        }
        let mut stamp = vision_key(&[self.map.tiles.len() as u64]);
        for tile in self.map.tiles.values() {
            stamp = vision_key(&[
                stamp,
                tile.pos.0 as i64 as u64,
                tile.pos.1 as i64 as u64,
                tile.terrain.id() as u64,
                tile.feature.map_or(u32::MAX, Name::id) as u64,
                tile.hills as u64,
                tile.district.map_or(u32::MAX, Name::id) as u64,
                tile.owner_city.unwrap_or(u32::MAX) as u64,
            ]);
        }
        *self.vision_frames.map_geometry.borrow_mut() = Some((epoch, stamp));
        stamp
    }

    /// The part of [`Self::world_stamp`] that can alter a sight ray.  A turn
    /// boundary changes remembered-tile timestamps, but it does not change a
    /// unit's line of sight; keeping that clock out of this stamp is what lets
    /// the compact frame survive a turn when all sight sources stand still.
    pub(super) fn vision_geometry_stamp(&self) -> u64 {
        let topology = match self.map.topology {
            crate::world::Topology::Cylinder => 0,
            crate::world::Topology::Rectangle => 1,
            crate::world::Topology::Globe(frequency) => 2_u64.wrapping_add(frequency as i64 as u64),
        };
        let stamp = vision_key(&[
            self.map_vision_geometry_stamp(),
            self.map.width as i64 as u64,
            self.map.height as i64 as u64,
            topology,
            self.is_arena() as u64,
            self.tactics.fog as u64,
        ]);
        // `see_from_level` reads exactly these city facts in addition to the
        // map.  Keep the geometry stamp in lockstep with the unit-ray cache's
        // existing world stamp, but leave the turn counter out as explained
        // above.  The walk itself lives on [`Cities`], which re-folds it only
        // after a city has been exposed mutably; every sight ask used to pay
        // for it, districts included, and iterating `&city.districts`
        // allocated a `Vec` per city per ask on the way.
        vision_key(&[stamp, self.cities.vision_stamps().geometry])
    }

    /// Hash one viewer's sight sources into a compact, position-independent
    /// input signature.  This deliberately hashes only fields read by
    /// [`base_player_visibility`]: combat HP, movement points, and the rest of
    /// a unit's mutable state cannot change its sight and therefore do not
    /// evict the frame.
    fn base_vision_input_stamp(
        &self,
        viewer: usize,
        suzerains: &BTreeMap<usize, Option<usize>>,
        unit_stamps: &[u64],
    ) -> u64 {
        let mut stamp = unit_stamps
            .get(viewer)
            .copied()
            .unwrap_or_else(|| vision_key(&[viewer as u64]));
        // A seat's own borders are the widest input here: one ask used to
        // hash every tile of every city it owns, roughly 8.6 M tile hashes
        // over a game.  Borders move rarely, so [`Cities`] folds them once per
        // roster state and this reads the owner's number.
        let city_stamps = self.cities.vision_stamps();
        stamp = vision_key(&[stamp, city_stamps.owned(viewer)]);
        // A suzerain's city-state ring is a sight source.  Hash only the
        // relationships that actually reveal ground; changes to envoys that
        // leave the same city-state under the same suzerain do not force a
        // ray walk.  The surrounding refresh installs the suzerain memo, so
        // this poll is one cheap lookup per minor in the common path.
        for minor in self.players.iter().filter(|minor| {
            minor.alive
                && minor.is_minor
                && !minor.is_barbarian
                && suzerains.get(&minor.id).copied().flatten() == Some(viewer)
        }) {
            stamp = vision_key(&[stamp, 4, minor.id as u64, city_stamps.centres(minor.id)]);
        }
        for spy in self.spies.values().filter(|spy| {
            spy.owner == viewer && spy.captured_by.is_none() && spy.ready_turn <= self.turn
        }) {
            stamp = vision_key(&[stamp, 5, spy.id as u64, spy.city.unwrap_or(u32::MAX) as u64]);
            if let Some(position) = spy
                .city
                .and_then(|city| self.cities.get(&city).map(|city| city.pos))
            {
                stamp = vision_key(&[stamp, position.0 as i64 as u64, position.1 as i64 as u64]);
            }
        }
        stamp
    }

    fn collect_unit_vision_input_stamps(&self) -> Vec<u64> {
        let mut stamps: Vec<u64> = (0..self.players.len())
            .map(|viewer| vision_key(&[viewer as u64]))
            .collect();
        for unit in self.units.values() {
            let Some(stamp) = stamps.get_mut(unit.owner) else {
                continue;
            };
            *stamp = vision_key(&[
                *stamp,
                1,
                unit.id as u64,
                unit.kind.id() as u64,
                unit.pos.0 as i64 as u64,
                unit.pos.1 as i64 as u64,
                unit.air_patrol_pos.is_some() as u64,
                unit.air_patrol_pos
                    .map(|pos| pos.0 as i64 as u64)
                    .unwrap_or_default(),
                unit.air_patrol_pos
                    .map(|pos| pos.1 as i64 as u64)
                    .unwrap_or_default(),
            ]);
            for promotion in &unit.promotions {
                *stamp = vision_key(&[*stamp, 2, promotion.id() as u64]);
            }
        }
        stamps
    }

    /// Reuse the all-viewer unit stamp fan-out until any unit has been
    /// exposed mutably.  The epoch intentionally over-invalidates: a hitpoint
    /// write does not alter sight, but it is always safer to recompute the
    /// small signature than to miss a moved, promoted, transferred, or
    /// patrolling unit.  The signature still keeps the existing frame when
    /// the actual sight inputs are unchanged.
    fn with_unit_vision_input_stamps<R>(&self, read: impl FnOnce(&[u64]) -> R) -> R {
        let epoch = self.units.vision_epoch();
        let stale = match self.vision_frames.unit_stamps.borrow().as_ref() {
            Some((cached_epoch, stamps)) => {
                *cached_epoch != epoch || stamps.len() != self.players.len()
            }
            None => true,
        };
        if stale {
            let stamps = self.collect_unit_vision_input_stamps();
            *self.vision_frames.unit_stamps.borrow_mut() = Some((epoch, stamps));
        }
        let cache = self.vision_frames.unit_stamps.borrow();
        let (_, stamps) = cache
            .as_ref()
            .expect("unit vision stamps are installed before use");
        read(stamps)
    }

    /// Stamp every input to one player's current sight.  The result is small
    /// enough to carry beside a cached [`TileBits`] frame, while the fields it
    /// folds are the exact source identities and positions the visibility
    /// derivation reads.  The viewer set is included because shared vision is
    /// itself a sight input (teams, alliances, and emergencies can add or
    /// remove a live viewer without moving a unit).
    pub(super) fn suzerain_input_map(&self) -> BTreeMap<usize, Option<usize>> {
        self.players
            .iter()
            .filter(|player| player.is_minor && !player.is_barbarian)
            .map(|minor| (minor.id, self.suzerain_of(minor.id)))
            .collect()
    }

    /// A number that changes whenever anything `suzerain_input_map` or
    /// `visibility_viewers` reads, besides units and map geometry (which
    /// already have their own epochs above), could have changed: envoys,
    /// governor assignments, a governor's city changing hands, team
    /// membership and alliances all move only through a `Players` write; a
    /// turn boundary matters too, because alliance/emergency expiry and
    /// governor establishment are all `self.turn` comparisons. The city
    /// signature supplements the mutation generation so a whole roster
    /// replacement cannot accidentally reuse a relationship map from a
    /// different set of city owners.
    ///
    /// Emergency membership is folded directly rather than through a bumped
    /// counter: `active_emergencies` is a plain `pub Vec`, not a wrapper with
    /// a closed mutation surface like `Players` or `Cities` above, and a
    /// couple of engine paths (plus several tests) write it in place. The
    /// vector holds at most a handful of live emergencies, so folding the two
    /// fields `visibility_viewers` actually reads (`ends`, `members`) costs
    /// nothing close to the `BTreeSet` allocation this is guarding, and it
    /// cannot miss a write the way a counter at only two call sites could.
    pub(super) fn diplomacy_epoch(&self) -> u64 {
        let city_stamps = self.cities.vision_stamps();
        let mut stamp = vision_key(&[
            self.players.epoch(),
            self.cities.generation(),
            city_stamps.input,
            self.turn as u64,
            self.active_emergencies.len() as u64,
        ]);
        for emergency in &self.active_emergencies {
            stamp = vision_key(&[stamp, emergency.ends as u64, emergency.members.len() as u64]);
            for member in &emergency.members {
                stamp = vision_key(&[stamp, *member as u64]);
            }
        }
        stamp
    }

    /// A compact fingerprint of the live spy fields visibility reads. Spies
    /// remain a public `BTreeMap`, rather than a roster with an invalidation
    /// epoch, so this deliberately folds the tiny relevant view on every
    /// input-memo lookup. It is still much cheaper than having each shared
    /// viewer rescan the map while rebuilding its full input signature.
    fn spies_vision_input_stamp(&self) -> u64 {
        let mut stamp = vision_key(&[0x6b, self.spies.len() as u64]);
        for (&map_id, spy) in &self.spies {
            stamp = vision_key(&[
                stamp,
                map_id as u64,
                spy.id as u64,
                spy.owner as u64,
                spy.captured_by
                    .map(|owner| owner as u64)
                    .unwrap_or(u64::MAX),
                spy.ready_turn as u64,
                spy.city.unwrap_or(u32::MAX) as u64,
            ]);
        }
        stamp
    }

    /// The broad inputs that decide whether one pid's exact final visibility
    /// signature may be reused. A visible non-fogged arena reads none of the
    /// roster, diplomacy, or spy inputs, so leave those at stable sentinels.
    fn vision_input_memo_key(&self) -> VisionInputMemoKey {
        let geometry = self.vision_geometry_stamp();
        if self.is_arena() && !self.tactics.fog {
            return VisionInputMemoKey {
                geometry,
                units: 0,
                diplomacy: 0,
                spies: 0,
            };
        }
        VisionInputMemoKey {
            geometry,
            units: self.units.vision_epoch(),
            diplomacy: self.diplomacy_epoch(),
            spies: self.spies_vision_input_stamp(),
        }
    }

    fn matching_vision_input_stamp(&self, pid: usize, key: VisionInputMemoKey) -> Option<u64> {
        self.vision_frames
            .input_stamps
            .borrow()
            .as_ref()
            .filter(|(cached_key, _)| *cached_key == key)
            .and_then(|(_, stamps)| stamps.get(pid).copied().flatten())
    }

    fn store_vision_input_stamp(&self, pid: usize, key: VisionInputMemoKey, stamp: u64) {
        let mut cache = self.vision_frames.input_stamps.borrow_mut();
        match cache.as_mut() {
            Some((cached_key, stamps)) if *cached_key == key => {
                if stamps.len() <= pid {
                    stamps.resize_with(pid + 1, || None);
                }
                stamps[pid] = Some(stamp);
            }
            _ => {
                let mut stamps = vec![None; self.players.len().max(pid + 1)];
                stamps[pid] = Some(stamp);
                *cache = Some((key, stamps));
            }
        }
    }

    /// Read one pid's exact frame signature, rebuilding it only after a
    /// conservative input changes. `host_observed` is an externally writable
    /// `Arc<BTreeSet<_>>` with no mutation epoch, so its populated mirrored
    /// seat deliberately takes the original exact path instead of risking a
    /// stale live-host observation.
    pub(super) fn with_vision_input_stamp<R>(&self, pid: usize, read: impl FnOnce(u64) -> R) -> R {
        let key = (pid != MIRRORED_SEAT || self.host_observed.is_empty())
            .then(|| self.vision_input_memo_key());
        if let Some(key) = key {
            if let Some(stamp) = self.matching_vision_input_stamp(pid, key) {
                return read(stamp);
            }
        }
        self.with_suzerain_read_memo(|| {
            self.with_suzerain_input_map(|suzerains| {
                self.with_unit_vision_input_stamps(|unit_stamps| {
                    let stamp = self.vision_input_stamp_with_suzerains(pid, suzerains, unit_stamps);
                    if let Some(key) = key {
                        self.store_vision_input_stamp(pid, key, stamp);
                    }
                    read(stamp)
                })
            })
        })
    }

    /// Reuse the folded suzerain-of-every-minor map until the diplomacy
    /// epoch moves. `suzerain_of` is already a cheap lookup once the
    /// per-pass memo (`with_suzerain_read_memo`) is installed; this is what
    /// stopped the map itself from being a fresh `BTreeMap` allocation on
    /// every single-seat vision ask.
    pub(super) fn with_suzerain_input_map<R>(
        &self,
        read: impl FnOnce(&BTreeMap<usize, Option<usize>>) -> R,
    ) -> R {
        let epoch = self.diplomacy_epoch();
        let stale = match self.vision_frames.suzerain_map.borrow().as_ref() {
            Some((cached_epoch, _)) => *cached_epoch != epoch,
            None => true,
        };
        if stale {
            let map = self.suzerain_input_map();
            *self.vision_frames.suzerain_map.borrow_mut() = Some((epoch, map));
        }
        let cache = self.vision_frames.suzerain_map.borrow();
        let (_, map) = cache
            .as_ref()
            .expect("suzerain input map is installed before use");
        read(map)
    }

    pub(super) fn vision_input_stamp_with_suzerains(
        &self,
        pid: usize,
        suzerains: &BTreeMap<usize, Option<usize>>,
        unit_stamps: &[u64],
    ) -> u64 {
        let stamp = vision_key(&[
            self.vision_geometry_stamp(),
            pid as u64,
            self.is_arena() as u64,
            self.tactics.fog as u64,
        ]);
        if self.is_arena() && !self.tactics.fog {
            return stamp;
        }
        let mut stamp = self.with_visibility_viewers(pid, |viewers| {
            let mut stamp = vision_key(&[stamp, viewers.len() as u64]);
            for &viewer in viewers {
                stamp = vision_key(&[
                    stamp,
                    viewer as u64,
                    self.base_vision_input_stamp(viewer, suzerains, unit_stamps),
                ]);
            }
            stamp
        });
        if pid == MIRRORED_SEAT {
            for position in self.host_observed.iter() {
                stamp = vision_key(&[stamp, position.0 as i64 as u64, position.1 as i64 as u64]);
            }
        }
        stamp
    }

    #[cfg(test)]
    pub(super) fn vision_input_stamp(&self, pid: usize) -> u64 {
        self.with_vision_input_stamp(pid, |stamp| stamp)
    }

    /// Return the current sight frame, reusing its dense bitset when the
    /// stamped inputs still match.  The returned `Arc` keeps the hot callers
    /// (action legality, AI perception, and refresh publication) from cloning
    /// the map-sized bit vector merely to borrow it for a membership check.
    pub(super) fn vision_frame(&self, pid: usize, heights: &mut HeightField) -> Arc<TileBits> {
        self.with_vision_input_stamp(pid, |input_stamp| {
            if let Some(visible) = self.matching_vision_frame(pid, input_stamp) {
                visible
            } else {
                // The signature memo and frame cache have separate entries.
                // Keep any direct derivation under the same suzerain-query
                // scope the pre-memo path had.
                self.with_suzerain_read_memo(|| {
                    let visible = Arc::new(self.player_vision(heights, pid));
                    self.store_vision_frame(pid, input_stamp, Arc::clone(&visible));
                    visible
                })
            }
        })
    }

    pub(super) fn store_vision_frame(&self, pid: usize, input_stamp: u64, visible: Arc<TileBits>) {
        let mut frames = self.vision_frames.frames.borrow_mut();
        if frames.len() <= pid {
            frames.resize_with(pid + 1, || None);
        }
        frames[pid] = Some(VisionFrame {
            input_stamp,
            visible,
        });
    }

    pub(super) fn matching_vision_frame(
        &self,
        pid: usize,
        input_stamp: u64,
    ) -> Option<Arc<TileBits>> {
        self.vision_frames
            .frames
            .borrow()
            .get(pid)
            .and_then(|frame| frame.as_ref())
            .filter(|frame| frame.input_stamp == input_stamp)
            .map(|frame| Arc::clone(&frame.visible))
    }

    /// Authoritative current visibility for a player, including level-two
    /// Military Alliance shared vision. Explored/remembered tiles are not
    /// included here.
    /// The tiles a player can see right now, as bits.
    ///
    /// Callers that only ask whether a tile is visible want this: building a
    /// sorted set of a few hundred positions to answer one membership
    /// question was, by some way, the most expensive thing visibility did.
    pub(crate) fn player_vision_frame(&self, pid: usize) -> Arc<TileBits> {
        self.vision_frame(pid, &mut self.height_field())
    }

    /// Read-only visibility consumers also walk the suzerain relation while
    /// constructing an input stamp.  Keep that poll one-per-minor for the
    /// duration of the frame lookup, without requiring a mutable `Game`.
    fn with_suzerain_read_memo<R>(&self, pass: impl FnOnce() -> R) -> R {
        let installed = self.query_memo.suzerain.borrow().is_none();
        if installed {
            *self.query_memo.suzerain.borrow_mut() = Some(BTreeMap::new());
        }
        let result = pass();
        if installed {
            *self.query_memo.suzerain.borrow_mut() = None;
        }
        result
    }
}
