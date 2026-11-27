//! `walls-for-the-war-front`: while we are at war with a major, an unwalled
//! city of ours that one of that major's soldiers has been seen near starts
//! its Walls ahead of routine production.
//!
//! Tourney 8, 11-27 (4 of 40 reviewed Lv 8 games): Mecca and Hattin fell at
//! t87 and t90 with no walls (`wall_damage_when_held` 0), twenty turns after
//! the same enemy took Cairo at t67 and its army stood at Mecca from t67.
//! Mecca started Walls only at t85, when the reactive "under fire or facing a
//! confirmed siege" handoff fired, two turns before it fell. Mekele built an
//! Entertainment Complex three turns before it fell to Maya. The capital and
//! frontier wall genes only watch a peaceful neighbour, so no city started
//! its walls during the war until the siege had already begun.
//!
//! A visible non-recon land soldier of a major at war with us, within
//! [`WAR_FRONT_TILES`] of the city, while that major fields at least
//! [`WAR_FRONT_RATIO`] of our military power, starts the city's first Walls. A
//! sighting lasts [`WAR_FRONT_STANDARD`] Standard turns. Once invested, the
//! wall finishes while any war with a major lasts. A head that finishes this
//! turn, or a land soldier within [`WAR_FRONT_DEFENDER_TURNS`] of done, keeps
//! the queue; the head displaced keeps its progress. Sightings are
//! keyed by the city's tile, because live city ids change between frames.
//! The defence handoffs that run after this claim still have the last word.

use super::*;

const WAR_FRONT_TILES: i32 = 8;
const WAR_FRONT_STANDARD: u32 = 10;
const WAR_FRONT_RATIO: f64 = 0.5;
const WAR_FRONT_DEFENDER_TURNS: f64 = 2.0;

impl AdvancedAi {
    fn war_front_city(g: &Game, pid: usize, cid: u32) -> bool {
        g.cities
            .get(&cid)
            .is_some_and(|city| city.owner == pid && g.city_max_wall_hp(city) <= 0)
    }

    fn war_front_enemy(g: &Game, pid: usize, other: usize) -> bool {
        g.players.get(other).is_some_and(|player| {
            other != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && !player.is_free_city
                && g.is_at_war(pid, other)
        })
    }

    fn war_front_soldier(g: &Game, unit: &crate::game::Unit) -> bool {
        let spec = &g.rules.units[unit.kind];
        spec.class == "military"
            && spec.domain.as_deref().is_none_or(|domain| domain == "land")
            && spec.promotion_class != "recon"
            && (spec.is_melee_capable() || spec.has_ranged_attack())
    }

    /// A land soldier in training: one nearly done keeps the queue.
    fn war_front_defender(g: &Game, item: &Item) -> bool {
        matches!(item, Item::Unit { unit } if g.rules.units.get(unit).is_some_and(|spec| {
            spec.class == "military"
                && spec.domain.as_deref().is_none_or(|domain| domain == "land")
                && (spec.is_melee_capable() || spec.has_ranged_attack())
        }))
    }

    fn at_war_with_a_major(g: &Game, pid: usize) -> bool {
        (0..g.players.len()).any(|other| Self::war_front_enemy(g, pid, other))
    }

    /// Record this frame's enemy soldiers near our unwalled cities. No-op
    /// with the gene off.
    pub(super) fn remember_war_front_sightings(&mut self, g: &Game, pid: usize) {
        if !self.walls_for_the_war_front {
            return;
        }
        let window = g.standard_duration(WAR_FRONT_STANDARD);
        self.war_front_sightings.retain(|(_, rival), seen| {
            *seen <= g.turn && g.turn - *seen <= window && Self::war_front_enemy(g, pid, *rival)
        });
        if !Self::at_war_with_a_major(g, pid) {
            return;
        }
        let visible = g.player_vision_frame(pid);
        for cid in g.player_city_ids(pid) {
            if !Self::war_front_city(g, pid, cid) {
                continue;
            }
            let at = g.cities[&cid].pos;
            for unit in g.units.values() {
                if Self::war_front_enemy(g, pid, unit.owner)
                    && Self::war_front_soldier(g, unit)
                    && g.wdist(at, unit.pos) <= WAR_FRONT_TILES
                    && g.sees(&visible, unit.pos)
                    && g.unit_visible_to(unit.id, pid)
                {
                    self.war_front_sightings.insert((at, unit.owner), g.turn);
                }
            }
        }
    }

    /// The enemy (and its power over ours) whose soldier was seen near this
    /// unwalled city during the war. `None` with the gene off.
    pub(super) fn war_front_walls_threat(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<(usize, f64)> {
        if !self.walls_for_the_war_front || !Self::war_front_city(g, pid, cid) {
            return None;
        }
        let at = g.cities[&cid].pos;
        let ours = g.military_power(pid).max(1.0);
        let window = g.standard_duration(WAR_FRONT_STANDARD);
        self.war_front_sightings
            .iter()
            .filter(|((pos, rival), seen)| {
                *pos == at
                    && **seen <= g.turn
                    && g.turn - **seen <= window
                    && Self::war_front_enemy(g, pid, *rival)
            })
            .map(|((_, rival), _)| (*rival, g.military_power(*rival) / ours))
            .filter(|(_, ratio)| *ratio >= WAR_FRONT_RATIO)
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    pub(super) fn claim_walls_for_the_war_front(&self, g: &mut Game, pid: usize) {
        if !self.walls_for_the_war_front || !Self::at_war_with_a_major(g, pid) {
            return;
        }
        let walls = Item::Building {
            building: crate::name!("walls"),
        };
        for cid in g.player_city_ids(pid) {
            if !Self::war_front_city(g, pid, cid) || !g.can_produce(pid, cid, &walls) {
                continue;
            }
            let threat = self.war_front_walls_threat(g, pid, cid);
            if threat.is_none() && g.item_invested_production(cid, &walls) <= 0.0 {
                continue;
            }
            let head = g.cities[&cid].queue.first().cloned();
            if head.as_ref() == Some(&walls) {
                continue;
            }
            let production = g.city_yields(cid).production.max(0.1);
            if let Some(item) = &head {
                let remaining = g.item_remaining_cost_for_city(pid, cid, item);
                let turns = g
                    .host_production_turns(cid, item)
                    .filter(|turns| turns.is_finite() && *turns > 0.0)
                    .unwrap_or(remaining / production);
                if remaining <= production + f64::EPSILON
                    || (Self::war_front_defender(g, item) && turns <= WAR_FRONT_DEFENDER_TURNS)
                {
                    continue;
                }
            }
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: walls.clone(),
                },
            )
            .is_err()
            {
                continue;
            }
            let turns = g
                .host_production_turns(cid, &walls)
                .filter(|turns| turns.is_finite() && *turns > 0.0)
                .unwrap_or_else(|| g.item_remaining_cost_for_city(pid, cid, &walls) / production);
            let why = threat.map_or_else(
                || "the walls already invested finish during the war".to_string(),
                |(rival, ratio)| {
                    format!(
                        "at war with {}, which fields {ratio:.1}x our power, and its soldier was seen within {WAR_FRONT_TILES} tiles",
                        g.players[rival].civ
                    )
                },
            );
            think!(self.journal(), Military, Decision,
                "{} builds its walls on the war front", g.cities[&cid].name;
                "walls-for-the-war-front: {why}; {turns:.1} turns{}",
                head.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
