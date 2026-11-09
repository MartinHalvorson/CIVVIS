//! Visibility: line of sight, what each unit and player can see, and fog
//! memory.
//!
//! Sight heights and line of sight (`sight_height`, `has_line_of_sight`,
//! `unit_has_line_of_sight_from`), the tiles a unit sees, a player's and a
//! team's visibility and their refresh (including the parallel, deferred
//! and snapshot paths), revealing tiles, contacts made in sight, sharing
//! visibility memories, and fog memory.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

/// Every city, then every unit, as `(owner, tile index)`.
struct ContactRoster {
    entries: Vec<(u32, u32)>,
    /// How many leading entries are cities.
    cities: usize,
}

impl Game {
    /// Introduce two seats to each other, permanently and in both directions.
    ///
    /// Meeting is mutual: a scout that walks into sight of a border city has
    /// been seen doing it. Recording both halves here means contact does not
    /// depend on whose visibility happens to be refreshed first.
    pub(crate) fn record_contact(&mut self, first: usize, second: usize) {
        if first == second {
            return;
        }
        let diplomatic = |player: Option<&Player>| {
            player.is_some_and(|player| !player.is_barbarian && !player.is_free_city)
        };
        if !diplomatic(self.players.get(first)) || !diplomatic(self.players.get(second)) {
            return;
        }
        // Reaching for a seat mutably copies it, so ask before taking.
        // MOMENT_PLAYER_MET_MAJOR is +1 to each side, and completing the table
        // is MOMENT_PLAYER_MET_ALL_MAJORS at +3, or +5 first in the world.
        let major = |player: &Player| !player.is_minor && !player.is_barbarian;
        let living_major = |player: &Player| {
            player.alive && !player.is_minor && !player.is_barbarian && !player.is_free_city
        };
        let major_civilization =
            |player: &Player| !player.is_minor && !player.is_barbarian && !player.is_free_city;
        let city_state = |player: &Player| player.alive && player.is_minor && !player.is_barbarian;
        let between_majors = self.players.get(first).is_some_and(major)
            && self.players.get(second).is_some_and(major);
        for (observer, subject) in [(first, second), (second, first)] {
            if self.has_met(observer, subject) {
                continue;
            }
            // Civilization VI grants the first major civilization to discover
            // a city-state one Envoy *at that city-state*.  This is not an
            // unallocated envoy stock: it is already the first point of
            // influence, so the next deliberate send is no longer eligible
            // for Diplomatic League's first-send bonus.
            //
            // Check only the durable contact ledger, not map positions or
            // hidden state.  A city-state can be seen by several units in one
            // action; the first insertion below wins deterministically and
            // later sightings see that recorded contact.
            let observer_is_major = self.players.get(observer).is_some_and(living_major);
            let subject_is_city_state = self.players.get(subject).is_some_and(city_state);
            if observer_is_major && subject_is_city_state {
                let first_discovery = !self.players.iter().any(|other| {
                    other.id != observer
                        && major_civilization(other)
                        && other.met.contains(&subject)
                });
                if first_discovery {
                    let envoys = &mut self.players[observer].envoys;
                    match envoys.iter_mut().find(|(minor, _)| *minor == subject) {
                        Some((_, count)) => *count += 1,
                        None => envoys.push((subject, 1)),
                    }
                }
            }
            self.players[observer].met.insert(subject);
            if between_majors {
                self.add_historic_moment(observer, "MOMENT_PLAYER_MET_MAJOR");
                self.note_met_all_majors(observer);
            }
        }
    }

    /// `MOMENT_PLAYER_MET_ALL_MAJORS`, +3, or +5 as
    /// `MOMENT_PLAYER_MET_ALL_MAJORS_FIRST_IN_WORLD`.
    pub(super) fn note_met_all_majors(&mut self, pid: usize) {
        if self.players[pid].counters.contains_key("met_all_majors") {
            return;
        }
        let outstanding = self.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && !self.has_met(pid, other.id)
        });
        if outstanding {
            return;
        }
        self.first_historic_moment(
            pid,
            "met_all_majors",
            Some("MOMENT_PLAYER_MET_ALL_MAJORS"),
            Some("MOMENT_PLAYER_MET_ALL_MAJORS_FIRST_IN_WORLD"),
        );
    }

    /// Technology/civic visibility plus the non-stacking relationship sources
    /// Civ VI exposes: an outgoing trade route, one Delegation/Embassy, an
    /// Alliance, and active Listening Posts. Secret Agents and Master Spies
    /// provide two levels instead of one while embedded in a rival city.
    pub fn diplomatic_visibility(&self, source: usize, target: usize) -> f64 {
        // A live host's own answer outranks the derivation: the mirror fills
        // `observed_visibility` from `GetVisibilityOn` and the derived sum
        // below cannot see a rival's trade routes, spies or trees.
        if let Some(observed) = self
            .players
            .get(source)
            .and_then(|player| player.observed_visibility.get(&target))
        {
            return *observed;
        }
        self.tree_effect(source, "diplomatic_visibility")
            + self.diplomatic_mission_to(source, target).is_some() as u8 as f64
            + self.routes.iter().any(|route| {
                route.ends > self.turn
                    && route.owner == source
                    && self
                        .cities
                        .get(&route.dest)
                        .is_some_and(|city| city.owner == target)
            }) as u8 as f64
            + self.are_allied(source, target) as u8 as f64
            + self
                .spies
                .values()
                .filter(|spy| spy.owner == source && spy.captured_by.is_none())
                .filter_map(|spy| {
                    let mission = spy.mission.as_ref()?;
                    let city = self.cities.get(&mission.city)?;
                    (mission.kind == "listening_post"
                        && mission.ends > self.turn
                        && city.owner == target)
                        .then_some(if spy.level >= 2 { 2.0 } else { 1.0 })
                })
                .sum::<f64>()
    }

    /// Whether a resource is revealed to a player by its technology/civic.
    pub fn resource_visible_to(&self, pid: usize, res: &str) -> bool {
        self.rules
            .resources
            .get(res)
            .is_some_and(|spec| self.unlocked(pid, &spec.tech, &spec.civic))
    }

    pub(super) fn reveal(&mut self, pid: usize, pos: Pos, radius: i32) {
        if self.visibility_suppressed
            || pid >= self.players.len()
            || !self.map.tiles.contains_key(&pos)
        {
            return;
        }
        let mut heights = self.height_field();
        let height = self.see_from_level(pos);
        let visible = self.visible_tiles_from(&mut heights, pos, radius, height, false, false);
        // `apply` refreshes the complete team view after an ordinary action.
        // Here only the sight just revealed by this mutation is needed. In
        // particular, a moving unit must keep ground and Wonders seen en route
        // even when it ends the turn somewhere else; recomputing every other
        // unit's unchanged sight at every step merely duplicates the refresh
        // below. Recording contacts locally preserves the same mid-turn
        // diplomacy unlocks while an AI turn is being coalesced.
        self.record_contacts_in_sight(pid, &visible, None);
        self.refresh_visibility_snapshot(pid, &visible);
    }

    /// A sight height the sweep has already worked out, or a fresh one.
    #[inline]
    fn sight_height_via(&self, heights: &mut HeightField, pos: Pos) -> i32 {
        let Some(index) = self.map.tiles.index_of(pos) else {
            return self.sight_height(pos);
        };
        let tiles = heights.tiles;
        let Some(cache) = HeightField::table(tiles, &mut heights.sight) else {
            return self.sight_height(pos);
        };
        if cache[index] != HeightField::UNKNOWN {
            return cache[index] as i32;
        }
        let height = self.sight_height(pos);
        heights.sight[index] = height as i16;
        height
    }

    #[inline]
    fn blocker_height_via(
        &self,
        heights: &mut HeightField,
        pos: Pos,
        see_through_woods: bool,
    ) -> i32 {
        let Some(index) = self.map.tiles.index_of(pos) else {
            return self.blocker_height(pos, see_through_woods);
        };
        let tiles = heights.tiles;
        let slot = if see_through_woods {
            &mut heights.open
        } else {
            &mut heights.wooded
        };
        let Some(cache) = HeightField::table(tiles, slot) else {
            return self.blocker_height(pos, see_through_woods);
        };
        if cache[index] != HeightField::UNKNOWN {
            return cache[index] as i32;
        }
        let height = self.blocker_height(pos, see_through_woods);
        cache[index] = height as i16;
        height
    }

    /// The shipped `Terrains.SightModifier`/`SightThroughModifier`, which carry
    /// the same value for every terrain in the game database: Mountain 2, Hills
    /// 1, everything else — including all water — 0. Elevation is a terrain
    /// property alone; no feature raises the ground a unit stands on.
    fn terrain_sight_level(tile: &Tile) -> i32 {
        if tile.terrain == "mountain" {
            2
        } else {
            i32::from(tile.hills)
        }
    }

    /// How high a viewer standing here looks out from — the shipped
    /// `SightModifier`, plus this ruleset's City Center/Encampment vantage.
    pub(super) fn see_from_level(&self, pos: Pos) -> i32 {
        let Some(tile) = self.map.get(pos) else {
            return 0;
        };
        Self::terrain_sight_level(tile) + self.district_viewpoint_bonus(pos)
    }

    /// How high the terrain on a tile stands: the shipped `SightThroughModifier`
    /// of its terrain plus that of its feature. Woods and Rainforest each add 1,
    /// so a wooded Hill reaches 2 and stands exactly as tall as a Mountain;
    /// Mount Everest and Yosemite add 2 more while Crater Lake, the Pantanal and
    /// the Great Barrier Reef add nothing at all.
    fn sight_height(&self, pos: Pos) -> i32 {
        let Some(tile) = self.map.get(pos) else {
            return 0;
        };
        Self::terrain_sight_level(tile)
            + self.feature_sight_through(tile.feature.as_deref())
            + self.district_viewpoint_bonus(pos)
    }

    /// The same height as cover. Sentry sees through vegetation only: a
    /// Mountain, a wooded Hill's rock, and a Natural Wonder are unchanged.
    fn blocker_height(&self, pos: Pos, see_through_woods: bool) -> i32 {
        let Some(tile) = self.map.get(pos) else {
            return 0;
        };
        let feature = tile.feature.as_deref();
        let vegetation = see_through_woods && matches!(feature, Some("forest" | "jungle"));
        Self::terrain_sight_level(tile)
            + self.district_viewpoint_bonus(pos)
            + if vegetation {
                0
            } else {
                self.feature_sight_through(feature)
            }
    }

    fn feature_sight_through(&self, feature: Option<&str>) -> i32 {
        feature
            .and_then(|feature| self.rules.features.get(feature))
            .map_or(0, |feature| feature.sight_through)
    }

    /// The nearest unwrapped image of `to`, plus its distance from `from`, so
    /// a ray across the seam is drawn as the short way round rather than back
    /// across the whole cylinder. The ray consumes both values, and retaining
    /// the winning comparison avoids measuring the same three images again.
    pub(super) fn unwrapped_toward(&self, from: Pos, to: Pos) -> (Pos, i32) {
        let width = self.map.width;
        [-width, 0, width]
            .into_iter()
            .map(|shift| {
                let candidate = (to.0 + shift, to.1);
                (candidate, hex::distance(from, candidate))
            })
            .min_by_key(|(_, distance)| *distance)
            .unwrap_or_else(|| (to, hex::distance(from, to)))
    }

    /// Walk one hex corridor from `from` to `to`, checking every tile strictly
    /// between the two endpoints for cover.
    ///
    /// The corridor is consumed as it is drawn — one position of lookahead is
    /// enough to know whether the tile just passed was an interior one — so a
    /// sight ray costs no allocation at all.
    #[allow(clippy::too_many_arguments)]
    fn corridor_is_clear(
        &self,
        heights: &mut HeightField,
        from: Pos,
        to: Pos,
        distance: i32,
        nudge: (f64, f64, f64),
        viewer_height: i32,
        target_height: i32,
        see_through_woods: bool,
    ) -> bool {
        let width = self.map.width;
        let from_cube = (from.0 as f64, from.1 as f64, (-from.0 - from.1) as f64);
        let to_cube = (to.0 as f64, to.1 as f64, (-to.0 - to.1) as f64);
        let mut previous: Option<Pos> = None;
        let mut distinct = 0usize;
        for step in 0..=distance {
            let t = step as f64 / distance as f64;
            let q = from_cube.0 + (to_cube.0 - from_cube.0) * t + nudge.0;
            let r = from_cube.1 + (to_cube.1 - from_cube.1) * t + nudge.1;
            let s = from_cube.2 + (to_cube.2 - from_cube.2) * t + nudge.2;
            let position = hex::canon(cube_round(q, r, s), width);
            if previous == Some(position) {
                continue;
            }
            // A position becomes interior once a further one follows it, and
            // the corridor's own origin is never cover.
            if distinct >= 2 {
                let blocker =
                    self.blocker_height_via(heights, previous.unwrap(), see_through_woods);
                if blocker > viewer_height && blocker >= target_height {
                    return false;
                }
            }
            previous = Some(position);
            distinct += 1;
        }
        true
    }

    /// The same corridor on a globe, where cube coordinates do not reach.
    ///
    /// A flat map draws its sight line by interpolating hex coordinates, which
    /// only works because the whole world is one lattice. On Planet the line
    /// between two tiles is a great circle, so the ray is walked along the arc
    /// itself: sample the arc at each step and follow the tile graph to the
    /// tile nearest that point. The corridor that comes out is the same
    /// object — every tile strictly between the two ends — so the cover rules
    /// above it are unchanged.
    fn arc_is_clear(
        &self,
        heights: &mut HeightField,
        from: Pos,
        to: Pos,
        viewer_height: i32,
        target_height: i32,
        see_through_woods: bool,
    ) -> bool {
        let Some(sphere) = self.map.sphere() else {
            return true;
        };
        let (Some(start), Some(end)) = (sphere.center(from), sphere.center(to)) else {
            return true;
        };
        let steps = self.wdist(from, to).max(1);
        let angle = start
            .iter()
            .zip(end)
            .map(|(a, b)| a * b)
            .sum::<f64>()
            .clamp(-1.0, 1.0)
            .acos();
        let mut previous: Option<Pos> = None;
        let mut at = from;
        let mut distinct = 0usize;
        for step in 0..=steps {
            let t = step as f64 / steps as f64;
            // Along the arc, not along the chord: an interpolated chord would
            // drift off the surface and pick tiles beside the sight line.
            let (first, second) = if angle < 1e-9 {
                (1.0 - t, t)
            } else {
                (
                    ((1.0 - t) * angle).sin() / angle.sin(),
                    (t * angle).sin() / angle.sin(),
                )
            };
            let sample = [
                start[0] * first + end[0] * second,
                start[1] * first + end[1] * second,
                start[2] * first + end[2] * second,
            ];
            at = self.tile_nearest(at, sample);
            if previous == Some(at) {
                continue;
            }
            if distinct >= 2 {
                let blocker =
                    self.blocker_height_via(heights, previous.unwrap(), see_through_woods);
                if blocker > viewer_height && blocker >= target_height {
                    return false;
                }
            }
            previous = Some(at);
            distinct += 1;
        }
        true
    }

    /// Walk from a tile to whichever tile the given direction actually points
    /// at. Each step is short, so following the steepest neighbour arrives in
    /// one or two moves.
    fn tile_nearest(&self, mut at: Pos, point: [f64; 3]) -> Pos {
        let Some(sphere) = self.map.sphere() else {
            return at;
        };
        let toward = |pos: Pos| {
            sphere.center(pos).map_or(f64::MIN, |center| {
                center.iter().zip(point).map(|(a, b)| a * b).sum()
            })
        };
        loop {
            // Each step strictly improves, so the walk cannot circle.
            let mut best = (toward(at), at);
            for neighbor in self.nbrs(at) {
                let score = toward(neighbor);
                if score > best.0 {
                    best = (score, neighbor);
                }
            }
            if best.1 == at {
                return at;
            }
            at = best.1;
        }
    }

    /// The two small cube-coordinate nudges produce both direct corridors
    /// when a ray lies exactly on a hex edge. A target is visible when either
    /// corridor is open, matching Civ VI's "half-hidden" hex behavior.
    fn tile_has_visibility_line(
        &self,
        heights: &mut HeightField,
        from: Pos,
        to: Pos,
        viewer_height: i32,
        see_through_woods: bool,
    ) -> bool {
        let target_height = self.sight_height_via(heights, to);
        self.tile_has_line_to(
            heights,
            from,
            to,
            viewer_height,
            target_height,
            see_through_woods,
        )
    }

    /// A ranged strike's corridor. The target stands at its terrain and
    /// feature height only: a City Center's or Encampment's vantage raises
    /// what a viewer standing there sees, not how tall the tile stands to a
    /// shooter. Over the live King games of 2026-09-30 the host refused all
    /// ten range-2 strikes on a Hills city across a Hills tile from flat
    /// ground (`can_start=false`) that the district's +1 had read as clear,
    /// and none of the 47 recorded range-2 city hits needs that +1.
    fn tile_has_strike_line(
        &self,
        from: Pos,
        to: Pos,
        viewer_height: i32,
        see_through_woods: bool,
    ) -> bool {
        let mut heights = HeightField::none();
        // Read terrain directly instead of adding and removing the district
        // vantage, which repeats its lookup for every candidate ranged shot.
        let target_height = if let Some(tile) = self.map.get(to) {
            Self::terrain_sight_level(tile) + self.feature_sight_through(tile.feature.as_deref())
        } else {
            self.sight_height(to) - self.district_viewpoint_bonus(to)
        };
        self.tile_has_line_to(
            &mut heights,
            from,
            to,
            viewer_height,
            target_height,
            see_through_woods,
        )
    }

    fn tile_has_line_to(
        &self,
        heights: &mut HeightField,
        from: Pos,
        to: Pos,
        viewer_height: i32,
        target_height: i32,
        see_through_woods: bool,
    ) -> bool {
        let sphere = self.map.sphere().is_some();
        let wraps = self.map.topology.wraps_east_west();
        // On a cylinder `unwrapped_toward` chooses the same minimum as
        // `wdist`; let its retained distance answer this fast path below.
        // A globe does not have a cube ray and an arena must keep its
        // non-wrapping distance, so both retain the original check.
        if (sphere || !wraps) && self.wdist(from, to) <= 1 {
            return true;
        }
        if sphere {
            return self.arc_is_clear(
                heights,
                from,
                to,
                viewer_height,
                target_height,
                see_through_woods,
            );
        }
        let (unwrapped, distance) = self.unwrapped_toward(from, to);
        if distance == 0 || (wraps && distance == 1) {
            return true;
        }
        [(1e-6, 1e-6, -2e-6), (-1e-6, -1e-6, 2e-6)]
            .into_iter()
            .any(|nudge| {
                self.corridor_is_clear(
                    heights,
                    from,
                    unwrapped,
                    distance,
                    nudge,
                    viewer_height,
                    target_height,
                    see_through_woods,
                )
            })
    }

    fn district_viewpoint_bonus(&self, pos: Pos) -> i32 {
        if self.city_at(pos).is_some() {
            return 1;
        }
        let Some(tile) = self.map.get(pos) else {
            return 0;
        };
        let Some(kind) = tile
            .district
            .filter(|kind| self.district_has_defenses(*kind))
        else {
            return 0;
        };
        if tile.pillaged {
            return 0;
        }
        let active = |city: &City| {
            city.districts
                .iter()
                .any(|(district, p)| *district == kind && *p == pos)
                && if self.district_is_family(kind, crate::name!("encampment")) {
                    !city.encampment_pillaged
                } else {
                    city.defending_districts
                        .iter()
                        .any(|d| d.pos == pos && !d.pillaged)
                }
        };
        if let Some(city) = tile.owner_city.and_then(|city| self.cities.get(&city)) {
            return i32::from(active(city));
        }
        i32::from(self.cities.values().any(active))
    }

    fn visible_tiles_from(
        &self,
        heights: &mut HeightField,
        from: Pos,
        radius: i32,
        viewer_height: i32,
        see_through_woods: bool,
        flying: bool,
    ) -> TileBits {
        let mut visible = TileBits::with_capacity(self.map.tiles.len());
        // Sight range is a hard cap in Civ VI. Elevation decides what a unit can
        // see *past*, never how far it can see: a Mountain outside a Settler's
        // three tiles stays dark until something walks close enough to it.
        self.map.for_each_disk(from, radius, |target| {
            if flying
                || self.tile_has_visibility_line(
                    heights,
                    from,
                    target,
                    viewer_height,
                    see_through_woods,
                )
            {
                self.mark_visible(&mut visible, target);
            }
        });
        visible
    }

    #[inline]
    fn mark_visible(&self, visible: &mut TileBits, pos: Pos) {
        if let Some(index) = self.map.tiles.index_of(pos) {
            visible.insert(index);
        }
    }

    /// The positions in a visibility set, in map order.
    fn tiles_of(&self, bits: &TileBits) -> BTreeSet<Pos> {
        self.map.tiles.positions(bits).collect()
    }

    /// Civ VI line of sight for the ranges represented by this ruleset. At
    /// range 2 either unobstructed hex corridor is enough; hills provide a
    /// vantage point, while wooded hills and mountains remain taller cover.
    pub(super) fn has_line_of_sight(&self, from: Pos, to: Pos, unit_in_district: bool) -> bool {
        let distance = self.wdist(from, to);
        if distance <= 1 || distance >= 3 {
            return true; // adjacent fire is unconditional; range 3+ lobs shots
        }
        let attacker_height = Self::terrain_sight_level(&self.map.tiles[&from])
            + if unit_in_district {
                self.district_viewpoint_bonus(from)
            } else {
                0
            };
        self.tile_has_strike_line(from, to, attacker_height, false)
    }

    /// Whether a unit standing on `from` would have line of sight to `to`.
    ///
    /// The same test `do_ranged` applies before it will let a shot happen, but
    /// asked of a tile the unit is only *considering*. Movement scoring needs
    /// it because line of sight binds at exactly range 2 — adjacent fire is
    /// unconditional and range 3+ lobs — which is precisely where a Field
    /// Cannon parks.
    pub fn line_of_sight_from(&self, from: Pos, to: Pos) -> bool {
        if !self.map.tiles.contains_key(&from) || !self.map.tiles.contains_key(&to) {
            return false;
        }
        self.has_line_of_sight(from, to, true)
    }

    /// The line-of-sight test `do_ranged` applies, asked of a unit that may
    /// fire from `from`. This preserves a Ranger's `see_through_woods` rule
    /// while also letting threat readers inspect a legal future firing tile.
    pub(crate) fn unit_has_line_of_sight_from(&self, uid: u32, from: Pos, to: Pos) -> bool {
        let unit = &self.units[&uid];
        if self.unit_effect(unit, "see_through_woods") > 0.0 && self.wdist(from, to) == 2 {
            let attacker_height = self.see_from_level(from);
            return self.tile_has_strike_line(from, to, attacker_height, true);
        }
        self.has_line_of_sight(from, to, true)
    }

    /// The line-of-sight test `do_ranged` applies, asked of a unit that
    /// already exists. This is *not* `line_of_sight_from`: that one asks about
    /// a tile a unit is only considering standing on, and cannot know about
    /// the firing unit's own `see_through_woods`. Gating a candidate shot with
    /// the tile version would refuse a Ranger the engine would have allowed.
    pub(crate) fn unit_has_line_of_sight(&self, uid: u32, to: Pos) -> bool {
        self.unit_has_line_of_sight_from(uid, self.units[&uid].pos, to)
    }

    pub fn unit_sight(&self, uid: u32) -> i32 {
        let unit = &self.units[&uid];
        let spec = &self.rules.units[unit.kind];
        spec.sight + self.promotion_effect(unit, "sight") as i32
    }

    /// Terrain-aware tiles currently observed by one unit. Air units ignore
    /// terrain obstruction; ground and naval units use their data-defined
    /// sight, promotions, elevation, and City Center/Encampment vantage.
    pub fn unit_visible_tiles(&self, uid: u32) -> BTreeSet<Pos> {
        self.unit_visible_tiles_via(&mut self.height_field(), uid)
    }

    fn unit_visible_tiles_via(&self, heights: &mut HeightField, uid: u32) -> BTreeSet<Pos> {
        let mut seen = TileBits::with_capacity(self.map.tiles.len());
        self.union_unit_vision(&mut seen, heights, uid);
        self.tiles_of(&seen)
    }

    /// What one unit can see, from the sweep cache when the answer still
    /// stands.
    ///
    /// A unit's view depends only on where it stands, how far it sees, and
    /// the map — so between two moves the answer does not change, and a
    /// player's visibility is asked for hundreds of times a turn. Only the
    /// unit that just moved needs a fresh sweep; the rest of the army's
    /// views are still good.
    fn union_unit_vision(&self, into: &mut TileBits, heights: &mut HeightField, uid: u32) {
        let Some(unit) = self.units.get(&uid) else {
            return;
        };
        let spec = &self.rules.units[unit.kind];
        let origin = unit.air_patrol_pos.unwrap_or(unit.pos);
        let sight = self.unit_sight(uid);
        let see_through_woods = self.unit_effect(unit, "see_through_woods") > 0.0;
        let flying = spec.domain.as_deref() == Some("air");
        let viewer_height = self.see_from_level(origin);
        let key = vision_key(&[
            origin.0 as u64,
            origin.1 as u64,
            sight as u64,
            viewer_height as u64,
            see_through_woods as u64,
            flying as u64,
        ]);
        {
            let mut cache = self.vision.borrow_mut();
            if let Some(seen) = cache.lookup(heights.stamp, uid, key) {
                into.union_with(seen);
                return;
            }
        }
        let seen = self.visible_tiles_from(
            heights,
            origin,
            sight,
            viewer_height,
            see_through_woods,
            flying,
        );
        into.union_with(&seen);
        self.vision.borrow_mut().put(heights.stamp, uid, key, seen);
    }

    /// Reuse one pid's shared-vision viewer set until the diplomacy epoch
    /// moves, folding a pid's entry lazily so one nobody asks about this
    /// epoch is never built.
    pub(super) fn with_visibility_viewers<R>(
        &self,
        pid: usize,
        read: impl FnOnce(&BTreeSet<usize>) -> R,
    ) -> R {
        let epoch = self.diplomacy_epoch();
        {
            let cache = self.vision_frames.viewers.borrow();
            if let Some((cached_epoch, entries)) = cache.as_ref() {
                if *cached_epoch == epoch {
                    if let Some(Some(viewers)) = entries.get(pid) {
                        return read(viewers);
                    }
                }
            }
        }
        let viewers = self.visibility_viewers(pid);
        let mut cache = self.vision_frames.viewers.borrow_mut();
        match cache.as_mut() {
            Some((cached_epoch, entries)) if *cached_epoch == epoch => {
                if entries.len() <= pid {
                    entries.resize_with(pid + 1, || None);
                }
                entries[pid] = Some(viewers);
            }
            _ => {
                let mut entries = vec![None; self.players.len().max(pid + 1)];
                entries[pid] = Some(viewers);
                *cache = Some((epoch, entries));
            }
        }
        let installed = cache.as_ref().expect("just installed").1[pid]
            .as_ref()
            .expect("just installed");
        read(installed)
    }

    pub(super) fn base_player_visibility(&self, heights: &mut HeightField, pid: usize) -> TileBits {
        let mut visible = TileBits::with_capacity(self.map.tiles.len());
        for uid in self.player_unit_ids(pid) {
            self.union_unit_vision(&mut visible, heights, uid);
        }
        // Civ VI grants unconditional visibility to every owned border tile
        // and the one-tile ring beyond it, regardless of terrain blockers.
        for city in self.cities.values().filter(|city| city.owner == pid) {
            for position in &city.owned_tiles {
                self.mark_visible(&mut visible, *position);
                for neighbor in self.nbrs(*position) {
                    self.mark_visible(&mut visible, neighbor);
                }
            }
        }
        // Suzerain diplomacy reveals exactly three tiles around the
        // city-state itself. This is a fixed diplomatic reveal, rather than
        // ownership of the city-state's units or their individual sight.
        for minor in self.players.iter().filter(|minor| {
            minor.alive
                && minor.is_minor
                && !minor.is_barbarian
                && self.suzerain_of(minor.id) == Some(pid)
        }) {
            for city in self.cities.values().filter(|city| city.owner == minor.id) {
                self.map.for_each_disk(city.pos, 3, |position| {
                    self.mark_visible(&mut visible, position)
                });
            }
        }
        // Spies are city-based rather than map Units in this engine, but have
        // the stock three-tile sight while established and uncaptured.
        for spy in self.spies.values().filter(|spy| {
            spy.owner == pid && spy.captured_by.is_none() && spy.ready_turn <= self.turn
        }) {
            let Some(position) = spy
                .city
                .and_then(|city| self.cities.get(&city).map(|city| city.pos))
            else {
                continue;
            };
            let height = self.see_from_level(position);
            visible
                .union_with(&self.visible_tiles_from(heights, position, 3, height, false, false));
        }
        visible
    }

    /// Seats whose live map vision is directly shared with `pid`. Visibility
    /// does not relay through a partner's other relationships.
    pub(crate) fn visibility_viewers(&self, pid: usize) -> BTreeSet<usize> {
        let mut viewers = BTreeSet::from([pid]);
        let Some(player) = self.players.get(pid) else {
            return viewers;
        };
        viewers.extend(
            self.team_members(pid)
                .into_iter()
                .filter(|member| self.players[*member].alive),
        );
        viewers.extend(player.alliances.iter().filter_map(|(partner, alliance)| {
            (alliance.ends > self.turn && alliance.kind == "military" && alliance.level >= 2)
                .then_some(*partner)
        }));
        for emergency in self
            .active_emergencies
            .iter()
            .filter(|emergency| emergency.ends > self.turn && emergency.members.contains(&pid))
        {
            viewers.extend(emergency.members.iter().copied());
        }
        viewers
    }

    pub fn player_visibility(&self, pid: usize) -> BTreeSet<Pos> {
        self.player_visibility_via(&mut self.height_field(), pid)
    }

    fn player_visibility_via(&self, heights: &mut HeightField, pid: usize) -> BTreeSet<Pos> {
        let visible = self.vision_frame(pid, heights);
        self.tiles_of(&visible)
    }

    /// Build the memory a seat keeps of a city it has seen.
    ///
    /// `pub(crate)` for the Civilization VI bridge: `mirror::apply_city_memory` needs
    /// the same shape the engine's own fog bookkeeping produces, and duplicating it
    /// there would let the two drift.
    pub(crate) fn remember_city(&self, city: &City) -> RememberedCity {
        RememberedCity {
            id: city.id,
            name: city.name.clone(),
            owner: city.owner,
            pos: city.pos,
            pop: city.pop,
            hp: city.hp,
            is_capital: city.is_capital,
            original_owner: city.original_owner,
            captured_from: city.captured_from,
            occupied_from: city.occupied_from,
            wall_hp: city.wall_hp,
            wall_max: self.city_max_wall_hp(city),
            encampment_hp: city.encampment_hp,
            encampment_wall_hp: city.encampment_wall_hp,
            encampment_pillaged: city.encampment_pillaged,
            defending_districts: city
                .defending_districts
                .iter()
                .map(|d| d.remembered())
                .collect(),
            religion: self.city_religion(city).map(str::to_string),
            seen_turn: self.turn,
        }
    }

    /// Seeing the center does not disclose defenses on an unseen district.
    pub(crate) fn remember_city_for_viewer(
        &self,
        city: &City,
        pid: usize,
        visible: &BTreeSet<Pos>,
    ) -> RememberedCity {
        let mut memory = self.remember_city(city);
        memory
            .defending_districts
            .retain(|state| city.owner == pid || visible.contains(&state.pos));
        if let Some(previous) = self.players[pid]
            .remembered_cities
            .get(&city.id)
            .filter(|old| old.owner == city.owner)
        {
            for state in &previous.defending_districts {
                if !visible.contains(&state.pos) && city.owner != pid {
                    memory.defending_districts.push(*state);
                }
            }
        }
        memory
            .defending_districts
            .sort_unstable_by_key(|state| state.pos);
        memory
    }

    fn snapshot_tile(&self, position: Pos) -> Option<RememberedTile> {
        let tile = self.map.get(position)?.clone();
        let owner = tile
            .owner_city
            .and_then(|city| self.cities.get(&city))
            .map(|city| city.owner);
        Some(RememberedTile {
            tile,
            owner,
            seen_turn: self.turn,
        })
    }

    /// Refresh one player's last-known map from exactly the tiles currently
    /// visible to that player. Nothing under fog is touched.
    #[cfg(test)]
    pub(super) fn refresh_player_visibility(&mut self, pid: usize) {
        let memory_world = self.snapshot_world_stamp(pid);
        let mut heights = self.height_field();
        self.refresh_player_visibility_via(pid, &mut heights, memory_world, None);
    }

    fn refresh_player_visibility_via(
        &mut self,
        pid: usize,
        heights: &mut HeightField,
        memory_world: u64,
        roster: Option<&ContactRoster>,
    ) {
        if self.visibility_suppressed || pid >= self.players.len() {
            return;
        }
        let visible = self.vision_frame(pid, heights);
        // Ahead of the snapshot's early-out rather than inside it: that guard
        // is keyed on the seat's own memory and the world's cities, so a rival
        // unit walking into a standing scout's sight would not disturb it.
        self.record_contacts_in_sight(pid, &visible, roster);
        self.refresh_visibility_snapshot_with_world(pid, &visible, memory_world);
    }

    /// Whether a player's memory of one tile already matches what is there.
    /// Only `seen_turn` differs on a tile nobody has touched, and refreshing
    /// that in place saves cloning the tile — the common case by far, since
    /// most of what a unit sees when it moves is unchanged terrain.
    fn remembered_tile_is_current(&self, pid: usize, position: Pos) -> bool {
        let Some(tile) = self.map.get(position) else {
            return true;
        };
        let Some(remembered) = self.players[pid].remembered_tiles.get(&position) else {
            return false;
        };
        let owner = tile
            .owner_city
            .and_then(|city| self.cities.get(&city))
            .map(|city| city.owner);
        remembered.owner == owner && &remembered.tile == tile
    }

    /// Everything the *tile* half of the snapshot is drawn from, and nothing
    /// else.
    ///
    /// `remembered_tile_is_current` reads a tile's contents and the seat that
    /// owns the hex — not a city's population, its walls, its religion or
    /// anybody's research. Those all live in `memory_world_stamp`, which is what
    /// decides whether the whole pass can be skipped; folding them in here too
    /// would throw away a seat's reconciled ground every time any city
    /// anywhere gained a point of pressure.
    fn tile_memory_stamp(&self, pid: usize) -> u64 {
        let seat = &self.players[pid];
        let mut key = vision_key(&[
            self.map.tiles.epoch(),
            self.turn as u64,
            self.track_fog_memory as u64,
            seat.explored.len() as u64,
            seat.remembered_tiles.len() as u64,
        ]);
        for city in self.cities.values() {
            key = vision_key(&[key, city.id as u64, city.owner as u64]);
        }
        key
    }

    /// Everything shared by every seat's remembered map.
    ///
    /// Full team/all-player refreshes compute this once, then add the three
    /// seat-local collection lengths below. This avoids walking every city's
    /// defenses and religious pressures once per player in the same sweep.
    fn memory_world_stamp(&self) -> u64 {
        let mut key = vision_key(&[
            self.map.tiles.epoch(),
            self.turn as u64,
            self.world_era as u64,
            self.track_fog_memory as u64,
        ]);
        for other in self.players.iter() {
            // A city's outer defenses are read from the trees.
            key = vision_key(&[key, other.techs.len() as u64, other.civics.len() as u64]);
        }
        for city in self.cities.values() {
            key = vision_key(&[
                key,
                city.id as u64,
                city.owner as u64,
                city.pos.0 as u64,
                city.pos.1 as u64,
                city.pop as u64,
                city.hp as u64,
                city.is_capital as u64,
                city.original_owner as u64,
                city.captured_from.map_or(u64::MAX, |seat| seat as u64),
                city.occupied_from.map_or(u64::MAX, |seat| seat as u64),
                city.wall_hp as u64,
                city.encampment_hp as u64,
                city.encampment_wall_hp as u64,
                city.encampment_pillaged as u64,
                city.name.len() as u64,
                city.buildings.len() as u64,
            ]);
            for state in &city.defending_districts {
                key = vision_key(&[
                    key,
                    state.kind.id() as u64,
                    state.pos.0 as u64,
                    state.pos.1 as u64,
                    state.hp as u64,
                    state.wall_hp as u64,
                    state.pillaged as u64,
                ]);
            }
            // Which religion a city holds is decided by its pressures.
            for (religion, pressure) in &city.pressure {
                key = vision_key(&[key, religion.len() as u64, pressure.to_bits()]);
            }
        }
        key
    }

    fn snapshot_world_stamp(&self, pid: usize) -> u64 {
        let keeps_observation_memory = self.track_fog_memory
            && self
                .players
                .get(pid)
                .is_some_and(|player| !player.is_minor && !player.is_barbarian);
        if keeps_observation_memory {
            self.memory_world_stamp()
        } else {
            // With last-seen observation memory disabled, a snapshot only
            // grows explored ground and discovers Natural Wonders. Both are
            // determined by the map, not by every city's population,
            // religion, defenses or every civilization's tree lengths.
            vision_key(&[
                self.map.tiles.epoch(),
                self.turn as u64,
                self.world_era as u64,
                self.track_fog_memory as u64,
            ])
        }
    }

    fn memory_stamp_with_world(&self, pid: usize, world: u64) -> u64 {
        let seat = &self.players[pid];
        vision_key(&[
            world,
            seat.explored.len() as u64,
            seat.remembered_tiles.len() as u64,
            seat.remembered_cities.len() as u64,
        ])
    }

    /// Stop (or resume) maintaining the remembered map of fogged tiles and
    /// cities. Outcomes are unchanged either way; only observations are.
    pub fn set_fog_memory(&mut self, track: bool) {
        self.track_fog_memory = track;
    }

    /// Run a synchronous action sequence while coalescing full visibility
    /// maintenance at its boundary.
    ///
    /// Built-in agents expose no intermediate state: a server call enters
    /// `take_turn`, receives control back after the seat has finished, and
    /// only then can serialize another frame. The old path nevertheless
    /// rebuilt that seat's complete sight after every purchase, policy swap,
    /// production choice and unit step, then rebuilt every seat again at End
    /// Turn. This preserves the exact boundary state while doing those full
    /// sweeps once. Local `reveal` calls still record explored ground, Wonders
    /// and newly met civilizations immediately because agents can consult
    /// those gameplay facts later in the same turn.
    pub fn with_deferred_visibility<R>(&mut self, actions: impl FnOnce(&mut Game) -> R) -> R {
        self.with_deferred_visibility_pool(None, actions)
    }

    /// Coalesce a turn like [`Game::with_deferred_visibility`], computing
    /// each seat's final sight on persistent workers before publishing any of
    /// it. Contacts, discoveries, historic moments, and fog memory remain on
    /// the simulation thread in the original player order.
    pub(crate) fn with_deferred_visibility_pool<R>(
        &mut self,
        pool: Option<&WorkPool>,
        actions: impl FnOnce(&mut Game) -> R,
    ) -> R {
        self.visibility_batch.depth += 1;
        let result = actions(self);
        self.visibility_batch.depth -= 1;
        if self.visibility_batch.depth == 0 {
            match pool {
                Some(pool) => self.flush_deferred_visibility_parallel(pool),
                None => self.flush_deferred_visibility(),
            }
        }
        result
    }

    fn flush_deferred_visibility(&mut self) {
        if self.planning_role == PlanningRole::Seat {
            // A planning seat's deliberation is the only reader its private
            // world will ever have, and it has just finished: the boundary
            // publish is for observers this world cannot acquire.
            self.visibility_batch.refresh_all = false;
            self.visibility_batch.refresh_teams.clear();
            return;
        }
        if self.visibility_batch.refresh_all {
            self.visibility_batch.refresh_all = false;
            self.visibility_batch.refresh_teams.clear();
            self.refresh_all_visibility();
            return;
        }
        let teams = std::mem::take(&mut self.visibility_batch.refresh_teams);
        for pid in teams {
            self.refresh_team_visibility(pid);
        }
    }

    fn flush_deferred_visibility_parallel(&mut self, pool: &WorkPool) {
        if self.planning_role == PlanningRole::Seat {
            self.visibility_batch.refresh_all = false;
            self.visibility_batch.refresh_teams.clear();
            return;
        }
        if pool.threads() == 1 || self.players.len() < PARALLEL_VISIBILITY_MIN_PLAYERS {
            self.flush_deferred_visibility();
            return;
        }
        if self.visibility_batch.refresh_all {
            self.visibility_batch.refresh_all = false;
            self.visibility_batch.refresh_teams.clear();
            self.refresh_all_visibility_parallel(pool);
            return;
        }
        let teams = std::mem::take(&mut self.visibility_batch.refresh_teams);
        for pid in teams {
            self.refresh_team_visibility_parallel(pid, pool);
        }
    }

    pub(super) fn defer_or_refresh_visibility(&mut self, pid: usize, all: bool) {
        if self.visibility_suppressed {
            return;
        }
        if self.visibility_batch.depth > 0 {
            if all {
                self.visibility_batch.refresh_all = true;
                self.visibility_batch.refresh_teams.clear();
            } else if !self.visibility_batch.refresh_all {
                self.visibility_batch.refresh_teams.insert(pid);
            }
        } else if all {
            self.refresh_all_visibility();
        } else {
            self.refresh_team_visibility(pid);
        }
    }

    fn refresh_visibility_snapshot(&mut self, pid: usize, visible: &TileBits) {
        let memory_world = self.snapshot_world_stamp(pid);
        self.refresh_visibility_snapshot_with_world(pid, visible, memory_world);
    }

    fn refresh_visibility_snapshot_with_world(
        &mut self,
        pid: usize,
        visible: &TileBits,
        memory_world: u64,
    ) {
        if self.visibility_suppressed {
            return;
        }
        // This active-turn union is deliberately recorded before the
        // last-known snapshot's early-out. A unit can walk back across ground
        // whose tile memory is already current; the browser still needs to
        // know that it was in sight earlier in this player turn.
        self.record_turn_visibility(pid, visible);
        if self.remembered_under.len() < self.players.len() {
            self.remembered_under
                .resize(self.players.len(), (0, 0, TileBits::default()));
        }
        // A move reveals over a wider set than the refresh that follows it, so
        // the second of the pair is the common case rather than a corner of
        // one: everything it would record was recorded a moment ago.
        let stamp = self.memory_stamp_with_world(pid, memory_world);
        // When the stamp still matches, every tile the last pass covered was
        // reconciled then and nothing it reads has moved since — the stamp is
        // exactly the statement that the map, the cities and this seat's
        // memory are as they were. So a wider sight is not a reason to walk
        // the whole of it again: only the hexes that were not covered can
        // differ, and a step reveals a handful of those out of a few hundred.
        let tile_stamp = self.tile_memory_stamp(pid);
        let mut reconciled: Option<TileBits> = None;
        if let Some((under, tiles_under, covered)) = self.remembered_under.get(pid) {
            if *under == stamp && visible.is_subset_of(covered) {
                return;
            }
            if *tiles_under == tile_stamp {
                reconciled = Some(covered.clone());
            }
        }
        let taken = visible.clone();
        let fresh = match &reconciled {
            Some(covered) => {
                let mut bits = TileBits::with_capacity(self.map.tiles.len());
                for index in visible.iter() {
                    if !covered.contains(index) {
                        bits.insert(index);
                    }
                }
                self.tiles_of(&bits)
            }
            None => self.tiles_of(visible),
        };
        let visible: BTreeSet<Pos> = self.tiles_of(visible);
        self.refresh_visibility_snapshot_inner(pid, &visible, &fresh);
        // Record the world as it stands *after* the snapshot: taking one grows
        // what the seat has explored, which is itself part of the stamp the
        // next call will compute.
        let settled = self.memory_stamp_with_world(pid, memory_world);
        let settled_tiles = self.tile_memory_stamp(pid);
        if let Some(slot) = self.remembered_under.get_mut(pid) {
            *slot = (settled, settled_tiles, taken);
        }
    }

    /// Retain every visibility frame acquired by the player who is currently
    /// acting, including rival units that have since fallen back under fog.
    /// Other seats continue to expose only their exact current sight: their
    /// previous player turn has ended, even if the world turn has not.
    fn record_turn_visibility(&mut self, pid: usize, visible: &TileBits) {
        if !self.track_fog_memory || pid != self.current || pid >= self.players.len() {
            return;
        }
        // This runs behind every action the seat takes. A foreign unit that
        // stood in sight at the last action and has not changed is already
        // recorded exactly as a fresh clone would record it, so only the new
        // and the changed are cloned; the rest are only named as in sight.
        let viewers = self.visibility_viewers(pid);
        let recorded = &self.players[pid].turn_units;
        let mut in_sight: BTreeSet<u32> = BTreeSet::new();
        let mut changed: Vec<Unit> = Vec::new();
        for unit in self.units.values().filter(|unit| unit.owner != pid) {
            let position = unit.air_patrol_pos.unwrap_or(unit.pos);
            if !(self.sees(visible, position)
                && viewers
                    .iter()
                    .any(|viewer| self.unit_visible_to(unit.id, *viewer)))
            {
                continue;
            }
            in_sight.insert(unit.id);
            if recorded.get(&unit.id) != Some(unit) {
                changed.push(unit.clone());
            }
        }
        let tiles = &self.map.tiles;
        let player = &mut self.players[pid];
        player.turn_units.retain(|id, unit| {
            let position = unit.air_patrol_pos.unwrap_or(unit.pos);
            !tiles
                .index_of(position)
                .is_some_and(|index| visible.contains(index))
                || in_sight.contains(id)
        });
        player
            .turn_units
            .extend(changed.into_iter().map(|unit| (unit.id, unit)));
        player.turn_visible.extend(tiles.positions(visible));
    }

    /// Meet everyone standing in `pid`'s sight.
    ///
    /// A city or a unit is what makes an empire meetable; territory alone is
    /// not, which is why a border can be drawn on the map before its owner is
    /// anybody you have heard of. The unmet list is built first and the whole
    /// pass is skipped once it is empty, so a table that has already been
    /// introduced pays one loop over the seats and nothing else — the common
    /// case, since this runs behind every action a player takes.
    fn record_contacts_in_sight(
        &mut self,
        pid: usize,
        visible: &TileBits,
        roster: Option<&ContactRoster>,
    ) {
        if self.visibility_suppressed
            || self
                .players
                .get(pid)
                .is_none_or(|seat| seat.is_barbarian || seat.is_free_city)
        {
            return;
        }
        let seats = self.players.len();
        let mut unmet = vec![false; seats];
        for other in self.players.iter() {
            unmet[other.id] = other.id != pid && !other.is_barbarian && !other.is_free_city;
        }
        for known in &self.players[pid].met {
            if let Some(entry) = unmet.get_mut(*known) {
                *entry = false;
            }
        }
        let unmet_count = unmet.iter().filter(|wanted| **wanted).count();
        if unmet_count == 0 {
            return;
        }
        let built;
        let roster = match roster {
            Some(roster) => roster,
            None => {
                built = self.contact_roster();
                &built
            }
        };
        // Cities first: there are two orders of magnitude fewer of them than
        // units, and a civilization with a city in sight needs no second look.
        let mut found = vec![false; seats];
        let mut found_count = 0;
        let (cities, units) = roster.entries.split_at(roster.cities);
        for entries in [cities, units] {
            if found_count == unmet_count {
                break;
            }
            for &(owner, tile) in entries {
                let owner = owner as usize;
                if unmet[owner] && !found[owner] && visible.contains(tile as usize) {
                    found[owner] = true;
                    found_count += 1;
                }
            }
        }
        for other in (0..seats).filter(|other| found[*other]) {
            self.record_contact(pid, other);
        }
    }

    /// Where every city and unit stands, for a pass that asks each seat what it
    /// can see. Walking two id-keyed trees once per seat was most of what a
    /// full visibility refresh cost on a world with hundreds of seats.
    fn contact_roster(&self) -> ContactRoster {
        let tiles = &self.map.tiles;
        let mut entries = Vec::with_capacity(self.cities.len() + self.units.len());
        entries.extend(self.cities.values().filter_map(|city| {
            let tile = tiles.index_of(city.pos)?;
            Some((city.owner as u32, tile as u32))
        }));
        let cities = entries.len();
        entries.extend(self.units.values().filter_map(|unit| {
            let tile = tiles.index_of(unit.pos)?;
            Some((unit.owner as u32, tile as u32))
        }));
        ContactRoster { entries, cities }
    }

    /// `visible` is everything the seat can see; `fresh` is the part of it
    /// that still has to be reconciled with the world. They differ only when
    /// the caller has proved the rest was reconciled under the same stamp.
    fn refresh_visibility_snapshot_inner(
        &mut self,
        pid: usize,
        visible: &BTreeSet<Pos>,
        fresh: &BTreeSet<Pos>,
    ) {
        let newly_explored: Vec<Pos> = fresh
            .difference(&self.players[pid].explored)
            .copied()
            .collect();
        // The fogged-tile and remembered-city memory is only for
        // observations, so a search that never observes leaves it be. What
        // remains — explored ground and, below, Natural-Wonder discovery —
        // does change the game and is kept either way.
        // A city-state's or barbarian's remembered map is never read: no rule
        // and no agent consults it, allies pool sight only among major
        // civilizations, and the browser refuses to view a minor seat at all.
        // Most seats in a game are minor, so keeping their memory was most of
        // the memory kept.
        let observed_seat = !self.players[pid].is_minor && !self.players[pid].is_barbarian;
        if self.track_fog_memory && observed_seat {
            let turn = self.turn;
            let tiles: Vec<(Pos, RememberedTile)> = fresh
                .iter()
                .filter(|position| !self.remembered_tile_is_current(pid, **position))
                .filter_map(|position| self.snapshot_tile(*position).map(|tile| (*position, tile)))
                .collect();
            // A city founded on ground the seat has already explored is a map
            // change Civilization VI reports through fog. Give it an initial
            // memory even though no unit currently has sight of it. Existing
            // memories remain strictly last-seen: only current sight may
            // refresh their population, defenses, or other live details.
            let explored = &self.players[pid].explored;
            let remembered_cities = &self.players[pid].remembered_cities;
            let mut cities: Vec<RememberedCity> = self
                .cities
                .values()
                .filter(|city| {
                    visible.contains(&city.pos)
                        || (explored.contains(&city.pos)
                            && !remembered_cities.contains_key(&city.id))
                })
                .map(|city| self.remember_city_for_viewer(city, pid, visible))
                .collect();
            for previous in self.players[pid]
                .remembered_cities
                .values()
                .filter(|memory| !visible.contains(&memory.pos))
            {
                let mut memory = previous.clone();
                self.refresh_defending_district_memory(&mut memory, visible);
                if memory.defending_districts != previous.defending_districts {
                    cities.push(memory);
                }
            }
            let live_city_ids: BTreeSet<u32> = self.cities.keys().copied().collect();
            let player = &mut self.players[pid];
            // Only reach for the memory mutably when a stamp actually has to
            // move: taking it mutably is what copies it, and after the first
            // refresh of a turn every visible tile already carries this turn.
            for position in fresh.iter() {
                player.remembered_tiles.mark_seen(*position, turn);
            }
            player.explored.extend(newly_explored.iter().copied());
            for (position, tile) in tiles {
                player.remembered_tiles.remember(position, tile, turn);
            }
            player.remembered_cities.retain(|city, memory| {
                !visible.contains(&memory.pos) || live_city_ids.contains(city)
            });
            for city in cities {
                player.remembered_cities.insert(city.id, city);
            }
        } else {
            // Explored ground is kept for every seat either way: minor
            // civilizations steer their own exploration by it, and it decides
            // Natural-Wonder discovery.
            self.players[pid]
                .explored
                .extend(newly_explored.iter().copied());
        }

        if self.players[pid].is_minor || self.players[pid].is_barbarian {
            return;
        }

        let continents: BTreeSet<usize> = newly_explored
            .iter()
            .filter_map(|position| self.map.get(*position).and_then(|tile| tile.continent))
            .collect();
        let has_founded_city = self.cities.values().any(|city| city.owner == pid);
        for continent in continents {
            let key = format!("discovered_continent:{continent}");
            if self.players[pid].counters.contains_key(&key) {
                continue;
            }
            self.players[pid].counters.insert(key, 1);
            // Initial visibility establishes a home continent without
            // manufacturing a discovery moment before the capital exists.
            if has_founded_city {
                self.first_historic_moment(
                    pid,
                    &format!("continent:{continent}"),
                    None,
                    Some("MOMENT_FIND_NEW_CONTINENT_FIRST_IN_WORLD"),
                );
                self.dedication_trigger(pid, "continent_or_wonder", 1);
            }
        }

        // Only ground nobody had seen can close the ring, so this is asked on
        // the turns exploring actually got somewhere rather than on every
        // refresh behind every move.
        if !newly_explored.is_empty() {
            self.update_world_lap(pid, &newly_explored);
        }

        let wonders: BTreeSet<Name> = newly_explored
            .iter()
            .filter_map(|position| self.map.get(*position))
            .filter_map(|tile| tile.feature.as_ref())
            .filter(|feature| {
                self.rules
                    .features
                    .get(feature)
                    .is_some_and(|feature| feature.natural_wonder)
            })
            .cloned()
            .collect();
        for wonder in wonders {
            if !self.players[pid].discovered_natural_wonders.insert(wonder) {
                continue;
            }
            let first = !self
                .players
                .iter()
                .any(|other| other.id != pid && other.discovered_natural_wonders.contains(&wonder));
            self.add_historic_moment(
                pid,
                if first {
                    "MOMENT_FIND_NATURAL_WONDER_FIRST_IN_WORLD"
                } else {
                    "MOMENT_FIND_NATURAL_WONDER"
                },
            );
            self.dedication_trigger(pid, "continent_or_wonder", 1);
            if self.grants_city_state_unique_bonus(pid, "Kandy") {
                let era = self.world_era;
                self.grant_great_work(pid, "relic", era, "kandy");
            }
        }
    }

    /// Take note of the longitudes this seat's knowledge now reaches, and of
    /// whether they have closed the ring.
    ///
    /// A civilization does not begin knowing what shape it lives on. The world
    /// could be a plain that runs on forever, and every unknown quarter is
    /// somewhere it might keep running. What settles it is going round: once a
    /// people have seen ground at every longitude, with no gap left for the
    /// world to continue through, the two ends of their chart have to be the
    /// same ground. That is the moment the world is known to come back on
    /// itself, and it is the moment one lap of exploring arrives at.
    ///
    /// The award is era score (the circumnavigation moment), so the answer has
    /// to stay exact; only the cost changed. The chart's coverage is a running
    /// accumulation in [`Player::lap_bands`], so each call classifies only the
    /// ground that just left the fog — `new_positions` — instead of rescanning
    /// everything the seat has ever seen. The seat also reads the flag as
    /// `me.went_around` in [`crate::obs`].
    pub(super) fn update_world_lap(&mut self, pid: usize, new_positions: &[Pos]) {
        if self.map.width <= 0 || self.players[pid].went_around {
            return;
        }
        let bands = self.lap_band_count();
        // An empty accumulation is a seat this cache has never counted — a new
        // game, or a save from before the cache existed. Both are settled the
        // same way: one classification of the full explored set, after which
        // only fresh ground is ever looked at again.
        if self.players[pid].lap_bands.is_empty() {
            let everything: Vec<Pos> = self.players[pid].explored.iter().copied().collect();
            self.note_lap_bands(pid, bands, &everything);
        } else {
            self.note_lap_bands(pid, bands, new_positions);
        }
        if self.players[pid].lap_bands.len() < bands {
            return;
        }
        // The answer can never change back, so the accumulation has done its
        // job and does not need carrying or serializing any further.
        self.players[pid].lap_bands.clear();
        self.players[pid].went_around = true;
        self.first_historic_moment(
            pid,
            "circumnavigation",
            Some("MOMENT_WORLD_CIRCUMNAVIGATED"),
            Some("MOMENT_WORLD_CIRCUMNAVIGATED_FIRST_IN_WORLD"),
        );
        self.note_important(
            pid,
            "World",
            "have been the whole way around the world: the two ends of the chart are the same ground",
            None,
        );
    }

    /// How many longitude bands a lap of this world has to cover. A globe is
    /// carved into a fixed number of meridian slices; a cylinder's columns are
    /// its own bands.
    fn lap_band_count(&self) -> usize {
        if self.map.sphere().is_some() {
            GLOBE_LAP_BANDS
        } else {
            self.map.width as usize
        }
    }

    /// Fold these tiles into the seat's covered-longitude accumulation.
    fn note_lap_bands(&mut self, pid: usize, bands: usize, positions: &[Pos]) {
        for position in positions {
            let band = match self.map.sphere() {
                Some(sphere) => {
                    // Near a pole every longitude is a short walk from every
                    // other, so a circuit of the ice says nothing about how
                    // big the world is. A cylinder's rows are all one length,
                    // so this is a globe's problem only.
                    if sphere.latitude(*position).abs() > POLAR_LAP_LIMIT {
                        continue;
                    }
                    let turns =
                        (sphere.longitude(*position) / std::f64::consts::TAU).rem_euclid(1.0);
                    ((turns * bands as f64) as usize).min(bands - 1)
                }
                None => crate::hex::axial_to_offset(position.0, position.1)
                    .0
                    .rem_euclid(self.map.width) as usize,
            };
            self.players[pid].lap_bands.insert(band as u32);
        }
    }

    /// The vision passes poll `suzerain_of` for every (viewer, city-state)
    /// pair — an every-major envoy poll each, so one full refresh makes
    /// seats × minors × majors envoy queries when asked cold. The answers
    /// cannot change inside one pass: publishing sight records contacts,
    /// moments, and remembered ground, never envoys, governors, or a seat's
    /// life. So install the ordinary suzerain memo for the duration of the
    /// pass. A `QueryMemo` guard can never be alive here — it pins `&Game`,
    /// and every caller of a refresh holds `&mut Game` — so the slot is
    /// always free; the install mirrors the guard's outermost discipline
    /// anyway so nesting stays sound if that ever changes.
    fn with_suzerain_memo<R>(&mut self, pass: impl FnOnce(&mut Game) -> R) -> R {
        let installed = self.query_memo.suzerain.borrow().is_none();
        if installed {
            *self.query_memo.suzerain.borrow_mut() = Some(BTreeMap::new());
        }
        let result = pass(self);
        if installed {
            *self.query_memo.suzerain.borrow_mut() = None;
        }
        result
    }

    pub(super) fn refresh_all_visibility(&mut self) {
        if self.visibility_suppressed {
            return;
        }
        let memory_world = if self.track_fog_memory {
            self.memory_world_stamp()
        } else {
            self.snapshot_world_stamp(0)
        };
        self.with_suzerain_memo(|game| {
            let mut heights = game.height_field();
            let roster = game.contact_roster();
            for pid in 0..game.players.len() {
                game.refresh_player_visibility_via(pid, &mut heights, memory_world, Some(&roster));
            }
        });
    }

    /// Compute current sight from one immutable final position, then publish
    /// each result serially. Worker game clones inherit immutable unit rays;
    /// any missing entries use copy-on-write storage and are merged back before
    /// returning.
    fn refresh_visibility_parallel(
        &mut self,
        players: Vec<usize>,
        memory_world: u64,
        pool: &WorkPool,
    ) {
        if self.visibility_suppressed || players.is_empty() {
            return;
        }
        // Do the cheap stamp pass on the authoritative world first. A worker
        // clone starts with no compact frames, so sending an unchanged seat to
        // a worker would throw away exactly the reuse this cache is for. The
        // input-stamp memo lets an unchanged seat skip the old per-viewer
        // signature walk here as well as in single-seat callers.
        let mut cached = Vec::<(usize, u64, Arc<TileBits>)>::new();
        let mut pending = Vec::<(usize, u64)>::new();
        for pid in players.iter().copied() {
            self.with_vision_input_stamp(pid, |input_stamp| {
                match self.matching_vision_frame(pid, input_stamp) {
                    Some(visible) => cached.push((pid, input_stamp, visible)),
                    None => pending.push((pid, input_stamp)),
                }
            });
        }

        let count = pending.len();
        let mut fresh = Vec::<(usize, u64, TileBits)>::with_capacity(count);
        if count > 0 {
            let active = pool.threads().min(count);
            let states = (0..active).map(|_| self.clone()).collect::<Vec<_>>();
            let worker_players = Arc::new(pending.iter().map(|(pid, _)| *pid).collect::<Vec<_>>());
            let worker_players_for_pool = Arc::clone(&worker_players);
            let mut computed = pool.map_stateful(count, states, move |game, indices| {
                // Worker clones start with an empty query cache; the same
                // suzerain answers the serial pass memoizes would otherwise
                // be re-derived from scratch for every seat this worker computes.
                let memo = game.query_memo();
                let mut heights = game.height_field();
                let mut results = indices
                    .map(|index| {
                        // The authoritative pre-pass already stamped which
                        // seats need work.  Compute the ray answer directly
                        // here; asking the worker to stamp it a second time
                        // would spend the saved work before it reaches main.
                        let visible =
                            game.player_vision(&mut heights, worker_players_for_pool[index]);
                        (index, (visible, None))
                    })
                    .collect::<Vec<_>>();
                drop(memo);
                if let Some((_, (_, cache))) = results.last_mut() {
                    *cache = Some(game.vision.into_inner());
                }
                results
            });

            let vision_stamp = self.world_stamp();
            for (_, cache) in &mut computed {
                if let Some(cache) = cache.take() {
                    self.vision.borrow_mut().merge_current(cache, vision_stamp);
                }
            }
            for ((pid, input_stamp), (visible, _)) in pending.into_iter().zip(computed) {
                fresh.push((pid, input_stamp, visible));
            }
        }

        // Publish in the requested player order, exactly as the old all-worker
        // path did.  Contacts and fog memory are observer-visible state, so a
        // cached frame saves the ray work but not this serial publication.
        for pid in players {
            let (input_stamp, visible) = if let Some((_, stamp, visible)) =
                cached.iter().find(|(cached_pid, _, _)| *cached_pid == pid)
            {
                (*stamp, Arc::clone(visible))
            } else {
                let index = fresh
                    .iter()
                    .position(|(fresh_pid, _, _)| *fresh_pid == pid)
                    .expect("every pending visibility frame has a result");
                let (_, stamp, visible) = fresh.swap_remove(index);
                (stamp, Arc::new(visible))
            };
            self.store_vision_frame(pid, input_stamp, Arc::clone(&visible));
            self.record_contacts_in_sight(pid, &visible, None);
            self.refresh_visibility_snapshot_with_world(pid, &visible, memory_world);
        }
    }

    pub(super) fn refresh_all_visibility_parallel(&mut self, pool: &WorkPool) {
        if self.visibility_suppressed {
            return;
        }
        let memory_world = if self.track_fog_memory {
            self.memory_world_stamp()
        } else {
            self.snapshot_world_stamp(0)
        };
        self.refresh_visibility_parallel((0..self.players.len()).collect(), memory_world, pool);
    }

    fn refresh_team_visibility(&mut self, pid: usize) {
        if self.visibility_suppressed {
            return;
        }
        let members = self.team_members(pid);
        let memory_world = if self.track_fog_memory {
            self.memory_world_stamp()
        } else {
            self.snapshot_world_stamp(pid)
        };
        self.with_suzerain_memo(|game| {
            let mut heights = game.height_field();
            for member in members {
                if game.players[member].alive {
                    game.refresh_player_visibility_via(member, &mut heights, memory_world, None);
                }
            }
        });
    }

    fn refresh_team_visibility_parallel(&mut self, pid: usize, pool: &WorkPool) {
        if self.visibility_suppressed {
            return;
        }
        let members = self
            .team_members(pid)
            .into_iter()
            .filter(|member| self.players[*member].alive)
            .collect();
        let memory_world = if self.track_fog_memory {
            self.memory_world_stamp()
        } else {
            self.snapshot_world_stamp(pid)
        };
        self.refresh_visibility_parallel(members, memory_world, pool);
    }

    /// Permanently exchange the newest last-seen map state among players who
    /// share explored-map knowledge. Current visibility is still calculated
    /// separately and units are deliberately never copied into memory.
    pub(super) fn share_visibility_memories(&mut self, members: &[usize]) {
        if self.visibility_suppressed {
            return;
        }
        let explored: BTreeSet<Pos> = members
            .iter()
            .flat_map(|member| self.players[*member].explored.iter().copied())
            .collect();
        // Carry each tile's stamp alongside it: the stamp lives beside the
        // remembered map now rather than inside it, and it is what decides
        // whose memory of a hex is the fresher.
        let mut tiles: BTreeMap<Pos, (RememberedTile, u32)> = BTreeMap::new();
        let mut cities: BTreeMap<u32, RememberedCity> = BTreeMap::new();
        for member in members {
            let seat = &self.players[*member];
            for (position, memory) in seat.remembered_tiles.iter() {
                let seen = seat.remembered_tiles.seen_turn(position);
                let replace = tiles.get(position).is_none_or(|(_, old)| seen > *old);
                if replace {
                    tiles.insert(*position, (memory.clone(), seen));
                }
            }
            for (city, memory) in &self.players[*member].remembered_cities {
                let replace = cities
                    .get(city)
                    .is_none_or(|old| memory.seen_turn > old.seen_turn);
                if replace {
                    cities.insert(*city, memory.clone());
                }
            }
        }
        for member in members {
            let player = &mut self.players[*member];
            player.explored.extend(explored.iter().copied());
            for (position, (memory, seen)) in &tiles {
                let replace = player.remembered_tiles.get(position).is_none()
                    || *seen > player.remembered_tiles.seen_turn(position);
                if replace {
                    player
                        .remembered_tiles
                        .remember(*position, memory.clone(), *seen);
                }
            }
            for (city, memory) in &cities {
                let replace = player
                    .remembered_cities
                    .get(city)
                    .is_none_or(|old| memory.seen_turn > old.seen_turn);
                if replace {
                    player.remembered_cities.insert(*city, memory.clone());
                }
            }
        }
        // Memory just arrived from somewhere other than a sweep, so no seat's
        // last snapshot can be assumed to still describe it.
        self.remembered_under
            .iter_mut()
            .for_each(|(stamp, tiles, _)| {
                *stamp = 0;
                *tiles = 0;
            });
        // Allies pool what they know, and a ring closed between two of them is
        // still a closed ring: each side has sailed its half and the charts
        // agree where they meet. Shared memory is the one path that grows
        // `explored` without a visibility refresh, so the pooled ground is
        // handed to the lap accumulation here; re-counting a member's own
        // tiles is harmless because the accumulation is a set.
        let pooled: Vec<Pos> = explored.iter().copied().collect();
        for member in members {
            self.update_world_lap(*member, &pooled);
        }
    }

    pub(super) fn backfill_visibility_memory(&mut self) {
        for pid in 0..self.players.len() {
            let visible = self.player_visibility(pid);
            let missing_tiles: Vec<(Pos, RememberedTile)> = self.players[pid]
                .explored
                .iter()
                .filter(|position| !self.players[pid].remembered_tiles.contains_key(position))
                .filter_map(|position| self.snapshot_tile(*position).map(|tile| (*position, tile)))
                .collect();
            let missing_cities: Vec<RememberedCity> = self
                .cities
                .values()
                .filter(|city| self.players[pid].explored.contains(&city.pos))
                .filter(|city| !self.players[pid].remembered_cities.contains_key(&city.id))
                .map(|city| self.remember_city_for_viewer(city, pid, &visible))
                .collect();
            for (position, tile) in missing_tiles {
                let seen = tile.seen_turn;
                self.players[pid]
                    .remembered_tiles
                    .remember(position, tile, seen);
            }
            for city in missing_cities {
                self.players[pid].remembered_cities.insert(city.id, city);
            }
        }
    }

    /// Camouflaged recon and Naval Raider units are hidden unless an
    /// adjacent unit, Naval Raider, or Destroyer detects them.
    pub fn unit_visible_to(&self, uid: u32, viewer: usize) -> bool {
        let Some(unit) = self.units.get(&uid) else {
            return false;
        };
        if unit.owner == viewer {
            return true;
        }
        let raider = self.rules.units[unit.kind].promotion_class == "naval_raider";
        let camouflaged = self.promotion_effect(unit, "camouflage") > 0.0;
        if !raider && !camouflaged {
            return true;
        }
        // On a mirrored board every foreign unit arrived from the Civ 6 export,
        // and the export carries a rival's units ONLY under current visibility —
        // the HOST has already run its own detection over this raider before
        // showing it to the seat. Re-deriving stealth here vetoed that ground
        // truth: measured live on `civvis-20260807T162004Z` turns 237–251
        // (#1362), a rival UNIT_NUCLEAR_SUBMARINE the export carried in full was
        // planted and then hidden from the seat's board, orders and threat
        // reads, because no destroyer of ours stood near it. `host_observed` is
        // exactly the set of tiles the host proved this seat can see; trust it
        // the same way `player_vision` already does. Seat-0-gated like that
        // site, and an ordinary CIVVIS game leaves the set empty.
        if viewer == MIRRORED_SEAT
            && self
                .host_observed
                .contains(&unit.air_patrol_pos.unwrap_or(unit.pos))
        {
            return true;
        }
        self.player_unit_ids(viewer).into_iter().any(|other_id| {
            let other = &self.units[&other_id];
            let origin = other.air_patrol_pos.unwrap_or(other.pos);
            let target = unit.air_patrol_pos.unwrap_or(unit.pos);
            let distance = self.wdist(origin, target);
            distance <= 1
                || (raider
                    && self.unit_visible_tiles(other_id).contains(&target)
                    && (other.kind == "destroyer"
                        || self.rules.units[other.kind].promotion_class == "naval_raider"))
        })
    }

    pub(super) fn share_emergency_visibility(&mut self) {
        for emergency in self.active_emergencies.clone() {
            self.share_visibility_memories(&emergency.members.iter().copied().collect::<Vec<_>>());
        }
    }
}
