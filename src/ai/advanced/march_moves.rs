//! `march-uses-its-moves`: a staging or reinforcement march walks as far as
//! this turn's movement carries it toward the ring.
//!
//! Every march toward a siege ring took the router's single step: the
//! breadth-first router prices every tile at one, so the step it names is the
//! first edge of the shortest route in tiles, and a two-movement unit that
//! steps onto a hill, into a forest or across a river has spent its turn on
//! one tile while an open tile beside it led just as near. A flat step leaves
//! movement, and the unit's next decision then had to come back to the same
//! march; on the frame-0 replay of live King civvis-20261005T045443Z, 222 of
//! 400 staging unit-turns took exactly one step, 121 of them because the step
//! used the unit's whole movement and 86 because the next decision went to
//! another errand or held the unit at the Reserve. Live frame-0 MOVE_TO
//! orders were a single tile 47-62% of the time, and members of an assigned
//! siege force sat 15-25 tiles out for 20-60 turns.
//!
//! Under the gene the march reads every tile the unit can stop on this turn
//! on the router's own reverse distance field and walks the whole path, in
//! one `MoveTo`, to the one nearest the ring, the most movement left breaking
//! ties. The router's step stands wherever no tile beats it, and every gate
//! the caller already applies to its step (strike range, danger, a rival's
//! territory) applies to the destination and, where the caller says so, to
//! the path.

use std::collections::HashSet;

use super::AdvancedAi;
use crate::game::Game;
use crate::Pos;

/// How many of the best-placed reachable tiles the march vets before it
/// keeps the router's single step.
const MARCH_CANDIDATES: usize = 8;

impl AdvancedAi {
    /// The tile this turn's movement reaches nearest `goals` that beats the
    /// router's single `step`: fewer route steps from the goals, or as few
    /// with more movement left. `accept` vets each candidate's destination
    /// and the path the unit would walk to it, best first. `None` with the
    /// gene off, and wherever the step is already the best tile.
    pub(super) fn march_destination(
        &self,
        g: &Game,
        uid: u32,
        goals: &HashSet<Pos>,
        step: Pos,
        mut accept: impl FnMut(&Game, Pos, &[Pos]) -> bool,
    ) -> Option<Pos> {
        if !self.march_uses_its_moves {
            return None;
        }
        let (_, reach) = g.march_reach_toward_any(uid, goals)?;
        let (step_steps, step_left) = reach
            .iter()
            .find(|(pos, _, _)| *pos == step)
            .map(|(_, steps, left)| (*steps, *left))?;
        for (pos, steps, left) in reach.iter().take(MARCH_CANDIDATES) {
            if *pos == step {
                return None;
            }
            let beats = *steps < step_steps || (*steps == step_steps && *left > step_left + 1e-9);
            if !beats {
                return None;
            }
            if g.rules.is_water(&g.map.tiles[pos]) {
                continue;
            }
            let Some(path) = g.path_to(uid, *pos) else {
                continue;
            };
            if accept(g, *pos, &path) {
                return Some(*pos);
            }
        }
        None
    }
}
