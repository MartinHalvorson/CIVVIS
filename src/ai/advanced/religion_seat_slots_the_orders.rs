//! `religion-seat-slots-the-orders`: a Religion-target founder with an Apostle
//! can put Religious Orders into a slot held by a lower-ranked Economic card.
//!
//! Native Russia civvis-20261110T001935Z unlocked Reformed Church at t84,
//! with three Apostles and Religious Orders on the observed menu, but the
//! card stayed out until t103. Scripture and Urban Planning were wanted too,
//! so their membership in the portfolio protected them regardless of rank.
//! This opt-in lets the existing ordering resolve that specific conflict.
//! Earlier timed and emergency cards keep their rank; the existing solvency
//! guard still protects amenities and upkeep. Closed host windows stay closed.
//! Other typed slots cannot donate wanted cards through wildcard capacity.
//! Scripture stays with Work Ethic: its adjacency also feeds Production,
//! which makes that Faith card part of the Russian seat's industrial base.
//!
//! Shipped Base/Assets/Gameplay/Data/Policies.xml:179 assigns
//! POLICY_RELIGIOUS_ORDERS to SLOT_ECONOMIC at CIVIC_REFORMED_CHURCH;
//! Base/Assets/Text/en_US/Civics_Text.xml:770 describes the +5 theological edge.
//! This is an unpriced experiment, not evidence of a higher victory rate.

use super::*;

impl AdvancedAi {
    pub(super) fn religion_orders_may_displace(
        &self,
        g: &Game,
        pid: usize,
        desired: &[&str],
        incoming: &str,
        current: &str,
    ) -> bool {
        if !self.religion_seat_slots_the_orders
            || incoming != "religious_orders"
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
            || g.rules.policies[current].slot != "economic"
        {
            return false;
        }
        let Some(religion) = g.players[pid].religion.as_deref() else {
            return false;
        };
        if current == "scripture"
            && g.players[pid]
                .religion_beliefs
                .iter()
                .any(|belief| belief == "work_ethic")
        {
            return false;
        }
        if !g.units.values().any(|unit| {
            unit.owner == pid
                && unit.kind == "apostle"
                && unit.religion.as_deref() == Some(religion)
        }) {
            return false;
        }
        match (
            desired.iter().position(|card| *card == incoming),
            desired.iter().position(|card| *card == current),
        ) {
            (Some(ours), Some(theirs)) => ours < theirs,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests;
