//! `religion-seat-cuts-the-upkeep`: a drained Religion founder uses a free
//! policy window for Conscription before the standing army is disbanded.
//!
//! Native Russia civvis-20261110T090948Z t71 had 1 Gold, -8 income, an open
//! policy window and fourteen paying units. Conscription was available but
//! absent from the Religion portfolio. At t75 the window cost 105 Gold and
//! the bankrupt seat disbanded a Heavy Chariot and its only Crossbowman.
//! This opt-in borrows an ordinary card's fitting seat, including wildcard
//! capacity, and keeps relief while removing it would restore a deficit.
//! Scripture with Work Ethic and theological strength retain their seats.
//! Later Gold-card choices in the same portfolio respect those protections too.
//!
//! Price each unit's discount with its own zero floor. The host's formation
//! bill precedes GetMaintDiscountPerUnit in ReportScreen.lua:306,314-341;
//! see Game::unit_gold_maintenance_cost. Surcharges do not change the saving.
//! No policy lock is bypassed and no purchase, disband or engine rule changes.
//! Giving up another policy has an opportunity cost; victory benefit is unmeasured.

use super::solvency::{DRAIN_CARD_MIN_GOLD, DRAIN_RUNWAY_TURNS};
use super::*;

impl AdvancedAi {
    pub(super) fn religion_upkeep_card(&self, g: &Game, pid: usize) -> Option<(&'static str, f64)> {
        if !self.religion_seat_cuts_the_upkeep
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
            || g.players[pid].religion.is_none()
            || g.is_arena()
        {
            return None;
        }
        let player = &g.players[pid];
        let available = g.available_policies(pid);
        for card in ["levee_en_masse", "conscription"] {
            let name = Name::new(card);
            if !available.contains(&name) && !player.policies.contains(&name) {
                continue;
            }
            let discount = g.rules.policies[card].effects["unit_maintenance_discount"];
            let other_discount = (g.policy_effect(pid, "unit_maintenance_discount")
                - if player.policies.contains(&name) {
                    discount
                } else {
                    0.0
                })
            .max(0.0);
            let saving: f64 = g
                .player_unit_ids(pid)
                .into_iter()
                .map(|uid| {
                    let unit = &g.units[&uid];
                    let formation = match unit.formation {
                        1 => 1.5,
                        2.. => 2.0,
                        _ => 1.0,
                    };
                    let bill = g
                        .host_unit_facts
                        .get(&uid)
                        .and_then(|facts| facts.maintenance)
                        .unwrap_or(g.rules.units[unit.kind].maintenance * formation);
                    (bill - other_discount).max(0.0).min(discount)
                })
                .sum();
            // The base deck runs first and can have removed a turn-start
            // card while native income still includes its discount.
            let held = player.policies.contains(&name) || self.turn_start_policies.contains(&name);
            let needs_relief = if held {
                player.gold_per_turn < saving
            } else {
                player.gold_per_turn < 0.0
                    && player.gold < -player.gold_per_turn * DRAIN_RUNWAY_TURNS
            };
            if saving >= DRAIN_CARD_MIN_GOLD && needs_relief {
                return Some((card, saving));
            }
        }
        None
    }

    pub(super) fn religion_upkeep_keeps_card(g: &Game, pid: usize, current: &str) -> bool {
        matches!(
            current,
            "religious_orders" | "revelation" | "limitanei" | "praetorium"
        ) || (current == "scripture"
            && g.players[pid]
                .religion_beliefs
                .iter()
                .any(|b| b == "work_ethic"))
    }

    pub(super) fn religion_upkeep_may_displace(
        g: &Game,
        pid: usize,
        current: &str,
        saving: f64,
    ) -> bool {
        !Self::religion_upkeep_keeps_card(g, pid, current)
            && saving - Self::card_gold_estimate(g, pid, current) >= DRAIN_CARD_MIN_GOLD
    }
}

#[cfg(test)]
mod tests;
