use super::*;

impl AdvancedAi {
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
