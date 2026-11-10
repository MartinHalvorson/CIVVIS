//! A world played on to its last civilization leaves every seat one lane: conquest.

use super::*;

impl AdvancedAi {
    /// Once "one more turn" has made the survivor the only possible result,
    /// science, culture, faith and votes can win nothing, so every civilization
    /// plays for conquest for the rest of the game.
    pub(super) fn plays_for_the_last_civ(&mut self, g: &Game, pid: usize) {
        if g.plays_to_the_last_civ()
            && !g.players[pid].is_minor
            && !g.players[pid].is_barbarian
            && self.victory_target != Some(VictoryTarget::Domination)
        {
            self.retarget(VictoryTarget::Domination);
        }
    }

    /// PROBE: the land army a seat wants in the final war.
    pub(super) fn last_civ_army_target(
        &self,
        g: &Game,
        pid: usize,
        city_count: usize,
        shipped: usize,
    ) -> usize {
        let bodies = crate::lcs_knobs::get("bodies");
        let player = &g.players[pid];
        if bodies <= 0.0 || !g.plays_to_the_last_civ() || player.is_minor || player.is_barbarian {
            return shipped;
        }
        if player.gold_per_turn < 0.0 && player.gold < 20.0 * -player.gold_per_turn {
            return shipped;
        }
        shipped.max((bodies * city_count as f64).ceil() as usize)
    }

    /// PROBE: every idle queue trains the soldier the scorer rates best.
    pub(super) fn arm_for_the_final_war(&mut self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if crate::lcs_knobs::get("arm") <= 0.0 || !g.plays_to_the_last_civ() {
            return;
        }
        let player = &g.players[pid];
        if player.is_minor || player.is_barbarian {
            return;
        }
        let mut pending: f64 = g
            .player_city_ids(pid)
            .iter()
            .filter_map(|cid| match g.cities[cid].queue.first() {
                Some(Item::Unit { unit }) => Some(g.rules.units[unit].maintenance),
                _ => None,
            })
            .sum();
        for cid in g.player_city_ids(pid) {
            if !g.cities[&cid].queue.is_empty() {
                continue;
            }
            let counts = self.counts(g, pid);
            let mut best: Option<(f64, Item)> = None;
            for item in g.producible_items(pid, cid) {
                let Item::Unit { unit } = &item else { continue };
                let spec = &g.rules.units[unit];
                if spec.class != "military"
                    || spec.domain.as_deref() == Some("sea")
                    || spec.promotion_class == "recon"
                {
                    continue;
                }
                let value = self.production_value(g, pid, cid, &item, plan, &counts);
                if value.is_finite()
                    && value > -1_000.0
                    && best.as_ref().is_none_or(|(old, _)| value > *old)
                {
                    best = Some((value, item));
                }
            }
            let Some((_, item)) = best else { continue };
            let upkeep = match &item {
                Item::Unit { unit } => g.rules.units[unit].maintenance,
                _ => 0.0,
            };
            let net = g.players[pid].gold_per_turn - pending - upkeep;
            if net < 0.0 && g.players[pid].gold < 20.0 * -net {
                break;
            }
            if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
                pending += upkeep;
            }
        }
    }

    /// PROBE: the nearest city of a living rival civilization, one we can
    /// march to before one across water, kept while it stays among the nearest.
    pub(super) fn final_war_objective(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        if crate::lcs_knobs::get("objective") <= 0.0 || !g.plays_to_the_last_civ() {
            return None;
        }
        if g.players[pid].is_minor || g.players[pid].is_barbarian {
            return None;
        }
        let ours: Vec<_> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        let mut by_land: BTreeMap<usize, bool> = BTreeMap::new();
        let mut ranked: Vec<(u8, i32, u32, usize)> = Vec::new();
        for city in g.cities.values() {
            let owner = &g.players[city.owner];
            if city.owner == pid
                || !owner.alive
                || owner.is_minor
                || owner.is_barbarian
                || g.same_team(pid, city.owner)
            {
                continue;
            }
            let land = *by_land
                .entry(city.owner)
                .or_insert_with(|| self.rival_reachable_by_land(g, pid, city.owner));
            let reach = ours
                .iter()
                .map(|mine| g.wdist(*mine, city.pos))
                .min()
                .unwrap_or(i32::MAX);
            ranked.push((u8::from(!land), reach, city.id, city.owner));
        }
        let best = ranked.iter().copied().min()?;
        let kept =
            self.plan
                .as_ref()
                .and_then(|plan| plan.target_city)
                .and_then(|prior| {
                    ranked.iter().copied().find(|entry| {
                        entry.2 == prior && entry.0 == best.0 && entry.1 <= best.1 + 3
                    })
                });
        let (_, _, city, owner) = kept.unwrap_or(best);
        Some((owner, city))
    }
}
