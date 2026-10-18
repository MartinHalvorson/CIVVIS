use super::*;

impl AdvancedAi {
    pub(super) fn claim_conquest_frontline(
        &self,
        game: &mut Game,
        player: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        if !self.victory_planning
            || self.active_victory_target(game) != Some(VictoryTarget::Domination)
            || self.base.minor
            || self.base.barb
            || !matches!(
                plan.strategy,
                GrandStrategy::Expansion | GrandStrategy::Conquest
            )
            || plan.rush
            || plan.threatened_city.is_some()
            || self.base.plan_threatened_city.is_some()
            || self.threatened_city(game, player).is_some()
            || game.emergency_objective(player).is_some()
            || game.players[player].gold < 30.0
            || !game.players[player]
                .techs
                .contains(&crate::name!("writing"))
            || game.players.iter().any(|rival| {
                rival.id != player
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && game.is_at_war(player, rival.id)
            })
        {
            return None;
        }
        let cities = game.player_city_ids(player);
        if cities.len() < 3
            || !game.players.iter().any(|rival| {
                rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && self.campaign_target_legal(game, player, rival.id)
                    && game.player_city_ids(rival.id).iter().any(|city| {
                        Self::city_within_declaration_range(game, player, game.cities[city].pos)
                    })
            })
        {
            return None;
        }
        let frontline = |spec: &crate::rules::UnitSpec| {
            spec.class == "military"
                && spec.promotion_class != "recon"
                && !spec.siege
                && !spec.has_ranged_attack()
                && spec.is_melee_capable()
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        };
        let fielded: Vec<_> = game
            .player_unit_ids(player)
            .into_iter()
            .map(|unit| &game.rules.units[game.units[&unit].kind])
            .collect();
        if fielded
            .iter()
            .filter(|spec| spec.class == "military")
            .count()
            < cities.len()
        {
            return None;
        }
        let mut candidates = Vec::new();
        for city in &cities {
            let held = &game.cities[city];
            if held.is_capital
                || !held.queue.is_empty()
                || held
                    .production_progress
                    .values()
                    .any(|progress| *progress > 0.0)
                || (held.last_attacked > 0 && game.turn.saturating_sub(held.last_attacked) <= 4)
                || self
                    .base
                    .barbarian_local_alarm_for_controller(game, player, *city)
            {
                continue;
            }
            for item in game.producible_items(player, *city) {
                let Item::Unit { unit } = item else { continue };
                let spec = &game.rules.units[unit];
                if !frontline(spec) || spec.requires_resource.is_some() {
                    continue;
                }
                let obsolete = fielded
                    .iter()
                    .filter(|held| frontline(held) && held.strength + 15.0 <= spec.strength)
                    .count();
                if obsolete < 3 {
                    continue;
                }
                let fielded_strong = fielded
                    .iter()
                    .filter(|held| frontline(held) && held.strength >= spec.strength)
                    .count();
                let queued_strong = cities
                    .iter()
                    .flat_map(|held| game.cities[held].queue.iter())
                    .filter(|queued| match queued {
                        Item::Unit { unit } => {
                            let held = &game.rules.units[unit];
                            frontline(held) && held.strength >= spec.strength
                        }
                        _ => false,
                    })
                    .count();
                let missing = 2usize.saturating_sub(fielded_strong + queued_strong);
                let upkeep = spec.maintenance.max(0.0)
                    + game
                        .policy_effect(player, "unit_maintenance_surcharge")
                        .max(0.0);
                if missing == 0
                    || game.players[player].gold_per_turn < 2.0 + upkeep * missing as f64
                {
                    continue;
                }
                let turns = self.production_build_turns(game, player, *city, &Item::Unit { unit });
                candidates.push((spec.strength, turns, *city, unit));
            }
        }
        let (_, turns, city, unit) = candidates.into_iter().max_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| right.1.total_cmp(&left.1))
                .then_with(|| right.2.cmp(&left.2))
                .then_with(|| right.3.cmp(&left.3))
        })?;
        let item = Item::Unit { unit };
        game.apply(
            player,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .ok()?;
        think!(self.journal(), Military, Decision,
            "{} claims a {} for the modern frontline", game.cities[&city].name, unit;
            "at least three obsolete melee bodies need a stronger line; the two-body fielded-and-queued cap and conservative upkeep reserve hold; about {turns:.0} build turns";
            game.cities[&city].pos);
        Some((city, item))
    }

    pub(super) fn conquest_campaign_modernization_tech(
        &self,
        game: &Game,
        player: usize,
    ) -> Option<Name> {
        if !self.victory_planning
            || self.active_victory_target(game) != Some(VictoryTarget::Domination)
            || game.player_city_ids(player).len() < 2
            || !game.players[player]
                .techs
                .contains(&crate::name!("writing"))
            || !game.players.iter().any(|rival| {
                rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && self.campaign_target_legal(game, player, rival.id)
                    && game.player_city_ids(rival.id).iter().any(|city| {
                        Self::city_within_declaration_range(game, player, game.cities[city].pos)
                    })
            })
        {
            return None;
        }
        let mut goals: BTreeMap<Name, (usize, f64)> = BTreeMap::new();
        for unit in game.units.values().filter(|unit| unit.owner == player) {
            let held = &game.rules.units[unit.kind];
            if held.class != "military"
                || held.promotion_class == "recon"
                || matches!(held.domain.as_deref(), Some("sea" | "air"))
            {
                continue;
            }
            let Some(successor) = held.upgrade_to else {
                continue;
            };
            let successor = &game.rules.units[game.player_unit_replacement(player, successor)];
            let Some(technology) = successor.tech else {
                continue;
            };
            if !successor.buildable || game.players[player].techs.contains(&technology) {
                continue;
            }
            let gain = successor.strength.max(successor.ranged_attack_strength())
                - held.strength.max(held.ranged_attack_strength());
            if gain > 0.0 {
                let entry = goals.entry(technology).or_default();
                entry.0 += 1;
                entry.1 += gain;
            }
        }
        goals
            .into_iter()
            .filter(|(_, (count, _))| *count >= 3)
            .map(|(technology, (_, gain))| {
                let cost = Self::war_remaining_research_cost(game, player, technology).max(1.0);
                (technology, gain / cost)
            })
            .max_by(|(left, left_value), (right, right_value)| {
                left_value
                    .total_cmp(right_value)
                    .then_with(|| right.cmp(left))
            })
            .map(|(technology, _)| technology)
    }

    pub(super) fn conquest_campaign_objective(
        &self,
        game: &Game,
        player: usize,
        strategy: GrandStrategy,
        target_player: Option<usize>,
        target_city: Option<u32>,
    ) -> (Option<usize>, Option<u32>) {
        let unchanged = (target_player, target_city);
        if !matches!(strategy, GrandStrategy::Conquest | GrandStrategy::Expansion)
            || self.active_victory_target(game) != Some(VictoryTarget::Domination)
            || self.forced_target_player.is_some()
            || game.emergency_objective(player).is_some()
            || target_player.is_some_and(|rival| game.is_at_war(player, rival))
            || game.players.iter().any(|rival| {
                rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && game.is_at_war(player, rival.id)
            })
        {
            return unchanged;
        }
        let Some(preferred) = target_player else {
            return unchanged;
        };
        if game
            .players
            .get(preferred)
            .is_none_or(|rival| rival.is_minor)
            || target_city
                .is_some_and(|city| Self::capture_completes_domination(game, player, city))
        {
            return unchanged;
        }
        let eligible = |rival| {
            self.campaign_target_legal(game, player, rival)
                && self.war_policy_target_feasible(game, player, rival)
        };
        let reachable = |city: &crate::game::City| {
            Self::city_within_declaration_range(game, player, city.pos)
                || self.denial_reaches_far(game, player, city.owner, city.pos)
                || self.capital_prey_reaches_far(game, player, city.owner, city.pos)
        };
        if eligible(preferred)
            && target_city
                .and_then(|city| game.cities.get(&city))
                .is_some_and(|city| city.owner == preferred && reachable(city))
        {
            return unchanged;
        }
        let best_city = |rival| {
            game.cities
                .values()
                .filter(|city| city.owner == rival && reachable(city))
                .min_by(|left, right| {
                    self.campaign_city_value(game, player, left, GrandStrategy::Conquest)
                        .total_cmp(&self.campaign_city_value(
                            game,
                            player,
                            right,
                            GrandStrategy::Conquest,
                        ))
                        .then(left.id.cmp(&right.id))
                })
                .map(|city| city.id)
        };
        let replacement = eligible(preferred)
            .then(|| best_city(preferred))
            .flatten()
            .map(|city| (preferred, city))
            .or_else(|| {
                game.players
                    .iter()
                    .filter(|rival| {
                        rival.alive && !rival.is_minor && !rival.is_barbarian && eligible(rival.id)
                    })
                    .filter_map(|rival| best_city(rival.id).map(|city| (rival.id, city)))
                    .min_by(|left, right| {
                        self.campaign_target_value_with_culture(game, player, left.0, None)
                            .total_cmp(
                                &self.campaign_target_value_with_culture(
                                    game, player, right.0, None,
                                ),
                            )
                            .then(left.cmp(right))
                    })
            });
        match replacement {
            Some((rival, city)) => {
                think!(self.journal(), Strategy, Strategy,
                       "Campaign prepares a reachable objective at {}", game.cities[&city].name;
                       "the prior objective cannot pass the unchanged declaration-range guard";
                       game.cities[&city].pos);
                (Some(rival), Some(city))
            }
            None => (None, None),
        }
    }
}

#[cfg(test)]
mod tests;
