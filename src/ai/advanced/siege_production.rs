use super::*;

/// The first wall breaker equips an otherwise incomplete campaign. Use the
/// opening conquest reservation scale; ordinary production-time pricing still
/// prefers a faster city and existing/queued weapons close this reservation.
pub(super) const FIRST_WEAPON_RESERVATION: f64 = 400.0;

impl AdvancedAi {
    /// Keep a nearly finished wall breaker through a delegated city review.
    /// In the live King assault on Methone, Guayaquil had 87/100 production
    /// in a Trebuchet, but the later baseline governor replaced it while
    /// Methone was still unwalled. It finished walls two turns later and no
    /// siege weapon reached them before peace was offered.
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

    pub(super) fn restore_invested_wall_breaker_queues(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        claimed: &[(u32, Item)],
        defense_claim: Option<&(u32, Item)>,
    ) {
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
                    "an invested wall breaker for the active Domination assault was displaced by a later production writer; its banked progress is retained");
            }
        }
    }

    /// The live Domination lane can delegate its city queues to the baseline
    /// governor, which never calls `production_value`. Claim one idle or
    /// unstarted queue before that handoff when a named walled campaign has
    /// no siege gun, or an active assault has only one gun on its ring.
    pub(super) fn reserve_active_siege_battery(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || plan.strategy != GrandStrategy::Conquest
            || self.war_plan.is_some()
            || self.live_war_economy_requires_recovery(g, pid, &self.counts(g, pid))
        {
            return;
        }
        let siege_count = self.counts(g, pid).siege;
        if siege_count > 1 {
            return;
        }
        let Some(target) = plan.target_city.and_then(|id| g.cities.get(&id)) else {
            return;
        };
        if Some(target.owner) != plan.target_player
            || target.wall_hp <= 0
            || (siege_count == 1
                && (!g.is_at_war(pid, target.owner)
                    || !Self::domination_siege_present(g, pid, target.id)))
        {
            return;
        }
        let mut best: Option<(f64, f64, u32, Item)> = None;
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            let committed = city.queue.first();
            let protected = committed.is_some_and(|item| {
                matches!(item, Item::Unit { unit } if unit.as_str() == "settler")
                    || matches!(item, Item::Unit { unit } if g.rules.units[unit].siege)
                    || Self::active_queue_is_defensive(g, item)
                    || g.item_invested_production(cid, item) > 0.0
            });
            if plan.threatened_city == Some(cid)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                || protected
            {
                continue;
            }
            let production = g.city_yields(cid).production.max(1.0);
            for item in g.producible_items(pid, cid) {
                let Item::Unit { unit } = &item else { continue };
                let spec = &g.rules.units[unit];
                if spec.class != "military"
                    || !spec.siege
                    || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                {
                    continue;
                }
                let turns = g.item_remaining_cost_for_city(pid, cid, &item) / production;
                let strength = spec.ranged_attack_strength();
                if best
                    .as_ref()
                    .is_none_or(|(old_turns, old_strength, old_cid, _)| {
                        old_strength
                            .total_cmp(&strength)
                            .then_with(|| turns.total_cmp(old_turns))
                            .then_with(|| cid.cmp(old_cid))
                            .is_lt()
                    })
                {
                    best = Some((turns, strength, cid, item));
                }
            }
        }
        let Some((_, _, city, item)) = best else {
            return;
        };
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Economy, Decision,
            "{} starts {} for the walled campaign", g.cities[&city].name, Self::plain_item(&item);
            "{}", if siege_count == 0 {
                "the delegated governor has no wall breaker; the named assault needs its first gun before the army can take a city"
            } else {
                "one siege gun is already on the target's ring; a second sustains fire while the first heals"
            });
        }
    }

    /// A roster full of field units can still lack the ability to break walls.
    /// Counts include queued units. A named, active assault on walls needs two
    /// weapons so a wounded first gun does not end the bombardment; an
    /// unwalled or future campaign still reserves only one. An active siege
    /// keeps its first order when the empire replans for expansion or recovery.
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
            || spec.class != "military"
            || !spec.siege
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
        {
            return false;
        }
        if let Some(target) = plan.target_city {
            // The board's declaration gate requires a siege weapon on a
            // walled staging ring. An unwalled target may complete walls while
            // the army marches or fights, so equip a named campaign early.
            let valid_target = g.cities.get(&target).is_some_and(|city| {
                Some(city.owner) == plan.target_player
                    && ((city.wall_hp > 0 && g.is_at_war(pid, city.owner))
                        || (self.war_policy_via_board
                            && matches!(
                                plan.strategy,
                                GrandStrategy::Conquest | GrandStrategy::Expansion
                            )
                            && self.campaign_target_legal(g, pid, city.owner)))
            });
            if !valid_target {
                return false;
            }
            let active_walls =
                g.cities[&target].wall_hp > 0 && g.is_at_war(pid, g.cities[&target].owner);
            // Two contemporaneous siege units can keep reducing a wall while
            // one rotates out to heal. Beyond that, require an actual weapon
            // upgrade: ten strength is roughly 50% more combat damage.
            return counts.siege
                < if active_walls && plan.strategy == GrandStrategy::Conquest {
                    2
                } else {
                    1
                }
                || spec.ranged_attack_strength() >= counts.land_siege_power + 10.0;
        }

        // Once any siege weapon is fielded, a targetless defensive replan
        // preserves only a genuinely stronger queued replacement.
        if counts.siege != 0 && spec.ranged_attack_strength() < counts.land_siege_power + 10.0 {
            return false;
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
