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
//!
//! ## `founding-voyage-2`: the escort sails with it
//!
//! **Why (hotseat `live-6p-20261128T0446Zd`, seat p1, Kupe, 2026-11-28).**
//! The voyage was armed nowhere, so a CIVVIS seat that drew Kupe stood its
//! embarked Settler (39,22) and Warrior (38,22) on deep ocean, 4 of 4 moves,
//! turns 1 to 6, with only research and civics ordered (`hotseat.log`: "0
//! opening orders applied ... cities 0, units 2" on turns 2-4). The seat's
//! mirror knew 37 plots, every one ocean (`tiles`, turn 1). Replayed with the
//! CI-built decider of #6020 on that board (`--turn 1..4 --resumed`), version
//! one sails the Settler four plots east to (42,22) every turn and leaves the
//! Warrior where it stands, holding "for the reserve" on its own tile: the
//! Settler reaches land alone, on a turn when the guard is four plots back.
//!
//! **What version two adds.** Everything version one does, and after each
//! voyage step the Settler's soldier at sea sails after it: its bound guard,
//! or else the nearest soldier of ours (no ship, no plane) that stands
//! embarked within [`VOYAGE_ESCORT_REACH`] plots of where the Settler stood,
//! steps toward the Settler's new plot until it shares it or runs out of
//! movement. A Settler and a soldier share a plot (two stacking layers). The
//! landfall and the founding are the ordinary site search's, as in version
//! one. Off, nothing changes; with version one alone, nothing of this runs.
//! Grep the journal for `founding-voyage-2:`.

use super::*;

/// How far, in tiles, the voyage looks for the edge of the known sea.
pub(super) const FOUNDING_VOYAGE_RADIUS: i32 = 12;

/// `founding-voyage-2`: how near the Settler's plot, before its step, a
/// soldier at sea stands to sail with it. Kupe's Warrior starts beside it.
pub(super) const VOYAGE_ESCORT_REACH: i32 = 2;

impl AdvancedAi {
    /// See the module. Returns whether the Settler moved.
    pub(super) fn founding_voyage_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if !(self.founding_voyage || self.founding_voyage_2) || !g.player_city_ids(pid).is_empty() {
            return false;
        }
        let from = g.units.get(&uid).map(|unit| unit.pos);
        let sailed = self.voyage_step(g, pid, uid);
        // `founding-voyage-2`: the escort sails after the Settler.
        if sailed && self.founding_voyage_2 {
            if let Some(from) = from {
                self.voyage_escort_follows(g, pid, uid, from);
            }
        }
        sailed
    }

    /// Version one's step: see the module.
    fn voyage_step(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
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

    /// `founding-voyage-2`: the soldier that sails with the Settler, read
    /// before its step from `from`: its bound guard, else the nearest soldier
    /// of ours at sea within [`VOYAGE_ESCORT_REACH`] with movement left.
    fn voyage_escort(&self, g: &Game, pid: usize, settler: u32, from: Pos) -> Option<u32> {
        let at_sea_soldier = |uid: u32| {
            g.units.get(&uid).is_some_and(|unit| {
                let spec = &g.rules.units[unit.kind];
                uid != settler
                    && unit.owner == pid
                    && spec.class == "military"
                    && spec.strength > 0.0
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && unit.moves_left > 0.0
                    && g.is_embarked(unit)
            })
        };
        if let Some(guard) = self
            .settler_guards
            .get(&settler)
            .copied()
            .filter(|guard| at_sea_soldier(*guard))
        {
            return Some(guard);
        }
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                at_sea_soldier(*uid) && g.wdist(g.units[uid].pos, from) <= VOYAGE_ESCORT_REACH
            })
            .min_by_key(|uid| (g.wdist(g.units[uid].pos, from), *uid))
    }

    /// `founding-voyage-2`: step the escort toward the Settler's plot until it
    /// shares it or has no movement left.
    fn voyage_escort_follows(&self, g: &mut Game, pid: usize, settler: u32, from: Pos) {
        let Some(escort) = self.voyage_escort(g, pid, settler, from) else {
            return;
        };
        let Some(to) = g.units.get(&settler).map(|unit| unit.pos) else {
            return;
        };
        let goals: HashSet<Pos> = [to].into_iter().collect();
        let mut stepped = 0;
        while let Some(unit) = g.units.get(&escort) {
            if unit.pos == to || unit.moves_left <= 0.0 || stepped >= 8 {
                break;
            }
            let Some(next) = g
                .route_step_to_any(escort, &goals)
                .filter(|next| g.can_move(escort, *next))
            else {
                break;
            };
            if !self.base.path_move(g, pid, escort, next) {
                break;
            }
            stepped += 1;
        }
        if stepped > 0 {
            let kind = plain(g.units[&escort].kind.as_str());
            let at = g.units[&escort].pos;
            think!(self.journal(), Expansion, Detail, "The {kind} sails with the Settler";
                   "founding-voyage-2: the Settler sailed from {from:?} to {to:?}; its escort \
                    follows to {at:?}"; at);
        }
    }
}

#[cfg(test)]
mod tests;
