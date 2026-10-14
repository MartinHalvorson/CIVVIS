//! `stalled-settler-takes-a-safe-site`: a Settler that has spent too long
//! out of a city takes the best legal site a few tiles away whose ground and
//! first step are out of every visible hostile's reach, instead of holding
//! for a threatened site it keeps being refused.
//!
//! ## What the live runs say (forensic of 2026-10-07)
//!
//! Emperor, 118 live runs of 10-06/07: the median run holds one Settler at
//! turn 100, but 27 hold two or more and 12 hold three or more, and 15 of
//! the finished runs held two or more at BOTH turn 80 and turn 100 (the
//! empire median is 8 cities at turn 100). Read walker by walker on those
//! 15 — a walker is a Settler alive 20 turns or more, a still turn one on
//! which a walker did not move — the journal lines written on still turns
//! name the dominant hold in each run:
//!
//! | dominant hold on still turns | runs |
//! |---|---:|
//! | "HELD short … the safe-step guard rejected every neighbour" / "waits outside a barbarian's reach" | 8 |
//! | a Loyalty refusal of the site | 4 |
//! | waiting for a guard | 2 |
//! | "the next tile refuses it" (a zone of control) or our own unit on it | 1 |
//!
//! Live `civvis-20261007T132739Z` (game 355) is the shape of it: three
//! cities at turn 100 with three Settlers alive. One left the capital at
//! turn 51 and was still walking at 130 — 76 moves within four tiles of
//! its two cities, back onto a city tile fourteen times — cycling targets
//! seven to ten tiles out that a Gaulish army and barbarians kept under
//! threat ("Settler marching to (28, 34) | 9 tiles away, the site is worth
//! -52.5", the negative being the site's own safety penalty). The stall
//! counter retires each target after `SETTLER_STALL_LIMIT` turns and the
//! next pick is just as exposed; `founds_where_it_stands` cannot found
//! within three tiles of a city; and `settler-walk-deadline`'s two-tile
//! search near the cities finds no legal plot. Nothing ends the walk.
//!
//! ## What the gene does
//!
//! It shares the walk clock `settler-walk-deadline` keeps, so it keeps that
//! gene's two exclusions: a turn on an own city tile or embarked is not a
//! turn out, and an embarked Settler is left to its crossing. Once a
//! Settler has spent [`STALLED_SETTLER_WALK_STANDARD`] standard turns out
//! (fifteen on Online, past the deadline's twelve), it takes the best legal
//! site within [`STALLED_SETTLER_RADIUS`] tiles under every guard the
//! deadline and the ordinary founding keep (`settler_legal_sites_within`),
//! but only a site whose own ground is under the step-risk limit and whose
//! route's first step is too, so the safe-step guard that held it does not
//! hold the new walk at once. Sites are ranked by worth less
//! [`STALLED_SETTLER_TILE_PRICE`] a tile of route; it founds at once when
//! the best is underfoot. When nothing qualifies, the ordinary step runs
//! untouched. Off, every path is byte-identical.
//!
//! It also keeps the deadline's value floor, the test whose absence cost
//! that gene's first fires probe 0.56 cities a seat and 18 wins in a
//! hundred by packing cities beside the capital: a site must be worth, net
//! of the deadline's step margin, at least
//! `SETTLER_WALK_DEADLINE_VALUE_SHARE` of the site the Settler was walking
//! to. The floor is read once, at the takeover, and frozen
//! (`stalled_settler_floor`), so the gene's own pick never becomes its
//! floor and a retarget cannot lower it. A Settler with no plan has no
//! floor, as at the deadline.

use super::settler_walk_deadline::{
    SETTLER_WALK_DEADLINE_STEP_MARGIN, SETTLER_WALK_DEADLINE_VALUE_SHARE,
};
use super::{AdvancedAi, SETTLER_STEP_RISK_LIMIT};
use crate::game::{Action, Game};
use crate::think;
use crate::Pos;

/// Standard turns out of a city before a stalled Settler stops chasing its
/// ranked site: fifteen on Online speed, three past `settler-walk-deadline`
/// so the deadline's nearer founding is asked first.
pub const STALLED_SETTLER_WALK_STANDARD: u32 = 30;

/// How far a stalled Settler looks: two turns of a Settler's walk on open
/// ground, far enough to clear the three-tile spacing around the cities it
/// keeps falling back to.
pub const STALLED_SETTLER_RADIUS: i32 = 4;

/// Worth charged per tile of route to a site, so a nearer site wins a close
/// call.
pub const STALLED_SETTLER_TILE_PRICE: f64 = 2.0;

impl AdvancedAi {
    /// The value floor of this Settler's plan as `settler-walk-deadline`
    /// reads it: half the worth of the site it is walking to, `None` with no
    /// site. The frozen floor wins once the gene has taken over.
    pub(super) fn stalled_settler_plan_floor(&self, g: &Game, pid: usize, uid: u32) -> Option<f64> {
        if let Some(frozen) = self.stalled_settler_floor.get(&uid) {
            return *frozen;
        }
        self.settler_targets
            .get(&uid)
            .map(|target| self.settle_value(g, pid, *target) * SETTLER_WALK_DEADLINE_VALUE_SHARE)
    }

    /// Whether `pos` clears the plan's floor by the deadline's own test: its
    /// worth, less the deadline's step margin when it is not underfoot, at
    /// least the floor.
    pub(super) fn stalled_settler_clears_floor(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        pos: Pos,
    ) -> bool {
        let Some(floor) = self.stalled_settler_plan_floor(g, pid, uid) else {
            return true;
        };
        let margin = if pos == g.units[&uid].pos {
            0.0
        } else {
            SETTLER_WALK_DEADLINE_STEP_MARGIN
        };
        self.settle_value(g, pid, pos) - margin >= floor
    }

    /// The best legal site within [`STALLED_SETTLER_RADIUS`] whose ground and
    /// first step are under the step-risk limit and that clears the plan's
    /// value floor, and what it is worth net of the walk. `None` when nothing
    /// qualifies.
    pub(super) fn stalled_settler_site(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
    ) -> Option<(Pos, f64)> {
        let here = g.units[&uid].pos;
        let visible = self.battlefront_visibility(g, pid);
        self.settler_legal_sites_within(g, pid, uid, STALLED_SETTLER_RADIUS)
            .into_iter()
            .filter(|pos| self.stalled_settler_clears_floor(g, pid, uid, *pos))
            .filter_map(|pos| {
                if pos == here {
                    if !g.can_found_city(uid) {
                        return None;
                    }
                } else {
                    let step = g.route_step(uid, pos, 0)?;
                    if !g.can_move(uid, step)
                        || self.settlement_tile_risk(g, pid, Some(uid), step, &visible)
                            > SETTLER_STEP_RISK_LIMIT
                    {
                        return None;
                    }
                }
                if self.settlement_tile_risk(g, pid, Some(uid), pos, &visible)
                    > SETTLER_STEP_RISK_LIMIT
                {
                    return None;
                }
                let walk = if pos == here {
                    0
                } else {
                    g.route_distance(uid, pos, 0)
                        .map_or(g.wdist(here, pos), |steps| steps as i32)
                };
                if walk > STALLED_SETTLER_RADIUS + 1 {
                    return None;
                }
                Some((
                    pos,
                    self.settle_value(g, pid, pos) - STALLED_SETTLER_TILE_PRICE * walk as f64,
                ))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    /// The stalled walk's turn, when it has come: `Some(acted)` when the
    /// Settler founded or stepped toward a safe site within reach, `None`
    /// when it is still inside its walk allowance, embarked, or nothing
    /// within reach qualifies — the ordinary step then runs untouched.
    pub(super) fn stalled_settler_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.stalled_settler_takes_a_safe_site {
            return None;
        }
        let (_, out) = self.note_settler_walk(g, pid, uid);
        let allowance = g.standard_duration(STALLED_SETTLER_WALK_STANDARD).max(1);
        if out < allowance || g.is_embarked(&g.units[&uid]) {
            return None;
        }
        // The takeover: freeze the plan's floor before any pick of ours
        // replaces the plan.
        if !self.stalled_settler_floor.contains_key(&uid) {
            let floor = self.stalled_settler_plan_floor(g, pid, uid);
            self.stalled_settler_floor.insert(uid, floor);
        }
        let here = g.units[&uid].pos;
        let (site, worth) = self.stalled_settler_site(g, pid, uid)?;
        if site == here {
            return Some(self.found_stalled_settler(g, pid, uid, out, allowance, worth));
        }
        if self.settler_targets.get(&uid) != Some(&site) {
            think!(self.journal(), Expansion, Detail,
                   "Stalled settler takes the safe site at {site:?}";
                   "{out} turns out of a city against an allowance of {allowance}; the best \
                    legal site within {STALLED_SETTLER_RADIUS} tiles whose ground and first \
                    step are out of every visible hostile's reach is worth {worth:.1} net of \
                    the walk"; site);
        }
        self.settler_targets.insert(uid, site);
        self.settler_relaxed_targets.insert(uid, site);
        self.settler_stalls.remove(&uid);
        self.settler_closest.remove(&uid);
        if self.settler_step_toward_safe(g, pid, uid, site) {
            return Some(true);
        }
        None
    }

    /// Found underfoot once the walk has stalled. A Decision line asserts an
    /// applied action, never an intention, so it is journaled after the
    /// engine answers; the site is priced before.
    fn found_stalled_settler(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        out: u32,
        allowance: u32,
        worth: f64,
    ) -> bool {
        let here = g.units[&uid].pos;
        if !g.can_found_city(uid) {
            return false;
        }
        self.settler_targets.remove(&uid);
        self.settler_relaxed_targets.remove(&uid);
        self.settler_stalls.remove(&uid);
        self.settler_blocked_turns.remove(&uid);
        self.settler_closest.remove(&uid);
        self.stalled_settler_floor.remove(&uid);
        let founded = g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
        if founded {
            think!(self.journal(), Expansion, Decision,
                   "Founding a city at {here:?} after a stalled walk";
                   "{out} turns out of a city against an allowance of {allowance}; the site is \
                    worth {worth:.1}, out of every visible hostile's reach"; here);
        } else {
            think!(self.journal(), Expansion, Detail,
                   "Founding refused at {here:?} after a stalled walk";
                   "the engine would not take the city; the settler will re-plan"; here);
        }
        founded
    }
}
