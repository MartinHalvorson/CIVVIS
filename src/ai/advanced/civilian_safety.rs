//! `civilian-out-of-reach`: settlers and builders stay out of a barbarian's
//! reach, flee it, and are stacked with a guard when they cannot.
//!
//! Opt-in (`Kind::OptIn` in `genes.rs`), ships off, priced by `gene_screen`
//! beside every other gene — see `docs/CIVILIAN_SAFETY.md`. Its own file
//! because `src/ai/advanced.rs` is the most contended file in the
//! repository.
//!
//! Three engine facts make the rule exact, and the shipped answers weak:
//!
//! - **Capture is a move.** `resolve_entered_units` transfers a Settler or
//!   Builder to whichever military unit steps onto its tile; a civilian
//!   cannot be attacked at all. The tiles a raider can *end its next move
//!   on* — `Game::threat_reach`, its full allowance, through blockers — are
//!   therefore exactly the tiles a civilian must not stand on alone.
//! - **Barbarians move after every major in the same world turn**, so a
//!   civilian left on such a tile at the end of our turn is taken before it
//!   moves again. "One turn away" means "inside `threat_reach` right now".
//! - **A stacked civilian cannot be captured**: `can_enter_past` refuses the
//!   raider a tile holding a friendly military unit. A guard on the tile is
//!   protection outright; a guard beside it is not.
//!
//! What ships today in a native game prices a Settler with a geometric disk
//! (`wdist ≤ moves.ceil()`, `settlement_tile_risk`) under the *military*
//! model — the capture tiers and the retreat block are gated on the
//! host-only `live_formationless_settler_shadow` — as a soft score a single
//! on-tile guard discounts under the limit, and the stacked-guard system is
//! host-only too. The Builder path (`builder-barbarian-safety`, off) already
//! reads the exact flood but credits no escort and never flees off a job.
//!
//! With this gene on, for every Settler and Builder, every turn:
//!
//! 1. **Flee first.** A civilian standing inside the reach with no guard on
//!    its tile moves before anything else — to the reachable tile outside
//!    the reach that keeps the most progress toward its goal, else the tile
//!    fewest raiders can reach and the farthest from the nearest.
//! 2. **Never step into reach.** The next route step is refused when it is
//!    inside the reach and unguarded; a safe neighbour that still makes
//!    progress is taken instead, otherwise the civilian holds where it is
//!    safe. The doorstep exception: a Settler may enter its own site inside
//!    the reach when it keeps movement to found the city this same turn —
//!    the city is there before the raider moves.
//! 3. **Summon the guard.** A threatened Settler that cannot stay out of the
//!    reach calls the nearest healthy land military unit that can reach its
//!    tile this turn, binds it (`settler_guards`), and the pair walks
//!    together: the settler pulls its stacked guard along with every step,
//!    and the guard's own turn (`stacked_guard_step`) keeps it on the
//!    settler's tile. The bond is released when no raider is within
//!    `SETTLER_ESCORT_THREAT_RADIUS` or the settler is gone.
//!
//! Visibility follows every other civilian-risk path: raiders inside the
//! turn-start vision frame (`battlefront_visibility`), never through fog.
//! The live seat only ever exports what it sees, so this is also what the
//! bridge can act on.
//!
//! Off, every touched path is unchanged.

use super::{AdvancedAi, SETTLER_ESCORT_THREAT_RADIUS, STACKED_GUARD_MIN_HP};
use crate::game::{Action, Game};
use crate::reasoning::plain;
use crate::think;
use crate::Pos;

/// Raiders farther than this many times their movement allowance from a
/// civilian cannot reach it even on roads (a road step costs at least a
/// quarter move); everything nearer is flooded exactly.
const REACH_SCAN_MOVE_MULTIPLE: f64 = 2.0;
/// Jobs and sites are priced against raiders this far around the civilian
/// — the raid leash is ten tiles and a Horseman moves four.
pub(super) const REACH_SCAN_RADIUS: i32 = 10;

/// One known raider and the tiles it could end its next move on.
struct Raider {
    pos: Pos,
    sea: bool,
    /// Sorted for binary search.
    reach: Vec<Pos>,
}

/// Everything a known barbarian raider could take next turn, around one
/// civilian.
pub(super) struct BarbarianReach {
    raiders: Vec<Raider>,
}

impl BarbarianReach {
    pub(super) fn is_empty(&self) -> bool {
        self.raiders.is_empty()
    }

    /// Could a raider stand on `pos` next turn? A ship takes a coastal land
    /// tile from the water beside it, as `barbarian_capture_reaches` prices
    /// the Builder's job.
    pub(super) fn covers(&self, g: &Game, pos: Pos) -> bool {
        self.raiders_covering(g, pos) > 0
    }

    fn raiders_covering(&self, g: &Game, pos: Pos) -> usize {
        self.raiders
            .iter()
            .filter(|raider| {
                if !raider.sea {
                    return raider.pos == pos || raider.reach.binary_search(&pos).is_ok();
                }
                g.nbrs(pos).into_iter().any(|neighbour| {
                    g.map
                        .get(neighbour)
                        .is_some_and(|tile| g.rules.is_water(tile))
                        && raider.reach.binary_search(&neighbour).is_ok()
                })
            })
            .count()
    }

    /// Hex distance to the nearest raider, `i32::MAX` when there is none.
    pub(super) fn nearest(&self, g: &Game, pos: Pos) -> i32 {
        self.raiders
            .iter()
            .map(|raider| g.wdist(raider.pos, pos))
            .min()
            .unwrap_or(i32::MAX)
    }
}

impl AdvancedAi {
    /// The reach of every visible barbarian raider that could matter to a
    /// civilian at `around`: raiders within `radius`, each flooded with its
    /// full allowance through blockers (`threat_reach`). Recon units are
    /// engine-managed scouts, not raiders — see `barbarian_scouts_are_scouts`.
    pub(super) fn barbarian_reach(
        &self,
        g: &Game,
        pid: usize,
        around: Pos,
        radius: i32,
    ) -> BarbarianReach {
        let mut raiders = Vec::new();
        let Some(barbarian) = g.barb_pid else {
            return BarbarianReach { raiders };
        };
        if !g.is_at_war(pid, barbarian) {
            return BarbarianReach { raiders };
        }
        let visible = self.battlefront_visibility(g, pid);
        for unit in g.units.values() {
            if unit.owner != barbarian
                || g.wdist(unit.pos, around) > radius
                || !g.sees(&visible, unit.pos)
                || !g.unit_visible_to(unit.id, pid)
            {
                continue;
            }
            let spec = &g.rules.units[unit.kind];
            let domain = spec.domain.as_deref();
            if spec.class != "military" || domain == Some("air") || spec.promotion_class == "recon"
            {
                continue;
            }
            let farthest = (g.unit_max_moves(unit.id) * REACH_SCAN_MOVE_MULTIPLE).ceil() as i32 + 1;
            if g.wdist(unit.pos, around) > farthest.max(1) + 1 {
                continue;
            }
            let mut reach = g.threat_reach(unit.id);
            reach.sort_unstable();
            reach.dedup();
            raiders.push(Raider {
                pos: unit.pos,
                sea: domain == Some("sea"),
                reach,
            });
        }
        BarbarianReach { raiders }
    }

    /// A civilian on `pos` is safe from capture there: nothing can reach
    /// it, or one of our military units shares the tile, or it is inside
    /// one of our cities.
    pub(super) fn civilian_safe_at(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        pos: Pos,
        reach: &BarbarianReach,
    ) -> bool {
        !reach.covers(g, pos) || self.civilian_guarded_at(g, pid, uid, pos)
    }

    /// Whether the Settler's bound guard really protects `pos` this turn.
    ///
    /// A body physically stacked with a civilian stops a capture only while
    /// it can survive the hostile phase.  The default `settler-guard-holds`
    /// policy already gives that fact one definition: a bound, healthy guard
    /// that is not outmatched.  Do not let the earlier civilian-reach pass
    /// accidentally credit the same wounded guard as an unconditional shield
    /// and skip the retreat that definition requires.
    fn bound_guard_protects_settler_at(
        &self,
        g: &Game,
        pid: usize,
        settler: u32,
        pos: Pos,
    ) -> bool {
        let Some(guard) = self.settler_guards.get(&settler).copied() else {
            return false;
        };
        let Some(unit) = g.units.get(&guard) else {
            return false;
        };
        if unit.owner != pid
            || unit.pos != pos
            || g.rules.units[unit.kind].class != "military"
            || matches!(g.rules.units[unit.kind].domain.as_deref(), Some("sea" | "air"))
        {
            return false;
        }
        if !self.settler_guard_holds_on() {
            return true;
        }
        unit.hp >= STACKED_GUARD_MIN_HP
            && !self.guard_outmatched_at(
                g,
                pid,
                unit,
                pos,
                &self.battlefront_visibility(g, pid),
            )
    }

    fn civilian_guarded_at(&self, g: &Game, pid: usize, uid: u32, pos: Pos) -> bool {
        if g
            .city_at(pos)
            .is_some_and(|city| g.cities[&city].owner == pid)
        {
            return true;
        }
        // The formationless live Settler repair deliberately binds the one
        // military unit whose turn is reserved to the Settler.  Under that
        // default policy, an arbitrary bystander is not a safe excuse to
        // suppress an emergency retreat, and neither is the bound guard once
        // it has dropped below the shared survival floor.
        if g.units.get(&uid).is_some_and(|unit| unit.kind == "settler")
            && self.settler_guard_holds_on()
        {
            return self.bound_guard_protects_settler_at(g, pid, uid, pos);
        }
        g.unit_ids_at(pos).iter().any(|other| {
            *other != uid
                && g.units.get(other).is_some_and(|unit| {
                    unit.owner == pid && g.rules.units[unit.kind].class == "military"
                })
        })
    }

    /// The tile a civilian is trying to reach: its site or job, else the
    /// nearest of our cities.
    fn civilian_goal(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let unit = &g.units[&uid];
        let planned = match unit.kind.as_str() {
            "settler" => self.settler_targets.get(&uid).copied(),
            "builder" => self.builder_targets.get(&uid).copied(),
            _ => None,
        };
        planned.or_else(|| {
            g.player_city_ids(pid)
                .into_iter()
                .map(|city| g.cities[&city].pos)
                .min_by_key(|pos| (g.wdist(*pos, unit.pos), *pos))
        })
    }

    /// Rule 1: a civilian standing inside the reach, unguarded, moves before
    /// anything else. `Some(moved)` when the rule decided the unit's turn;
    /// `None` when it stands safe (or on a site it can found right now).
    pub(super) fn civilian_flee_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        let current = g.units[&uid].pos;
        let kind = g.units[&uid].kind;
        let reach = self.barbarian_reach(g, pid, current, REACH_SCAN_RADIUS);
        if reach.is_empty() || self.civilian_safe_at(g, pid, uid, current, &reach) {
            return None;
        }
        // A settler on its own site founds: the city is protection, and it
        // is what the walk was for.
        if kind == "settler"
            && self.settler_targets.get(&uid) == Some(&current)
            && g.can_found_city(uid)
        {
            return None;
        }
        // Rule 3 before rule 1: a guard that can stand on this tile now makes
        // it safe without giving up the ground.
        if kind == "settler" && self.summon_guard_to(g, pid, uid, current) {
            return None;
        }
        let goal = self.civilian_goal(g, pid, uid);
        let here_covering = reach.raiders_covering(g, current);
        let here_nearest = reach.nearest(g, current);
        let mut options: Vec<(bool, usize, i32, i32, Pos)> = g
            .reachable(uid)
            .into_iter()
            // `reachable` contains full-turn destinations. `can_move` asks
            // whether a position is one immediate step away, so applying it
            // here silently drops the two-step safe escape this emergency
            // path exists to use. Re-check the exact route the `MoveTo`
            // order will follow instead.
            .filter(|pos| *pos != current && g.path_to(uid, *pos).is_some())
            .map(|pos| {
                let safe = self.civilian_safe_at(g, pid, uid, pos, &reach);
                let covering = reach.raiders_covering(g, pos);
                let progress = goal.map_or(0, |goal| g.wdist(pos, goal));
                (safe, covering, progress, -reach.nearest(g, pos), pos)
            })
            .collect();
        // Safe tiles first; among them the one nearest the goal, then the
        // farthest from any raider. Failing a safe tile, the fewest raiders
        // reaching it, then distance from them — and only when that beats
        // standing still.
        options.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| if a.0 { a.2.cmp(&b.2) } else { a.3.cmp(&b.3) })
                .then_with(|| a.3.cmp(&b.3))
                .then_with(|| a.4.cmp(&b.4))
        });
        for (safe, covering, _, neg_nearest, pos) in options {
            if !safe
                && (covering > here_covering
                    || (covering == here_covering && -neg_nearest <= here_nearest))
            {
                break;
            }
            if self.base.path_move(g, pid, uid, pos) {
                think!(self.journal(), Expansion, Detail, "{} flees a barbarian's reach", plain(&kind);
                       "{} raider(s) could take {current:?} next turn; {pos:?} is {}",
                       here_covering,
                       if safe { "out of reach" } else { "the least exposed tile it can reach" }; pos);
                return Some(true);
            }
        }
        think!(self.journal(), Expansion, Detail, "{} holds inside a barbarian's reach", plain(&kind);
               "{} raider(s) could take {current:?} and no reachable tile is better", here_covering; current);
        Some(false)
    }

    /// Rule 3: bring the nearest healthy land military unit that can reach
    /// `pos` this turn onto it and bind it to the settler. `true` when a
    /// guard now shares the tile.
    fn summon_guard_to(&mut self, g: &mut Game, pid: usize, settler: u32, pos: Pos) -> bool {
        if self.bound_guard_protects_settler_at(g, pid, settler, pos) {
            return true;
        }
        let bound: Vec<u32> = self.settler_guards.values().copied().collect();
        let mut candidates: Vec<(i32, i32, u32)> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && unit.hp >= STACKED_GUARD_MIN_HP
                    && unit.linked_to.is_none()
                    && (!bound.contains(uid) || self.settler_guards.get(&settler) == Some(uid))
                    && (unit.pos == pos
                        || (unit.moves_left > 0.0 && g.reachable(*uid).contains(&pos)))
            })
            .map(|uid| {
                let unit = &g.units[&uid];
                (
                    g.wdist(unit.pos, pos),
                    -(g.unit_strength(unit, false) as i32),
                    uid,
                )
            })
            .collect();
        candidates.sort_unstable();
        for (_, _, guard) in candidates {
            // A routed order, not the adjacent step `path_move` issues: the
            // guard may be several tiles out. A guard that gets under way
            // but not all the way is bound too — its own step
            // (`stacked_guard_step`) finishes the walk.
            let before = g.units[&guard].pos;
            let arrived = before == pos
                || (g
                    .apply(
                        pid,
                        &Action::MoveTo {
                            unit: guard,
                            to: pos,
                        },
                    )
                    .is_ok()
                    && g.units[&guard].pos == pos);
            let under_way = arrived || g.units[&guard].pos != before;
            if arrived || under_way {
                self.settler_guards.insert(settler, guard);
                self.guard_wait.remove(&settler);
                think!(self.journal(), Expansion, Detail, "A guard is called to the settler";
                       "sharing the tile blocks capture outright"; pos);
                return arrived;
            }
        }
        false
    }

    /// Release a settler's guard when nothing threatens the walk any more,
    /// so the army gets its unit back.
    fn release_guard_if_quiet(
        &mut self,
        g: &Game,
        pid: usize,
        settler: u32,
        reach: &BarbarianReach,
    ) {
        if !self.settler_guards.contains_key(&settler) {
            return;
        }
        let pos = g.units[&settler].pos;
        let quiet = reach.nearest(g, pos) > SETTLER_ESCORT_THREAT_RADIUS
            && self
                .barbarian_reach(g, pid, pos, SETTLER_ESCORT_THREAT_RADIUS)
                .is_empty();
        if quiet {
            self.settler_guards.remove(&settler);
            self.guard_wait.remove(&settler);
        }
    }

    /// The stacked guard follows the settler onto `to` when it still has
    /// the movement; a settler never leaves its guard behind on purpose.
    fn pull_guard_along(&mut self, g: &mut Game, pid: usize, settler: u32, from: Pos, to: Pos) {
        let Some(guard) = self.settler_guards.get(&settler).copied() else {
            return;
        };
        let can_follow = g
            .units
            .get(&guard)
            .is_some_and(|unit| unit.owner == pid && unit.pos == from && unit.moves_left > 0.0);
        if can_follow && g.reachable(guard).contains(&to) {
            self.base.path_move(g, pid, guard, to);
        }
    }

    /// Could the bound guard, stacked on `from`, stand on `to` with the
    /// settler after this step?
    fn guard_can_follow(&self, g: &Game, pid: usize, settler: u32, from: Pos, to: Pos) -> bool {
        self.settler_guards.get(&settler).is_some_and(|guard| {
            g.units.get(guard).is_some_and(|unit| {
                unit.owner == pid
                    && unit.pos == from
                    && unit.moves_left > 0.0
                    && g.reachable(*guard).contains(&to)
            })
        })
    }

    /// Rule 2 for a Settler's march: the route step is refused inside the
    /// reach unless a guard walks in with it or the site is founded this
    /// turn; a safe neighbour that still makes progress is taken instead;
    /// otherwise the settler holds on safe ground. Returns whether it moved.
    pub(super) fn settler_step_out_of_reach(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        let current = g.units[&uid].pos;
        let reach = self.barbarian_reach(g, pid, current, REACH_SCAN_RADIUS);
        self.release_guard_if_quiet(g, pid, uid, &reach);
        if reach.is_empty() {
            return self.settler_step_toward_safe(g, pid, uid, target);
        }
        let Some(next) = g
            .route_step(uid, target, 0)
            .filter(|next| g.can_move(uid, *next))
        else {
            return self.settler_step_toward_safe(g, pid, uid, target);
        };
        let founds_on_arrival =
            next == target && g.units[&uid].moves_left - g.step_cost_for(uid, current, next) > 1e-9;
        let next_safe = self.civilian_safe_at(g, pid, uid, next, &reach)
            || self.guard_can_follow(g, pid, uid, current, next)
            || founds_on_arrival;
        if next_safe {
            let moved = self.settler_step_toward_safe(g, pid, uid, target);
            if moved {
                let now = g.units[&uid].pos;
                if now != current {
                    self.pull_guard_along(g, pid, uid, current, now);
                }
            }
            return moved;
        }
        // The route enters the reach alone: bring a guard onto this tile and
        // walk in together, else sidestep, else hold.
        if self.summon_guard_to(g, pid, uid, current)
            && self.guard_can_follow(g, pid, uid, current, next)
        {
            let moved = self.settler_step_toward_safe(g, pid, uid, target);
            if moved {
                let now = g.units[&uid].pos;
                if now != current {
                    self.pull_guard_along(g, pid, uid, current, now);
                }
            }
            return moved;
        }
        let current_distance = g.wdist(current, target);
        let mut sidesteps: Vec<(i32, i32, Pos)> = g
            .nbrs(current)
            .into_iter()
            .filter(|pos| {
                *pos != next
                    && g.map.get(*pos).is_some()
                    && g.can_move(uid, *pos)
                    && self.civilian_safe_at(g, pid, uid, *pos, &reach)
            })
            .map(|pos| {
                (
                    g.wdist(pos, target) - current_distance,
                    -reach.nearest(g, pos),
                    pos,
                )
            })
            .filter(|(regress, _, _)| *regress <= 0)
            .collect();
        sidesteps.sort_unstable();
        for (_, _, pos) in sidesteps {
            if self.base.path_move(g, pid, uid, pos) {
                think!(self.journal(), Expansion, Detail, "Settler sidesteps a barbarian's reach";
                       "{next:?} could be taken next turn; {pos:?} keeps out of it"; pos);
                return true;
            }
        }
        think!(self.journal(), Expansion, Detail, "Settler waits outside a barbarian's reach";
               "{next:?} could be taken next turn and no safe step makes progress"; current);
        false
    }

    /// Rule 2 for a Builder: the route step is refused inside the reach; a
    /// safe neighbour that still makes progress is taken; else it holds.
    pub(super) fn builder_step_out_of_reach(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        let current = g.units[&uid].pos;
        let reach = self.barbarian_reach(g, pid, current, REACH_SCAN_RADIUS);
        if reach.is_empty() {
            return self.builder_step_toward_barbarian_safe(g, pid, uid, target);
        }
        let Some(next) = g
            .route_step(uid, target, 0)
            .filter(|next| g.can_move(uid, *next))
        else {
            return self.builder_step_toward_barbarian_safe(g, pid, uid, target);
        };
        if self.civilian_safe_at(g, pid, uid, next, &reach) {
            return self.builder_step_toward_barbarian_safe(g, pid, uid, target);
        }
        let current_distance = g.wdist(current, target);
        let mut sidesteps: Vec<(i32, i32, Pos)> = g
            .nbrs(current)
            .into_iter()
            .filter(|pos| {
                *pos != next
                    && g.map.get(*pos).is_some()
                    && g.can_move(uid, *pos)
                    && self.civilian_safe_at(g, pid, uid, *pos, &reach)
            })
            .map(|pos| {
                (
                    g.wdist(pos, target) - current_distance,
                    -reach.nearest(g, pos),
                    pos,
                )
            })
            .filter(|(regress, _, _)| *regress < 0)
            .collect();
        sidesteps.sort_unstable();
        for (_, _, pos) in sidesteps {
            if self.base.path_move(g, pid, uid, pos) {
                return true;
            }
        }
        think!(self.journal(), Expansion, Detail, "Builder waits outside a barbarian's reach";
               "{next:?} could be taken next turn"; current);
        false
    }

    /// Job filter for the Builder's sweep: a job tile a raider could stand
    /// on next turn is not a job today.
    pub(super) fn builder_job_out_of_reach(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        pos: Pos,
        reach: &BarbarianReach,
    ) -> bool {
        self.civilian_safe_at(g, pid, uid, pos, reach)
    }
}
