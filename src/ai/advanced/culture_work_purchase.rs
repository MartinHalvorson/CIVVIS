//! A Culture seat can fill empty slots from a rival's public deal inventory.
//! Native Pericles 20261106T222235Z finished its first Amphitheater at turn 78
//! but still had no tourism at 119. At 129 its Writer points were 364 against
//! the next Writer's 620-point price. Buying an existing work does not wait
//! for that race. The host supplies the actual tradeable work and price;
//! this policy owns the objective, timing, counterparty and spending ceiling.

use super::*;

impl AdvancedAi {
    /// A native Great Work purchase: counterpart's simulator seat, maximum
    /// Gold-equivalent price, maximum lump Gold, and maximum GPT. `income`
    /// and `cultural_activation_pending` are facts exported by the host. `remaining_gold` reserves other planned purchases;
    /// war and slot facts stay on the observed board.
    pub fn culture_work_purchase_ask(
        &self,
        g: &Game,
        pid: usize,
        remaining_gold: f64,
        income: Option<f64>,
        cultural_activation_pending: bool,
    ) -> Option<(usize, i32, i32, i32)> {
        if self.lane_lost
            || !g.victory_conditions.culture
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || g.turn % 6 != 3
            || cultural_activation_pending
            || self
                .plan
                .as_ref()
                .is_some_and(|plan| plan.threatened_city.is_some())
            || g.cities.values().any(|city| {
                city.owner == pid
                    && city.last_attacked > 0
                    && g.turn.saturating_sub(city.last_attacked) <= 4
            })
        {
            return None;
        }
        let player = g.players.get(pid)?;
        let income = income.filter(|value| value.is_finite())?;
        if !player.gold.is_finite() || !remaining_gold.is_finite() || income <= 5.0 {
            return None;
        }
        if !["writing", "art", "music", "artifact", "relic"]
            .into_iter()
            .any(|kind| g.can_house_additional_great_work(pid, kind))
        {
            return None;
        }
        // Native 012706Z t165 quoted 105 Gold + 12 GPT (405 equivalent)
        // against our 227 ceiling, despite both payment components fitting
        // their reserves. Bound the components independently, then also cap
        // the combined price. The host must enforce all three limits.
        // A solvent treasury can replenish some of its liquidity from income.
        // Native 192016Z/t75 held 160 Gold and earned 32 GPT, but its fixed
        // 90-Gold reserve rejected a 108 Gold + 11 GPT Writing quote. Credit
        // two turns of observed income against that reserve, retaining at
        // least 30 actual Gold. This never borrows unearned cash: the spend
        // still fits both the current treasury and other planned purchases.
        let cash_reserve =
            (60.0 + 5.0 * g.player_city_ids(pid).len() as f64 - 2.0 * income).max(30.0);
        let max_gold = (player.gold.min(remaining_gold) - cash_reserve)
            .clamp(0.0, 500.0)
            .floor() as i32;
        // Preserve the existing 5 GPT floor and 20% income headroom.
        let max_gpt = ((income - 5.0) * 0.8).clamp(0.0, 20.0).floor() as i32;
        let ceiling = (max_gold + 25 * max_gpt).min(500);
        if ceiling < 80 {
            return None;
        }
        let majors: Vec<_> = g
            .players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && !g.same_team(pid, other.id)
            })
            .collect();
        let sellers: Vec<_> = majors
            .into_iter()
            .filter(|other| g.has_met(pid, other.id) && !g.is_at_war(pid, other.id))
            .collect();
        let index = (g.turn / 6) as usize % sellers.len().max(1);
        Some((sellers.get(index)?.id, ceiling, max_gold, max_gpt))
    }
}

#[cfg(test)]
mod tests;
