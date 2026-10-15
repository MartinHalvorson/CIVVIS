//! `frontier-walls-before-the-surprise`: the early warning that protects an
//! original capital also matters to an exposed non-capital city. A visible
//! non-recon land soldier within eight tiles, belonging to a peaceful met
//! major with twice our military power, arms that city's first Walls and
//! brings Masonry forward. A sighting lasts ten Online turns. Once invested,
//! the wall finishes even when the sighting expires. The original-capital
//! policy remains independent; this opt-in does not buy a second garrison.

use super::*;

const FRONTIER_WATCH_TILES: i32 = 8;
const FRONTIER_WATCH_STANDARD: u32 = 15;
const FRONTIER_WALLS_RATIO: f64 = 2.0;
const FRONTIER_SETTLER_FINISH_TURNS: f64 = 2.0;

impl AdvancedAi {
    fn frontier_watch_city(g: &Game, pid: usize, cid: u32) -> bool {
        g.cities.get(&cid).is_some_and(|city| {
            city.owner == pid && !city.is_capital && g.city_max_wall_hp(city) <= 0
        })
    }

    fn frontier_watch_suspect(g: &Game, pid: usize, other: usize) -> bool {
        g.players.get(other).is_some_and(|player| {
            other != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && !player.is_free_city
                && g.has_met(pid, other)
                && !g.same_team(pid, other)
                && !g.is_at_war(pid, other)
                && !g.are_friends(pid, other)
                && g.alliance_with(pid, other).is_none()
        })
    }

    fn frontier_watch_soldier(g: &Game, unit: &crate::game::Unit) -> bool {
        let spec = &g.rules.units[unit.kind];
        spec.class == "military"
            && spec.domain.as_deref().is_none_or(|domain| domain == "land")
            && spec.promotion_class != "recon"
            && (spec.is_melee_capable() || spec.has_ranged_attack())
    }

    pub(super) fn remember_frontier_watch_sightings(&mut self, g: &Game, pid: usize) {
        if !self.frontier_walls_before_the_surprise {
            return;
        }
        let window = g.standard_duration(FRONTIER_WATCH_STANDARD);
        self.frontier_watch_sightings.retain(|(cid, _), seen| {
            Self::frontier_watch_city(g, pid, *cid) && *seen <= g.turn && g.turn - *seen <= window
        });
        let visible = g.player_vision_frame(pid);
        for cid in g.player_city_ids(pid) {
            if !Self::frontier_watch_city(g, pid, cid) {
                continue;
            }
            let at = g.cities[&cid].pos;
            for unit in g.units.values() {
                if Self::frontier_watch_suspect(g, pid, unit.owner)
                    && Self::frontier_watch_soldier(g, unit)
                    && g.wdist(at, unit.pos) <= FRONTIER_WATCH_TILES
                    && g.sees(&visible, unit.pos)
                    && g.unit_visible_to(unit.id, pid)
                {
                    self.frontier_watch_sightings
                        .insert((cid, unit.owner), g.turn);
                }
            }
        }
    }

    fn frontier_walls_threat(&self, g: &Game, pid: usize, cid: u32) -> Option<(usize, f64)> {
        if !self.frontier_walls_before_the_surprise || !Self::frontier_watch_city(g, pid, cid) {
            return None;
        }
        let ours = g.military_power(pid).max(1.0);
        let window = g.standard_duration(FRONTIER_WATCH_STANDARD);
        self.frontier_watch_sightings
            .iter()
            .filter(|((city, rival), seen)| {
                *city == cid
                    && **seen <= g.turn
                    && g.turn - **seen <= window
                    && Self::frontier_watch_suspect(g, pid, *rival)
            })
            .map(|((_, rival), _)| (*rival, g.military_power(*rival) / ours))
            .filter(|(_, ratio)| *ratio >= FRONTIER_WALLS_RATIO)
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    pub(super) fn frontier_walls_research_threat(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<(u32, usize, f64)> {
        if !self.frontier_walls_before_the_surprise {
            return None;
        }
        g.player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                self.frontier_walls_threat(g, pid, cid)
                    .map(|(rival, ratio)| (cid, rival, ratio))
            })
            .max_by(|a, b| a.2.total_cmp(&b.2).then(b.0.cmp(&a.0)).then(b.1.cmp(&a.1)))
    }

    pub(super) fn frontier_walls_research_goal(&self, g: &Game, pid: usize) -> Option<Name> {
        if !self.frontier_walls_before_the_surprise
            || g.players[pid].techs.contains(&crate::name!("masonry"))
        {
            return None;
        }
        self.frontier_walls_research_threat(g, pid)
            .map(|_| crate::name!("masonry"))
    }

    pub(super) fn claim_frontier_walls_before_the_surprise(&self, g: &mut Game, pid: usize) {
        if !self.frontier_walls_before_the_surprise {
            return;
        }
        let walls = Item::Building {
            building: crate::name!("walls"),
        };
        for cid in g.player_city_ids(pid) {
            if !Self::frontier_watch_city(g, pid, cid) || !g.can_produce(pid, cid, &walls) {
                continue;
            }
            let threat = self.frontier_walls_threat(g, pid, cid);
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
                    || (matches!(item, Item::Unit { unit } if unit == "settler")
                        && turns <= FRONTIER_SETTLER_FINISH_TURNS)
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
                .unwrap_or_else(|| {
                    g.item_remaining_cost_for_city(pid, cid, &walls)
                        / g.city_yields(cid).production.max(0.1)
                });
            let why = threat.map_or_else(
                || "the already invested wall finishes".to_string(),
                |(rival, ratio)| format!(
                    "{} fields {ratio:.1}x our power and a soldier was seen within {FRONTIER_WATCH_TILES} tiles",
                    g.players[rival].civ
                ),
            );
            think!(self.journal(), Military, Decision,
                "{} prepares its frontier walls", g.cities[&cid].name;
                "frontier-walls-before-the-surprise: {why}; {turns:.1} turns{}",
                head.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
