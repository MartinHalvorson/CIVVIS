//! Price the existing wartime upgrade goals by the research left to reach them.
//!
//! Native Indonesia115056Z68/0 chose the Stirrups path for three Heavy Chariots
//! (66 aggregate base strength) over Machinery for three Archers (45). The
//! observed remaining prerequisite bills were 345 and 112 science respectively.
//! This unpriced opt-in keeps the same eligible armies and priority slot, but
//! compares gain per remaining beaker using the existing war path-cost helper.
//! Material supply, Gold, production and survival are not guaranteed by this
//! ranking. In particular, do not reject an unlock merely because its resource
//! is not stockpiled: that technology may be the one that reveals it.

use super::*;

impl AdvancedAi {
    pub(super) fn modernization_research_value(
        &self,
        g: &Game,
        pid: usize,
        technology: Name,
        gain: f64,
    ) -> f64 {
        if !self.modernization_prices_the_path {
            return gain;
        }
        // The helper unions ancestors, excludes known nodes, uses native
        // remaining quotes without a second speed/boost discount, and deducts
        // active progress only when a host quote has not already done so.
        // A fully banked path has an immediate return; keep its score finite.
        gain / Self::war_remaining_research_cost(g, pid, technology).max(1.0)
    }
}

#[cfg(test)]
mod tests;
