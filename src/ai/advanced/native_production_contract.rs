//! Snapshots which cities may start the empire's first Spaceport before
//! speculative actions run, so a planned foundation the live bridge never
//! places cannot authorize a slow first pad elsewhere.

use super::*;

impl AdvancedAi {
    /// Snapshot the existing first-pad producer rule before speculative actions.
    /// A planned district immediately creates a simulated foundation, even when
    /// a later queue revision means the native bridge will never place it. That
    /// discarded commitment must not authorize another city's slow first pad.
    /// `None` preserves the untreated native production stream.
    pub fn native_first_pad_allowed_cities(
        &self,
        observed: &Game,
        pid: usize,
    ) -> Option<std::collections::BTreeSet<u32>> {
        self.first_pad_in_the_forge.then(|| {
            let _memo = observed.query_memo();
            observed
                .player_city_ids(pid)
                .into_iter()
                .filter(|cid| self.base.first_pad_city_allowed(observed, pid, *cid))
                .collect()
        })
    }
}

#[cfg(test)]
mod tests;
