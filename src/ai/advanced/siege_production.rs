use super::*;

/// The first wall breaker equips an otherwise incomplete campaign. Use the
/// opening conquest reservation scale; ordinary production-time pricing still
/// prefers a faster city and existing/queued weapons close this reservation.
pub(super) const FIRST_WEAPON_RESERVATION: f64 = 400.0;

impl AdvancedAi {
    /// Remember wall breakers whose production would otherwise be lost when a
    /// later city governor writes over the queue. The target can complete
    /// walls while the gun is under construction, so an active assault keeps
    /// the commitment even before its first wall is observed.
    pub(super) fn invested_wall_breaker_queues(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Vec<(u32, Item)> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || plan.strategy != GrandStrategy::Conquest
            || !plan
                .target_city
                .and_then(|id| g.cities.get(&id))
                .is_some_and(|target| {
                    Some(target.owner) == plan.target_player && g.is_at_war(pid, target.owner)
                })
        {
            return Vec::new();
        }
        g.player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let item = g.cities[&cid].queue.first()?;
                let Item::Unit { unit } = item else {
                    return None;
                };
                let spec = g.rules.units.get(unit)?;
                (spec.siege
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && g.item_invested_production(cid, item) > 0.0)
                    .then(|| (cid, item.clone()))
            })
            .collect()
    }

    /// Reclaim a previously chosen wall breaker after all routine queue
    /// writers. A confirmed defender, recent attack, or another siege weapon
    /// remains in charge; a completed peace ends this claim.
    pub(super) fn restore_wall_breaker_queues(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        claimed: &[(u32, Item)],
        defense_claim: Option<&(u32, Item)>,
    ) {
        if !plan
            .target_city
            .and_then(|id| g.cities.get(&id))
            .is_some_and(|target| {
                Some(target.owner) == plan.target_player && g.is_at_war(pid, target.owner)
            })
        {
            return;
        }
        for (cid, item) in claimed {
            let Some(city) = g.cities.get(cid) else {
                continue;
            };
            if city.owner != pid
                || plan.threatened_city == Some(*cid)
                || defense_claim.is_some_and(|(protected, _)| protected == cid)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                || city.queue.first().is_some_and(|current| {
                    matches!(current, Item::Unit { unit }
                        if g.rules.units.get(unit).is_some_and(|spec| spec.siege))
                })
                || !g.can_produce(pid, *cid, item)
            {
                continue;
            }
            let city_name = city.name.clone();
            if g.apply(
                pid,
                &Action::Produce {
                    city: *cid,
                    item: item.clone(),
                },
            )
            .is_ok()
                && self.journal().wants(crate::reasoning::Level::Decision)
            {
                think!(self.journal(), Economy, Decision,
                    "{} resumes {}", city_name, Self::plain_item(item);
                    "the active Domination assault's wall breaker was displaced by a later production writer");
            }
        }
    }

    /// The delegated city governor does not call `production_value`, where the
    /// ordinary missing-siege reservation lives. Give a walled Domination
    /// assault one real bombardment unit before delegation fills every idle
    /// queue. An enemy that already knows Masonry can raise walls during the
    /// first few war turns, so a mostly healthy open objective also earns its
    /// first gun while the army marches. A fielded or queued land gun normally
    /// closes this reservation;
    /// a failed positive damage budget can reserve up to three in total. If
    /// only a new, low-production city is idle, a much faster city may give up
    /// an uninvested routine queue instead: a 140-turn gun cannot reinforce a
    /// siege that needs relief now. Likewise, a slow queued gun does not close
    /// the reservation if a second city can deliver one much sooner.
    pub(super) fn reserve_delegated_domination_siege(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Item)> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination) {
            return None;
        }
        let Some(target) = plan
            .target_city
            .and_then(|cid| g.cities.get(&cid))
            .filter(|city| {
                city.owner != pid
                    && !g.players[city.owner].is_minor
                    && g.is_at_war(pid, city.owner)
                    && (city.wall_hp > 0
                        || (city.hp >= 160
                            && g.players[city.owner]
                                .techs
                                .contains(&crate::name!("masonry"))))
            })
        else {
            return None;
        };
        let objective = target.pos;
        let target_name = target.name.clone();
        let counts = self.counts(g, pid);
        let nearby_force: Vec<_> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|id| g.wdist(g.units[id].pos, objective) <= 5)
            .collect();
        let nearby_taker = nearby_force.iter().any(|id| {
            let spec = &g.rules.units[g.units[id].kind];
            spec.is_melee_capable() && spec.ranged_strength == 0.0 && spec.bombard_strength == 0.0
        });
        let breach_shortfall = self.siege_positive_damage_budget
            && counts.siege < 3
            && nearby_taker
            && self
                .conversion_siege_budget(g, pid, target.id, &nearby_force)
                .is_some_and(|(finish, endurance)| finish > endurance * 0.8);
        if self.live_war_economy_requires_recovery(g, pid, &counts) {
            return None;
        }
        let queued_arrival = if counts.land_siege_power > 0.0
            && !breach_shortfall
            && counts.siege == 1
            && !g.player_unit_ids(pid).into_iter().any(|id| {
                let spec = &g.rules.units[g.units[&id].kind];
                spec.siege && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            }) {
            g.player_city_ids(pid)
                .into_iter()
                .filter_map(|cid| {
                    let item = g.cities[&cid].queue.first()?;
                    let Item::Unit { unit } = item else {
                        return None;
                    };
                    let spec = g.rules.units.get(unit)?;
                    (spec.siege && !matches!(spec.domain.as_deref(), Some("sea" | "air"))).then(
                        || {
                            self.production_build_turns(g, pid, cid, item)
                                + f64::from(g.wdist(g.cities[&cid].pos, objective))
                                    / spec.moves.max(1.0)
                        },
                    )
                })
                .min_by(f64::total_cmp)
        } else {
            None
        };
        if counts.land_siege_power > 0.0 && !breach_shortfall && queued_arrival.is_none() {
            return None;
        }

        let best = {
            let _memo = g.query_memo();
            let mut best_idle: Option<(f64, u32, Name)> = None;
            let mut best_fresh: Option<(f64, u32, Name)> = None;
            for cid in g.player_city_ids(pid) {
                if plan.threatened_city == Some(cid) {
                    continue;
                }
                let city = &g.cities[&cid];
                let fresh_routine = match city.queue.as_slice() {
                    [] => false,
                    [queued]
                        if g.item_invested_production(cid, queued) <= f64::EPSILON
                            && !(city.last_attacked > 0
                                && g.turn.saturating_sub(city.last_attacked) <= 4) =>
                    {
                        match queued {
                            Item::Unit { unit } => matches!(unit.as_str(), "builder" | "trader"),
                            Item::Building { building } => !matches!(
                                building.as_str(),
                                "walls" | "medieval_walls" | "renaissance_walls"
                            ),
                            _ => false,
                        }
                    }
                    _ => continue,
                };
                if !city.queue.is_empty() && !fresh_routine {
                    continue;
                }
                for item in g.producible_items(pid, cid) {
                    let Item::Unit { unit } = item else { continue };
                    let spec = &g.rules.units[&unit];
                    if spec.class != "military"
                        || !spec.siege
                        || !spec.has_ranged_attack()
                        || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    {
                        continue;
                    }
                    let arrival = self.production_build_turns(g, pid, cid, &item)
                        + f64::from(g.wdist(g.cities[&cid].pos, objective)) / spec.moves.max(1.0);
                    let best = if fresh_routine {
                        &mut best_fresh
                    } else {
                        &mut best_idle
                    };
                    if best.as_ref().is_none_or(|(old, old_city, old_unit)| {
                        arrival.total_cmp(old).is_lt()
                            || (arrival == *old && (cid, unit) < (*old_city, *old_unit))
                    }) {
                        *best = Some((arrival, cid, unit));
                    }
                }
            }
            match (best_idle, best_fresh) {
                (Some(idle), Some(fresh))
                    if fresh.0 <= 30.0 && idle.0 >= fresh.0 + 8.0 && idle.0 >= fresh.0 * 1.5 =>
                {
                    Some((fresh.0, fresh.1, fresh.2, Some(idle.0)))
                }
                (None, Some(fresh)) if fresh.0 <= 30.0 => Some((fresh.0, fresh.1, fresh.2, None)),
                (Some(idle), _) => Some((idle.0, idle.1, idle.2, None)),
                _ => None,
            }
        };
        let Some((arrival, city, unit, slow_idle)) = best else {
            return None;
        };
        let faster_than_queued = queued_arrival.is_some_and(|queued| {
            arrival <= 20.0 && queued >= arrival + 8.0 && queued >= arrival * 1.5
        });
        if counts.land_siege_power > 0.0 && !breach_shortfall && !faster_than_queued {
            return None;
        }
        let displaced = g.cities[&city].queue.first().cloned();
        let item = Item::Unit { unit };
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_err()
        {
            return None;
        }
        if let Some(old) = displaced {
            let alternative = slow_idle.map_or_else(
                || "no idle city could build the gun in time".to_string(),
                |turns| format!("the fastest idle city needed about {turns:.0} turns"),
            );
            think!(self.journal(), Military, Detail,
                "{} gives its fresh {} queue to the siege gun", g.cities[&city].name, Self::plain_item(&old);
                "no production was invested in it; the gun arrives in about {arrival:.0} turns, while {alternative}";
                objective);
        }
        think!(self.journal(), Military, Decision,
            "{} reserves a {} for the walled assault", g.cities[&city].name, unit;
            "{}; expected arrival at {} in about {arrival:.0} turns",
            if breach_shortfall { "the staged force cannot breach before its health runs out" }
            else if faster_than_queued { "the first queued gun would reach the front too late" }
            else { "the delegated governor has no land siege weapon" },
            target_name;
            objective);
        Some((city, item))
    }

    /// A roster full of field units can still lack the ability to break walls.
    /// Counts include queued units, so only the first siege order gets this
    /// composition exception to the ordinary army ceiling. An active siege
    /// keeps this requirement when the empire replans for expansion or recovery.
    pub(super) fn missing_domination_siege(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
        spec: &crate::rules::UnitSpec,
    ) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !spec.siege
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
        {
            return false;
        }
        // A full roster of obsolete Catapults is not a modern siege train.
        // Ten strength is a meaningful step (roughly 50% more damage under
        // the combat curve). Field formations and queued replacements count;
        // the governor's queue-excluded census keeps a reservation from
        // cancelling itself. This still opens only one missing weapon slot.
        if counts.siege != 0 && spec.ranged_attack_strength() < counts.land_siege_power + 10.0 {
            return false;
        }
        if let Some(target) = plan.target_city {
            return g.cities.get(&target).is_some_and(|city| {
                Some(city.owner) == plan.target_player
                    && city.wall_hp > 0
                    && g.is_at_war(pid, city.owner)
            });
        }

        // A defensive replan can select a different enemy whose cities have
        // not been revealed yet. That does not make the known hostile walls
        // disappear. Preserve the first weapon already ordered for them;
        // without a current target, an empty city receives no new reservation.
        // The governor excludes this city's queue from `counts` when valuing
        // its commitment; a fielded or separately queued weapon still closes
        // the requirement. Emergency defense remains upstream of rescoring.
        let queued_weapon = g.cities.get(&cid).is_some_and(|city| {
            city.owner == pid
                && city.queue.first().is_some_and(|item| match item {
                    Item::Unit { unit } => g.rules.units.get(unit).is_some_and(|queued| {
                        queued.siege && !matches!(queued.domain.as_deref(), Some("sea" | "air"))
                    }),
                    _ => false,
                })
        });
        queued_weapon
            && g.cities
                .values()
                .any(|city| city.wall_hp > 0 && g.is_at_war(pid, city.owner))
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod war_strategy_tests;

#[cfg(test)]
mod modernization_tests;
