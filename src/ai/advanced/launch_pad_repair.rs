//! `launch-pad-spares-the-repair`: the original first-pad hold leaves an
//! already selected, still legal district or building repair at the head.
//!
//! Native Immortal Australia, civvis-20261103T070446Z: Geelong began its
//! first pad at t159 while producing 63 against the best placing city's 72.
//! Its Industrial Zone and buildings were then pillaged; production fell to
//! 26. At t160 the repair claim chose a district repair and the pad hold
//! replaced it, quoting 44 host turns left for the Spaceport. Across the
//! 11-01/03 journal census, 72 distinct game/turn/city observations in 17
//! runs show the hold displacing a repair (198 lines, including replans).
//!
//! This opt-in changes only that priority conflict. It chooses no repair,
//! relies on the earlier writer's safety decision, and rechecks legality so
//! an obsolete or foreign repair does not hold the queue. The original pad
//! hold, its defence guards and later defence/loyalty handoffs remain intact.
//! No native execution, completion-time or strength result is claimed.

use super::*;

impl AdvancedAi {
    pub(super) fn launch_pad_spares_repair(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> bool {
        self.launch_pad_spares_the_repair
            && matches!(item, Item::Repair { .. })
            && g.can_produce(pid, cid, item)
    }
}

#[cfg(test)]
mod tests;
