use super::*;

impl AdvancedAi {
    pub(super) fn reserve_delegated_conquest_frontline(
        &self,
        game: &mut Game,
        player: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        if !self.lane_delegates_now(
            self.active_victory_target(game),
            self.phase_specialization_active(game),
        ) || self.base.minor
            || self.base.barb
        {
            return None;
        }
        let mut candidates = Vec::new();
        for city in game.player_city_ids(player) {
            let held = &game.cities[&city];
            if !held.queue.is_empty()
                || (held.last_attacked > 0 && game.turn.saturating_sub(held.last_attacked) <= 4)
                || self
                    .base
                    .barbarian_local_alarm_for_controller(game, player, city)
            {
                continue;
            }
            for item in game.producible_items(player, city) {
                let Item::Unit { unit } = item else { continue };
                let gap = self.conquest_campaign_frontline_gap(game, player, city, plan, unit);
                if gap == 0 {
                    continue;
                }
                let item = Item::Unit { unit };
                let turns = self.production_build_turns(game, player, city, &item);
                if !turns.is_finite() || turns < 0.0 {
                    continue;
                }
                candidates.push((game.rules.units[unit].strength, turns, city, unit, gap));
            }
        }
        let (_, turns, city, unit, gap) = candidates.into_iter().max_by(|left, right| {
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
            "{} reserves {} for the contemporary frontline", game.cities[&city].name, unit;
            "delegated production has {gap} unfilled contemporary frontline slot(s) beyond at least three obsolete bodies; the ordinary current production, material, treasury and queued-upkeep gates hold; about {turns:.1} build turns";
            game.cities[&city].pos);
        Some((city, item))
    }

    pub(super) fn conquest_campaign_frontline_gap(
        &self,
        game: &Game,
        player: usize,
        city: u32,
        plan: &StrategicPlan,
        unit: Name,
    ) -> usize {
        let spec = &game.rules.units[unit];
        let is_frontliner = |kind: Name| {
            let candidate = &game.rules.units[kind];
            candidate.class == "military"
                && !matches!(candidate.domain.as_deref(), Some("sea" | "air"))
                && matches!(
                    candidate.promotion_class.as_str(),
                    "melee" | "anti_cavalry" | "light_cavalry" | "heavy_cavalry"
                )
                && !candidate.has_ranged_attack()
                && candidate.ranged_attack_strength() <= 0.0
        };
        if !is_frontliner(unit)
            || spec.strength < 45.0
            || !self.victory_planning
            || self.active_victory_target(game) != Some(VictoryTarget::Domination)
            || self.war_plan.is_some()
            || !matches!(
                plan.strategy,
                GrandStrategy::Expansion | GrandStrategy::Conquest
            )
            || plan.rush
            || plan.threatened_city.is_some()
            || game.player_city_ids(player).len() < 2
            || !game.players[player]
                .techs
                .contains(&crate::name!("writing"))
            || !game.can_produce(player, city, &Item::Unit { unit })
            || !plan.target_player.is_some_and(|target| {
                game.players.get(target).is_some_and(|rival| {
                    rival.alive
                        && !rival.is_minor
                        && !rival.is_barbarian
                        && self.campaign_target_legal(game, player, target)
                        && plan.target_city.is_some_and(|target_city| {
                            game.cities.get(&target_city).is_some_and(|objective| {
                                objective.owner == target
                                    && Self::city_within_declaration_range(
                                        game,
                                        player,
                                        objective.pos,
                                    )
                            })
                        })
                })
            })
            || game.players.iter().any(|rival| {
                rival.id != player
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && game.is_at_war(player, rival.id)
            })
        {
            return 0;
        }
        let (modern, obsolete) = game
            .units
            .values()
            .filter(|held| held.owner == player && held.hp > 0 && is_frontliner(held.kind))
            .fold((0usize, 0usize), |(modern, obsolete), held| {
                let strength = game.rules.units[held.kind].strength;
                (
                    modern + usize::from(strength >= spec.strength),
                    obsolete + usize::from(strength + 15.0 <= spec.strength),
                )
            });
        if modern >= 2 || obsolete < 3 {
            return 0;
        }
        let surcharge = game
            .policy_effect(player, "unit_maintenance_surcharge")
            .max(0.0);
        let (queued_modern, queued_upkeep) = game
            .cities
            .values()
            .filter(|candidate| candidate.owner == player && candidate.id != city)
            .filter_map(|candidate| candidate.queue.first().and_then(Self::war_item_unit))
            .fold(
                (0usize, 0.0),
                |(queued_modern, queued_upkeep), (queued, formation)| {
                    let queued_spec = &game.rules.units[queued];
                    let bodies = match formation {
                        1 => 1.5,
                        2.. => 2.0,
                        _ => 1.0,
                    };
                    let military_surcharge = if queued_spec.class == "military" {
                        surcharge
                    } else {
                        0.0
                    };
                    (
                        queued_modern
                            + usize::from(
                                is_frontliner(queued) && queued_spec.strength >= spec.strength,
                            ),
                        queued_upkeep
                            + queued_spec.maintenance.max(0.0) * bodies
                            + military_surcharge,
                    )
                },
            );
        let gap = 2usize.saturating_sub(modern + queued_modern);
        let income = game.players[player].gold_per_turn;
        let mut floor = 30.0 + (-income).max(0.0);
        if self.solvent_or_hold {
            floor = super::gold_and_cards::upgrade_floor_from(
                floor,
                game.unit_gold_maintenance(player),
            );
        }
        if gap == 0
            || !income.is_finite()
            || !game.players[player].gold.is_finite()
            || game.players[player].gold < floor
            || income - queued_upkeep - spec.maintenance.max(0.0) - surcharge < 2.0
        {
            return 0;
        }
        gap
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
