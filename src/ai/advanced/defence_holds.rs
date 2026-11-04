//! `defence-holds-under-fire`: while a city is under fire, the walls or land
//! defender in its queue are not displaced by a civilian build.
//!
//! **Why (10-30/31).** The defence pass (`redirect_unsafe_city_queue_for_defense`)
//! queues walls or a defender in a threatened city, but the strategic scorer
//! values a queued wall at 19-36, so the general production pass switches the
//! city back to whatever outbids it by its margin. The live journals repeat
//! "pauses amphitheater for walls" 36 times and "switches to amphitheater,
//! displacing walls" 50 times, and Acropolis, Government Plaza, Theater
//! Square, Industrial Zone and Campus likewise. Of the 10 cities lost unwalled
//! at t45-100 on 6p Immortal seats since 10-30 12Z (Masonry known in all
//! 10), 8 had their walls displaced in the turns before they fell. In 5 the
//! displacer was a civilian building: an Amphitheater four times (149-236 on
//! a Great Person's activation path) and a Workshop once. Pharsalos
//! (070824Z-cont1) held Star Fort walls one turn, t140, and an Amphitheater
//! the next.
//!
//! **What it does.** In the general pass, a defensive commitment
//! (`active_queue_is_defensive`: walls, a wall or encampment repair, a land
//! soldier) is not displaced by a non-defensive candidate while the city is
//! under fire: damaged (city or walls), or with a visible land soldier of a
//! civilization we are at war with within [`DEFENCE_HOLD_TILES`]. A defender
//! may still replace walls and the reverse; a vetoed commitment still gives
//! way. Off, nothing changes.

use super::*;

/// How close a visible enemy land soldier must stand for the hold.
pub(crate) const DEFENCE_HOLD_TILES: i32 = 4;

impl AdvancedAi {
    /// Whether `current`, the city's defensive commitment, holds against the
    /// non-defensive `candidate`. `false` with the gene off.
    pub(super) fn defence_holds_under_fire(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        current: &Item,
        candidate: &Item,
    ) -> bool {
        self.defence_holds_under_fire
            && Self::active_queue_is_defensive(g, current)
            && !Self::active_queue_is_defensive(g, candidate)
            && self.city_under_fire_for_the_hold(g, pid, cid)
    }

    /// The city is damaged, its walls are, or a visible land soldier of a
    /// civilization at war with us stands within [`DEFENCE_HOLD_TILES`].
    pub(super) fn city_under_fire_for_the_hold(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.hp < CITY_MAX_HP || city.wall_hp < g.city_max_wall_hp(city) {
            return true;
        }
        g.units.values().any(|unit| {
            let spec = &g.rules.units[unit.kind];
            unit.owner != pid
                && unit.hp > 0
                && spec.class == "military"
                && matches!(spec.domain.as_deref(), None | Some("land"))
                && g.is_at_war(pid, unit.owner)
                && g.wdist(unit.pos, city.pos) <= DEFENCE_HOLD_TILES
                && self.battlefront_unit_visible(g, pid, unit.id)
        })
    }
}

#[cfg(test)]
mod tests;
