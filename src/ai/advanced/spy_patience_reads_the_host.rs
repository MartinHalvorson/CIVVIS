//! Spy patience reads the host: a Spy the host reports idle is not left
//! waiting out an order that has already ended.
//!
//! `spy_mission_patience` leaves a Spy alone for a fixed window after each
//! order (`SPY_MISSION_ORDER_PATIENCE`, `SPY_TRAVEL_ORDER_PATIENCE`) because
//! the rebuilt mirror could not see the host's running operation, and without
//! the window the same order went out every turn. The live mirror now carries
//! that operation (`HostUnitFacts::spy_operation`, with the host's menu in
//! `spy_missions`), so the window outlives the truth whenever an operation
//! ends early (a failure, or an Online mission shorter than the window), and
//! whenever the order never left at all: most branches set the window whether
//! or not their `g.apply` succeeded.
//!
//! Live Deity Korea science lane, 2026-11-05, 17 games: 309 of 1,317 Spy-turns
//! (23%) ended with the Spy idle, the host reporting no operation and a
//! non-empty mission menu, and no order sent that turn. In
//! civvis-20261105T154934Z a level-3 Spy in Boston sat idle at turns 137-138
//! and 143-146 between Steal Tech Boosts; in 122513Z a Spy idled at 101-103
//! after its turn-96 steal, and a one-shot replay of turn 102 (fresh state, no
//! window) orders it Gain Sources.
//!
//! The rule: an order's window is spent once a later turn finds the Spy idle
//! by the host's own report. Frames of the turn the order was given keep it,
//! so no order is sent twice while the host is still applying it. With no host
//! facts (a native board, an older mod) nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether the host reports `spy` idle: no operation running and a
    /// mission menu offered where it stands.
    fn host_reports_spy_idle(g: &Game, spy: u32) -> bool {
        g.host_unit_facts
            .get(&spy)
            .is_some_and(|facts| facts.spy_operation.is_none() && facts.spy_missions.is_some())
    }

    /// Drop the patience of every Spy whose order came on an earlier turn and
    /// whom the host now reports idle.
    pub(super) fn release_spent_spy_patience(&mut self, g: &Game) {
        let spent: Vec<u32> = self
            .spy_orders_until
            .keys()
            .copied()
            .filter(|spy| {
                self.spy_order_turns
                    .get(spy)
                    .is_some_and(|ordered| *ordered < g.turn)
                    && Self::host_reports_spy_idle(g, *spy)
            })
            .collect();
        for spy in spent {
            self.spy_orders_until.remove(&spy);
        }
    }

    /// After the Spy pass: remember the turn of every patience entry this
    /// pass set or changed, and forget those that are gone.
    pub(super) fn note_spy_order_turns(&mut self, g: &Game, before: &BTreeMap<u32, u32>) {
        for (spy, until) in &self.spy_orders_until {
            if before.get(spy) != Some(until) {
                self.spy_order_turns.insert(*spy, g.turn);
            }
        }
        let live = &self.spy_orders_until;
        self.spy_order_turns.retain(|spy, _| live.contains_key(spy));
    }
}

#[cfg(test)]
mod tests;
