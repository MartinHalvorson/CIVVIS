use super::*;

impl AdvancedAi {
    /// Keep useful capture cavalry when training the preferred successor
    /// would spend the Aluminum that makes the launch wing viable.
    pub(super) fn air_resource_capture_body(
        &self,
        g: &Game,
        pid: usize,
        preferred: (Name, bool),
    ) -> (Name, bool) {
        if !self.air_resource_wing_reserved(g, pid) {
            return preferred;
        }
        let ready = |kind| {
            g.units
                .values()
                .filter(|unit| {
                    unit.owner == pid
                        && unit.hp >= 50
                        && Self::war_unit_is_at_least(g, pid, unit.kind, kind)
                })
                .count()
                >= air_surge::AIR_SURGE_LAUNCH_BODIES
        };
        let spec = &g.rules.units[preferred.0];
        let Some(resource) = spec.requires_resource else {
            return preferred;
        };
        if ready(preferred.0)
            || self.air_resource_spending_preserves_wing(
                g,
                pid,
                resource,
                spec.resource_maintenance,
                spec.resource_cost,
            )
        {
            return preferred;
        }
        g.units
            .values()
            .filter(|unit| unit.owner == pid && unit.hp >= 50)
            .map(|unit| unit.kind)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .filter(|kind| {
                let spec = &g.rules.units[*kind];
                spec.class == "military"
                    && spec.is_melee_capable()
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && (!preferred.1
                        || matches!(
                            spec.promotion_class.as_str(),
                            "light_cavalry" | "heavy_cavalry"
                        ))
                    && ready(*kind)
            })
            .max_by(|left, right| {
                g.rules.units[*left]
                    .strength
                    .total_cmp(&g.rules.units[*right].strength)
                    .then_with(|| right.cmp(left))
            })
            .map(|kind| {
                let cavalry = matches!(
                    g.rules.units[kind].promotion_class.as_str(),
                    "light_cavalry" | "heavy_cavalry"
                );
                (kind, cavalry)
            })
            .unwrap_or(preferred)
    }

    pub(super) fn air_surge_body_preserving_wing(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<(Name, bool)> {
        Self::air_surge_body(g, pid)
            .map(|preferred| self.air_resource_capture_body(g, pid, preferred))
    }

    fn air_resource_wing_reserved(&self, g: &Game, pid: usize) -> bool {
        if !self.air_surge_enabled()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.players[pid]
                .techs
                .contains(&Name::new(air_surge::AIR_SURGE_GOAL_TECH))
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
            // Last: it prices every city's pressure; every gate above is a
            // read and settles the answer first on most turns.
            && self.threatened_city(g, pid).is_none()
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
        let floor = self.upgrade_treasury_floor(g, pid, floor);
        self.modernize_army_preserving_air_wing(g, pid, floor, 0.0);
        BasicAi::use_opportunistic_unit_tools(g, pid);
    }
}

#[cfg(test)]
mod tests;
