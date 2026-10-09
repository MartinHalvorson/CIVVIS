use super::*;

impl AdvancedAi {
    /// Bring an obsolete, otherwise unclaimed land combat unit back to upgrade
    /// ground before its ordinary campaign march takes it farther away. This
    /// prepares a future offer; it never overrides a host refusal or upgrades
    /// after moving. Escorts, appointed packages and nearby combat keep priority.
    pub(super) fn domination_upgrade_return_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<bool> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || self.war_plan.is_some()
            || self.unit_is_reserved(uid)
            || self.guard_is_bound_to_any_settler(uid)
            || !plan.target_player.is_some_and(|target| {
                g.players.get(target).is_some_and(|p| {
                    p.alive
                        && !p.is_minor
                        && !p.is_barbarian
                        && (plan.strategy == GrandStrategy::Conquest || g.is_at_war(pid, target))
                })
            })
        {
            return None;
        }
        let unit = g.units.get(&uid)?;
        let from = &g.rules.units[unit.kind];
        if unit.owner != pid
            || unit.moves_left <= 0.0
            || unit.linked_to.is_some()
            || from.class != "military"
            || from.promotion_class == "recon"
            || matches!(from.domain.as_deref(), Some("sea" | "air"))
            || g.is_embarked(unit)
            || !matches!(
                g.unit_gold_upgrade_detail(pid, uid),
                Err("foreign territory" | "neutral territory")
            )
        {
            return None;
        }
        let here = unit.pos;
        // The territory refusal can hide a second failure. Read the exact
        // native successor and bill when present, then check cash and material
        // independently before committing even one movement point.
        let host = g
            .host_unit_facts
            .get(&uid)
            .and_then(|facts| facts.upgrade.as_ref());
        let target = host
            .and_then(|offer| offer.to)
            .or_else(|| g.unit_upgrade_target(pid, unit.kind))?;
        let to = &g.rules.units[target];
        let gain = to.strength.max(to.ranged_attack_strength())
            - from.strength.max(from.ranged_attack_strength());
        if gain < 15.0 {
            return None;
        }
        let gold = host.and_then(|offer| offer.cost).or_else(|| {
            g.unit_upgrade_price_in_formation(pid, unit.kind, target, unit.formation)
                .map(|quote| quote.0)
        })?;
        let resources = g.unit_upgrade_resource_price(pid, uid, target)?;
        let floor = 30.0 + (-g.players[pid].gold_per_turn).max(0.0);
        if !gold.is_finite()
            || gold < 0.0
            || g.players[pid].gold - gold < floor
            || to.requires_resource.is_some_and(|resource| {
                g.strategic_stockpile(pid, resource) + f64::EPSILON < resources
            })
            || !self.air_resource_upgrade_preserves_wing(g, pid, uid, target, resources)
        {
            return None;
        }
        // Do not peel a body off an actual battle or enemy city's approach.
        // Remembered hostile units are included in the board's unit collection.
        if Self::upgrade_return_near_combat(g, pid, here) {
            return None;
        }
        // The nearest border is enough; marching all the way into a City
        // Center adds time and can deadlock behind a garrison. Bound both the
        // number of route searches and the actual route length, so a short
        // geometric distance cannot send this unit around a closed frontier.
        let mut homes: Vec<_> = g
            .map
            .tiles
            .iter()
            .filter_map(|(pos, tile)| {
                (tile
                    .owner_city
                    .and_then(|cid| g.cities.get(&cid))
                    .is_some_and(|city| city.owner == pid)
                    && !g.rules.is_water(tile)
                    && g.rules.is_passable(tile)
                    && g.wdist(here, *pos) <= 12)
                    .then_some((g.wdist(here, *pos), *pos))
            })
            .collect();
        homes.sort_unstable();
        let (home, mut next) = homes.into_iter().take(8).find_map(|(_, home)| {
            let distance = g.route_distance(uid, home, 0)?;
            if distance == 0 || distance > 12 {
                return None;
            }
            g.route_step(uid, home, 0)
                .filter(|next| {
                    g.can_move(uid, *next) && !Self::upgrade_return_near_combat(g, pid, *next)
                })
                .map(|next| (home, next))
        })?;
        let mut acted = false;
        for _ in 0..12 {
            if g.apply(
                pid,
                &Action::Move {
                    unit: uid,
                    to: next,
                },
            )
            .is_err()
            {
                break;
            }
            acted = true;
            if next == home {
                break;
            }
            let Some(step) = g.route_step(uid, home, 0).filter(|step| {
                g.can_move(uid, *step) && !Self::upgrade_return_near_combat(g, pid, *step)
            }) else {
                break;
            };
            next = step;
        }
        if acted {
            // Arriving with movement left must not let the ordinary campaign
            // fallback march straight back out. Upgrading needs a fresh turn.
            self.base.fortify_or_stop(g, pid, uid);
            think!(self.journal(), Military, Decision,
                "Returning an obsolete unit to upgrade ground";
                "a {gain:.0}-strength upgrade costs {gold:.0} Gold, material is ready, and the bounded return route avoids nearby combat";
                next);
        }
        Some(acted)
    }

    fn upgrade_return_near_combat(g: &Game, pid: usize, pos: Pos) -> bool {
        g.units.values().any(|other| {
            g.is_at_war(pid, other.owner)
                && g.rules.units[other.kind].class == "military"
                && g.wdist(pos, other.pos) <= 6
        }) || g
            .cities
            .values()
            .any(|city| g.is_at_war(pid, city.owner) && g.wdist(pos, city.pos) <= 6)
    }

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
                let resources = g.unit_upgrade_resource_price(pid, uid, target)?;
                if !self.air_resource_upgrade_preserves_wing(g, pid, uid, target, resources) {
                    return None;
                }
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

#[cfg(test)]
mod return_tests;
