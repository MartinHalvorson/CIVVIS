//! Fog-safe battlefield observations and city-pressure calculations.
//!
//! This keeps the evidence shared by campaign, city governance, and
//! empire-wide recovery in one module without coupling it to their callers.

use super::*;

impl AdvancedAi {
    /// Local military pressure on one city: *observed* hostile military
    /// strength within six tiles over the friendly strength answering it,
    /// city defenses included. Zero when no visible hostile unit is in reach.
    ///
    /// This is the number `threatened_city` has always computed and then
    /// discarded for every city but the worst one. Naming it lets the same
    /// evidence reach the city's own decisions — what it builds and what its
    /// citizens work — instead of only the empire-wide recovery alarm.
    pub(super) fn city_pressure_with_visibility(
        g: &Game,
        pid: usize,
        cid: u32,
        visible: &crate::world::TileBits,
    ) -> f64 {
        let Some(city) = g.cities.get(&cid) else {
            return 0.0;
        };
        let hostile: f64 = g
            .units
            .values()
            .filter(|unit| unit.owner != pid && g.is_at_war(pid, unit.owner))
            .filter(|unit| g.sees(visible, unit.pos) && g.unit_visible_to(unit.id, pid))
            .filter(|unit| g.wdist(city.pos, unit.pos) <= 6)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .map(|unit| crate::game::effective_strength(g.unit_strength(unit, false), unit.hp))
            .sum();
        if hostile <= 0.0 {
            return 0.0;
        }
        hostile / Self::city_friendly_strength(g, pid, cid).max(1.0)
    }

    /// Strength a major we are at PEACE with has parked within reach of `cid`,
    /// weighted for the fact that it has not declared.
    ///
    /// See [`Self::frontier_massing_alarm`]. Deliberately the same expression
    /// as [`Self::city_pressure_with_visibility`] with one filter widened, so
    /// the two cannot drift apart about what "within reach" or "military"
    /// means; the extra conditions are the two that separate a build-up from
    /// a neighbour minding its own garrison.
    pub(super) fn frontier_massing_pressure(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        visible: &crate::world::TileBits,
    ) -> f64 {
        if !self.frontier_massing_alarm {
            return 0.0;
        }
        let Some(city) = g.cities.get(&cid) else {
            return 0.0;
        };
        let owner_of = |pos| {
            g.map
                .get(pos)
                .and_then(|tile| tile.owner_city)
                .and_then(|owner| g.cities.get(&owner))
                .map(|owner| owner.owner)
        };
        let massed: f64 = g
            .units
            .values()
            .filter(|unit| unit.owner != pid && !g.is_at_war(pid, unit.owner))
            .filter(|unit| {
                let owner = &g.players[unit.owner];
                owner.alive && !owner.is_minor && !owner.is_barbarian
            })
            // A treaty is a hard block on `start_war`, so a rival still bound
            // by one cannot be massing for anything this turn.
            .filter(|unit| g.peace_treaty_until(pid, unit.owner).is_none())
            .filter(|unit| g.sees(visible, unit.pos) && g.unit_visible_to(unit.id, pid))
            .filter(|unit| g.wdist(city.pos, unit.pos) <= 6)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            // In their own borders it is a garrison. Out of them, and this
            // close to one of our cities, it is a staging area.
            .filter(|unit| owner_of(unit.pos).is_none_or(|owner| owner == pid))
            .map(|unit| crate::game::effective_strength(g.unit_strength(unit, false), unit.hp))
            .sum();
        if massed <= 0.0 {
            return 0.0;
        }
        0.5 * massed / Self::city_friendly_strength(g, pid, cid).max(1.0)
    }

    pub(super) fn city_friendly_strength(g: &Game, pid: usize, cid: u32) -> f64 {
        let Some(city) = g.cities.get(&cid) else {
            return 0.0;
        };
        g.city_strength(cid)
            + g.units
                .values()
                .filter(|unit| unit.owner == pid && g.wdist(city.pos, unit.pos) <= 6)
                .filter(|unit| g.rules.units[unit.kind].class == "military")
                .map(|unit| crate::game::effective_strength(g.unit_strength(unit, true), unit.hp))
                .sum::<f64>()
    }

    /// Whether a visible hostile can actually strike the City Center on its
    /// next fresh turn.  A radius-only check is too broad around terrain and
    /// zone-of-control bottlenecks, while `attack_reach` is the simulator's
    /// own terrain- and movement-accurate envelope.  The caller supplies the
    /// same turn-start visibility used for live pressure so an enemy revealed
    /// by a later action cannot retroactively rewrite this plan.
    pub(super) fn imminent_city_attack(
        g: &Game,
        pid: usize,
        cid: u32,
        visible: &crate::world::TileBits,
    ) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        g.units.values().any(|unit| {
            unit.owner != pid
                && g.is_at_war(pid, unit.owner)
                && g.wdist(city.pos, unit.pos) <= THREAT_RELIEF_RADIUS
                && g.sees(visible, unit.pos)
                && g.unit_visible_to(unit.id, pid)
                && g.rules.units[unit.kind].class == "military"
                && g.attack_reach(unit.id).contains(&city.pos)
        })
    }

    /// The stock pressure calculation sees only live contacts. The evaluator
    /// arm adds an independently auditable, decayed memory term after that
    /// calculation, so visible sightings cannot be counted twice.
    pub(super) fn remembered_city_pressure(&self, g: &Game, pid: usize, cid: u32) -> f64 {
        if !self.belief_pressure {
            return 0.0;
        }
        let Some(city) = g.cities.get(&cid) else {
            return 0.0;
        };
        let remembered = self.belief.remembered_hidden_military_threat(
            g,
            pid,
            city.pos,
            THREAT_RELIEF_RADIUS,
            BELIEF_PRESSURE_HORIZON,
        );
        remembered / Self::city_friendly_strength(g, pid, cid).max(1.0)
    }

    /// Combat state at the last City Center sighting, if this controller has
    /// one. Campaign code uses this rather than re-reading a hidden city's
    /// live durability or garrison-derived strength.
    pub(super) fn remembered_city(&self, cid: u32) -> Option<&CitySighting> {
        self.belief.cities.get(&cid)
    }

    /// Capture one coherent battlefront observation before any action can
    /// reveal more of the map.  The stored unit set matters for camouflage:
    /// moving a detector later in the turn must not cause the planning frame
    /// to treat a formerly hidden unit as known at turn start.
    pub(super) fn capture_battlefront_frame(&mut self, g: &Game, pid: usize) {
        if !self.battlefront_observation {
            self.battlefront_frame = None;
            return;
        }
        let visible = g.player_vision_frame(pid);
        let units = g
            .units
            .values()
            .filter(|unit| g.sees(&visible, unit.pos) && g.unit_visible_to(unit.id, pid))
            .map(|unit| unit.id)
            .collect();
        self.battlefront_frame = Some(BattlefrontFrame { visible, units });
    }

    /// The battlefront's turn-start tile frame, or current vision when this
    /// helper is used outside a controller turn (including focused tests).
    pub(super) fn battlefront_visibility(&self, g: &Game, pid: usize) -> Arc<TileBits> {
        self.battlefront_frame
            .as_ref()
            .map(|frame| Arc::clone(&frame.visible))
            .unwrap_or_else(|| g.player_vision_frame(pid))
    }

    /// Whether a unit belonged to the same turn-start observation frame.
    pub(super) fn battlefront_unit_visible(&self, g: &Game, pid: usize, uid: u32) -> bool {
        self.battlefront_frame
            .as_ref()
            .map(|frame| frame.units.contains(&uid))
            .unwrap_or_else(|| g.unit_visible_to(uid, pid))
    }

    pub(super) fn city_pressure_with_belief(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        visible: &crate::world::TileBits,
    ) -> f64 {
        Self::city_pressure_with_visibility(g, pid, cid, visible)
            + self.remembered_city_pressure(g, pid, cid)
            // `frontier_massing_alarm`: contributes 0.0 exactly when off.
            + self.frontier_massing_pressure(g, pid, cid, visible)
    }

    #[cfg(test)]
    pub(super) fn city_pressure(g: &Game, pid: usize, cid: u32) -> f64 {
        let visible = g.player_vision_frame(pid);
        Self::city_pressure_with_visibility(g, pid, cid, &visible)
    }

    #[cfg(test)]
    pub(super) fn belief_city_pressure(&self, g: &Game, pid: usize, cid: u32) -> f64 {
        let visible = g.player_vision_frame(pid);
        self.city_pressure_with_belief(g, pid, cid, &visible)
    }

    /// Evaluate the independent city/unit distance matrix from compact,
    /// immutable inputs. Strength modifiers and city defenses are resolved on
    /// the simulation thread first, so workers share no `Game` caches and the
    /// floating-point sums retain unit-ID order exactly.
    pub(super) fn city_pressures(&self, g: &Game, pid: usize, cities: &[u32]) -> Vec<f64> {
        let visible = g.player_vision_frame(pid);
        let Some(pool) = self.work_pool.as_ref() else {
            return cities
                .iter()
                .map(|city| self.city_pressure_with_belief(g, pid, *city, &visible))
                .collect();
        };
        let relevant = g
            .units
            .values()
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .filter(|unit| {
                unit.owner == pid
                    || (g.is_at_war(pid, unit.owner)
                        && g.sees(&visible, unit.pos)
                        && g.unit_visible_to(unit.id, pid))
            })
            .collect::<Vec<_>>();
        // Pool dispatch dominates tiny empires. This boundary is comparisons,
        // not a map/player constant: large armies with few cities and wide
        // empires with small garrisons reach the same measured work floor.
        if cities.len() < 2 || cities.len().saturating_mul(relevant.len()) < 128 {
            return cities
                .iter()
                .map(|city| self.city_pressure_with_belief(g, pid, *city, &visible))
                .collect();
        }

        // Add remembered pressure after the pool's exact live-contact result.
        // That preserves the stock summation order and makes the treatment's
        // only new contribution an explicit scalar per city.
        let remembered = cities
            .iter()
            .map(|city| self.remembered_city_pressure(g, pid, *city))
            .collect::<Vec<_>>();
        let units = relevant
            .into_iter()
            .map(|unit| {
                let friendly = unit.owner == pid;
                CityPressureUnit {
                    position: unit.pos,
                    friendly,
                    strength: crate::game::effective_strength(
                        g.unit_strength(unit, friendly),
                        unit.hp,
                    ),
                }
            })
            .collect::<Vec<_>>();
        let sites = cities
            .iter()
            .map(|city| CityPressureSite {
                position: g.cities[city].pos,
                defense: g.city_strength(*city),
            })
            .collect::<Vec<_>>();
        let distance = match g.map.topology {
            crate::world::Topology::Cylinder => CityDistance::Cylinder(g.map.width),
            crate::world::Topology::Rectangle => CityDistance::Bounded,
            crate::world::Topology::Globe(frequency) => {
                CityDistance::Globe(crate::sphere::sphere(frequency))
            }
        };
        let units = Arc::new(units);
        let sites = Arc::new(sites);
        let distance = Arc::new(distance);
        pool.map(cities.len(), move |index| {
            let site = sites[index];
            let hostile = units
                .iter()
                .filter(|unit| !unit.friendly)
                .filter(|unit| distance.between(site.position, unit.position) <= 6)
                .map(|unit| unit.strength)
                .sum::<f64>();
            if hostile <= 0.0 {
                return 0.0;
            }
            let friendly = site.defense
                + units
                    .iter()
                    .filter(|unit| unit.friendly)
                    .filter(|unit| distance.between(site.position, unit.position) <= 6)
                    .map(|unit| unit.strength)
                    .sum::<f64>();
            hostile / friendly.max(1.0)
        })
        .into_iter()
        .zip(remembered)
        .map(|(observed, remembered)| observed + remembered)
        .collect()
    }
}
