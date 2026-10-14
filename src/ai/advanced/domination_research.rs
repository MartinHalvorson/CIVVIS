//! Research the missing wall-breaking capability before an expedition stalls.
//!
//! A Conquest target with observed walls is a concrete technology debt. This
//! opt-in unlocks the cheapest missing land siege technology only while the
//! empire has neither a siege unit committed nor an unlocked siege design.
//! Production, strategic materials and later modernization remain separate
//! decisions. Existing defensive and appointed breakthrough goals run first.

use super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};

/// `domination-siege-research-2`: the target's wall pool, current or full,
/// from which a stronger buildable land siege design is worth researching
/// although one is already fielded. 300 is the Medieval tier. Live King
/// 2026-10-03T072557Z: the Cree held 400-HP walls from t123, our siege train
/// stayed on Trebuchets (Bombard 45) until a single Bombard at t142, and the
/// siege of Mistahi-Sipihk sat in Stage from t108 to t154 at "7.9 turns to
/// breach vs 5.2 endurance"; Metal Casting came at t142.
const SIEGE_UPGRADE_WALL_HP: i32 = 300;
use crate::game::{Game, Item};
use crate::name::Name;

impl AdvancedAi {
    /// An expanding conquest economy needs campuses before optional branches
    /// consume its early research. Military commitments and urgent defenses
    /// retain their earlier slots in the research chooser.
    pub(super) fn domination_campus_unlock_goal(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<&'static str> {
        (self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.player_city_ids(pid).len() >= 2
            && !g.players[pid].techs.contains(&crate::name!("writing")))
        .then_some("writing")
    }

    pub(super) fn domination_siege_research_goal(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<Name> {
        if !(self.domination_siege_research || self.domination_siege_research_2)
            || plan.strategy != GrandStrategy::Conquest
        {
            return None;
        }
        let city = g.cities.get(&plan.target_city?)?;
        if Some(city.owner) != plan.target_player
            || city.owner == pid
            || g.same_team(pid, city.owner)
            || !self.campaign_target_legal(g, pid, city.owner)
        {
            return None;
        }
        let walls = if self.battlefront_observation {
            let report = self.remembered_city(city.id)?;
            if report.owner != city.owner {
                return None;
            }
            report.wall_hp
        } else {
            city.wall_hp
        };
        if walls <= 0 {
            return None;
        }
        let land_siege = |kind: Name| {
            let spec = &g.rules.units[kind];
            spec.class == "military"
                && spec.siege
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        };
        let fielded = g
            .units
            .values()
            .any(|unit| unit.owner == pid && land_siege(unit.kind))
            || g.cities
                .values()
                .filter(|city| city.owner == pid)
                .any(|city| {
                    city.queue
                        .iter()
                        .any(|item| matches!(item, Item::Unit { unit } if land_siege(*unit)))
                });
        if fielded && !self.domination_siege_research_2 {
            return None;
        }
        let designs: Vec<_> = g
            .rules
            .units
            .iter()
            .filter(|(kind, spec)| {
                land_siege(**kind)
                    && spec.buildable
                    && g.player_unit_replacement(pid, **kind) == **kind
                    && spec
                        .unique_to
                        .as_deref()
                        .is_none_or(|civ| civ == g.players[pid].civ)
                    && spec
                        .obsolete_tech
                        .is_none_or(|tech| !g.players[pid].techs.contains(&tech))
            })
            .collect();
        // If the design is already unlocked, another technology does not fix
        // a production or strategic-resource shortage. Give those systems time
        // to field it instead of escalating the research goal every turn.
        let unlocked = |spec: &crate::rules::UnitSpec| {
            spec.tech
                .is_none_or(|tech| g.players[pid].techs.contains(&tech))
        };
        let cheapest = |techs: Vec<Name>| {
            techs.into_iter().min_by(|left, right| {
                Self::war_remaining_research_cost(g, pid, *left)
                    .total_cmp(&Self::war_remaining_research_cost(g, pid, *right))
                    .then_with(|| left.cmp(right))
            })
        };
        if !fielded && !designs.iter().any(|(_, spec)| unlocked(spec)) {
            return cheapest(designs.iter().filter_map(|(_, spec)| spec.tech).collect());
        }
        if !self.domination_siege_research_2
            || walls.max(g.city_max_wall_hp(city)) < SIEGE_UPGRADE_WALL_HP
        {
            return None;
        }
        // `domination-siege-research-2`: walls of the Medieval tier or better
        // against our best buildable gun. The next design only helps if the
        // empire can feed it, so one whose strategic resource we neither
        // stock nor own a deposit of (Artillery's Oil in live King games 23
        // and 26) is not a goal.
        let best = designs
            .iter()
            .filter(|(_, spec)| unlocked(spec))
            .map(|(_, spec)| spec.ranged_attack_strength())
            .fold(0.0_f64, f64::max);
        let fed = |spec: &crate::rules::UnitSpec| {
            spec.requires_resource.is_none_or(|resource| {
                g.strategic_stockpile(pid, resource) > 0.0
                    || g.player_city_ids(pid).into_iter().any(|cid| {
                        g.cities[&cid].owned_tiles.iter().any(|pos| {
                            g.map
                                .tiles
                                .get(pos)
                                .is_some_and(|tile| tile.resource == Some(resource))
                        })
                    })
            })
        };
        cheapest(
            designs
                .iter()
                .filter(|(_, spec)| {
                    !unlocked(spec) && spec.ranged_attack_strength() > best && fed(spec)
                })
                .filter_map(|(_, spec)| spec.tech)
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod campus_unlock_tests;
