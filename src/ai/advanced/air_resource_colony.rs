//! Acquire one guarded city-center resource for the committed Bomber wing.

use super::{AdvancedAi, StrategicPlan};
use crate::Pos;
use crate::game::{Action, Game, Item};
use crate::name::Name;
use crate::think;
use std::collections::BTreeSet;

impl AdvancedAi {
    /// An available deposit already held by us or a peaceful Suzerain is a
    /// connection job. Do not pay for a new city while that job is pending.
    fn air_resource_colony_connection_pending(g: &Game, pid: usize, resource: Name) -> bool {
        let improvement = Name::new(&g.rules.resources[resource].improvement);
        g.cities
            .values()
            .filter(|city| {
                city.owner == pid
                    || (g.suzerain_of(city.owner) == Some(pid) && !g.is_at_war(pid, city.owner))
            })
            .any(|city| {
                city.owned_tiles.iter().any(|site| {
                    let tile = &g.map.tiles[site];
                    tile.resource == Some(resource)
                        && g.players[pid].explored.contains(site)
                        && !tile.flooded
                        && !tile.submerged
                        && *site != city.pos
                        && (tile.pillaged || tile.improvement != Some(improvement))
                        && (tile.improvement == Some(improvement)
                            || g.valid_improvements(pid, *site).contains(&improvement))
                })
            })
    }

    fn air_resource_colony_sites(&self, g: &Game, pid: usize, uid: Option<u32>) -> BTreeSet<Pos> {
        if !self.air_surge_enabled()
            || self.active_victory_target(g) != Some(super::VictoryTarget::Domination)
            || self.war_plan.is_some()
        {
            return BTreeSet::new();
        }
        let Some(resource) = self.air_resource_shortfall(g, pid) else {
            return BTreeSet::new();
        };
        if Self::air_resource_colony_connection_pending(g, pid, resource) {
            return BTreeSet::new();
        }
        let known = g
            .map
            .tiles
            .iter()
            .filter_map(|(site, tile)| (tile.resource == Some(resource)).then_some(*site))
            .collect();
        self.air_resource_settlement_sites(g, pid, &known)
            .into_iter()
            .filter(|site| {
                // These conservative information checks also apply when the
                // optional generic Loyalty forecast is withheld.
                !Self::beyond_loyalty_reach(g, pid, *site)
                    && !Self::beside_unresolved_major_border(g, *site)
                    && !self.settler_capture_scars.contains_key(site)
                    && !self
                        .settler_threat_deferrals
                        .get(site)
                        .is_some_and(|until| *until > g.turn)
                    && !self.settler_targets.iter().any(|(other, target)| {
                        Some(*other) != uid
                            && g.wdist(*target, *site) < 4
                            && g.units
                                .get(other)
                                .is_some_and(|unit| unit.owner == pid && unit.kind == "settler")
                    })
            })
            .collect()
    }

    /// Claim at most one idle factory. The city-count/payback gates price an
    /// ordinary economic colony, while this one unlocks an already committed
    /// airfield. No queue replacement, purchase, or broader city target.
    pub(super) fn reserve_air_resource_colony(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.air_surge_enabled()
            || self.active_victory_target(g) != Some(super::VictoryTarget::Domination)
            || plan.threatened_city.is_some()
            || g.units
                .values()
                .any(|unit| unit.owner == pid && unit.kind == "settler")
            || g.cities
                .values()
                .filter(|city| city.owner == pid)
                .any(|city| {
                    city.queue
                        .iter()
                        .any(|item| matches!(item, Item::Unit { unit } if unit == "settler"))
                })
        {
            return false;
        }
        let sites = self.air_resource_colony_sites(g, pid, None);
        if sites.is_empty() {
            return false;
        }
        let item = Item::Unit {
            unit: crate::name!("settler"),
        };
        for (_, cid, site) in self.air_resource_colony_factories(g, pid, &sites, false) {
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: item.clone(),
                },
            )
            .is_ok()
            {
                think!(self.journal(), Expansion, Detail,
                       "{} trains a Settler for Bomber supply", g.cities[&cid].name;
                       "one defended resource colony can unlock the committed airfield \
                        after ordinary expansion has closed"; site);
                return true;
            }
        }
        false
    }

    /// The ordinary scorer gives a Settler a veto at the city target. Keep
    /// one already queued supply request through that review, while the
    /// governor's earlier siege and insolvency responses retain priority.
    pub(super) fn air_resource_colony_queue_committed(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        let item = Item::Unit {
            unit: crate::name!("settler"),
        };
        if g.cities[&cid].queue.first() != Some(&item)
            || plan.threatened_city.is_some()
            || g.units
                .values()
                .any(|unit| unit.owner == pid && unit.kind == "settler")
        {
            return false;
        }
        let sites = self.air_resource_colony_sites(g, pid, None);
        if sites.is_empty() {
            return false;
        }
        self.air_resource_colony_factories(g, pid, &sites, true)
            .first()
            .is_some_and(|(_, city, _)| *city == cid)
    }

    fn air_resource_colony_factories(
        &self,
        g: &Game,
        pid: usize,
        sites: &BTreeSet<Pos>,
        queued: bool,
    ) -> Vec<(f64, u32, Pos)> {
        let item = Item::Unit {
            unit: crate::name!("settler"),
        };
        let reserve = g.standard_duration(super::air_surge::AIR_SURGE_ENDGAME_RESERVE) as f64;
        let remaining = g.max_turns.saturating_sub(g.turn) as f64;
        let mut options = Vec::new();
        for city in g.player_city_ids(pid) {
            let source = &g.cities[&city];
            let legal = if queued {
                source.queue.first() == Some(&item)
                    && Self::production_commitment_is_legal(g, pid, city, &item)
            } else {
                source.queue.is_empty() && source.pop >= 2 && g.can_produce(pid, city, &item)
            };
            if !legal
                || (source.last_attacked > 0 && g.turn.saturating_sub(source.last_attacked) <= 4)
            {
                continue;
            }
            let turns = self.production_build_turns(g, pid, city, &item);
            if !turns.is_finite() || Self::settler_queue_loyalty_risk(g, city, turns).is_some() {
                continue;
            }
            let routes = self.future_settler_routes(g, pid, source.pos);
            for site in sites {
                if let Some(route) = routes.get(site) {
                    // Budget one hex per turn plus the existing air-surge
                    // reserve. Ordinary escort and safe-step movement still
                    // decide whether any actual step can be taken.
                    let arrival = turns + *route as f64;
                    if arrival + reserve < remaining {
                        options.push((arrival, city, *site));
                    }
                }
            }
        }
        options.sort_by(|a, b| a.0.total_cmp(&b.0).then((a.1, a.2).cmp(&(b.1, b.2))));
        options
    }

    /// Give only an untargeted, non-opening walker the resource appointment.
    /// Cached ordinary targets and all existing escort/refusal rules remain.
    pub(super) fn air_resource_colony_settler_target(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        avoid: Option<Pos>,
    ) -> Option<Pos> {
        let unit = g.units.get(&uid)?;
        if unit.owner != pid || unit.kind != "settler" || self.early_settler_home(uid).is_some() {
            return None;
        }
        if self
            .air_resource_colony_target
            .is_some_and(|(other, site)| {
                other != uid
                    && self.settler_targets.get(&other) == Some(&site)
                    && g.units
                        .get(&other)
                        .is_some_and(|unit| unit.owner == pid && unit.kind == "settler")
            })
        {
            return None;
        }
        let reserve = g.standard_duration(super::air_surge::AIR_SURGE_ENDGAME_RESERVE) as usize;
        self.air_resource_colony_sites(g, pid, Some(uid))
            .into_iter()
            .filter(|site| Some(*site) != avoid && !self.settler_site_is_dead(uid, *site))
            .filter_map(|site| {
                let route = g.route_distance(uid, site, 0)?;
                (route + reserve < g.max_turns.saturating_sub(g.turn) as usize)
                    .then_some((route, site))
            })
            .min()
            .map(|(_, site)| site)
    }

    pub(super) fn air_resource_colony_target_refused(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        site: Pos,
    ) -> bool {
        self.air_resource_colony_target == Some((uid, site))
            && self.air_surge_enabled()
            && self.active_victory_target(g) == Some(super::VictoryTarget::Domination)
            && !self
                .air_resource_colony_sites(g, pid, Some(uid))
                .contains(&site)
    }
}

#[cfg(test)]
mod tests;
