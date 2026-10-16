//! Promote siege guns for the known walls on their current major-war campaign.
use super::*;

impl AdvancedAi {
    pub(super) fn wall_campaign_promotion(
        &self,
        _g: &Game,
        _pid: usize,
        _uid: u32,
        _strategy: GrandStrategy,
        _available: &[Name],
        _target_city: Option<u32>,
    ) -> Option<Name> {
        // Baseline stub: the new regression must fail on the actual old choice.
        None
    }
}

#[cfg(test)]
mod tests;
