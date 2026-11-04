//! `capital-watch-arms-the-slinger`: one solvent, healthy prewar upgrade.
//!
//! A peaceful major's existing capital-watch warning can arrive while the
//! assigned Domination seat is still expanding. Its ordinary upgrade pass
//! needs a named offensive or an active war, and the generic peaceful pass
//! keeps 120 Gold. One healthy Slinger need not wait for those stages when
//! its actual offer leaves the projected upkeep reserve and positive income.
//! City-saving purchases retain priority. The wall policy stays independent.
use super::*;

const HEALTHY_SLINGER_HP: i32 = 75;

impl AdvancedAi {
    pub(super) fn capital_watch_slinger_upgrade(&mut self, g: &mut Game, pid: usize) -> bool {
        if !self.capital_watch_arms_the_slinger
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || self.capital_watch_slinger_turn == Some(g.turn)
            || self.forced_target_player.is_some()
            || self.war_plan.is_some()
            || self.air_surge_plan.is_some()
            || self
                .plan
                .as_ref()
                .is_some_and(|plan| plan.strategy == GrandStrategy::Recovery)
            || g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && !other.is_free_city
                    && g.is_at_war(pid, other.id)
            })
        {
            return false;
        }
        let Some((watched, ratio)) = self.capital_watched_major_power(g, pid) else {
            return false;
        };
        let Some(capital) = Self::capital_watch_city(g, pid) else {
            return false;
        };
        let home = g.cities[&capital].pos;
        let archer = g.player_unit_replacement(pid, crate::name!("archer"));
        let player = &g.players[pid];
        let bill = g.unit_gold_maintenance(pid);
        if !player.gold.is_finite() || !player.gold_per_turn.is_finite() || !bill.is_finite() {
            return false;
        }
        let best = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                if unit.kind != "slinger"
                    || unit.hp < HEALTHY_SLINGER_HP
                    || g.is_embarked(unit)
                    || unit.linked_to.is_some()
                    || self.amphibious_locked(uid)
                    || self.guard_is_reserved_for_civilian(uid)
                    || g.wdist(unit.pos, home) > 8
                {
                    return None;
                }
                // The normal offer honors a native refusal, movement, territory,
                // successor, exact price and resource bill; a quote alone is not permission.
                let (target, price, resources) = g.unit_gold_upgrade_offer(pid, uid)?;
                let successor = &g.rules.units[target];
                if target != archer
                    || successor.promotion_class != "ranged"
                    || resources != 0.0
                    || !price.is_finite()
                    || price < 0.0
                {
                    return None;
                }
                let delta = Self::upgrade_upkeep_delta(g, uid, target);
                let income = player.gold_per_turn - delta;
                let reserve = super::gold_and_cards::UPKEEP_RESERVE_TURNS * (bill + delta).max(0.0);
                if !delta.is_finite()
                    || !income.is_finite()
                    || income <= 0.0
                    || player.gold - price < reserve
                {
                    return None;
                }
                let from = &g.rules.units[unit.kind];
                let gain = successor.strength.max(successor.ranged_attack_strength())
                    - from.strength.max(from.ranged_attack_strength());
                (gain > 0.0).then_some((
                    gain * f64::from(unit.hp) / price.max(1.0),
                    uid,
                    price,
                    reserve,
                    income,
                ))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)));
        let Some((_, uid, price, reserve, income)) = best else {
            return false;
        };
        if g.apply(pid, &Action::UpgradeUnit { unit: uid }).is_err() {
            return false;
        }
        self.capital_watch_slinger_turn = Some(g.turn);
        think!(self.journal(), Military, Decision,
            "Arming a healthy Slinger before the watched war";
            "capital-watch-arms-the-slinger: rival {watched} at {ratio:.1}x our power; {price:.0} Gold, projected income {income:+.1}, keep {reserve:.1} for upkeep; one upgrade proposal this turn");
        true
    }
}

#[cfg(test)]
mod tests;
