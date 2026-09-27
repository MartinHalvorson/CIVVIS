use super::*;

impl AdvancedAi {
    fn air_resource_wing_reserved(&self, g: &Game, pid: usize) -> bool {
        if !self.air_surge_enabled()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.players[pid]
                .techs
                .contains(&Name::new(air_surge::AIR_SURGE_GOAL_TECH))
            || self.threatened_city(g, pid).is_some()
        {
            return false;
        }
        let (fields, bombers) = Self::air_surge_supply_commitments(g, pid);
        (fields > 0 || bombers > 0)
            && (bombers >= air_surge::AIR_SURGE_LAUNCH_BOMBERS
                || g.turn
                    .saturating_add(g.standard_duration(air_surge::AIR_SURGE_ENDGAME_RESERVE))
                    < g.max_turns)
            && Self::air_surge_bomber_goal(g, pid) >= air_surge::AIR_SURGE_LAUNCH_BOMBERS
    }

    /// A discretionary land consumer may use spare fuel, but cannot turn a
    /// viable launch wing into a shortage. Existing queues and free upkeep
    /// follow the same demand accounting as the actual resource processor.
    fn air_resource_spending_preserves_wing(
        &self,
        g: &Game,
        pid: usize,
        resource: Name,
        demand: f64,
        cost: f64,
    ) -> bool {
        let Some(bomber) = Self::air_surge_bomber(g, pid) else {
            return true;
        };
        if g.rules.units[bomber].requires_resource != Some(resource)
            || !self.air_resource_wing_reserved(g, pid)
        {
            return true;
        }
        Self::air_surge_bomber_goal_after_spending(g, pid, demand, cost)
            >= air_surge::AIR_SURGE_LAUNCH_BOMBERS
    }

    pub(super) fn air_resource_upgrade_preserves_wing(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        target: Name,
        cost: f64,
    ) -> bool {
        let to = &g.rules.units[target];
        if to.promotion_class == "air_bomber" {
            return true;
        }
        let Some(resource) = to.requires_resource else {
            return true;
        };
        let unit = &g.units[&uid];
        let from = &g.rules.units[unit.kind];
        let existing = if from.requires_resource == Some(resource) {
            from.resource_maintenance
        } else {
            0.0
        };
        let extra = if unit.free_upkeep {
            0.0
        } else {
            to.resource_maintenance - existing
        };
        self.air_resource_spending_preserves_wing(g, pid, resource, extra, cost)
    }

    pub(super) fn air_resource_item_preserves_wing(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> bool {
        let (unit, formation) = match item {
            Item::Unit { unit } => (*unit, 1.0),
            Item::Formation { unit, formation } => (*unit, if *formation >= 2 { 3.0 } else { 2.0 }),
            _ => return true,
        };
        let spec = &g.rules.units[unit];
        if spec.promotion_class == "air_bomber" || g.cities[&cid].queue.contains(item) {
            return true;
        }
        let Some(resource) = spec.requires_resource else {
            return true;
        };
        let discount = g
            .governor_effect(pid, cid, "strategic_resource_discount_pct")
            .clamp(0.0, 100.0);
        self.air_resource_spending_preserves_wing(
            g,
            pid,
            resource,
            spec.resource_maintenance,
            spec.resource_cost * formation * (100.0 - discount) / 100.0,
        )
    }

    pub(super) fn modernize_army_preserving_air_wing(
        &self,
        g: &mut Game,
        pid: usize,
        floor: f64,
        veteran_weight: f64,
    ) -> usize {
        BasicAi::modernize_army_with_filter(
            g,
            pid,
            floor,
            veteran_weight,
            |board, uid, target, cost| {
                self.air_resource_upgrade_preserves_wing(board, pid, uid, target, cost)
            },
        )
    }

    pub(super) fn upgrade_units_preserving_air_wing(&self, g: &mut Game, pid: usize) {
        if g.players[pid].is_barbarian {
            return;
        }
        let at_war = g
            .players
            .iter()
            .any(|p| p.id != pid && p.alive && !p.is_barbarian && g.is_at_war(pid, p.id));
        let floor = if at_war || BasicAi::barbarian_military_gap(g, pid) {
            30.0
        } else {
            120.0
        };
        self.modernize_army_preserving_air_wing(g, pid, floor, 0.0);
        BasicAi::use_opportunistic_unit_tools(g, pid);
    }
}

#[cfg(test)]
mod tests;
