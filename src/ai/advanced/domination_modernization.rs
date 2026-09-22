use super::*;

impl AdvancedAi {
    /// Existing land combat units with an unlocked, materially stronger direct
    /// successor. This is a funding need, so an empty treasury or a march away
    /// from friendly territory must not hide it. Actual purchases still use
    /// the authoritative per-unit offer below.
    fn domination_upgrade_need(&self, g: &Game, pid: usize) -> Vec<(u32, f64)> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination) {
            return Vec::new();
        }
        g.player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                let from = &g.rules.units[unit.kind];
                if from.class != "military"
                    || from.promotion_class == "recon"
                    || matches!(from.domain.as_deref(), Some("sea" | "air"))
                {
                    return None;
                }
                let target = g.unit_upgrade_target(pid, unit.kind)?;
                let to = &g.rules.units[target];
                let (_, resources) =
                    g.unit_upgrade_price_in_formation(pid, unit.kind, target, unit.formation)?;
                if to.requires_resource.is_some_and(|resource| {
                    g.strategic_stockpile(pid, resource) + f64::EPSILON < resources
                }) {
                    return None;
                }
                let gain = to.strength.max(to.ranged_attack_strength())
                    - from.strength.max(from.ranged_attack_strength());
                (gain >= 5.0).then_some((uid, gain * unit.hp.clamp(0, 100) as f64 / 100.0))
            })
            .collect()
    }

    /// Positive income can still leave a named offensive's cohort obsolete.
    /// Fund the two cheapest resource-ready upgrades within four turns, plus
    /// the same emergency floor used by the actual purchase pass. A host's
    /// price takes precedence, but never grants permission to upgrade here.
    pub(super) fn domination_upgrade_funding_shortfall(&self, g: &Game, pid: usize) -> bool {
        let discount = g.policy_effect(pid, "unit_maintenance_discount");
        let saves_upkeep = g.player_unit_ids(pid).into_iter().any(|uid| {
            let unit = &g.units[&uid];
            let bill = g
                .host_unit_facts
                .get(&uid)
                .and_then(|facts| facts.maintenance)
                .unwrap_or(g.rules.units[unit.kind].maintenance);
            bill > discount
        });
        if !saves_upkeep {
            return false;
        }
        let mut prices: Vec<f64> = self
            .domination_upgrade_need(g, pid)
            .into_iter()
            .filter_map(|(uid, _)| {
                let unit = &g.units[&uid];
                let target = g.unit_upgrade_target(pid, unit.kind)?;
                let modeled = g
                    .unit_upgrade_price_in_formation(pid, unit.kind, target, unit.formation)?
                    .0;
                Some(
                    g.host_unit_facts
                        .get(&uid)
                        .and_then(|facts| facts.upgrade.as_ref())
                        .and_then(|offer| offer.cost)
                        .unwrap_or(modeled),
                )
            })
            .collect();
        if prices.len() < 2 {
            return false;
        }
        prices.sort_by(f64::total_cmp);
        let income = g.players[pid].gold_per_turn;
        let needed = prices[0] + prices[1] + 30.0 + (-income).max(0.0);
        g.players[pid].gold + 4.0 * income.max(0.0) < needed
    }

    pub(super) fn domination_upgrade_civic_goal(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<&'static str> {
        if !g.players[pid]
            .civics
            .contains(&crate::name!("political_philosophy"))
            || g.players[pid].civics.contains(&crate::name!("mercenaries"))
            || g.players[pid]
                .civics
                .contains(&crate::name!("urbanization"))
            || self.domination_upgrade_need(g, pid).len() < 2
        {
            return None;
        }
        Some("mercenaries")
    }

    pub(super) fn domination_upgrade_policy(&self, g: &Game, pid: usize) -> Option<&'static str> {
        let need = self.domination_upgrade_need(g, pid).len();
        let held = g.players[pid]
            .policies
            .iter()
            .any(|card| matches!(card.as_str(), "professional_army" | "force_modernization"));
        if need < 2 && !(held && need > 0) {
            return None;
        }
        let available = g.available_policies(pid);
        ["force_modernization", "professional_army"]
            .into_iter()
            .find(|card| {
                available.contains(&Name::new(card))
                    || g.players[pid].policies.contains(&Name::new(card))
            })
    }

    /// A named Domination offensive uses the same small cash floor as a war,
    /// plus one turn of any current deficit. The old 120-Gold peacetime floor
    /// could keep the army obsolete while its declaration waited for strength.
    /// City-saving purchases run before this pass; appointed war packages own
    /// their own budget. Neither legal offers nor host prices are relaxed.
    pub(super) fn fund_domination_upgrades(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if self.war_plan.is_some()
            || !plan.target_player.is_some_and(|target| {
                g.players.get(target).is_some_and(|p| {
                    p.alive
                        && !p.is_minor
                        && !p.is_barbarian
                        && (plan.strategy == GrandStrategy::Conquest || g.is_at_war(pid, target))
                })
            })
        {
            return;
        }
        // The ordinary policy pass follows discretionary spending. Seat a
        // newly useful discount now; an already-held card needs no extra pass.
        // A mirrored host quote remains authoritative until the next frame,
        // even when the local policy swap projects a cheaper upgrade.
        if self
            .domination_upgrade_policy(g, pid)
            .is_some_and(|card| !g.players[pid].policies.contains(&Name::new(card)))
        {
            self.strategic_policies(g, pid, self.policy_lane(g, pid, plan));
        }
        let floor = 30.0 + (-g.players[pid].gold_per_turn).max(0.0);
        loop {
            let best = self
                .domination_upgrade_need(g, pid)
                .into_iter()
                .filter_map(|(uid, gain)| {
                    let (_, gold, _) = g.unit_gold_upgrade_offer(pid, uid)?;
                    (g.players[pid].gold - gold >= floor).then_some((gain / gold.max(1.0), uid))
                })
                .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)));
            let Some((_, uid)) = best else { break };
            if g.apply(pid, &Action::UpgradeUnit { unit: uid }).is_err() {
                break;
            }
            think!(self.journal(), Military, Decision,
                "Modernizing a standing Domination unit";
                "the named offensive needs stronger bodies before it can stage; keep {floor:.0} Gold for emergencies and current maintenance");
        }
    }
}

#[cfg(test)]
mod tests;
