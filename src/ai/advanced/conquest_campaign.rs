use super::*;

impl AdvancedAi {
    pub(super) fn conquest_campaign_supply_site_credit(
        &self,
        game: &Game,
        player: usize,
        position: Pos,
    ) -> f64 {
        if !self.settlement_safety
            || !self.victory_planning
            || self.active_victory_target(game) != Some(VictoryTarget::Domination)
            || self.war_plan.is_some()
            || game.player_city_ids(player).len() < 2
            || !game.players[player]
                .techs
                .contains(&crate::name!("writing"))
            || game.blocked_city_sites.contains(&position)
        {
            return 0.0;
        }
        let Some(plan) = self.plan.as_ref() else {
            return 0.0;
        };
        if !matches!(
            plan.strategy,
            GrandStrategy::Expansion | GrandStrategy::Conquest
        ) || plan.rush
            || plan.threatened_city.is_some()
            || !plan.target_player.is_some_and(|target| {
                game.players.get(target).is_some_and(|rival| {
                    rival.alive
                        && !rival.is_minor
                        && !rival.is_barbarian
                        && self.campaign_target_legal(game, player, target)
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
            return 0.0;
        }
        let Some(center) = game.map.get(position) else {
            return 0.0;
        };
        if center.owner_city.is_some()
            || center.flooded
            || center.submerged
            || game.rules.is_water(center)
            || !game.rules.is_passable(center)
            || game.tile_is_natural_wonder(center)
        {
            return 0.0;
        }
        let resources: BTreeSet<Name> = game
            .wdisk(position, 1)
            .into_iter()
            .filter(|tile_position| game.players[player].explored.contains(tile_position))
            .filter_map(|tile_position| game.map.get(tile_position))
            .filter(|tile| {
                tile.owner_city.is_none()
                    && !tile.flooded
                    && !tile.submerged
                    && !tile.pillaged
                    && tile.district.is_none()
                    && tile.district_foundation.is_none()
                    && !game.rules.is_water(tile)
                    && game.rules.is_passable(tile)
                    && !game.tile_is_natural_wonder(tile)
            })
            .filter_map(|tile| tile.resource)
            .filter(|resource| {
                game.resource_visible_to(player, resource.as_str())
                    && game.rules.resources[*resource].class == "strategic"
            })
            .collect();
        if resources.is_empty() {
            return 0.0;
        }
        let mut cohorts: BTreeMap<Name, (usize, f64)> = BTreeMap::new();
        for unit in game
            .units
            .values()
            .filter(|unit| unit.owner == player && unit.hp > 0)
        {
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
            let successor = game.player_unit_replacement(player, successor);
            let target = &game.rules.units[successor];
            let Some(resource) = target
                .requires_resource
                .filter(|resource| resources.contains(resource))
            else {
                continue;
            };
            let gain = target.strength.max(target.ranged_attack_strength())
                - held.strength.max(held.ranged_attack_strength());
            if !target.buildable || gain < 15.0 {
                continue;
            }
            let Some(bill) = game
                .unit_upgrade_resource_price(player, unit.id, successor)
                .filter(|bill| bill.is_finite() && *bill > 0.0)
            else {
                continue;
            };
            let entry = cohorts.entry(resource).or_insert((0, bill));
            entry.0 += 1;
            entry.1 = entry.1.min(bill);
        }
        let needs_access = cohorts.into_iter().any(|(resource, (count, bill))| {
            count >= 3
                && game.strategic_stockpile(player, resource) < 2.0 * bill
                && game.strategic_resource_rate(player, resource.as_str()) <= 0.0
                && !game.player_city_ids(player).iter().any(|city| {
                    game.cities[city].owned_tiles.iter().any(|tile_position| {
                        game.map
                            .get(*tile_position)
                            .is_some_and(|tile| tile.resource == Some(resource))
                    })
                })
        });
        if !needs_access {
            return 0.0;
        }
        let visible = self.battlefront_visibility(game, player);
        if self.settlement_tile_risk(game, player, None, position, &visible) > 0.0
            || self
                .settle_site_frontier_loyalty_verdict(game, player, position)
                .is_some()
        {
            return 0.0;
        }
        12.0
    }

    pub(super) fn conquest_campaign_prepare_frontline(
        &self,
        game: &mut Game,
        player: usize,
        plan: &StrategicPlan,
    ) {
        if !self.victory_planning
            || self.active_victory_target(game) != Some(VictoryTarget::Domination)
            || plan.strategy != GrandStrategy::Expansion
            || plan.rush
            || self.war_plan.is_some()
            || game.player_city_ids(player).len() < 2
            || !game.players[player]
                .techs
                .contains(&crate::name!("writing"))
            || !plan.target_player.is_some_and(|target| {
                game.players.get(target).is_some_and(|rival| {
                    rival.alive
                        && !rival.is_minor
                        && !rival.is_barbarian
                        && self.campaign_target_legal(game, player, target)
                        && plan.target_city.is_some_and(|city_id| {
                            game.cities.get(&city_id).is_some_and(|city| {
                                city.owner == target
                                    && Self::city_within_declaration_range(game, player, city.pos)
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
            return;
        }
        let is_frontliner = |kind: Name| {
            let spec = &game.rules.units[kind];
            spec.class == "military"
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                && spec.ranged_strength == 0.0
                && spec.bombard_strength == 0.0
                && matches!(
                    spec.promotion_class.as_str(),
                    "melee" | "anti_cavalry" | "light_cavalry" | "heavy_cavalry"
                )
        };
        let frontliner_kinds: std::collections::HashSet<Name> = game
            .rules
            .units
            .keys()
            .copied()
            .filter(|kind| is_frontliner(*kind))
            .collect();
        let mut floor = 30.0 + (-game.players[player].gold_per_turn).max(0.0);
        if self.solvent_or_hold {
            floor = super::gold_and_cards::upgrade_floor_from(
                floor,
                game.unit_gold_maintenance(player),
            );
        }
        let mut income = game.players[player].gold_per_turn;
        for _attempt in 0..2 {
            let offers: Vec<_> = game
                .player_unit_ids(player)
                .into_iter()
                .filter_map(|unit_id| {
                    let unit = &game.units[&unit_id];
                    if unit.hp < 75 || !frontliner_kinds.contains(&unit.kind) {
                        return None;
                    }
                    let (target, gold, resources) =
                        game.unit_gold_upgrade_offer(player, unit_id)?;
                    if game.unit_upgrade_target(player, unit.kind) != Some(target)
                        || !frontliner_kinds.contains(&target)
                        || game.players[player].gold - gold < floor
                        || !self.air_resource_upgrade_preserves_wing(
                            game, player, unit_id, target, resources,
                        )
                    {
                        return None;
                    }
                    let strength = game.rules.units[target].strength;
                    let gain = strength - game.rules.units[unit.kind].strength;
                    let upkeep = Self::upgrade_upkeep_delta(game, unit_id, target);
                    (gain >= 15.0 && income - upkeep >= 2.0).then_some((
                        unit_id,
                        gain * unit.hp as f64 / 100.0 / gold.max(1.0),
                        strength,
                        upkeep,
                    ))
                })
                .collect();
            let strongest = offers.iter().map(|offer| offer.2).fold(0.0, f64::max);
            let contemporary = game
                .player_unit_ids(player)
                .into_iter()
                .filter(|unit_id| {
                    let unit = &game.units[unit_id];
                    unit.hp >= 75
                        && frontliner_kinds.contains(&unit.kind)
                        && game.unit_strength(unit, false) >= strongest
                })
                .count();
            if contemporary >= 2 {
                break;
            }
            let Some((unit_id, _, _, upkeep)) = offers.into_iter().max_by(|left, right| {
                left.1
                    .total_cmp(&right.1)
                    .then_with(|| right.0.cmp(&left.0))
            }) else {
                break;
            };
            if game
                .apply(player, &Action::UpgradeUnit { unit: unit_id })
                .is_err()
            {
                break;
            }
            income -= upkeep;
            think!(self.journal(), Military, Decision,
                "Preparing a contemporary conquest frontliner";
                "peaceful expansion toward the named frontier; current legal upgrade preserves {floor:.0} Gold and projected income {income:+.1}, at most two healthy contemporary takers");
        }
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
