//! One bounded, default-off forward-airfield preparation project.
//!
//! Host quotes price only the offered stage. Known land edges are a planning
//! bound, not a future Native travel time or proof that an Engineer will arrive.
//! Outside-territory permission: Improvements.xml:50; the support chain comes
//! from Buildings.xml:196-197 and Units.xml:932.
use super::*;
use std::collections::VecDeque;

const STAGE_STANDARD_TURNS: u32 = 16;
const PROJECT_STANDARD_TURNS: u32 = 40;
const PROJECT_LAND_EDGES: usize = 12;
const PROJECT_UPKEEP: f64 = 6.0;

#[derive(Clone)]
pub(super) struct FieldProject {
    // City coordinates survive the production bridge rebuilding its IDs.
    pub city: Pos,
    pub capital: Pos,
    pub field: Pos,
    pub started: u32,
}

fn stage_rank(g: &Game, item: &Item) -> Option<u8> {
    match item {
        Item::Unit { unit } if unit == "military_engineer" => Some(0),
        Item::Building { building } => {
            let family = g
                .rules
                .buildings
                .get(building)?
                .replaces
                .unwrap_or(*building);
            match family.as_str() {
                "armory" => Some(1),
                "barracks" | "stable" => Some(2),
                _ => None,
            }
        }
        Item::District { district, .. } if g.district_family(*district) == "encampment" => Some(3),
        _ => None,
    }
}

fn known_land(g: &Game, pid: usize, pos: Pos) -> bool {
    let Some(tile) = g.map.get(pos) else {
        return false;
    };
    g.players[pid].explored.contains(&pos)
        && g.rules.is_passable(tile)
        && !g.rules.is_water(tile)
        && !g.blocked_city_sites.contains(&pos)
        && !g.closed_borders.contains(&pos)
        && tile
            .owner_city
            .is_none_or(|cid| g.cities.get(&cid).is_some_and(|city| city.owner == pid))
}

fn land_distances(g: &Game, pid: usize, from: Pos, danger: &BTreeSet<Pos>) -> BTreeMap<Pos, usize> {
    let mut distances = BTreeMap::new();
    if !known_land(g, pid, from) || danger.contains(&from) {
        return distances;
    }
    distances.insert(from, 0);
    let mut queue = VecDeque::from([from]);
    while let Some(pos) = queue.pop_front() {
        let steps = distances[&pos];
        if steps >= PROJECT_LAND_EDGES {
            continue;
        }
        for next in g.nbrs(pos) {
            if !distances.contains_key(&next) && known_land(g, pid, next) && !danger.contains(&next)
            {
                distances.insert(next, steps + 1);
                queue.push_back(next);
            }
        }
    }
    distances
}

impl AdvancedAi {
    fn forward_field_capital(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> Option<u32> {
        let foe = plan.target_player?;
        let major = |owner: usize| {
            g.players
                .get(owner)
                .is_some_and(|p| p.alive && !p.is_minor && !p.is_barbarian && !p.is_free_city)
        };
        if foe == pid || !major(foe) || g.same_team(pid, foe) || !g.is_at_war(pid, foe) {
            return None;
        }
        g.cities
            .values()
            .filter(|city| {
                city.owner == foe
                    && city.is_capital
                    && g.players
                        .get(city.original_owner)
                        .is_some_and(|p| !p.is_minor && !p.is_barbarian && !p.is_free_city)
                    && g.players[pid].explored.contains(&city.pos)
            })
            .min_by_key(|city| (Some(city.id) != plan.target_city, city.id))
            .map(|city| city.id)
    }

    fn forward_field_wing(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> Option<Vec<u32>> {
        if !self.forward_airfield_project
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || plan.strategy != GrandStrategy::Conquest
            || plan.threatened_city.is_some()
            || self.threatened_city(g, pid).is_some()
            || self.war_plan.is_some()
            || self
                .air_surge_plan
                .as_ref()
                .is_some_and(|surge| surge.phase != super::air_surge::AirSurgePhase::Exploit)
            || !g.players[pid].techs.contains(&crate::name!("flight"))
            || !g.players[pid]
                .techs
                .contains(&crate::name!("military_engineering"))
        {
            return None;
        }
        let player = &g.players[pid];
        let bill = g.unit_gold_maintenance(pid);
        if !player.gold.is_finite()
            || !player.gold_per_turn.is_finite()
            || !bill.is_finite()
            || player.gold_per_turn <= PROJECT_UPKEEP
            || player.gold
                < super::gold_and_cards::UPKEEP_RESERVE_TURNS * (bill + PROJECT_UPKEEP).max(0.0)
            || self.live_war_economy_requires_recovery(g, pid, &self.counts(g, pid))
        {
            return None;
        }
        let capital = self.forward_field_capital(g, pid, plan)?;
        let wing: Vec<_> = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && unit.hp >= 50
                    && g.rules.units[unit.kind].domain.as_deref() == Some("air")
                    && g.rules.units[unit.kind].promotion_class == "air_bomber"
                    && g.unit_attack_range(unit.id) > 0
            })
            .map(|unit| unit.id)
            .collect();
        if wing.len() < 2 {
            return None;
        }
        let target = g.cities[&capital].pos;
        // A queued field already in range owns its investment. Do not open
        // a second infrastructure chain while waiting for that base.
        if g.cities.values().any(|city| {
            city.owner == pid
                && city.queue.first().is_some_and(|item| {
                    Self::production_commitment_is_legal(g, pid, city.id, item)
                        && matches!(item,
                Item::District { district, pos } if g.district_family(*district) == "aerodrome"
                    && wing.iter().filter(|uid|
                        g.wdist(*pos, target) <= g.unit_attack_range(**uid)).take(2).count() == 2)
                })
        }) {
            return None;
        }
        let bases: Vec<_> = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| g.air_capacity_at(pid, *pos) > 0)
            .collect();
        if wing
            .iter()
            .filter(|uid| {
                bases
                    .iter()
                    .any(|pos| g.wdist(*pos, target) <= g.unit_attack_range(**uid))
            })
            .take(2)
            .count()
            == 2
        {
            return None;
        }
        Some(wing)
    }

    fn forward_field_danger(&self, g: &Game, pid: usize) -> BTreeSet<Pos> {
        let visible = self.battlefront_visibility(g, pid);
        let mut danger = BTreeSet::new();
        for unit in g.units.values().filter(|unit| {
            unit.owner != pid
                && g.is_at_war(pid, unit.owner)
                && g.sees(&visible, unit.pos)
                && self.battlefront_unit_visible(g, pid, unit.id)
                && g.rules.units[unit.kind].class == "military"
        }) {
            danger.insert(unit.pos);
            if g.rules.units[unit.kind].domain.as_deref() == Some("air") {
                danger.extend(g.wdisk(unit.pos, g.unit_attack_range(unit.id)));
            } else {
                danger.extend(g.threat_reach(unit.id));
                // The Native controller's capture fallback is geometric when
                // its path query cannot answer for a civilian destination.
                if !g.host_observed.is_empty() {
                    danger.extend(g.wdisk(unit.pos, g.unit_max_moves(unit.id).ceil() as i32));
                }
            }
        }
        for city in g.cities.values().filter(|city| {
            city.owner != pid
                && g.is_at_war(pid, city.owner)
                && g.players[pid].explored.contains(&city.pos)
        }) {
            danger.extend(g.wdisk(city.pos, 2));
        }
        danger
    }

    fn forward_field_site(g: &Game, pid: usize, field: Pos, capital: Pos, wing: &[u32]) -> bool {
        let Some(tile) = g.map.get(field) else {
            return false;
        };
        known_land(g, pid, field)
            && tile.resource.is_none()
            && tile.improvement.is_none()
            && tile.district.is_none()
            && tile.district_foundation.is_none()
            && tile.wonder.is_none()
            && !tile.hills
            && !tile.flooded
            && g.city_at(field).is_none()
            && wing
                .iter()
                .filter(|uid| g.wdist(field, capital) <= g.unit_attack_range(**uid))
                .take(2)
                .count()
                == 2
            && g.valid_improvements(pid, field)
                .contains(&crate::name!("airstrip"))
    }

    fn forward_field_project_valid(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        project: &FieldProject,
        wing: &[u32],
    ) -> bool {
        self.forward_field_capital(g, pid, plan)
            .map(|cid| g.cities[&cid].pos)
            == Some(project.capital)
            && g.turn.saturating_sub(project.started) <= g.standard_duration(PROJECT_STANDARD_TURNS)
            && g.city_at(project.city)
                .and_then(|cid| g.cities.get(&cid))
                .is_some_and(|city| {
                    city.owner == pid
                        && plan.threatened_city != Some(city.id)
                        && (city.last_attacked == 0
                            || g.turn.saturating_sub(city.last_attacked) > 4)
                })
            && Self::forward_field_site(g, pid, project.field, project.capital, wing)
    }

    fn forward_field_stage(&self, g: &Game, pid: usize, city: u32) -> Option<(u8, f64, Item)> {
        let mut best: Option<(u8, f64, String, Item)> = None;
        for item in g.producible_items(pid, city) {
            let Some(rank) = stage_rank(g, &item) else {
                continue;
            };
            let turns = match g.host_production_turns(city, &item) {
                Some(turns) => turns,
                None if g.host_observed.is_empty() => {
                    self.production_build_turns(g, pid, city, &item)
                }
                None => continue,
            };
            if !turns.is_finite()
                || turns < 0.0
                || turns > f64::from(g.standard_duration(STAGE_STANDARD_TURNS))
            {
                continue;
            }
            let key = format!("{item:?}");
            if best
                .as_ref()
                .is_none_or(|(old_rank, old_turns, old_key, _)| {
                    (rank, turns, &key) < (*old_rank, *old_turns, old_key)
                })
            {
                best = Some((rank, turns, key, item));
            }
        }
        best.map(|(rank, turns, _, item)| (rank, turns, item))
    }

    fn forward_field_choose(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        wing: &[u32],
        danger: &BTreeSet<Pos>,
    ) -> Option<FieldProject> {
        let capital = g.cities[&self.forward_field_capital(g, pid, plan)?].pos;
        let engineers: Vec<_> = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && unit.kind == "military_engineer")
            .map(|unit| unit.id)
            .collect();
        if engineers.is_empty()
            && g.cities.values().any(|city| {
                city.owner == pid
                    && city
                        .queue
                        .first()
                        .is_some_and(|item| stage_rank(g, item).is_some())
            })
        {
            return None;
        }
        let engineer_routes: Vec<_> = engineers
            .iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                unit.charges > 0
                    && unit.hp >= 75
                    && !g.is_embarked(unit)
                    && unit.linked_to.is_none()
                    && !self.amphibious_locked(**uid)
            })
            .map(|uid| land_distances(g, pid, g.units[uid].pos, danger))
            .collect();
        let fields: Vec<_> = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|field| {
                !danger.contains(field) && Self::forward_field_site(g, pid, *field, capital, wing)
            })
            .collect();
        let mut best: Option<(u8, f64, usize, u32, Pos)> = None;
        for city in g.cities.values().filter(|city| {
            city.owner == pid
                && plan.threatened_city != Some(city.id)
                && (city.last_attacked == 0 || g.turn.saturating_sub(city.last_attacked) > 4)
        }) {
            let (rank, turns) = if engineers.is_empty() {
                if !city.queue.is_empty() {
                    continue;
                }
                let Some((rank, turns, _)) = self.forward_field_stage(g, pid, city.id) else {
                    continue;
                };
                (rank + 1, turns)
            } else {
                (0, 0.0)
            };
            let distances = land_distances(g, pid, city.pos, danger);
            for field in &fields {
                let Some(&steps) = distances.get(field) else {
                    continue;
                };
                if !engineers.is_empty()
                    && !engineer_routes
                        .iter()
                        .any(|route| route.contains_key(field))
                {
                    continue;
                }
                let candidate = (rank, turns, steps, city.id, *field);
                if best.as_ref().is_none_or(|old| candidate < *old) {
                    best = Some(candidate);
                }
            }
        }
        best.map(|(_, _, _, city, field)| FieldProject {
            city: g.cities[&city].pos,
            capital,
            field,
            started: g.turn,
        })
    }

    pub(super) fn claim_forward_airfield_project(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.forward_airfield_project {
            return;
        }
        // Keep an expired/invalid attempt as a receipt while this capital stays
        // the objective. A repeated frame must not reset its finite budget.
        if self
            .forward_airfield_project_state
            .as_ref()
            .is_some_and(|project| {
                self.forward_field_capital(g, pid, plan)
                    .map(|cid| g.cities[&cid].pos)
                    != Some(project.capital)
            })
        {
            self.forward_airfield_project_state = None;
        }
        let Some(wing) = self.forward_field_wing(g, pid, plan) else {
            return;
        };
        let danger = self.forward_field_danger(g, pid);
        let _memo = g.query_memo();
        let project = match self.forward_airfield_project_state.clone() {
            Some(project) => project,
            None => {
                let Some(project) = self.forward_field_choose(g, pid, plan, &wing, &danger) else {
                    return;
                };
                self.forward_airfield_project_state = Some(project.clone());
                project
            }
        };
        let Some(city) = g.city_at(project.city) else {
            return;
        };
        if !self.forward_field_project_valid(g, pid, plan, &project, &wing)
            || danger.contains(&project.field)
            || !g.cities[&city].queue.is_empty()
            || g.units
                .values()
                .any(|unit| unit.owner == pid && unit.kind == "military_engineer")
        {
            return;
        }
        let Some((_, turns, item)) = self.forward_field_stage(g, pid, city) else {
            return;
        };
        let name = g.cities[&city].name.clone();
        drop(_memo);
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision, "{name} prepares a forward airfield";
                "forward-airfield-project: offered {} at {turns:.1} turns; one producer for capital {:?}, field {:?}; future stages and arrival are not yet observed",
                Self::plain_item(&item), project.capital, project.field);
        }
    }

    pub(super) fn forward_airfield_project_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.forward_airfield_project || stage_rank(g, item).is_none() {
            return false;
        }
        let Some(project) = self
            .forward_airfield_project_state
            .as_ref()
            .filter(|p| g.cities.get(&cid).is_some_and(|city| p.city == city.pos))
        else {
            return false;
        };
        let Some(wing) = self.forward_field_wing(g, pid, plan) else {
            return false;
        };
        self.forward_field_project_valid(g, pid, plan, project, &wing)
            && Self::production_commitment_is_legal(g, pid, cid, item)
            && !self.forward_field_danger(g, pid).contains(&project.field)
    }

    pub(super) fn forward_airfield_engineer_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        if !self.forward_airfield_project {
            return None;
        }
        let project = self.forward_airfield_project_state.clone()?;
        let wing = self.forward_field_wing(g, pid, plan)?;
        if !self.forward_field_project_valid(g, pid, plan, &project, &wing) {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.owner != pid
            || unit.kind != "military_engineer"
            || unit.charges <= 0
            || unit.moves_left <= 0.0
            || unit.hp < 75
            || g.is_embarked(unit)
            || unit.linked_to.is_some()
            || self.amphibious_locked(uid)
            || self.guard_is_reserved_for_civilian(uid)
            || g.player_city_ids(pid)
                .iter()
                .any(|cid| g.district_contribution_target(pid, *cid).is_some())
        {
            return None;
        }
        let current = unit.pos;
        let danger = self.forward_field_danger(g, pid);
        let remembered =
            self.barbarian_reach(g, pid, current, super::civilian_safety::REACH_SCAN_RADIUS);
        // Let the ordinary handler keep its emergency escape/work authority.
        if danger.contains(&current) || remembered.covers(g, current) {
            return None;
        }
        if current == project.field {
            let action = Action::Improve {
                unit: uid,
                improvement: crate::name!("airstrip"),
            };
            let built = g.apply(pid, &action).is_ok();
            if built {
                // Keep the original receipt: a fresh Native board may still
                // reject this modeled build. An observed base releases the
                // reservation through forward_field_wing without a new clock.
                think!(self.journal(), Military, Decision, "The Engineer lays the forward field";
                    "forward-airfield-project: ordinary legal Airstrip action at {:?}; slots, rebases and later attacks retain their own permissions", current);
            }
            return Some(built);
        }
        let distances = land_distances(g, pid, project.field, &danger);
        let Some(&remaining) = distances.get(&current) else {
            return Some(false);
        };
        let next = g
            .nbrs(current)
            .into_iter()
            .filter(|next| g.can_move(uid, *next) && !remembered.covers(g, *next))
            .filter_map(|next| {
                distances
                    .get(&next)
                    .filter(|steps| **steps < remaining)
                    .map(|steps| (*steps, next))
            })
            .min()
            .map(|(_, next)| next);
        let Some(next) = next else { return Some(false) };
        let moved = g
            .apply(
                pid,
                &Action::Move {
                    unit: uid,
                    to: next,
                },
            )
            .is_ok();
        if moved {
            think!(self.journal(), Military, Decision, "The Engineer walks toward the forward field";
                "forward-airfield-project: one legal known-land step to {:?}; destination {:?} is rechecked each frame", next, project.field);
        }
        Some(moved)
    }
}

#[cfg(test)]
mod tests;
