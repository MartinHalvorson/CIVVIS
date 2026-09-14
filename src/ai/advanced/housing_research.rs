//! `housing-research-2`: buy a usable housing unlock for a capped empire.
//!
//! V1 considers only two district names and their printed technology prices.
//! That misses Pottery's Granary and Sanitation's Sewer, and pursues Engineering
//! even when none of the capped cities has an Aqueduct site. V2 keeps the same
//! empire-wide trigger and research priority, but checks the construction the
//! missing research would actually enable. Known housing is production's job.

use super::{AdvancedAi, HOUSING_CARD_HEADROOM};
use crate::game::{Game, Item};
use crate::name::Name;
use std::collections::BTreeSet;

#[derive(Clone, Copy)]
enum HousingUnlock {
    Building(Name),
    District(Name),
}

impl HousingUnlock {
    fn tech(self, g: &Game) -> Option<Name> {
        match self {
            Self::Building(name) => g.rules.buildings[name].tech,
            Self::District(name) => g.rules.districts[name].tech,
        }
    }

    /// Construction legality belongs to the engine: this includes the civ's
    /// replacement, prerequisites, district capacity, placement and host vetoes.
    fn usable_gain(self, g: &Game, pid: usize, cid: u32) -> f64 {
        match self {
            Self::Building(building) => {
                if g.can_produce(pid, cid, &Item::Building { building }) {
                    g.rules.buildings[building].housing
                } else {
                    0.0
                }
            }
            Self::District(district) => g
                .district_sites(cid, district)
                .into_iter()
                .filter(|pos| {
                    g.can_produce(
                        pid,
                        cid,
                        &Item::District {
                            district,
                            pos: *pos,
                        },
                    )
                })
                .map(|pos| {
                    let water = if g.district_family(district) == "aqueduct" {
                        g.aqueduct_housing_gain(&g.cities[&cid])
                    } else {
                        0.0
                    };
                    g.district_housing(district.as_str(), pos) + water
                })
                .fold(0.0, f64::max),
        }
    }
}

impl AdvancedAi {
    fn housing_research_path_cost(g: &Game, pid: usize, missing: &BTreeSet<Name>) -> f64 {
        let mut cost = 0.0;
        for tech in missing {
            let active = g.players[pid].research.as_deref() == Some(tech.as_str());
            // A boost on active research is already in research_progress.
            let boost = if !active && Self::boost_in_hand(g, pid, tech.as_str(), true) {
                let china = if g.has_ability(pid, "dynastic_cycle") {
                    0.1
                } else {
                    0.0
                };
                (Self::boost_frac(g, tech.as_str(), true) + china).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let progress = if active {
                g.players[pid].research_progress
            } else {
                0.0
            };
            cost += (g.tech_cost(tech.as_str()) * (1.0 - boost) - progress).max(0.0);
        }
        // Overflow is a shared research bank, not a discount for every node.
        (cost - g.players[pid].research_overflow).max(0.0)
    }

    pub(super) fn usable_housing_tech(&self, g: &Game, pid: usize) -> Option<Name> {
        if !self.housing_research_2 {
            return None;
        }
        let cities = g.player_city_ids(pid);
        let capped: Vec<u32> = cities
            .iter()
            .copied()
            .filter(|cid| g.city_housing_headroom(&g.cities[cid]) < HOUSING_CARD_HEADROOM)
            .collect();
        if capped.len() < 2 || capped.len() * 2 < cities.len() {
            return None;
        }
        let unlocks: Vec<HousingUnlock> = g
            .rules
            .buildings
            .iter()
            .filter(|(_, spec)| !spec.wonder && spec.housing > 0.0)
            .map(|(name, _)| HousingUnlock::Building(*name))
            .chain(g.rules.districts.iter().filter_map(|(name, spec)| {
                (spec.housing > 0.0
                    || spec
                        .effects
                        .get("appeal_housing_max")
                        .copied()
                        .unwrap_or(0.0)
                        > 0.0
                    || g.district_family(*name) == "aqueduct")
                    .then_some(HousingUnlock::District(*name))
            }))
            .collect();
        let blocked: Vec<u32> = capped
            .into_iter()
            .filter(|cid| {
                !unlocks
                    .iter()
                    .any(|item| item.usable_gain(g, pid, *cid) > 0.0)
            })
            .collect();
        if blocked.is_empty() {
            return None;
        }
        let goals: BTreeSet<Name> = unlocks
            .iter()
            .filter_map(|item| item.tech(g))
            .filter(|tech| !g.players[pid].techs.contains(tech))
            .collect();
        if goals.is_empty() {
            return None;
        }

        // One private counterfactual, only after the empire and production
        // gates bind. Restore the known technologies before each goal so a
        // previously considered unlock cannot make the next one look legal.
        let mut future = g.clone();
        let mut best: Option<(f64, Name)> = None;
        for goal in goals {
            let Some(path) = g.rules.tech_ancestors.get(goal.as_str()) else {
                continue;
            };
            future.players[pid].techs = g.players[pid].techs.clone();
            let missing: BTreeSet<Name> = path
                .iter()
                .map(|tech| Name::new(tech))
                .chain(std::iter::once(goal))
                .filter(|tech| !g.players[pid].techs.contains(tech))
                .collect();
            future.players[pid].techs.extend(missing.iter().copied());
            let cost = Self::housing_research_path_cost(g, pid, &missing);
            let benefit: f64 = blocked
                .iter()
                .map(|cid| {
                    let gain = unlocks
                        .iter()
                        .filter(|item| item.tech(g) == Some(goal))
                        .map(|item| item.usable_gain(&future, pid, *cid))
                        .fold(0.0, f64::max);
                    gain.min(HOUSING_CARD_HEADROOM - g.city_housing_headroom(&g.cities[cid]))
                })
                .sum();
            if benefit <= 0.0 {
                continue;
            }
            let price = cost / benefit;
            if best.is_none_or(|(old_price, old_goal)| {
                price
                    .total_cmp(&old_price)
                    .then(goal.cmp(&old_goal))
                    .is_lt()
            }) {
                best = Some((price, goal));
            }
        }
        best.map(|(_, goal)| goal)
    }
}

#[cfg(test)]
mod tests;
