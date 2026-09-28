use super::*;

/// The first wall breaker equips an otherwise incomplete campaign. Use the
/// opening conquest reservation scale; ordinary production-time pricing still
/// prefers a faster city and existing/queued weapons close this reservation.
pub(super) const FIRST_WEAPON_RESERVATION: f64 = 400.0;

impl AdvancedAi {
    /// The delegated city governor does not call `production_value`, where the
    /// ordinary missing-siege reservation lives. Give a walled Domination
    /// assault one real bombardment unit before delegation fills every idle
    /// queue. A fielded or queued land gun closes this reservation.
    pub(super) fn reserve_delegated_domination_siege(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination) {
            return false;
        }
        let Some(target) = plan
            .target_city
            .and_then(|cid| g.cities.get(&cid))
            .filter(|city| {
                city.wall_hp > 0
                    && city.owner != pid
                    && !g.players[city.owner].is_minor
                    && g.is_at_war(pid, city.owner)
            })
        else {
            return false;
        };
        let objective = target.pos;
        let target_name = target.name.clone();
        let counts = self.counts(g, pid);
        if counts.land_siege_power > 0.0 || self.live_war_economy_requires_recovery(g, pid, &counts)
        {
            return false;
        }

        let best = {
            let _memo = g.query_memo();
            let mut best: Option<(f64, u32, Name)> = None;
            for cid in g.player_city_ids(pid) {
                if !g.cities[&cid].queue.is_empty() || plan.threatened_city == Some(cid) {
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
                    if best.as_ref().is_none_or(|(old, old_city, old_unit)| {
                        arrival.total_cmp(old).is_lt()
                            || (arrival == *old && (cid, unit) < (*old_city, *old_unit))
                    }) {
                        best = Some((arrival, cid, unit));
                    }
                }
            }
            best
        };
        let Some((arrival, city, unit)) = best else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: Item::Unit { unit },
            },
        )
        .is_err()
        {
            return false;
        }
        think!(self.journal(), Military, Decision,
            "{} reserves a {} for the walled assault", g.cities[&city].name, unit;
            "the delegated governor has no land siege weapon; expected arrival at {} in about {arrival:.0} turns",
            target_name;
            objective);
        true
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
