//! Native Culture armies need upgrade material during a major war too.
//! 20261108T070117Z t197: four Bombards quoted Artillery at 155 Gold and
//! one Oil each, no Oil income/stock, a peaceful rival offering 70 Oil.
//! Preserve the quoted upgrade cash and buy a bounded unit-fuel buffer through
//! the existing market transport. Only native observations enter this path.
use super::gun_resource::{GUN_RESOURCE_FUEL_LOW_TURNS, GUN_RESOURCE_FUEL_TURNS};
use super::*;

#[derive(Default)]
struct Material {
    upfront: f64,
    upkeep: f64,
    gold: f64,
    gain: f64,
    unit: Option<Name>,
    uid: u32,
}

type Reading = (Vec<ArmyUpgradeMaterialWant>, BTreeMap<Name, f64>);

impl AdvancedAi {
    pub(super) fn culture_war_material_reading(&self, g: &Game, pid: usize) -> Option<Reading> {
        // An absent income observation is not a zero-income offer. This also
        // leaves ordinary simulator decisions on their existing path.
        let income = g.observed_strategic_income_adjustments.get(&pid)?;
        let player = g.players.get(pid)?;
        if !self.army_upgrade_materials
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || !player.gold.is_finite()
            || !player.gold_per_turn.is_finite()
            || !g.players.iter().any(|other| {
                other.alive && !other.is_minor && !other.is_barbarian && g.is_at_war(pid, other.id)
            })
        {
            return None;
        }
        let floor = 100.0 + (-player.gold_per_turn).max(0.0);
        let capacity = g.strategic_stockpile_capacity(pid);
        let mut materials: BTreeMap<Name, Material> = BTreeMap::new();
        let mut upgrades = Vec::new();
        for uid in g.player_unit_ids(pid) {
            let unit = &g.units[&uid];
            let Some(facts) = g
                .host_unit_facts
                .get(&uid)
                .filter(|facts| facts.civ6_id.is_some())
            else {
                continue;
            };
            let from = &g.rules.units[unit.kind];
            if let Some(resource) = from.requires_resource.filter(|resource| {
                income.contains_key(resource) && player.strategic_resources.contains_key(resource)
            }) {
                if !unit.free_upkeep && from.resource_maintenance > 0.0 {
                    let material = materials.entry(resource).or_default();
                    material.upkeep += from.resource_maintenance;
                    material.unit.get_or_insert(unit.kind);
                    material.uid = uid;
                }
            }
            if from.class != "military"
                || from.promotion_class == "recon"
                || matches!(from.domain.as_deref(), Some("sea" | "air"))
                || unit.linked_to.is_some()
                || self.amphibious_locked(uid)
                || self.guard_is_reserved_for_civilian(uid)
                || f64::from(unit.hp) <= self.base.w.withdraw_hp + 10.0
            {
                continue;
            }
            let Some(quote) = facts.upgrade.as_ref() else {
                continue;
            };
            let (Some(target), Some(gold), Some(bill)) =
                (quote.to, quote.cost, quote.resources.as_ref())
            else {
                continue;
            };
            let to = &g.rules.units[target];
            let Some(resource) = bill.resource else {
                continue;
            };
            let gain = to.strength.max(to.ranged_attack_strength())
                - from.strength.max(from.ranged_attack_strength());
            if to.class != "military"
                || matches!(to.domain.as_deref(), Some("sea" | "air"))
                || to.requires_resource != Some(resource)
                || !income.contains_key(&resource)
                || !player.strategic_resources.contains_key(&resource)
                || !gold.is_finite()
                || gold < 0.0
                || !bill.cost.is_finite()
                || bill.cost <= 0.0
                || !gain.is_finite()
                || gain < 15.0
            {
                continue;
            }
            let previous = if from.requires_resource == Some(resource) {
                from.resource_maintenance
            } else {
                0.0
            };
            let fuel = (to.resource_maintenance - previous).max(0.0);
            upgrades.push((gain, uid, target, resource, gold, bill.cost, fuel));
        }
        upgrades.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        for (gain, uid, target, resource, gold, bill, fuel) in upgrades {
            let material = materials.entry(resource).or_default();
            let rate = g.strategic_resource_rate(pid, resource.as_str());
            let required = material.upfront
                + bill
                + (material.upkeep + fuel - rate).max(0.0) * GUN_RESOURCE_FUEL_TURNS;
            if material.gold + gold + floor > player.gold || required > capacity {
                continue;
            }
            material.upfront += bill;
            material.upkeep += fuel;
            material.gold += gold;
            if material.unit.is_none() || gain > material.gain {
                material.unit = Some(target);
                material.uid = uid;
                material.gain = gain;
            }
        }
        let mut wants = Vec::new();
        let mut reserves = BTreeMap::new();
        for (resource, material) in materials {
            let stock = g.strategic_stockpile(pid, resource);
            let rate = g.strategic_resource_rate(pid, resource.as_str());
            if !stock.is_finite() || !rate.is_finite() || stock < 0.0 || rate < 0.0 {
                continue;
            }
            let burn = (material.upkeep - rate).max(0.0);
            let low = material.upfront + burn * GUN_RESOURCE_FUEL_LOW_TURNS;
            reserves.insert(resource, low);
            let amount = (material.upfront + burn * GUN_RESOURCE_FUEL_TURNS - stock).ceil();
            if stock + rate >= low
                || amount < 1.0
                || stock + amount > capacity
                || amount > f64::from(u32::MAX)
                || player.gold < material.gold + floor
            {
                continue;
            }
            let Some(unit) = material.unit else { continue };
            wants.push(ArmyUpgradeMaterialWant {
                resource: format!("RESOURCE_{}", resource.as_str().to_ascii_uppercase()),
                resource_id: resource.as_str().to_string(),
                amount: amount as u32,
                unit: unit.as_str().to_string(),
                uid: material.uid,
                gold_reserve: material.gold + floor,
                gain: material.gain,
            });
        }
        wants.sort_by(|a, b| {
            b.gain
                .total_cmp(&a.gain)
                .then_with(|| a.resource.cmp(&b.resource))
        });
        wants.truncate(1);
        Some((wants, reserves))
    }
}

#[cfg(test)]
mod tests;
