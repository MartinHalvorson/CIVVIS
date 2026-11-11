//! Upfront material for one affordable land-army upgrade.
//!
//! Native Emperor `civvis-20261014T230322Z` fielded ten Heavy Chariots at
//! turn 89. Their host quote named a Knight, 160 Gold and ten Iron; no
//! Iron stock or income existed, while two peaceful rivals offered 25.
//! The siege/air resource buyer supplied no ordinary land-upgrade want.
//! Native Culture armies at war use `culture_war_materials` for their
//! observed upgrade bills and a renewable unit-fuel buffer.
//!
//! `land-army-asks-the-host-price` opts native Domination upgrades into the
//! existing bounded host cash-price path. Deity Macedon 100029Z had four
//! Warriors, no Iron and peaceful Iron sellers at turn50. Horse/Iron barter
//! was declined at51/55 and refused as barter_short at61. Iron obtained at70
//! finally allowed two Hypaspists at72. A model book is not the host price.
//! Keep the existing single-upgrade want and its reserved cash; this changes
//! the proposed material deal, not its acceptance or the upgrade itself.
//! With less cash than the transport minimum, retain the existing barter path:
//! the initial implementation removed a native Iron barter at091313Z/59.
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
    /// Shared minimum spendable cash for a native resource price request.
    /// Below this transport floor, the opt-in retains the existing barter path.
    pub const NATIVE_RESOURCE_CASH_QUOTE_MINIMUM: i32 = 30;

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
        if let Some((wants, _)) = self.culture_war_material_reading(g, pid, None) {
            return wants;
        }
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
        if let Some((_, reserves)) = self.culture_war_material_reading(g, pid, None) {
            return reserves;
        }
        let mut reserves = BTreeMap::new();
        for quote in self.army_upgrade_material_quotes(g, pid) {
            let reserve = reserves.entry(quote.resource).or_insert(0.0_f64);
            *reserve = reserve.max(quote.bill);
        }
        reserves
    }

    /// Native defensive purchases fund only upgrades still affordable after
    /// this frame's other spending. The bool selects the bounded host cash
    /// price path, also used by the opt-in native land-army material buyer;
    /// the bridge can then defer a new alliance proposal for the material deal.
    pub fn army_upgrade_material_wants_after_spending(
        &self,
        g: &Game,
        pid: usize,
        remaining_gold: f64,
    ) -> (Vec<ArmyUpgradeMaterialWant>, bool) {
        if let Some((wants, _)) = self.culture_war_material_reading(g, pid, Some(remaining_gold)) {
            return (wants, true);
        }
        let wants = self.army_upgrade_material_wants(g, pid);
        let host_price = self.land_army_asks_the_host_price
            && remaining_gold.is_finite()
            && wants.first().is_some_and(|want| {
                let resource = Name::new(&want.resource_id);
                remaining_gold.min(g.players[pid].gold).floor()
                    >= want.gold_reserve.ceil()
                        + f64::from(Self::NATIVE_RESOURCE_CASH_QUOTE_MINIMUM)
                    && g.observed_strategic_income_adjustments
                        .get(&pid)
                        .is_some_and(|income| income.contains_key(&resource))
                    && g.host_unit_facts.get(&want.uid).is_some_and(|facts| {
                        facts.civ6_id.is_some()
                            && facts.upgrade.as_ref().is_some_and(|quote| {
                                quote.to.is_some_and(|target| target == want.unit)
                                    && quote
                                        .cost
                                        .is_some_and(|gold| gold.is_finite() && gold >= 0.0)
                                    && quote.resources.as_ref().is_some_and(|bill| {
                                        bill.resource == Some(resource)
                                            && bill.cost.is_finite()
                                            && bill.cost > 0.0
                                    })
                            })
                    })
            });
        (wants, host_price)
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
