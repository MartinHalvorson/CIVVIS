//! `retainers-garrison-the-short-cities`: while Retainers is slotted, an idle
//! land soldier garrisons each of our cities that is short of Amenities.
//!
//! Retainers (`garrison_amenity`) gives +1 Amenity only to a city holding a
//! military unit. Live Immortal 2026-10-31, the seats carrying
//! `deck-prices-growth` that slotted it: on 1,014 short city-turns only 110
//! (11%) held a soldier. The shortfall is shallow: at t140, 106 of 118 short
//! cities stood at -1, the step from Displeased (-15% growth, -5% yields) to
//! Content. At peace the army stood 1.4-1.8 units a city, 7-9% inside one and
//! 72-79% within three tiles, so the garrison costs a walk of a few tiles.
//!
//! An idle unit (`peacetime_step`, before patrol) claims the nearest short
//! city with no soldier on it within [`RETAINERS_GARRISON_RANGE`]. The claim
//! holds while the city stands at or below zero Amenities, so the city the
//! garrison has just made Content keeps it; it drops when the unit is gone,
//! the city is lost, or the city stands above zero.
use super::BasicAi;
use crate::game::{Game, Unit};
use crate::Pos;
use crate::think;

/// The farthest an idle soldier walks to garrison a short city.
pub(crate) const RETAINERS_GARRISON_RANGE: i32 = 6;

impl BasicAi {
    fn retainers_garrison_soldier(g: &Game, unit: &Unit) -> bool {
        let spec = &g.rules.units[unit.kind];
        spec.class == "military"
            && spec.domain.as_deref().is_none_or(|domain| domain == "land")
            && spec.promotion_class != "recon"
            && !g.is_embarked(unit)
    }

    fn city_holds_a_soldier(g: &Game, pid: usize, pos: Pos) -> bool {
        g.unit_ids_at(pos).iter().any(|uid| {
            let unit = &g.units[uid];
            unit.owner == pid && g.rules.units[unit.kind].class == "military"
        })
    }

    /// See the module. `Some(acted)` when the step took this unit, `None` to
    /// leave it to the rest of `peacetime_step`.
    pub(super) fn retainers_garrison_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.retainers_garrison_the_short_cities || self.minor || self.barb {
            return None;
        }
        if g.policy_effect(pid, "garrison_amenity") <= 0.0 {
            self.retainers_garrison_claims.clear();
            return None;
        }
        self.retainers_garrison_claims.retain(|cid, claimant| {
            g.cities
                .get(cid)
                .is_some_and(|city| city.owner == pid && g.city_amenity_surplus(city) <= 0)
                && g.units.get(claimant).is_some_and(|unit| unit.owner == pid)
        });
        if !Self::retainers_garrison_soldier(g, &g.units[&uid]) {
            return None;
        }
        let claimed = self
            .retainers_garrison_claims
            .iter()
            .find(|(_, claimant)| **claimant == uid)
            .map(|(cid, _)| *cid);
        let target = match claimed {
            Some(cid) => cid,
            None => {
                let upos = g.units[&uid].pos;
                let pick = g
                    .player_city_ids(pid)
                    .into_iter()
                    .filter(|cid| !self.retainers_garrison_claims.contains_key(cid))
                    .filter(|cid| {
                        let city = &g.cities[cid];
                        g.city_amenity_surplus(city) < 0
                            && !Self::city_holds_a_soldier(g, pid, city.pos)
                            && g.wdist(upos, city.pos) <= RETAINERS_GARRISON_RANGE
                    })
                    .min_by_key(|cid| (g.wdist(upos, g.cities[cid].pos), *cid))?;
                self.retainers_garrison_claims.insert(pick, uid);
                think!(self.journal, Military, Detail,
                       "A soldier garrisons {} for Retainers", g.cities[&pick].name;
                       "retainers-garrison-the-short-cities: the city is short of Amenities \
                        and holds no soldier; Retainers gives a garrisoned city one");
                pick
            }
        };
        let pos = g.cities[&target].pos;
        if g.units[&uid].pos == pos {
            return Some(self.fortify_or_stop(g, pid, uid));
        }
        if self.step_toward(g, pid, uid, pos) {
            return Some(true);
        }
        // No route this turn: release the city for a nearer unit.
        self.retainers_garrison_claims.remove(&target);
        None
    }
}

#[cfg(test)]
mod tests;
