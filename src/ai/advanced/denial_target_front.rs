//! `denial-target-leads-the-fronts`: while we are at war with several
//! rivals, the one the victory counter names (`actionable_victory_denial`,
//! answered by Conquest) leads the fronts ahead of the second-front pick, the
//! running front and the campaign's own rival. It never opens a war: the
//! counter's rival must already be one of the wars we are fighting.
//!
//! **Why (live Emperor civvis-20261018T071140Z, pin with the loop-93
//! genes).** From turn 120 the counter named Byzantium, its religion race at
//! 75% and from turn 140 the host's culture clock at 37 turns. We were at war
//! with Byzantium from turn 140, at 1.4 times its power then and 2.9 times by
//! 175, while the campaign stayed on the Aztecs (`one_war_front`, then
//! `campaign_target`). It took Tenochtitlan at 158, and the front reached
//! Byzantium only at 165, when its culture race read 88%. Byzantium won on
//! Culture at 176 against our 17 cities. Across 22 earlier gate passers, the
//! counter named the eventual winner around turns 130-151 in 18, and the
//! campaign's most-targeted rival was the winner in 6.
//!
//! Off, the chain is unchanged.

use super::*;

impl AdvancedAi {
    /// The rival the wartime chain leads with under the gene: the counter's
    /// rival when it is answered by Conquest and is one of `active_fronts`.
    /// `None` with the gene off.
    pub(super) fn denial_target_front(
        &self,
        actionable_denial: Option<(usize, GrandStrategy)>,
        active_fronts: &[usize],
    ) -> Option<usize> {
        if !self.denial_target_leads_the_fronts {
            return None;
        }
        actionable_denial
            .filter(|(rival, counter)| {
                *counter == GrandStrategy::Conquest && active_fronts.contains(rival)
            })
            .map(|(rival, _)| rival)
    }
}

#[cfg(test)]
mod tests;
