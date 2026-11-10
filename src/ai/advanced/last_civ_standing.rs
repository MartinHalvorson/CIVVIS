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
        if player.gold_per_turn < 0.0 && player.gold < 20.0 * -player.gold_per_turn {
            return;
        }
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
            if let Some((_, item)) = best {
                let _ = g.apply(pid, &Action::Produce { city: cid, item });
            }
        }
    }
}
