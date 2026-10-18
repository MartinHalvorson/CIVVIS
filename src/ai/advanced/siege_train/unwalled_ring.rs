//! `unwalled-ring-steps-in`: a fit melee member of a siege, two to
//! [`CLOSING_REACH`] tiles from a city with no standing wall, that
//! `swarm_close` did not send in to swing this turn, walks onto the nearest
//! free land tile of the city's ring it reaches this turn, where the hostile
//! blows that reach that tile (`approach_danger`) stay below its health. It
//! swings from the ring next turn through `siege_blow` / `swarm_blow`. Not the
//! reserved taker, which keeps its own step.
//!
//! **Why (live Emperor, the pin era of 2026-10-18).** In 12 of 20 Invest or
//! Reduce episodes against a city with its walls down (four turns or more,
//! three or more members staged) the city never fell below 150 of 200. Over
//! their 81 turns we struck it 57 times for about 9 a turn, while the ring
//! stood sealed on 2.2 of 6 tiles and the city healed 20 a turn. Healthy melee
//! stood two or three tiles out on 56 unit-turns against 30 on the ring:
//! `swarm_close` moves a member in only when it can still swing that turn,
//! which the city's zone of control rarely allows, and the fallback walk stops
//! at `CITY_STRIKE_RANGE` outside a strike an unwalled city does not have.
//! Norrköping (civvis-20261018T134130Z) held at 200/200 with no walls from turn
//! 116 to 125 with ten of twelve members staged; the two episodes whose city
//! did fall, Wente Mapu and Ostia, had the most melee on the ring.
//!
//! Off, nothing runs.

use super::*;

impl AdvancedAi {
    /// See the module. `None` where the gene is off or no ring tile is
    /// taken; `Some(true)` once the member has walked onto the ring.
    pub(super) fn unwalled_ring_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        city: &CityView,
    ) -> Option<bool> {
        if !self.unwalled_ring_steps_in || city.wall_hp > 0 || city.hp <= 0 {
            return None;
        }
        let unit = g.units.get(&uid)?.clone();
        let distance = g.wdist(unit.pos, city.pos);
        let siege = self.sieges.get(&city.id);
        if arm_of(g, uid) != Arm::Melee
            || !(2..=CLOSING_REACH).contains(&distance)
            || unit.moves_left <= 0.0
            || g.is_embarked(&unit)
            || !self.siege_member_fit(g, uid)
            || siege.is_some_and(|siege| siege.taker == Some(uid))
        {
            return None;
        }
        let reachable: BTreeSet<Pos> = g.reachable(uid).into_iter().collect();
        let mut ring: Vec<Pos> = g
            .nbrs(city.pos)
            .into_iter()
            .filter(|pos| {
                reachable.contains(pos)
                    && g.unit_ids_at(*pos).is_empty()
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            })
            .collect();
        ring.sort_by_key(|pos| (g.wdist(unit.pos, *pos), *pos));
        for dest in ring {
            let mut after = g.speculative_clone();
            if after
                .apply(
                    pid,
                    &Action::MoveTo {
                        unit: uid,
                        to: dest,
                    },
                )
                .is_err()
            {
                continue;
            }
            let Some(arrived) = after.units.get(&uid).filter(|moved| moved.pos == dest) else {
                continue;
            };
            if self.approach_danger(&after, pid, dest, uid) >= f64::from(arrived.hp) {
                continue;
            }
            if !self.base.path_walk_to(g, pid, uid, dest) {
                return None;
            }
            self.force_groups_dirty = true;
            think!(self.journal(), Military, Decision,
                "Siege of {}: the {} steps onto the open ring", g.cities[&city.id].name, unit.kind;
                "unwalled-ring-steps-in: walls {}/{}, the city at {}; {distance} tiles out, {dest:?} seals a ring tile it heals through",
                city.wall_hp, city.wall_max, city.hp;
                city.pos);
            return Some(true);
        }
        None
    }
}
