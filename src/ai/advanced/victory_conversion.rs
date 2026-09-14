//! Ten independent experiments that convert an empire into a victory.
//! Forecasts are estimates, never legality overrides. All effects are zero
//! with their switch off; the existing recovery, capture and purchase gates
//! remain authoritative. No win-rate uplift is assumed by these constants.

use super::*;
use crate::game::{expected_damage, QuickDeal};
use std::collections::VecDeque;

/// The production picker has already priced construction and its raw value.
#[derive(Clone, Copy)]
pub(super) struct ProductionQuote {
    pub(super) turns: f64,
    pub(super) raw: f64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct ConversionState {
    samples: BTreeMap<usize, VecDeque<(u32, GrandStrategy, f64)>>,
    pub(super) horizon: Option<f64>,
    pub(super) parks: BTreeSet<Pos>,
    pub(super) resorts: BTreeSet<Pos>,
    woods: BTreeSet<Pos>,
    pub(super) upgrade_goal: Option<Name>,
    pub(super) upgrade_units: Vec<u32>,
    pub(super) upgrade_reserve: f64,
    upgrade_started: Option<u32>,
}

/// At least five turns of movement in the *same* victory counter are needed.
/// Flat or falling progress supplies no deadline, rather than an infinite
/// urgency. Values above 100 are useful for the strict culture finish line.
fn closing_eta(samples: &VecDeque<(u32, GrandStrategy, f64)>) -> Option<f64> {
    let &(now, lane, progress) = samples.back()?;
    let &(then, _, before) = samples
        .iter()
        .find(|(turn, old_lane, _)| *old_lane == lane && now.saturating_sub(*turn) >= 5)?;
    let rate = (progress - before) / now.saturating_sub(then).max(1) as f64;
    (rate > 0.05).then(|| ((101.0 - progress) / rate).max(1.0))
}

impl AdvancedAi {
    fn conversion_culture(&self, g: &Game) -> bool {
        g.victory_conditions.culture
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
    }

    fn conversion_domination(&self, g: &Game) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Domination)
    }

    /// Called once at turn start, before research, diplomacy or spending.
    pub(super) fn observe_victory_conversion(&mut self, g: &Game, pid: usize) {
        self.conversion.horizon = None;
        if self.victory_deadline_budget
            && (self.conversion_culture(g) || self.conversion_domination(g))
        {
            for player in &g.players {
                if !player.alive
                    || player.is_minor
                    || player.is_barbarian
                    || (player.id != pid
                        && (!g.has_met(pid, player.id) || g.same_team(pid, player.id)))
                {
                    continue;
                }
                let focus = self.rival_victory_pressure(g, player.id);
                let (lane, progress) = if player.id == pid && self.conversion_culture(g) {
                    let bar = g
                        .players
                        .iter()
                        .filter(|p| {
                            p.id != pid
                                && p.alive
                                && !p.is_minor
                                && !p.is_barbarian
                                && !g.same_team(pid, p.id)
                        })
                        .map(|p| g.domestic_tourists(p.id))
                        .max()
                        .unwrap_or(1)
                        .max(1);
                    (
                        GrandStrategy::Culture,
                        100.0 * g.foreign_tourists(pid) as f64 / bar as f64,
                    )
                } else if player.id == pid && self.conversion_domination(g) {
                    let capitals: Vec<_> = g
                        .cities
                        .values()
                        .filter(|c| {
                            c.is_capital
                                && !g.players[c.original_owner].is_minor
                                && !g.players[c.original_owner].is_barbarian
                        })
                        .collect();
                    (
                        GrandStrategy::Conquest,
                        100.0 * capitals.iter().filter(|c| c.owner == pid).count() as f64
                            / capitals.len().max(1) as f64,
                    )
                } else {
                    (focus.strategy, focus.progress as f64)
                };
                let samples = self.conversion.samples.entry(player.id).or_default();
                if samples
                    .back()
                    .is_some_and(|(t, l, _)| *t > g.turn || *l != lane)
                {
                    samples.clear();
                }
                if samples.back().is_some_and(|(t, _, _)| *t == g.turn) {
                    samples.pop_back();
                }
                samples.push_back((g.turn, lane, progress));
                while samples.front().is_some_and(|(t, _, _)| {
                    g.turn.saturating_sub(*t) > g.standard_duration(30).max(10)
                }) {
                    samples.pop_front();
                }
            }
            let ours = self
                .conversion
                .samples
                .get(&pid)
                .and_then(closing_eta)
                .unwrap_or(f64::INFINITY);
            let rival = self
                .conversion
                .samples
                .iter()
                .filter(|(other, samples)| {
                    **other != pid
                        && g.players[**other].alive
                        && !g.same_team(pid, **other)
                        && samples.back().is_some_and(|(turn, _, _)| *turn == g.turn)
                })
                .filter_map(|(_, samples)| closing_eta(samples))
                .min_by(f64::total_cmp);
            self.conversion.horizon = rival.filter(|eta| *eta < ours);
        }
        self.plan_tourism_land(g, pid);
        self.plan_upgrade_window(g, pid);
    }

    pub(super) fn conversion_horizon(&self, g: &Game) -> f64 {
        let remaining = g
            .max_turns
            .min(g.game_speed.turn_limit())
            .saturating_sub(g.turn) as f64;
        if self.victory_deadline_budget {
            self.conversion
                .horizon
                .unwrap_or(remaining)
                .min(remaining)
                .max(1.0)
        } else {
            remaining.max(1.0)
        }
    }

    /// A legal building preview uses the engine's real housing, theming,
    /// power and tourism modifiers. The delta is native-model minus native-
    /// model, so live yield corrections cannot masquerade as an improvement.
    fn building_tourism_gain(g: &Game, pid: usize, cid: u32, building: Name) -> (f64, bool) {
        if g.cities[&cid].buildings.contains(&building) {
            return (0.0, false);
        }
        let mut after = g.clone();
        // A live housing snapshot pins existing placements. Compare both
        // counterfactuals using automatic housing, so adding a legal slot can
        // activate a work without claiming credit for unrelated rearranging.
        after.observed_great_work_housing = None;
        let before = after.tourism_per_turn_model(pid);
        let housing = after.housed_great_works(pid);
        after.cities.get_mut(&cid).unwrap().buildings.push(building);
        // A host-reported blocked creator is concrete supply. Count one
        // usable work conservatively; do not fabricate creator/era metadata
        // and thus do not invent a theme from an unidentified work.
        for need in &g.players[pid].live_great_person_activation_needs {
            if let Some(kind) = &need.required_great_work {
                let slots = &g.rules.buildings[building].great_work_slots;
                if (slots.contains_key(kind) || slots.contains_key("any"))
                    && after.can_house_additional_great_work(pid, kind)
                {
                    *after.players[pid]
                        .counters
                        .entry(format!("great_work:{kind}"))
                        .or_default() += 1;
                }
            }
        }
        (
            (after.tourism_per_turn_model(pid) - before).max(0.0),
            after.housed_great_works(pid) != housing,
        )
    }

    pub(super) fn conversion_production_adjustment(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        plan: &StrategicPlan,
        quote: ProductionQuote,
    ) -> f64 {
        let ProductionQuote { turns, raw } = quote;
        if raw <= 0.0 || plan.strategy == GrandStrategy::Recovery {
            return 0.0;
        }
        let culture = self.conversion_culture(g);
        let domination = self.conversion_domination(g);
        let mut bonus = 0.0;
        let horizon = self.conversion_horizon(g);
        if self.victory_deadline_budget
            && (culture || domination)
            && self.conversion.horizon.is_some()
        {
            let military = matches!(item, Item::Unit { unit } | Item::Formation { unit, .. } if g.rules.units[*unit].class == "military");
            let survival = plan.threatened_city == Some(cid)
                && (military || matches!(item, Item::Repair { .. } | Item::Building { .. }));
            if !survival {
                let long_investment = matches!(item, Item::Unit { unit } if *unit == "settler")
                    || matches!(item, Item::Wonder { .. });
                if turns >= horizon
                    || (long_investment && turns + g.standard_duration(25) as f64 >= horizon)
                {
                    bonus -= raw * 0.8;
                } else if (domination && military)
                    || (culture
                        && matches!(item, Item::Building { building } if !g.rules.buildings[*building].great_work_slots.is_empty()))
                {
                    bonus += raw * 0.25 * (1.0 - turns / horizon);
                }
            }
        }
        if culture && (self.culture_tourism_payback || self.great_work_completion_value) {
            if let Item::Building { building } = item {
                let spec = &g.rules.buildings[*building];
                if !spec.great_work_slots.is_empty()
                    || spec.effects.keys().any(|k| k.contains("tourism"))
                {
                    let (gain, completes_housing) =
                        Self::building_tourism_gain(g, pid, cid, *building);
                    if self.culture_tourism_payback {
                        let markets: f64 = g
                            .players
                            .iter()
                            .filter(|p| {
                                p.id != pid
                                    && p.alive
                                    && !p.is_minor
                                    && !p.is_barbarian
                                    && !g.same_team(pid, p.id)
                                    && g.has_met(pid, p.id)
                            })
                            .map(|p| g.international_tourism_multiplier(pid, p.id, false))
                            .sum();
                        bonus += (gain * (horizon - turns).max(0.0) * markets * 2.0).min(raw * 1.5);
                        if gain == 0.0 && spec.great_person_points.is_empty() {
                            bonus -= raw * 0.25;
                        }
                    }
                    if self.great_work_completion_value
                        && gain > 0.0
                        && !spec.great_work_slots.is_empty()
                        && completes_housing
                    {
                        bonus += (gain * 100.0).min(raw * 0.75);
                    }
                }
            }
        }
        if domination && (self.reinforce_before_stall || self.siege_positive_damage_budget) {
            bonus += self.conversion_reinforcement_bonus(g, pid, cid, item, plan, quote);
        }
        if self.capture_hold_chain && domination && g.cities[&cid].occupied_from.is_some() {
            let city = &g.cities[&cid];
            let rate = g.city_loyalty_per_turn(city);
            if matches!(item, Item::Repair { .. }) && (city.hp < CITY_MAX_HP || rate < 0.0) {
                bonus += raw * 0.75;
            }
            if let Item::Building { building } = item {
                let spec = &g.rules.buildings[*building];
                if rate < 0.0
                    && (spec.effects.get("loyalty_per_turn").copied().unwrap_or(0.0) > 0.0
                        || spec.amenity > 0.0)
                {
                    bonus += raw * 0.75;
                }
            }
        }
        if culture && self.tourism_land_reservation {
            if let Item::District { pos, .. } | Item::Wonder { pos, .. } = item {
                if self.conversion.parks.contains(pos) || self.conversion.resorts.contains(pos) {
                    bonus -= raw * 0.8;
                }
            }
        }
        bonus.clamp(-raw * 0.9, raw * 2.5)
    }

    /// Price the exact offered work transfer, including identity-based themes.
    /// The hypothetical trade never executes on the real game.
    pub(super) fn completion_deal_bonus(&self, g: &Game, pid: usize, deal: &QuickDeal) -> f64 {
        if !self.great_work_completion_value
            || !self.conversion_culture(g)
            || deal.category != "great_work"
            || deal.direction != "buy"
        {
            return 0.0;
        }
        let mut after = g.clone();
        if after
            .apply(
                pid,
                &Action::Trade {
                    player: deal.partner,
                    offer: Box::new(deal.offer.clone()),
                    request: Box::new(deal.request.clone()),
                },
            )
            .is_err()
        {
            return 0.0;
        }
        let delta = (after.tourism_per_turn_model(pid) - g.tourism_per_turn_model(pid)).max(0.0);
        let cost = deal.offer.gold.max(0.0)
            + deal.offer.gold_per_turn.max(0.0) * g.standard_duration(30) as f64;
        delta * self.conversion_horizon(g) / (1.0 + cost)
    }

    /// Reserve only for usable sites, shortly before their unlock. The clone
    /// grants the civic for *planning* and never changes the player's tree.
    fn tourism_opportunity(&self, g: &Game, pid: usize) -> Option<(Name, f64, u32)> {
        if !self.conversion_culture(g) {
            return None;
        }
        let mut choices = Vec::new();
        let park_window = g.players[pid]
            .civics
            .contains(&crate::name!("conservation"))
            || g.players[pid].civic.as_deref() == Some("conservation");
        let band_window = g.players[pid].civics.contains(&crate::name!("cold_war"))
            || g.players[pid].civic.as_deref() == Some("cold_war");
        if !park_window && !band_window {
            return None;
        }
        let mut future = g.clone();
        if park_window {
            future.players[pid]
                .civics
                .insert(crate::name!("conservation"));
        }
        if band_window {
            future.players[pid].civics.insert(crate::name!("cold_war"));
        }
        let mut parks = future.national_park_sites(pid);
        parks.sort_by_key(|site| {
            (
                std::cmp::Reverse(site.iter().map(|pos| g.tile_appeal(*pos)).sum::<i32>()),
                *site,
            )
        });
        parks.truncate(3);
        let active_naturalist = g
            .units
            .values()
            .any(|u| u.owner == pid && u.kind == "naturalist");
        let horizon = self.conversion_horizon(g);
        // Bound the route search to the three closest purchase cities. Each
        // probe starts at a real purchase city, never at a veteran overseas.
        let mut cities = g.player_city_ids(pid);
        cities.sort_by_key(|cid| {
            (
                parks
                    .iter()
                    .map(|site| g.wdist(g.cities[cid].pos, site[0]))
                    .min()
                    .unwrap_or(0),
                *cid,
            )
        });
        cities.truncate(3);
        for cid in cities {
            if park_window && !active_naturalist && !parks.is_empty() {
                let mut probe = future.clone();
                let uid = probe.spawn_unit("naturalist", pid, g.cities[&cid].pos);
                let best = parks
                    .iter()
                    .filter_map(|site| {
                        let distance = site
                            .iter()
                            .filter_map(|pos| probe.route_distance(uid, *pos, 0))
                            .min()? as f64;
                        let active = (horizon
                            - distance / probe.rules.units["naturalist"].moves.max(1.0)
                            - 1.0)
                            .max(0.0);
                        Some(
                            site.iter()
                                .map(|pos| g.tile_appeal(*pos).max(0) as f64)
                                .sum::<f64>()
                                * active,
                        )
                    })
                    .max_by(f64::total_cmp);
                if let Some(value) = best.filter(|value| *value > 0.0) {
                    let cost = g.naturalist_purchase_cost(pid);
                    choices.push((value / cost.max(1.0), crate::name!("naturalist"), cost, cid));
                }
            }
            if band_window {
                let mut probe = future.clone();
                let uid = probe.spawn_unit("rock_band", pid, g.cities[&cid].pos);
                // A new band must take its free promotion before it can perform.
                // Use an actually offered starter promotion only in the preview.
                if let Some(promotion) = probe.available_promotions(uid).first().copied() {
                    probe
                        .units
                        .get_mut(&uid)
                        .unwrap()
                        .promotions
                        .insert(promotion);
                }
                let mut venues: Vec<_> = probe
                    .map
                    .tiles
                    .keys()
                    .filter_map(|pos| {
                        if !g.players[pid].explored.contains(pos) {
                            return None;
                        }
                        let value = probe.rock_concert_ai_value(pid, uid, *pos)?;
                        Some((
                            value / (1.0 + g.wdist(g.cities[&cid].pos, *pos) as f64),
                            *pos,
                        ))
                    })
                    .collect();
                venues.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
                let best = venues
                    .into_iter()
                    .take(12)
                    .filter_map(|(_, pos)| {
                        let distance = probe.route_distance(uid, pos, 0)? as f64
                            / probe.rules.units["rock_band"].moves.max(1.0);
                        (distance + 1.0 < horizon).then(|| {
                            probe.rock_concert_ai_value(pid, uid, pos).unwrap() / (1.0 + distance)
                        })
                    })
                    .max_by(f64::total_cmp);
                if let Some(value) = best {
                    if let Some(cost) = future.unit_purchase_cost(pid, cid, "rock_band", "faith") {
                        choices.push((value / cost.max(1.0), crate::name!("rock_band"), cost, cid));
                    }
                }
            }
        }
        choices
            .into_iter()
            .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
            .map(|(_, kind, cost, cid)| (kind, cost, cid))
    }

    pub(super) fn conversion_faith_reserve(&self, g: &Game, pid: usize, ordinary: f64) -> f64 {
        if !self.culture_faith_reservation || !self.conversion_culture(g) {
            return ordinary;
        }
        self.tourism_opportunity(g, pid)
            .map(|(_, cost, _)| cost)
            .unwrap_or(0.0)
    }

    pub(super) fn conversion_culture_purchase(&self, g: &mut Game, pid: usize) -> bool {
        if !self.culture_faith_reservation || !self.conversion_culture(g) {
            return false;
        }
        if let Some((kind, _, cid)) = self.tourism_opportunity(g, pid) {
            let action = Action::Buy {
                city: cid,
                unit: kind,
                formation: 0,
                currency: "faith".into(),
            };
            if g.apply(pid, &action).is_ok() {
                return true;
            }
        }
        // The gene owns this purchase pass even when banking for an unlock.
        true
    }

    fn plan_tourism_land(&mut self, g: &Game, pid: usize) {
        self.conversion.parks.clear();
        self.conversion.resorts.clear();
        self.conversion.woods.clear();
        if !self.tourism_land_reservation
            || !self.conversion_culture(g)
            || !g.players[pid].civics.contains(&crate::name!("humanism"))
        {
            return;
        }
        let mut future = g.clone();
        future.players[pid]
            .civics
            .insert(crate::name!("conservation"));
        let plantable: BTreeSet<_> = g
            .player_city_ids(pid)
            .into_iter()
            .flat_map(|cid| g.cities[&cid].owned_tiles.iter().copied())
            .filter(|pos| {
                g.map.tiles[pos].improvement.is_none()
                    && future
                        .builder_operations(pid, *pos)
                        .iter()
                        .any(|op| op == "plant_woods")
            })
            .collect();
        for pos in &plantable {
            future.map.tiles.get_mut(pos).unwrap().feature = Some(crate::name!("forest"));
        }
        let mut sites = future.national_park_sites(pid);
        sites.sort_by_key(|site| {
            (
                std::cmp::Reverse(site.iter().map(|p| g.tile_appeal(*p)).sum::<i32>()),
                *site,
            )
        });
        let mut cities = BTreeSet::new();
        for site in sites {
            let cid = g.map.tiles[&site[0]].owner_city.unwrap();
            if cities.len() < 3 && cities.insert(cid) {
                for pos in site.iter().flat_map(|pos| g.nbrs(*pos)).chain(site) {
                    if plantable.contains(&pos) {
                        self.conversion.woods.insert(pos);
                    }
                }
                self.conversion.parks.extend(site);
            }
        }
        for cid in g.player_city_ids(pid) {
            let best = g.cities[&cid]
                .owned_tiles
                .iter()
                .copied()
                .filter(|pos| {
                    let tile = &g.map.tiles[pos];
                    tile.improvement.is_none()
                        && tile.district.is_none()
                        && tile.wonder.is_none()
                        && tile.resource.is_none()
                        && !tile.hills
                        && !tile.flooded
                        && !tile.submerged
                        && matches!(tile.terrain.as_str(), "grassland" | "plains" | "desert")
                        && g.tile_appeal(*pos) >= 4
                        && !self.conversion.parks.contains(pos)
                        && g.nbrs(*pos)
                            .iter()
                            .any(|p| g.map.get(*p).is_some_and(|t| g.rules.is_water(t)))
                })
                .max_by_key(|pos| (g.tile_appeal(*pos), std::cmp::Reverse(*pos)));
            if let Some(pos) = best {
                self.conversion.resorts.insert(pos);
            }
        }
    }

    pub(super) fn conversion_land_penalty(&self, g: &Game, pos: Pos, improvement: &str) -> f64 {
        if !self.tourism_land_reservation {
            return 0.0;
        }
        let tourism = g.rules.improvements[improvement]
            .effects
            .get("appeal_tourism")
            .copied()
            .unwrap_or(0.0)
            > 0.0;
        let damages_appeal = g.rules.improvements[improvement]
            .effects
            .get("adjacent_appeal")
            .copied()
            .unwrap_or(0.0)
            < 0.0
            && self.conversion_preserves_woods(g, pos);
        if damages_appeal
            || self.conversion.parks.contains(&pos)
            || (self.conversion.resorts.contains(&pos) && !tourism)
        {
            -10_000.0
        } else {
            0.0
        }
    }

    pub(super) fn conversion_land_builder(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.tourism_land_reservation || !self.conversion_culture(g) {
            return None;
        }
        let from = g.units.get(&uid)?.pos;
        let target = self
            .conversion
            .woods
            .iter()
            .copied()
            .filter(|pos| {
                g.builder_operations(pid, *pos)
                    .iter()
                    .any(|op| op == "plant_woods")
            })
            .filter(|pos| g.wdist(from, *pos) <= 8)
            .filter_map(|pos| g.route_distance(uid, pos, 0).map(|d| (d, pos)))
            .min()?
            .1;
        if target == from {
            let applied = g
                .apply(
                    pid,
                    &Action::Improve {
                        unit: uid,
                        improvement: crate::name!("plant_woods"),
                    },
                )
                .is_ok();
            if applied {
                self.builder_targets.remove(&uid);
            }
            return applied.then_some(true);
        }
        self.builder_step_toward_barbarian_safe(g, pid, uid, target)
            .then_some(true)
    }

    pub(super) fn conversion_plot_bonus(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        pos: Pos,
        cost: f64,
    ) -> f64 {
        if !self.tourism_land_reservation
            || !self.conversion_culture(g)
            || !g.players[pid].civics.contains(&crate::name!("humanism"))
        {
            return 0.0;
        }
        let mut after = g.clone();
        after.players[pid]
            .civics
            .insert(crate::name!("conservation"));
        after.map.tiles.get_mut(&pos).unwrap().owner_city = Some(cid);
        after.cities.get_mut(&cid).unwrap().owned_tiles.push(pos);
        let value = after
            .national_park_sites(pid)
            .iter()
            .filter(|site| site.contains(&pos))
            .map(|site| {
                site.iter()
                    .map(|p| after.tile_appeal(*p).max(0) as f64)
                    .sum::<f64>()
            })
            .max_by(f64::total_cmp)
            .unwrap_or(0.0);
        if value > 0.0 {
            (value * self.conversion_horizon(g) * 0.5 - cost).clamp(0.0, 600.0)
        } else {
            0.0
        }
    }

    pub(super) fn conversion_preserves_woods(&self, g: &Game, pos: Pos) -> bool {
        self.tourism_land_reservation
            && (self.conversion.parks.contains(&pos)
                || g.nbrs(pos).iter().any(|p| {
                    self.conversion.parks.contains(p) || self.conversion.resorts.contains(p)
                }))
    }

    /// A bounded campaign beam: the next capital plus one subsequent capital.
    /// Only seen cities and actual approach routes are considered. Existing
    /// war/forced-target selection still chooses the opponent.
    pub(super) fn conversion_campaign_cost(
        &self,
        g: &Game,
        pid: usize,
        city: &crate::game::City,
    ) -> Option<f64> {
        if !self.capital_campaign_router || !self.conversion_domination(g) || city.owner == pid {
            return None;
        }
        let visible = g.player_visibility(pid);
        if !visible.contains(&city.pos) && self.remembered_city(city.id).is_none() {
            return None;
        }
        let mut units: Vec<_> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|id| {
                let spec = &g.rules.units[g.units[id].kind];
                spec.class == "military" && spec.is_melee_capable()
            })
            .collect();
        units.sort_by_key(|id| (g.wdist(g.units[id].pos, city.pos), *id));
        let march = units
            .into_iter()
            .take(4)
            .filter_map(|id| {
                let distance = if g.is_at_war(pid, city.owner) {
                    g.route_distance(id, city.pos, 1)
                } else {
                    let mut posts: Vec<_> = g
                        .wdisk(city.pos, 5)
                        .into_iter()
                        .filter(|pos| {
                            self.campaign_staging_position(g, pid, city.owner, id, city.pos, *pos)
                        })
                        .collect();
                    posts.sort_by_key(|pos| (g.wdist(g.units[&id].pos, *pos), *pos));
                    posts
                        .into_iter()
                        .take(4)
                        .filter_map(|pos| g.route_distance(id, pos, 0))
                        .min()
                }?;
                Some(distance as f64 / g.rules.units[g.units[&id].kind].moves.max(1.0))
            })
            .min_by(f64::total_cmp)?;
        let onward = g
            .cities
            .values()
            .filter(|c| {
                c.id != city.id
                    && c.is_capital
                    && c.owner != pid
                    && !g.players[c.original_owner].is_minor
                    && !g.players[c.original_owner].is_barbarian
                    && !g.same_team(pid, c.owner)
                    && (visible.contains(&c.pos) || self.remembered_city(c.id).is_some())
            })
            .map(|c| g.wdist(city.pos, c.pos))
            .min()
            .unwrap_or(0) as f64
            / 2.0;
        let (hp, walls) = self
            .remembered_city(city.id)
            .map(|c| (c.hp, c.wall_hp))
            .unwrap_or((city.hp, city.wall_hp));
        let reduction = hp.max(0) as f64 / 35.0 + walls.max(0) as f64 / 25.0;
        let hold = if Self::should_defer_city_capture(g, pid, city.id) {
            10_000.0
        } else {
            self.conversion_hold_cost(g, pid, city.id) / 15.0
        };
        Some(march + reduction + onward + hold + if city.is_capital { 0.0 } else { 8.0 })
    }

    pub(super) fn conversion_campaign_target(
        &self,
        g: &Game,
        pid: usize,
        target: Option<usize>,
    ) -> Option<u32> {
        if !self.capital_campaign_router || !self.conversion_domination(g) {
            return None;
        }
        let target = target?;
        g.cities
            .values()
            .filter(|city| city.owner == target)
            .filter_map(|city| {
                self.conversion_campaign_cost(g, pid, city)
                    .map(|cost| (cost, city.id))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, cid)| cid)
    }

    fn plan_upgrade_window(&mut self, g: &Game, pid: usize) {
        let old_goal = self.conversion.upgrade_goal;
        self.conversion.upgrade_goal = None;
        self.conversion.upgrade_units.clear();
        self.conversion.upgrade_reserve = 0.0;
        if !self.upgrade_window_campaign || !self.conversion_domination(g) {
            return;
        }
        let mut packages: BTreeMap<Name, Vec<(u32, f64)>> = BTreeMap::new();
        for uid in g.player_unit_ids(pid) {
            let unit = &g.units[&uid];
            let spec = &g.rules.units[unit.kind];
            if spec.class != "military" || unit.hp < 60 {
                continue;
            }
            let Some(next) = spec.upgrade_to.map(|n| g.player_unit_replacement(pid, n)) else {
                continue;
            };
            let Some(tech) = g.rules.units[next].tech else {
                continue;
            };
            if Self::war_remaining_research_cost(g, pid, tech) / Self::war_science_per_turn(g, pid)
                > g.standard_duration(15).max(5) as f64
            {
                continue;
            }
            let Some((gold, _)) =
                g.unit_upgrade_price_in_formation(pid, unit.kind, next, unit.formation)
            else {
                continue;
            };
            packages.entry(tech).or_default().push((uid, gold));
        }
        let best = packages
            .into_iter()
            .filter(|(_, units)| units.len() >= 2)
            .min_by(|a, b| {
                let value = |(tech, units): &(Name, Vec<(u32, f64)>)| {
                    (Self::war_remaining_research_cost(g, pid, *tech)
                        / Self::war_science_per_turn(g, pid)
                        + units.iter().map(|(_, gold)| gold).sum::<f64>() / 100.0)
                        / units.len() as f64
                };
                value(a).total_cmp(&value(b)).then(a.0.cmp(&b.0))
            });
        if let Some((tech, units)) = best {
            if old_goal != Some(tech) {
                self.conversion.upgrade_started = Some(g.turn);
            }
            self.conversion.upgrade_goal = Some(tech);
            // An unaffordable or resource-starved upgrade cannot freeze war
            // forever. A timed-out package relinquishes its spending reserve.
            if self
                .conversion
                .upgrade_started
                .is_some_and(|turn| g.turn.saturating_sub(turn) > g.standard_duration(20).max(5))
            {
                return;
            }
            self.conversion.upgrade_goal = Some(tech);
            for (uid, gold) in units.into_iter().take(4) {
                self.conversion.upgrade_units.push(uid);
                self.conversion.upgrade_reserve += gold;
            }
        }
    }

    pub(super) fn conversion_upgrade_research(&self) -> Option<&str> {
        (self.upgrade_window_campaign && !self.conversion.upgrade_units.is_empty())
            .then_some(self.conversion.upgrade_goal)
            .flatten()
            .map(|n| n.as_str())
    }

    pub(super) fn conversion_upgrade_first(&self, g: &mut Game, pid: usize) {
        if !self.upgrade_window_campaign {
            return;
        }
        let Some(goal) = self.conversion.upgrade_goal else {
            return;
        };
        if !g.players[pid].techs.contains(&goal) {
            return;
        }
        for uid in &self.conversion.upgrade_units {
            let _ = g.apply(pid, &Action::UpgradeUnit { unit: *uid });
        }
    }

    pub(super) fn conversion_upgrade_launch_ready(&self, g: &Game, pid: usize) -> bool {
        if !self.upgrade_window_campaign || self.threatened_city(g, pid).is_some() {
            return true;
        }
        let Some(goal) = self.conversion.upgrade_goal else {
            return true;
        };
        if self.conversion.upgrade_units.is_empty() {
            return true;
        }
        if !g.players[pid].techs.contains(&goal) {
            return false;
        }
        let waiting = self
            .conversion
            .upgrade_units
            .iter()
            .filter(|id| {
                g.units.get(id).is_some_and(|u| {
                    g.rules.units[u.kind]
                        .upgrade_to
                        .map(|n| g.player_unit_replacement(pid, n))
                        .is_some_and(|n| g.rules.units[n].tech == Some(goal))
                })
            })
            .count();
        waiting <= self.conversion.upgrade_units.len() / 3
    }

    pub(super) fn conversion_hold_cost(&self, g: &Game, pid: usize, cid: u32) -> f64 {
        if !self.capture_hold_chain || !self.conversion_domination(g) {
            return 0.0;
        }
        let city = &g.cities[&cid];
        // Do not read the hidden live city, and never postpone the last capital.
        if city.owner == pid
            || !g.player_visibility(pid).contains(&city.pos)
            || (city.is_capital
                && g.cities
                    .values()
                    .filter(|c| {
                        c.is_capital
                            && c.owner != pid
                            && !g.same_team(pid, c.owner)
                            && !g.players[c.original_owner].is_minor
                    })
                    .count()
                    <= 1)
        {
            return 0.0;
        }
        let mut after = g.clone();
        let captured = after.cities.get_mut(&cid).unwrap();
        captured.owner = pid;
        captured.occupied_from = Some(city.owner);
        captured.loyalty = 50.0;
        Arc::make_mut(&mut after.observed_city_loyalty_per_turn).remove(&cid);
        let rate = after.city_loyalty_per_turn(&after.cities[&cid]);
        let relief = g
            .cities
            .values()
            .filter(|c| c.owner == pid)
            .map(|c| g.wdist(c.pos, city.pos))
            .min()
            .unwrap_or(30) as f64
            / 2.0
            + 3.0;
        if rate < 0.0 && 50.0 / -rate < relief {
            600.0
        } else {
            0.0
        }
    }

    pub(super) fn conversion_garrison_priority(&self, g: &Game, city: &crate::game::City) -> f64 {
        if !self.capture_hold_chain {
            return city.loyalty;
        }
        let rate = g.city_loyalty_per_turn(city);
        if rate < 0.0 {
            city.loyalty / -rate
        } else {
            1000.0 + city.loyalty
        }
    }

    /// Stage and production share the same health budget. Damage uses the
    /// engine's strength and mean-roll functions; the wall reduction factors
    /// are those documented at `Game::city_take_damage`.
    pub(super) fn conversion_siege_budget(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        force: &[u32],
    ) -> Option<(f64, f64)> {
        let city = g.cities.get(&cid)?;
        if !g.player_visibility(pid).contains(&city.pos) {
            return None;
        }
        let mut wall_dps = 0.0;
        let mut city_dps = 0.0;
        let mut endurance = 0.0_f64;
        let mut taker = false;
        for uid in force {
            let Some(unit) = g
                .units
                .get(uid)
                .filter(|u| u.owner == pid && g.wdist(u.pos, city.pos) <= 5)
            else {
                continue;
            };
            let spec = &g.rules.units[unit.kind];
            if spec.class != "military" {
                continue;
            }
            let ranged = spec.ranged_strength > 0.0 || spec.bombard_strength > 0.0;
            let attack = if ranged {
                g.unit_ranged_attack_strength(unit)
            } else {
                g.unit_strength(unit, true)
            };
            let damage = expected_damage(attack, g.city_strength(cid));
            city_dps += damage;
            wall_dps += damage
                * if spec.siege {
                    1.0
                } else if ranged {
                    0.5
                } else {
                    0.15
                };
            let incoming =
                expected_damage(g.city_ranged_strength(cid), g.unit_strength(unit, false));
            endurance += (unit.hp as f64 - 20.0).max(0.0) / incoming.max(1.0);
            taker |= !ranged && spec.is_melee_capable();
        }
        let (sealed, ring) = super::siege_train::ring_state(g, cid);
        let heal = if ring > 0 && sealed == ring {
            0.0
        } else {
            20.0
        };
        if !taker || city_dps <= heal || (city.wall_hp > 0 && wall_dps <= 0.0) {
            return Some((f64::INFINITY, endurance));
        }
        let turns = city.wall_hp.max(0) as f64 / wall_dps.max(1.0)
            + city.hp.max(0) as f64 / (city_dps - heal);
        Some((turns + 1.0, endurance))
    }

    pub(super) fn conversion_siege_ready(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        force: &[u32],
    ) -> bool {
        !self.siege_positive_damage_budget
            || self
                .conversion_siege_budget(g, pid, cid, force)
                .is_some_and(|(turns, endurance)| turns <= endurance * 0.8)
    }

    fn conversion_reinforcement_bonus(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        plan: &StrategicPlan,
        quote: ProductionQuote,
    ) -> f64 {
        let ProductionQuote { turns, raw } = quote;
        let (Item::Unit { unit } | Item::Formation { unit, .. }) = item else {
            return 0.0;
        };
        let spec = &g.rules.units[*unit];
        if spec.class != "military" {
            return 0.0;
        }
        let Some(target) = plan
            .target_city
            .filter(|id| g.cities.get(id).is_some_and(|c| g.is_at_war(pid, c.owner)))
        else {
            return 0.0;
        };
        let pos = g.cities[&target].pos;
        let force: Vec<_> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|id| g.wdist(g.units[id].pos, pos) <= 5)
            .collect();
        let Some((finish, endurance)) = self.conversion_siege_budget(g, pid, target, &force) else {
            return 0.0;
        };
        let march = g.wdist(g.cities[&cid].pos, pos) as f64 / spec.moves.max(1.0);
        let arrival = turns + march;
        if arrival >= self.conversion_horizon(g) {
            return 0.0;
        }
        let missing_siege = g.cities[&target].wall_hp > 0
            && !force.iter().any(|id| g.rules.units[g.units[id].kind].siege);
        let missing_taker = !force.iter().any(|id| {
            let s = &g.rules.units[g.units[id].kind];
            s.class == "military"
                && s.is_melee_capable()
                && s.ranged_strength == 0.0
                && s.bombard_strength == 0.0
        });
        let fills_role = (missing_siege && spec.siege)
            || (missing_taker
                && spec.is_melee_capable()
                && spec.ranged_strength == 0.0
                && spec.bombard_strength == 0.0);
        let queued = g.player_city_ids(pid).into_iter().filter(|id| *id != cid).filter(|id| g.cities[id].queue.first().is_some_and(|i| matches!(i, Item::Unit { unit: other } | Item::Formation { unit: other, .. } if g.rules.units[*other].siege == spec.siege && g.rules.units[*other].class == "military"))).count();
        if queued >= 2 {
            return 0.0;
        }
        if self.siege_positive_damage_budget && fills_role {
            return raw * 1.25 / (1.0 + arrival / 10.0);
        }
        if self.reinforce_before_stall
            && (fills_role || finish >= endurance * 0.8 || arrival >= endurance * 0.5)
        {
            raw * 0.8 / (1.0 + arrival / 15.0)
        } else {
            0.0
        }
    }

    pub(super) fn conversion_hold_governor_bonus(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        governor: &str,
    ) -> f64 {
        if !self.capture_hold_chain || !self.conversion_domination(g) || governor != "victor" {
            return 0.0;
        }
        let city = &g.cities[&cid];
        if city.owner != pid || city.occupied_from.is_none() {
            return 0.0;
        }
        let delta = g.city_loyalty_per_turn(city);
        if delta < 0.0 {
            800.0 + 400.0 / (1.0 + city.loyalty / -delta)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests;
