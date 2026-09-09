//! Recon disruption: the `pass-picket` opt-in gene spends the idle recon
//! arm on the neighbours — hold the pass, watch the border.
//!
//! Operator goal (2026-08-24): *"a heuristic for … our recon units to disrupt
//! and suppress our enemies — particularly neighboring civs. … block
//! important chokepoints, such as mountain passes … scout out their territory
//! … keep an eye on our neighboring civs and watch their movements."* The
//! companion `settler-screen` gene (screen a seen rival Settler with two to
//! four units) left the code on 2026-09-08 by operator directive: rank 263,
//! P(>0) 0.5%, pooled Diff -1.15 pp; its measurements stay in the ranking's
//! "Removed from the code" table.
//!
//! ## The engine facts the gene rests on
//!
//! 1. **A foreign unit blocks its tile at peace.** `Game::can_enter_past`
//!    refuses any tile a foreign unit stands on unless the mover is military,
//!    the occupant a civilian, and the two at war — a capture. Nothing is
//!    declared: no war, no zone of control (`in_enemy_zoc_for` is war-only
//!    for military units), no blow. A unit on the one tile of a pass is a
//!    wall.
//! 2. **Mountains are the only impassable terrain** (`data/terrains.json`),
//!    so a pass is a tile whose occupation disconnects the land walk between
//!    two places — an articulation point of the walkable graph, read exactly
//!    by flooding the walk again with the tile removed.
//! 3. **General units are planned in parallel** on clones of the controller
//!    (`advanced_units`), so anything two units must agree on has to be
//!    decided before the batch from the start-of-turn board. The gene draws
//!    one plan per turn ([`AdvancedAi::recon_disruption_plan`]); a unit's
//!    step only reads its own order.
//!
//! ## The gene
//!
//! - **`pass-picket`** — [`AdvancedAi::plan_pickets`]: for every met major at
//!   peace whose nearest city is within [`NEIGHBOUR_RANGE`] of ours, the land
//!   walk between the two cities is read and the first articulation tile
//!   outside their borders on the rival's side is the post; when the walk has
//!   no single tile that cuts it, the post is the first tile of the walk
//!   outside their borders — a border watch that sees what comes out. A
//!   recon unit with nothing left to explore walks to its post and holds it.
//!   Exploration and the upgrade walk still come first; the post replaces
//!   only the patrol and the fortify the idle Scout took before.
//!
//! The gene holds no ground in a war: the step sits on the peacetime tail
//! alone, and a post is held only by a unit whose alternative was a patrol.
//!
//! ## Where each hook sits
//!
//! - `recon_disruption_plan`: `advanced_units`, after the once-per-turn
//!   tactical passes and before the unit loop — once per turn, from the
//!   start-of-turn board.
//! - `recon_disruption_step`: the peacetime tail of
//!   `advanced_military_step_with_decline`, immediately before
//!   `BasicAi::military_step` — after every raider, camp, village, staging
//!   and home-return order, so only a unit nothing else wanted gets here.
//!
//! Both are off in `AdvancedAi::new()` and `legacy()`, a `Kind::OptIn` row in
//! `genes.rs`, and byte-identical when off (the plan returns before reading
//! the board). Fires probe under `docs/gene_screens/fires/`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::AdvancedAi;
use crate::game::Game;
use crate::think;
use crate::Pos;

/// A neighbour is a met major at peace whose nearest city is within this of
/// ours.
pub(super) const NEIGHBOUR_RANGE: i32 = 18;
/// A post is re-read every this many turns, and at once when it no longer
/// stands (a border grew over it, a city rose on it).
const PICKET_REFRESH: u32 = 10;
/// Slack around the straight line between the two cities inside which the
/// walk between them is read.
const PICKET_WINDOW_SLACK: i32 = 6;
/// A post stands at least this far from the rival's city.
const PICKET_MIN_FROM_CITY: i32 = 2;

/// The orders the gene drew for this turn.
#[derive(Clone, Debug, Default)]
pub(super) struct ReconPlan {
    /// The turn the plan was drawn for; a step on any other turn has no
    /// orders.
    turn: Option<u32>,
    /// One post per neighbour.
    posts: BTreeMap<usize, PicketPost>,
    /// Post assignments by recon unit.
    pickets: BTreeMap<u32, Pos>,
}

impl ReconPlan {
    /// The post drawn for a neighbour, for explainers and tests.
    #[cfg(test)]
    pub(super) fn post(&self, rival: usize) -> Option<&PicketPost> {
        self.posts.get(&rival)
    }

    /// The post a recon unit was sent to, for explainers and tests.
    #[cfg(test)]
    pub(super) fn picket(&self, uid: u32) -> Option<Pos> {
        self.pickets.get(&uid).copied()
    }
}

/// A picket post toward one neighbour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PicketPost {
    /// The tile held.
    pub(super) at: Pos,
    /// The turn the walk was read.
    read_on: u32,
    /// `true` when holding the tile cuts the walk between the two cities;
    /// `false` for a border watch on a walk no single tile cuts.
    pub(super) pass: bool,
}

impl AdvancedAi {
    /// Draw this turn's recon orders from the start-of-turn board. Nothing is
    /// read with the gene off.
    pub(super) fn recon_disruption_plan(&mut self, g: &Game, pid: usize) {
        if !(self.pass_picket || self.pass_picket_2) {
            return;
        }
        if g.is_arena() || self.base.minor || self.base.barb {
            return;
        }
        let mut plan = ReconPlan {
            turn: Some(g.turn),
            posts: std::mem::take(&mut self.recon_disruption.posts),
            ..ReconPlan::default()
        };
        self.plan_pickets(g, pid, &mut plan);
        self.recon_disruption = plan;
    }

    /// Whether `other` is a neighbour the gene acts against: a met major
    /// on another team, at peace with us.
    fn disruption_rival(g: &Game, pid: usize, other: usize) -> bool {
        other != pid
            && !g.players[other].is_minor
            && !g.players[other].is_barbarian
            && g.has_met(pid, other)
            && !g.same_team(pid, other)
            && !g.is_at_war(pid, other)
    }

    /// Whether this is a unit the gene may send, and whether it is recon.
    /// The seat's own land military unit, except a settler guard; a unit that
    /// is not recon stays in the city it garrisons.
    fn disruption_unit(&self, g: &Game, pid: usize, uid: u32) -> Option<bool> {
        let unit = g.units.get(&uid)?;
        let spec = &g.rules.units[unit.kind];
        if unit.owner != pid
            || spec.class != "military"
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
            || g.is_embarked(unit)
        {
            return None;
        }
        if self.settler_guards.values().any(|guard| *guard == uid) {
            return None;
        }
        let recon = spec.promotion_class == "recon";
        if !recon && g.city_at(unit.pos).is_some() {
            return None;
        }
        Some(recon)
    }

    // ------------------------------------------------------------------
    // pass-picket
    // ------------------------------------------------------------------

    /// One post per neighbour, and a recon unit for each.
    fn plan_pickets(&self, g: &Game, pid: usize, plan: &mut ReconPlan) {
        let explored = &g.players[pid].explored;
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|city| g.cities[&city].pos)
            .collect();
        if ours.is_empty() {
            plan.posts.clear();
            return;
        }
        let mut posts: BTreeMap<usize, PicketPost> = BTreeMap::new();
        for rival in 0..g.players.len() {
            if !Self::disruption_rival(g, pid, rival) {
                continue;
            }
            let theirs: Vec<Pos> = g
                .player_city_ids(rival)
                .into_iter()
                .map(|city| g.cities[&city].pos)
                .filter(|pos| explored.contains(pos))
                .collect();
            let Some((span, home, away)) = ours
                .iter()
                .flat_map(|home| {
                    theirs
                        .iter()
                        .map(move |away| (g.wdist(*home, *away), *home, *away))
                })
                .min()
            else {
                continue;
            };
            if span > NEIGHBOUR_RANGE {
                continue;
            }
            if let Some(post) = plan.posts.get(&rival) {
                if post.read_on + PICKET_REFRESH > g.turn && Self::post_stands(g, pid, post.at) {
                    posts.insert(rival, *post);
                    continue;
                }
            }
            if let Some(post) = Self::read_picket_post(g, pid, rival, home, away) {
                posts.insert(rival, post);
            }
        }
        plan.posts = posts;
        // The nearest free recon unit to each post, one per post.
        let mut recon: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| self.disruption_unit(g, pid, *uid) == Some(true))
            .collect();
        for post in plan.posts.values() {
            let Some(index) = recon
                .iter()
                .enumerate()
                .min_by_key(|(_, uid)| (g.wdist(g.units[uid].pos, post.at), **uid))
                .map(|(index, _)| index)
            else {
                break;
            };
            let uid = recon.swap_remove(index);
            plan.pickets.insert(uid, post.at);
        }
    }

    /// Whether a post can still be held: land we may stand on, no city.
    fn post_stands(g: &Game, pid: usize, at: Pos) -> bool {
        g.map.get(at).is_some_and(|tile| {
            !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
                && g.city_at(at).is_none()
                && tile
                    .owner_city
                    .and_then(|city| g.cities.get(&city))
                    .is_none_or(|city| city.owner == pid)
        })
    }

    /// The land walk between two tiles over `walkable` ground with `cut`
    /// removed: the tiles between the ends, from `from` toward `to`.
    fn land_walk(
        g: &Game,
        from: Pos,
        to: Pos,
        walkable: &dyn Fn(Pos) -> bool,
        cut: Option<Pos>,
    ) -> Option<Vec<Pos>> {
        let mut parent: BTreeMap<Pos, Pos> = BTreeMap::new();
        let mut seen: BTreeSet<Pos> = BTreeSet::new();
        let mut queue = VecDeque::new();
        seen.insert(from);
        queue.push_back(from);
        while let Some(cur) = queue.pop_front() {
            if cur == to {
                let mut walk = Vec::new();
                let mut step = to;
                while let Some(prev) = parent.get(&step) {
                    if *prev == from {
                        break;
                    }
                    walk.push(*prev);
                    step = *prev;
                }
                walk.reverse();
                return Some(walk);
            }
            for next in g.nbrs(cur) {
                if seen.contains(&next) || Some(next) == cut || !walkable(next) {
                    continue;
                }
                seen.insert(next);
                parent.insert(next, cur);
                queue.push_back(next);
            }
        }
        None
    }

    /// The post toward one neighbour: the first tile of the walk from their
    /// city to ours, outside their borders, whose removal cuts the walk; or
    /// the first such tile at all when no single tile cuts it.
    fn read_picket_post(
        g: &Game,
        pid: usize,
        rival: usize,
        home: Pos,
        away: Pos,
    ) -> Option<PicketPost> {
        let explored = &g.players[pid].explored;
        let span = g.wdist(home, away);
        let walkable = |pos: Pos| {
            g.wdist(pos, home) + g.wdist(pos, away) <= span + PICKET_WINDOW_SLACK
                && explored.contains(&pos)
                && g.map
                    .get(pos)
                    .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
        };
        let walk = Self::land_walk(g, away, home, &walkable, None)?;
        let candidates: Vec<Pos> = walk
            .iter()
            .copied()
            .filter(|pos| {
                g.wdist(*pos, away) >= PICKET_MIN_FROM_CITY
                    && Self::post_stands(g, pid, *pos)
                    && g.map
                        .get(*pos)
                        .and_then(|tile| tile.owner_city)
                        .and_then(|city| g.cities.get(&city))
                        .is_none_or(|city| city.owner != rival)
            })
            .collect();
        let first = *candidates.first()?;
        let pass = candidates
            .iter()
            .copied()
            .find(|cut| Self::land_walk(g, away, home, &walkable, Some(*cut)).is_none());
        Some(PicketPost {
            at: pass.unwrap_or(first),
            read_on: g.turn,
            pass: pass.is_some(),
        })
    }

    /// A visible rival Settler on the far side of this actual pass. Border
    /// watches in open terrain do not take priority over exploration, nor
    /// do distant settlers justify a long intercept march.
    fn settler_at_picket(&self, g: &Game, pid: usize, at: Pos) -> bool {
        let Some((&rival, _)) = self
            .recon_disruption
            .posts
            .iter()
            .find(|(_, post)| post.at == at && post.pass && Self::post_stands(g, pid, at))
        else {
            return false;
        };
        if !Self::disruption_rival(g, pid, rival) {
            return false;
        }
        let Some(home) = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .min_by_key(|home| g.wdist(*home, at))
        else {
            return false;
        };
        let visible = self.battlefront_visibility(g, pid);
        g.player_unit_ids(rival).into_iter().any(|uid| {
            let unit = &g.units[&uid];
            unit.kind == "settler"
                && g.wdist(unit.pos, at) <= 2
                && g.wdist(unit.pos, home) > g.wdist(at, home)
                && g.sees(&visible, unit.pos)
                && self.battlefront_unit_visible(g, pid, uid)
        })
    }

    // ------------------------------------------------------------------
    // the step
    // ------------------------------------------------------------------

    /// This unit's recon order for the turn. `Some(true)` when it moved,
    /// `Some(false)` when it holds its post, `None` when the plan has nothing
    /// for it and the ordinary peacetime step follows.
    pub(super) fn recon_disruption_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !(self.pass_picket || self.pass_picket_2) || self.recon_disruption.turn != Some(g.turn) {
            return None;
        }
        let here = g.units.get(&uid)?.pos;
        if self.pass_picket || self.pass_picket_2 {
            if let Some(post) = self.recon_disruption.pickets.get(&uid).copied() {
                if self.pass_picket_2 && self.settler_at_picket(g, pid, post) {
                    if here == post {
                        return Some(false);
                    }
                    if g.can_move(uid, post) && self.base.path_move(g, pid, uid, post) {
                        think!(self.journal(), Military, Decision,
                               "Scout {uid} blocks a Settler's pass at {post:?}";
                               "a visible rival Settler is approaching this choke point; \
                                the assigned Scout can occupy it this turn";
                               post);
                        return Some(true);
                    }
                }
                // Exploration and the upgrade walk still come first; the
                // post replaces the patrol.
                if self.base.should_explore(g, pid, uid, false)
                    && self.base.explore_step(g, pid, uid)
                {
                    return Some(true);
                }
                if self.base.modernization_step(g, pid, uid) {
                    return Some(true);
                }
                if here == post {
                    return Some(false);
                }
                if let Some(next) = g
                    .route_step(uid, post, 0)
                    .filter(|next| g.can_move(uid, *next))
                {
                    if self.base.path_move(g, pid, uid, next) {
                        think!(self.journal(), Military, Detail,
                               "{} {uid} walks to its picket post {post:?}", g.units[&uid].kind;
                               "nothing is left to explore; the post is the pass toward a \
                                neighbour, or the border tile that sees what comes out of it";
                               post);
                        return Some(true);
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
