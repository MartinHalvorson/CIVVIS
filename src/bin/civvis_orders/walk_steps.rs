//! A one-order walk carries its steps across the bridge.
//!
//! The planner walks a unit two ways: hex by hex (`Action::Move`, one action a
//! step) or in one action (`Action::MoveTo`, a path the engine walks). Both
//! reach Firaxis as one `MOVE_TO` to the last hex: stepped walks are folded by
//! `coalesce_unit_paths_except`. But a stepped walk crosses the bridge with
//! its steps, and three things read them: the cut-back of a walk that ends on
//! a hex our own unit holds (`retarget_friendly_held_walks`), the first
//! unrevealed step kept as a probe (`first_unknown_coalesced_steps`), and the
//! step-by-step retry of a walk the host refused whole (`local_retry`). A
//! one-order walk had no steps, so none of them applied to it.
//!
//! Live civvis-20261115T120431Z, turns 34-38: the rush's survivors walked
//! home through a one-tile pass. One War-Cart stood parked in the pass at
//! (23,34). The next, at (22,32), was sent to (22,35) beyond it in one order
//! on four turns running and the host answered `move_no_path` each time; the
//! third, behind the second, was refused on three turns and went from 100 to
//! 57 health standing there. A stepped walk would have been retried a step
//! at a time and closed up behind the unit in the pass.
//!
//! The steps are not recomputed: the engine records every hop it walks in
//! `Game::unit_move_trails`, so the planned board already holds the path each
//! `MoveTo` took. A healthy walk still reaches the host as the one `MOVE_TO`
//! it always was, because the steps are folded again before they are sent.

use civvis::game::{Action, UnitMoveTrail};
use civvis::Pos;

/// Reads the planned board's move trails in step with the action log.
#[derive(Default)]
pub(super) struct Reader {
    /// How many of each unit's hops this turn earlier actions account for.
    consumed: std::collections::BTreeMap<u32, usize>,
}

impl Reader {
    /// The hexes `action` walked through before the hex it names, in order.
    /// Empty for anything but a `MoveTo` whose walk the trail shows reaching
    /// its hex in two hops or more: a single hop, a walk cut short, and a
    /// unit with no trail all leave the action as the one order it was.
    pub(super) fn steps_before(
        &mut self,
        trails: &[UnitMoveTrail],
        turn: u32,
        action: &Action,
    ) -> Vec<Pos> {
        let (unit, to, whole) = match action {
            Action::Move { unit, to } => (*unit, *to, false),
            Action::MoveTo { unit, to } => (*unit, *to, true),
            _ => return Vec::new(),
        };
        let hops: Vec<Pos> = trails
            .iter()
            .filter(|trail| trail.unit == unit && trail.turn == turn)
            .flat_map(|trail| trail.path.iter().skip(1).copied())
            .collect();
        let done = self.consumed.entry(unit).or_insert(0);
        if !whole {
            if hops.get(*done) == Some(&to) {
                *done += 1;
            }
            return Vec::new();
        }
        let Some(reached) = hops.iter().skip(*done).position(|hex| *hex == to) else {
            return Vec::new();
        };
        let steps = hops[*done..*done + reached].to_vec();
        *done += reached + 1;
        steps
    }
}

#[cfg(test)]
#[path = "walk_steps/tests.rs"]
mod tests;
