//! `founding-voyage`: an embarked Settler of a seat that holds no city sails
//! to the edge of what it can see when no site is reachable, so the Māori
//! start finds land and founds its capital.
//!
//! **Why (live, both Māori seats).** 20261101T110607Z (King) and
//! 20261103T142545Z (Immortal): the starting Settler and Warrior stood
//! embarked on deep ocean with full movement from turn 1 to turn 6, no site was
//! ever named, and the controller reported the defeat "no cities remain" at
//! turn 6. At turn 1 the seat knew 37 plots, every one ocean.
//! - The mirror plans through fog with two priors: `assumed_traversable`,
//!   grown from revealed land and read by every domain, and
//!   `assumed_navigable`, grown from revealed water and read by ships alone
//!   (`Game::class_can_traverse`). With no revealed land there is no land
//!   prior, and an embarked Settler is not a ship, so every unknown plot is
//!   untraversable for it: no site, no frontier goal, nothing to do.
//! - `settler_frontier_step` (the stranded Settler's recovery) waits for
//!   `SETTLER_IDLE_PATIENCE` idle turns and looks only for unexplored land; the
//!   defeat lands first.
//! - The native screen reveals the whole board, so it founds from the same
//!   start at once.
//!
//! **What the gene does.** When the Settler step did not act, the seat holds
//! no city, and the Settler stands embarked, it steps along known water toward
//! the known plot at the edge of the fog that lies nearest the middle of the
//! map (ties: nearest the Settler). Each step reveals more of the board; once
//! land is in sight the ordinary site search takes over. Off, nothing changes.

use super::*;

/// How far, in tiles, the voyage looks for the edge of the known sea.
pub(super) const FOUNDING_VOYAGE_RADIUS: i32 = 12;

impl AdvancedAi {
    /// See the module. Returns whether the Settler moved.
    pub(super) fn founding_voyage_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if !self.founding_voyage || !g.player_city_ids(pid).is_empty() {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        if unit.owner != pid || unit.moves_left <= 0.0 || !g.is_embarked(unit) {
            return false;
        }
        let current = unit.pos;
        let centre = (g.map.width / 2, g.map.height / 2);
        let edge = g
            .wdisk(current, FOUNDING_VOYAGE_RADIUS)
            .into_iter()
            .filter(|pos| {
                *pos != current
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| !g.rules.is_unknown(tile))
                    && g.unit_can_traverse(uid, *pos)
                    // Fog is a plot the mirror marks unknown or does not
                    // carry at all (a fresh mirror holds only what is seen).
                    && g.nbrs(*pos).iter().any(|next| {
                        g.map
                            .get(*next)
                            .is_none_or(|tile| g.rules.is_unknown(tile))
                    })
            })
            .min_by_key(|pos| (g.wdist(*pos, centre), g.wdist(*pos, current), *pos));
        let Some(goal) = edge else {
            return false;
        };
        let goals: HashSet<Pos> = [goal].into_iter().collect();
        let Some(next) = g
            .route_step_to_any(uid, &goals)
            .filter(|next| g.can_move(uid, *next))
        else {
            return false;
        };
        if !self.base.path_move(g, pid, uid, next) {
            return false;
        }
        think!(self.journal(), Expansion, Detail, "Settler sails to find land";
               "the seat holds no city and no site is in sight, so it sails toward the \
                edge of the known sea at {goal:?}, nearest the middle of the map"; next);
        true
    }
}

#[cfg(test)]
mod tests;
