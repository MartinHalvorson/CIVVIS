//! Upfront material for one affordable land-army upgrade.
//!
//! Native Emperor `civvis-20261014T230322Z` fielded ten Heavy Chariots at
//! turn 89. Their host quote named a Knight, 160 Gold and ten Iron; no
//! Iron stock or income existed, while two peaceful rivals offered 25.
//! The siege/air resource buyer supplied no ordinary land-upgrade want.
use super::*;

/// One material purchase, with the cash that must remain for its upgrade.
#[derive(Clone, Debug, PartialEq)]
pub struct ArmyUpgradeMaterialWant {
    pub resource: String,
    pub resource_id: String,
    pub amount: u32,
    pub unit: String,
    pub uid: u32,
    pub gold_reserve: f64,
    pub gain: f64,
}

struct ArmyUpgradeMaterialQuote {
    resource: Name,
    bill: f64,
    target: Name,
    uid: u32,
    gold_reserve: f64,
    gain: f64,
}

impl AdvancedAi {
    pub fn army_upgrade_materials_enabled(&self) -> bool {
        self.army_upgrade_materials
    }

    /// One useful upgrade, not an unfunded bill for the whole obsolete army.
    /// Material is preparation; host refusals and the existing upgrade pass
    /// still determine whether and where the upgrade can execute.
    pub fn army_upgrade_material_wants(
        &self,
        g: &Game,
        pid: usize,
    ) -> Vec<ArmyUpgradeMaterialWant> {
        let mut wants: Vec<_> = self
            .army_upgrade_material_quotes(g, pid)
            .into_iter()
            .filter_map(|quote| {
                let stock = g.strategic_stockpile(pid, quote.resource);
                let income = g.strategic_resource_rate(pid, quote.resource.as_str());
                if stock + income.max(0.0) + f64::EPSILON >= quote.bill
                    || stock + (quote.bill - stock).ceil() > g.strategic_stockpile_capacity(pid)
                {
                    return None;
                }
                let amount = (quote.bill - stock).ceil();
                if amount < 1.0 || amount > f64::from(u32::MAX) {
                    return None;
                }
                Some(ArmyUpgradeMaterialWant {
                    resource: format!("RESOURCE_{}", quote.resource.as_str().to_ascii_uppercase()),
                    resource_id: quote.resource.as_str().to_string(),
                    amount: amount as u32,
                    unit: quote.target.as_str().to_string(),
                    uid: quote.uid,
                    gold_reserve: quote.gold_reserve,
                    gain: quote.gain,
                })
            })
            .collect();
        wants.sort_by(|a, b| {
            b.gain
                .total_cmp(&a.gain)
                .then_with(|| a.gold_reserve.total_cmp(&b.gold_reserve))
                .then_with(|| a.uid.cmp(&b.uid))
        });
        wants.truncate(1);
        wants
    }

    pub fn army_upgrade_material_reserves(&self, g: &Game, pid: usize) -> BTreeMap<Name, f64> {
        let mut reserves = BTreeMap::new();
        for quote in self.army_upgrade_material_quotes(g, pid) {
            let reserve = reserves.entry(quote.resource).or_insert(0.0_f64);
            *reserve = reserve.max(quote.bill);
        }
        reserves
    }

    fn army_upgrade_material_quotes(&self, g: &Game, pid: usize) -> Vec<ArmyUpgradeMaterialQuote> {
        let Some(player) = g.players.get(pid) else {
            return Vec::new();
        };
        if !self.army_upgrade_materials
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !player.gold.is_finite()
            || !player.gold_per_turn.is_finite()
        {
            return Vec::new();
        }
        let floor = 30.0 + (-player.gold_per_turn).max(0.0);
        g.player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                let from = &g.rules.units[unit.kind];
                if from.class != "military"
                    || from.promotion_class == "recon"
                    || matches!(from.domain.as_deref(), Some("sea" | "air"))
                    || unit.linked_to.is_some()
                    || self.amphibious_locked(uid)
                    || self.guard_is_reserved_for_civilian(uid)
                    || f64::from(unit.hp) <= self.base.w.withdraw_hp + 10.0
                {
                    return None;
                }
                let host = g
                    .host_unit_facts
                    .get(&uid)
                    .and_then(|facts| facts.upgrade.as_ref());
                let target = match host {
                    Some(offer) => offer.to?,
                    None => g.unit_upgrade_target(pid, unit.kind)?,
                };
                let to = &g.rules.units[target];
                if to.class != "military" || matches!(to.domain.as_deref(), Some("sea" | "air")) {
                    return None;
                }
                let gain = to.strength.max(to.ranged_attack_strength())
                    - from.strength.max(from.ranged_attack_strength());
                if !gain.is_finite() || gain < 15.0 {
                    return None;
                }
                let gold = host.and_then(|offer| offer.cost).or_else(|| {
                    g.unit_upgrade_price_in_formation(pid, unit.kind, target, unit.formation)
                        .map(|quote| quote.0)
                })?;
                let resource = to.requires_resource?;
                let bill = g.unit_upgrade_resource_price(pid, uid, target)?;
                let stock = g.strategic_stockpile(pid, resource);
                let income = g.strategic_resource_rate(pid, resource.as_str());
                if !gold.is_finite()
                    || gold < 0.0
                    || player.gold + f64::EPSILON < gold + floor
                    || !bill.is_finite()
                    || bill <= 0.0
                    || !stock.is_finite()
                    || !income.is_finite()
                    || income + f64::EPSILON < to.resource_maintenance.max(0.0)
                    || bill > g.strategic_stockpile_capacity(pid)
                {
                    return None;
                }
                Some(ArmyUpgradeMaterialQuote {
                    resource,
                    bill,
                    target,
                    uid,
                    gold_reserve: gold + floor,
                    gain,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
