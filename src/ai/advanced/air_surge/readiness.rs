//! Build a minimal air capability without keeping a completed war appointment.

use super::*;

impl AdvancedAi {
    pub(super) fn domination_air_readiness_active(&self, g: &Game, pid: usize) -> bool {
        self.air_surge_enabled()
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.players[pid]
                .techs
                .contains(&crate::name!("industrialization"))
            && g.player_city_ids(pid).len() >= 2
            && self.threatened_city(g, pid).is_none()
            && g.turn
                .saturating_add(g.standard_duration(AIR_SURGE_ENDGAME_RESERVE))
                < g.max_turns
            && Self::air_surge_bomber(g, pid).is_some()
            && g.player_unit_ids(pid)
                .into_iter()
                .filter(|uid| g.rules.units[g.units[uid].kind].promotion_class == "air_bomber")
                .count()
                < AIR_SURGE_LAUNCH_BOMBERS
    }

    /// Count all bomber generations, including pending orders, so a Jet Bomber
    /// fulfils the same preparation contract as its obsolete predecessor.
    pub(super) fn domination_air_readiness_counts(g: &Game, pid: usize) -> (usize, usize) {
        let mut fields = 0;
        let mut bombers = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.rules.units[g.units[uid].kind].promotion_class == "air_bomber")
            .count();
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            fields += usize::from(Self::air_surge_field(g, pid).is_some_and(|field| {
                g.city_has_district_family(city, field) || city.queue.iter().any(|item| {
                    matches!(item, Item::District { district, .. } if g.district_family(*district) == field)
                })
            }));
            bombers += city
                .queue
                .iter()
                .filter(|item| {
                    matches!(item, Item::Unit { unit } | Item::Formation { unit, .. }
                    if g.rules.units[*unit].promotion_class == "air_bomber")
                })
                .count();
        }
        (fields, bombers)
    }

    pub(super) fn domination_air_readiness_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        turns: f64,
    ) -> Option<f64> {
        // Most menu entries cannot affect this contract. Avoid re-reading
        // empire readiness for every ordinary building, project and land unit.
        let relevant = match item {
            Item::District { district, .. } => g.rules.districts[district].specialty,
            Item::Unit { unit } | Item::Formation { unit, .. } => {
                g.rules.units[*unit].promotion_class == "air_bomber"
            }
            _ => false,
        };
        if !relevant || !self.domination_air_readiness_active(g, pid) {
            return None;
        }
        if self.air_surge_reserves_field_slot(g, pid, cid, item) {
            return Some(-10_000.0);
        }
        let (fields, mut bombers) = Self::domination_air_readiness_counts(g, pid);
        let committed = g.cities[&cid].queue.first() == Some(item);
        match item {
            Item::District { district, .. }
                if Self::air_surge_field(g, pid)
                    .is_some_and(|field| g.district_family(*district) == field) =>
            {
                (fields == 0 || (fields == 1 && committed))
                    .then_some(AIR_SURGE_AERODROME_VALUE - turns * 8.0)
            }
            Item::Unit { unit } | Item::Formation { unit, .. }
                if g.rules.units[*unit].promotion_class == "air_bomber" =>
            {
                if committed {
                    bombers = bombers.saturating_sub(1);
                }
                let upkeep = g.rules.units[*unit].maintenance;
                let deficit = (upkeep - g.players[pid].gold_per_turn).max(0.0);
                (bombers < AIR_SURGE_LAUNCH_BOMBERS
                    && Self::air_surge_metal_ready(g, pid)
                    && g.players[pid].gold >= 40.0 + deficit * 6.0)
                    .then_some(AIR_SURGE_BOMBER_VALUE - turns * 8.0)
            }
            _ => None,
        }
    }

    pub(super) fn domination_air_readiness_production(&mut self, g: &mut Game, pid: usize) -> bool {
        if !self.domination_air_readiness_active(g, pid) {
            return false;
        }
        let field = Self::air_surge_field(g, pid);
        let mut best: Option<(f64, u32, Item)> = None;
        for cid in g.player_city_ids(pid) {
            if !g.cities[&cid].queue.is_empty() {
                continue;
            }
            for item in g.producible_items(pid, cid) {
                let package_item = match &item {
                    Item::District { district, .. } => {
                        field.is_some_and(|field| g.district_family(*district) == field)
                    }
                    Item::Unit { unit } | Item::Formation { unit, .. } => {
                        g.rules.units[*unit].promotion_class == "air_bomber"
                    }
                    _ => false,
                };
                if !package_item {
                    continue;
                }
                let turns = g.item_remaining_cost_for_city(pid, cid, &item)
                    / (g.city_yields(cid).production * g.item_prod_mult(pid, cid, Some(&item)))
                        .max(0.1);
                let Some(value) = self.domination_air_readiness_value(g, pid, cid, &item, turns)
                else {
                    continue;
                };
                if value <= 0.0 || turns > g.max_turns.saturating_sub(g.turn) as f64 {
                    continue;
                }
                if best
                    .as_ref()
                    .is_none_or(|(old, city, _)| value > *old || (value == *old && cid < *city))
                {
                    best = Some((value, cid, item));
                }
            }
        }
        let Some((_, city, item)) = best else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_err()
        {
            return false;
        }
        think!(self.journal(), Military, Decision,
            "{} starts {} for Domination air readiness", g.cities[&city].name, Self::plain_item(&item);
            "prepare a base and a small bomber force independently of the current war appointment");
        true
    }
}

#[cfg(test)]
mod tests;
