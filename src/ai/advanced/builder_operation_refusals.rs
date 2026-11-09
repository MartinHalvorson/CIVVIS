//! A withheld clearing cannot consume this Builder's decision on another tile.

use super::*;

impl AdvancedAi {
    /// The bridge replaces these facts on every frame; ids belong to that
    /// frame's reconstructed board. Empty outside the Native order channel.
    pub fn set_refused_builder_operations(&mut self, operations: BTreeMap<u32, BTreeSet<Name>>) {
        self.refused_builder_operations = operations;
    }

    pub(super) fn advanced_builder_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
    ) -> bool {
        let Some(operations) = self
            .refused_builder_operations
            .get(&uid)
            .filter(|ops| !ops.is_empty())
        else {
            return self.advanced_builder_step_unfiltered(g, pid, uid, strategy);
        };
        // The outbound identity is unit + verb, with no plot for clearing.
        // Mask every eligible destination for this worker's decision, rather
        // than sending it to another job the same guard would still discard.
        let positions: Vec<_> = g
            .map
            .tiles
            .iter()
            .filter_map(|(pos, tile)| {
                tile.owner_city
                    .and_then(|cid| g.cities.get(&cid))
                    .is_some_and(|city| city.owner == pid)
                    .then_some(*pos)
            })
            .collect();
        let saved = Arc::clone(&g.blocked_improvements);
        let blocked = Arc::make_mut(&mut g.blocked_improvements);
        for pos in positions {
            blocked
                .entry(pos)
                .or_default()
                .extend(operations.iter().copied());
        }
        let acted = self.advanced_builder_step_unfiltered(g, pid, uid, strategy);
        // This worker may have spent its last charge and disappeared. Restore
        // only the temporary map; never look the unit up after its action.
        g.blocked_improvements = saved;
        acted
    }
}

#[cfg(test)]
mod tests;
