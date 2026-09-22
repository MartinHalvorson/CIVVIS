use super::*;

/// The first wall breaker equips an otherwise incomplete campaign. Use the
/// opening conquest reservation scale; ordinary production-time pricing still
/// prefers a faster city. A slow queue may admit an earlier first delivery.
pub(super) const FIRST_WEAPON_RESERVATION: f64 = 400.0;

impl AdvancedAi {
    /// A distant completion date is not a delivered wall breaker. Permit a
    /// materially faster first weapon while the existing one is still queued.
    /// Compare every other queue, so a new fast order closes the opportunity
    /// for equivalent cities. A fielded weapon closes it altogether.
    pub(super) fn faster_first_siege(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        let Item::Unit { unit } = item else {
            return false;
        };
        let Some(spec) = g.rules.units.get(unit) else {
            return false;
        };
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !spec.siege
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
            || !plan
                .target_city
                .and_then(|id| g.cities.get(&id))
                .is_some_and(|city| {
                    Some(city.owner) == plan.target_player
                        && city.wall_hp > 0
                        && g.is_at_war(pid, city.owner)
                })
            || g.player_unit_ids(pid).iter().any(|uid| {
                let field = &g.rules.units[&g.units[uid].kind];
                field.siege && !matches!(field.domain.as_deref(), Some("sea" | "air"))
            })
        {
            return false;
        }
        let turns = |city, candidate: &Item| {
            g.host_production_turns(city, candidate)
                .filter(|value| value.is_finite() && *value > 0.0)
                .unwrap_or_else(|| self.production_build_turns(g, pid, city, candidate))
                .ceil()
                .max(1.0)
        };
        let target = g.cities[&plan.target_city.unwrap()].pos;
        let distance = g.wdist(g.cities[&cid].pos, target);
        let ours = turns(cid, item);
        let mut queued = false;
        for other in g.player_city_ids(pid) {
            if other == cid {
                continue;
            }
            let Some(candidate @ Item::Unit { unit }) = g.cities[&other].queue.first() else {
                continue;
            };
            let other_spec = &g.rules.units[unit];
            if !other_spec.siege || matches!(other_spec.domain.as_deref(), Some("sea" | "air")) {
                continue;
            }
            queued = true;
            // Do not replace a better weapon with an obsolete one, or pay
            // for duplicate equipment to gain only a turn or two.
            let theirs = turns(other, candidate);
            if distance > g.wdist(g.cities[&other].pos, target)
                || spec.ranged_attack_strength() < other_spec.ranged_attack_strength()
                || ours + 3.0 > theirs
                || ours * 2.0 > theirs
            {
                return false;
            }
        }
        queued
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

#[cfg(test)]
mod delivery_tests;
