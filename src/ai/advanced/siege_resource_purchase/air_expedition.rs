//! Reconnaissance buys information before a remote air-supply colony is legal.
use super::*;
use crate::think;
use crate::Pos;

impl AdvancedAi {
    /// A known deposit whose nine-tile Loyalty neighborhood still needs eyes.
    /// Hidden resources, terrain and foreign cities do not select this lead.
    pub(in crate::ai::advanced) fn air_resource_frontier(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<Pos> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !self.base.explore_commit
            || self.base.minor
            || self.base.barb
            || g.is_arena()
        {
            return None;
        }
        let resource = self.air_resource_shortfall(g, pid)?;
        let known = &g.players[pid].explored;
        // An unconnected owned deposit is a Builder job. A healthy source
        // can still be insufficient for the wing, so only its backlog defers
        // the expedition, not ownership alone.
        if g.map.tiles.iter().any(|(pos, tile)| {
            known.contains(pos)
                && tile.resource == Some(resource)
                && !tile.flooded
                && !tile.submerged
                && tile
                    .owner_city
                    .and_then(|cid| g.cities.get(&cid))
                    .is_some_and(|c| {
                        c.owner == pid
                            && (tile.pillaged
                                || (*pos != c.pos
                                    && !tile.improvement.is_some_and(|improvement| {
                                        g.rules.improvements[improvement]
                                            .resources
                                            .contains(&resource)
                                            || g.rules.resources[resource].improvement
                                                == improvement
                                    })))
                    })
        }) {
            return None;
        }
        g.map
            .tiles
            .iter()
            .filter_map(|(pos, tile)| {
                if !known.contains(pos)
                    || tile.resource != Some(resource)
                    || tile.owner_city.is_some()
                    || tile.flooded
                    || tile.submerged
                    || !g.rules.is_passable(tile)
                    || g.rules.is_water(tile)
                    || !Self::beyond_loyalty_reach(g, pid, *pos)
                    || g.cities
                        .values()
                        .any(|city| city.owner == pid && g.wdist(city.pos, *pos) <= 3)
                {
                    return None;
                }
                let fog = g
                    .wdisk(*pos, 9)
                    .into_iter()
                    .filter(|p| !known.contains(p))
                    .count();
                let home = g
                    .cities
                    .values()
                    .filter(|city| city.owner == pid)
                    .map(|city| g.wdist(city.pos, *pos))
                    .min()?;
                Some((fog, std::cmp::Reverse(home), std::cmp::Reverse(*pos)))
            })
            .max()
            .map(|(_, _, pos)| pos.0)
    }

    pub(super) fn air_resource_expedition(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some(source) = self.air_resource_frontier(g, pid) else {
            return false;
        };
        let recon = |unit: Name| {
            g.rules
                .units
                .get(unit.as_str())
                .is_some_and(|s| s.promotion_class == "recon")
        };
        // One expedition, not a new empire-wide recon quota. Existing guards
        // retain their posts; this pass does not buy replacements for them.
        if g.units.values().any(|u| u.owner == pid && recon(u.kind))
            || g.cities.values().filter(|c| c.owner == pid).any(|c| {
                c.queue.iter().any(|item| {
                matches!(item, Item::Unit { unit } | Item::Formation { unit, .. } if recon(*unit))
            })
            })
            || self.live_war_economy_requires_recovery(g, pid, &self.counts(g, pid))
        {
            return false;
        }
        let floor = 40.0 + 6.0 * (-g.players[pid].gold_per_turn).max(0.0);
        let mut purchases = g
            .legal_purchase_actions(pid)
            .into_iter()
            .filter_map(|action| {
                let Action::Buy {
                    city,
                    unit,
                    formation: 0,
                    currency,
                } = &action
                else {
                    return None;
                };
                if currency != "gold" || !recon(*unit) {
                    return None;
                }
                let cost = g.unit_purchase_cost(pid, *city, unit.as_str(), currency)?;
                (g.players[pid].gold >= cost + floor
                    && self.faith_military_is_affordable(g, pid, unit.as_str()))
                .then_some((
                    g.wdist(g.cities[city].pos, source),
                    cost,
                    *city,
                    *unit,
                    action,
                ))
            })
            .collect::<Vec<_>>();
        purchases.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then_with(|| a.1.total_cmp(&b.1))
                .then_with(|| (a.2, a.3).cmp(&(b.2, b.3)))
        });
        for (_, cost, city, unit, action) in purchases {
            if g.apply(pid, &action).is_ok() {
                think!(self.journal(), Economy, Decision,
                    "{} buys {} to survey air supply", g.cities[&city].name, unit;
                    "the known air-supply deposit at {source:?} still has an uncharted Loyalty neighborhood; one scout costs {cost:.0} gold");
                return true;
            }
        }
        // No affordable legal purchase: reserve only an idle queue. Never
        // interrupt walls, recovery, or an existing construction project.
        let mut builds = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|city| {
                let c = &g.cities[&city];
                if !c.queue.is_empty()
                    || plan.threatened_city == Some(city)
                    || (c.last_attacked > 0 && g.turn.saturating_sub(c.last_attacked) <= 4)
                {
                    return None;
                }
                let unit = Name::new(&self.base.best_recon(g, pid, city)?);
                if !self.faith_military_is_affordable(g, pid, unit.as_str()) {
                    return None;
                }
                let item = Item::Unit { unit };
                let turns = g.item_remaining_cost_for_city(pid, city, &item)
                    / g.city_yields(city).production.max(1.0);
                Some((turns, city, item))
            })
            .collect::<Vec<_>>();
        builds.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        builds
            .into_iter()
            .any(|(_, city, item)| g.apply(pid, &Action::Produce { city, item }).is_ok())
    }
}
