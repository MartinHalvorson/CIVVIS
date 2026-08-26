//! Recon disruption: two opt-in genes that spend the recon arm on the
//! neighbours — hold the pass, watch the border, and screen the Settler that
//! comes out of it.
//!
//! Operator goal (2026-08-24): *"a heuristic for … our recon units to disrupt
//! and suppress our enemies — particularly neighboring civs. … block
//! important chokepoints, such as mountain passes … scout out their territory
//! … watch for when they produce settlers, predict where the settlers want to
//! go and block their path … use 2-4 units if we can afford it … constantly
//! screen the settler's possible movements to slow or completely block them
//! … keep an eye on our neighboring civs and watch their movements."*
//!
//! ## The engine facts each gene rests on
//!
//! 1. **A foreign unit blocks its tile at peace.** `Game::can_enter_past`
//!    refuses any tile a foreign unit stands on unless the mover is military,
//!    the occupant a civilian, and the two at war — a capture. A rival Settler
//!    therefore never steps onto a tile one of our units holds, and nothing
//!    is declared: no war, no zone of control (`in_enemy_zoc_for` is war-only
//!    for military units), no blow. A unit on the one tile of a pass is a
//!    wall; two or three beside a Settler in the open are a detour every
//!    turn, re-drawn every turn, because the Scout walks three to its two.
//! 2. **Mountains are the only impassable terrain** (`data/terrains.json`),
//!    so a pass is a tile whose occupation disconnects the land walk between
//!    two places — an articulation point of the walkable graph, read exactly
//!    by flooding the walk again with the tile removed.
//! 3. **A rival Settler is an ordinary unit: seen iff its tile is in our
//!    sight.** The house fog idiom (`sees(&visible, pos) && unit_visible_to`)
//!    is the only read; no production queue is consulted, because reading
//!    a rival's queue is the leak `fog-honest` exists to measure. The eye
//!    that sees the Settler come out of the border is the picket below.
//! 4. **Where it is going is on the map.** The rival's own Settler ranks
//!    sites by yields, water and distance; `settlement_prefilter_score` over
//!    the explored ground around it, discounted by distance and sharpened by
//!    the heading between two sightings, names its likeliest three sites.
//!    A stand's worth is then exact arithmetic: the Settler's shortest walk
//!    to each site with the stand held, against the walk without it.
//! 5. **General units are planned in parallel** on clones of the controller
//!    (`advanced_units`), so anything two units must agree on has to be
//!    decided before the batch from the start-of-turn board. Both genes draw
//!    one plan per turn ([`AdvancedAi::recon_disruption_plan`]); a unit's
//!    step only reads its own order.
//!
//! ## The two genes
//!
//! - **`settler-screen`** — [`AdvancedAi::plan_settler_screens`]: for every
//!   seen rival Settler within [`SCREEN_RANGE`] of one of our cities, the
//!   stands in this turn's reach of our nearby land units are priced by how
//!   many expected Settler steps they add (a site made unreachable is worth
//!   [`BLOCK_VALUE`]), and up to [`SCREEN_UNITS_MAX`] units take the best
//!   stands greedily, recon first, at most [`SCREEN_OTHERS_MAX`] of them not
//!   recon and none of those while a major war is on — that is "if we can
//!   afford it": a unit reaches this hook only when nothing above it in the
//!   peacetime tail wanted it. A recon unit with no stand in reach pursues
//!   the Settler's predicted walk. A stand is held (`Some(false)`) only while
//!   the plan names it; the next turn's plan re-reads the board.
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
//! Neither gene here holds ground in a war: the step sits on the peacetime
//! tail alone, a Settler is screened only at peace with its owner, and a post
//! is held only by a unit whose alternative was a patrol.
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
//! Both are off in `AdvancedAi::new()` and `legacy()`, `Kind::OptIn` rows in
//! `genes.rs`, and byte-identical when off (the plan returns before reading
//! the board). Fires probes under `docs/gene_screens/fires/`.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::AdvancedAi;
use crate::game::{Game, Unit};
use crate::think;
use crate::Pos;

/// A rival Settler is worth screening while it stands within this many
/// tiles of one of our cities: that is the ground our own next city wants.
pub(super) const SCREEN_RANGE: i32 = 12;
/// The sites a seen Settler may be walking to are read inside this radius.
const SITE_RADIUS: i32 = 8;
/// How many sites the prediction keeps.
const SITE_CANDIDATES: usize = 3;
/// Per tile between the Settler and a site: it has walked this far already,
/// so the nearer site is the likelier one.
const SITE_DISTANCE_WEIGHT: f64 = 3.0;
/// Per tile of progress toward a site between two sightings — the heading.
const SITE_HEADING_WEIGHT: f64 = 12.0;
/// The likeliest site's share of the prediction; the rest split the remainder.
const TOP_SITE_SHARE: f64 = 0.6;
/// Sightings older than this no longer give a heading.
const SIGHTING_MEMORY: u32 = 5;
/// Worth, in Settler steps, of a site made unreachable.
pub(super) const BLOCK_VALUE: f64 = 8.0;
/// Units per Settler, recon first.
pub(super) const SCREEN_UNITS_MAX: usize = 4;
/// Of those, units that are not recon.
const SCREEN_OTHERS_MAX: usize = 2;
/// A unit within this many tiles of the Settler is asked for a stand.
const SCREEN_UNIT_RANGE: i32 = 8;
/// Expected Settler steps a stand must add before a recon unit takes it —
/// half a step, so the one tile ahead on the likeliest walk is taken.
const SCREEN_MIN_GAIN_RECON: f64 = 0.5;
/// The same floor for a unit that is not recon: a whole step.
const SCREEN_MIN_GAIN_OTHER: f64 = 1.0;
/// A stand is priced only this close to the Settler's predicted walks.
const STAND_NEAR_WALK: i32 = 1;
/// A recon unit with no stand in reach pursues a Settler this close.
const PURSUIT_RANGE: i32 = 8;
/// The Settler's walks are flooded inside this radius of it.
const WALK_WINDOW: i32 = SITE_RADIUS + 3;

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

/// The orders both genes drew for this turn.
#[derive(Clone, Debug, Default)]
pub(super) struct ReconPlan {
    /// The turn the plan was drawn for; a step on any other turn has no
    /// orders.
    turn: Option<u32>,
    /// Screen orders by unit.
    screens: BTreeMap<u32, ScreenOrder>,
    /// Where each screened Settler was last seen, and when.
    sightings: BTreeMap<u32, (Pos, u32)>,
    /// One post per neighbour.
    posts: BTreeMap<usize, PicketPost>,
    /// Post assignments by recon unit.
    pickets: BTreeMap<u32, Pos>,
    /// Units the screens have spoken for this turn.
    sent: BTreeSet<u32>,
}

impl ReconPlan {
    /// The screen order drawn for a unit this turn, for explainers and tests.
    #[cfg(test)]
    pub(super) fn screen(&self, uid: u32) -> Option<&ScreenOrder> {
        self.screens.get(&uid)
    }

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

/// One unit's screen order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ScreenOrder {
    /// Walk to `at` and hold it: the Settler `settler` has to go round.
    Stand { at: Pos, settler: u32 },
    /// Walk toward `toward`, a tile ahead of `settler` on its predicted
    /// walk, to be in reach of a stand next turn.
    Pursue { toward: Pos, settler: u32 },
}

impl ScreenOrder {
    /// The tile the order walks to.
    #[cfg(test)]
    pub(super) fn tile(&self) -> Pos {
        match self {
            ScreenOrder::Stand { at, .. } => *at,
            ScreenOrder::Pursue { toward, .. } => *toward,
        }
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
    /// read with both genes off.
    pub(super) fn recon_disruption_plan(&mut self, g: &Game, pid: usize) {
        if !(self.settler_screen || self.pass_picket) {
            return;
        }
        if g.is_arena() || self.base.minor || self.base.barb {
            return;
        }
        let mut plan = ReconPlan {
            turn: Some(g.turn),
            sightings: std::mem::take(&mut self.recon_disruption.sightings),
            posts: std::mem::take(&mut self.recon_disruption.posts),
            ..ReconPlan::default()
        };
        if self.settler_screen {
            self.plan_settler_screens(g, pid, &mut plan);
        } else {
            plan.sightings.clear();
        }
        if self.pass_picket {
            self.plan_pickets(g, pid, &mut plan);
        } else {
            plan.posts.clear();
        }
        self.recon_disruption = plan;
    }

    /// Whether `other` is a neighbour either gene acts against: a met major
    /// on another team, at peace with us.
    fn disruption_rival(g: &Game, pid: usize, other: usize) -> bool {
        other != pid
            && !g.players[other].is_minor
            && !g.players[other].is_barbarian
            && g.has_met(pid, other)
            && !g.same_team(pid, other)
            && !g.is_at_war(pid, other)
    }

    /// Whether this is a unit either gene may send, and whether it is recon.
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

    /// Whether a flooded tile can be the END of a stand: no city, nothing
    /// military on it. `approach_reach` floods through friends.
    fn stand_open(g: &Game, pos: Pos) -> bool {
        g.city_at(pos).is_none()
            && !g
                .unit_ids_at(pos)
                .iter()
                .any(|oid| g.rules.units[g.units[oid].kind].class == "military")
    }

    // ------------------------------------------------------------------
    // settler-screen
    // ------------------------------------------------------------------

    /// Every seen rival Settler near our cities, and the stands that cost it
    /// the most steps.
    fn plan_settler_screens(&self, g: &Game, pid: usize, plan: &mut ReconPlan) {
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|city| g.cities[&city].pos)
            .collect();
        if ours.is_empty() {
            plan.sightings.clear();
            return;
        }
        let visible = self.battlefront_visibility(g, pid);
        let major_war = (0..g.players.len()).any(|other| {
            other != pid
                && !g.players[other].is_minor
                && !g.players[other].is_barbarian
                && g.is_at_war(pid, other)
        });
        // Seen Settlers, the one nearest our cities first.
        let mut settlers: Vec<(i32, u32)> = g
            .units
            .values()
            .filter(|unit| {
                unit.kind == "settler"
                    && Self::disruption_rival(g, pid, unit.owner)
                    && !g.is_embarked(unit)
                    && g.sees(&visible, unit.pos)
                    && g.unit_visible_to(unit.id, pid)
            })
            .map(|unit| {
                let near = ours
                    .iter()
                    .map(|city| g.wdist(*city, unit.pos))
                    .min()
                    .unwrap_or(i32::MAX);
                (near, unit.id)
            })
            .filter(|(near, _)| *near <= SCREEN_RANGE)
            .collect();
        settlers.sort_unstable();
        // A heading needs the previous sighting; keep the recent ones.
        let mut sightings: BTreeMap<u32, (Pos, u32)> = plan
            .sightings
            .iter()
            .filter(|(uid, (_, seen))| {
                g.units.contains_key(uid) && *seen + SIGHTING_MEMORY >= g.turn
            })
            .map(|(uid, seen)| (*uid, *seen))
            .collect();
        for (_, sid) in settlers {
            let settler = &g.units[&sid];
            let previous = sightings.insert(sid, (settler.pos, g.turn));
            let sites = Self::rival_settler_sites(g, pid, settler, previous);
            if sites.is_empty() {
                continue;
            }
            self.screen_one_settler(g, pid, settler, &sites, major_war, plan);
        }
        plan.sightings = sightings;
    }

    /// The likeliest sites a rival Settler is walking to, with their shares
    /// of the prediction. Legal for the rival by the founding rules
    /// (`can_found_city`: land, passable, no wonder or oasis, four from
    /// every known city, not another civ's ground), read over ground we have
    /// explored, scored by the settlement prefilter less distance, plus the
    /// heading since the previous sighting.
    fn rival_settler_sites(
        g: &Game,
        pid: usize,
        settler: &Unit,
        previous: Option<(Pos, u32)>,
    ) -> Vec<(Pos, f64)> {
        let rival = settler.owner;
        let explored = &g.players[pid].explored;
        let known_cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| explored.contains(&city.pos))
            .map(|city| city.pos)
            .collect();
        let heading = previous
            .filter(|(was, _)| *was != settler.pos)
            .map(|(was, _)| was);
        let mut sites: Vec<(Pos, f64)> = Vec::new();
        for pos in g.wdisk(settler.pos, SITE_RADIUS) {
            if pos == settler.pos || !explored.contains(&pos) {
                continue;
            }
            let Some(tile) = g.map.get(pos) else {
                continue;
            };
            if g.rules.is_water(tile)
                || !g.rules.is_passable(tile)
                || g.tile_is_natural_wonder(tile)
                || tile.feature.as_deref() == Some("oasis")
            {
                continue;
            }
            if known_cities.iter().any(|city| g.wdist(*city, pos) < 4) {
                continue;
            }
            if tile
                .owner_city
                .and_then(|city| g.cities.get(&city))
                .is_some_and(|city| city.owner != rival)
            {
                continue;
            }
            let mut score = Self::settlement_prefilter_score(g, pos)
                - SITE_DISTANCE_WEIGHT * f64::from(g.wdist(settler.pos, pos));
            if let Some(was) = heading {
                let progress = g.wdist(was, pos) - g.wdist(settler.pos, pos);
                score += SITE_HEADING_WEIGHT * f64::from(progress);
            }
            if score > 0.0 {
                sites.push((pos, score));
            }
        }
        sites.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        sites.truncate(SITE_CANDIDATES);
        let count = sites.len();
        for (index, site) in sites.iter_mut().enumerate() {
            site.1 = if count == 1 {
                1.0
            } else if index == 0 {
                TOP_SITE_SHARE
            } else {
                (1.0 - TOP_SITE_SHARE) / (count - 1) as f64
            };
        }
        sites
    }

    /// Whether a rival Settler can be walked through `pos`: land, passable,
    /// ground its owner may enter, no foreign city, nothing foreign standing
    /// on it (a religious unit shares the tile), and not a stand we hold.
    /// `movers` are the tiles of our units that may still move this turn,
    /// read as open ground — the unit that takes a stand leaves its tile.
    fn settler_can_walk(
        g: &Game,
        rival: usize,
        pos: Pos,
        held: &BTreeSet<Pos>,
        movers: &BTreeSet<Pos>,
    ) -> bool {
        if held.contains(&pos) {
            return false;
        }
        if movers.contains(&pos) {
            return true;
        }
        let Some(tile) = g.map.get(pos) else {
            return false;
        };
        if g.rules.is_water(tile) || !g.rules.is_passable(tile) {
            return false;
        }
        if let Some(owner) = tile
            .owner_city
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner)
        {
            if owner != rival
                && (g.city_at(pos).is_some()
                    || (!g.has_open_borders(rival, owner) && !g.is_at_war(rival, owner)))
            {
                return false;
            }
        }
        !g.unit_ids_at(pos).iter().any(|oid| {
            let other = &g.units[oid];
            other.owner != rival && g.rules.units[other.kind].class != "religious"
        })
    }

    /// The Settler's shortest walks, in steps, from `from` to each goal
    /// inside the window with the held stands in place — `None` when a goal
    /// cannot be reached — and the parents of the flood for the walks.
    fn settler_walks(
        g: &Game,
        rival: usize,
        from: Pos,
        goals: &[Pos],
        window: (Pos, i32),
        held: &BTreeSet<Pos>,
        movers: &BTreeSet<Pos>,
    ) -> (Vec<Option<i32>>, BTreeMap<Pos, Pos>) {
        let mut dist: BTreeMap<Pos, i32> = BTreeMap::new();
        let mut parent: BTreeMap<Pos, Pos> = BTreeMap::new();
        let mut queue = VecDeque::new();
        dist.insert(from, 0);
        queue.push_back(from);
        let mut open = goals.iter().filter(|goal| **goal != from).count();
        while let Some(cur) = queue.pop_front() {
            if open == 0 {
                break;
            }
            let here = dist[&cur];
            for next in g.nbrs(cur) {
                if dist.contains_key(&next)
                    || g.wdist(window.0, next) > window.1
                    || !Self::settler_can_walk(g, rival, next, held, movers)
                {
                    continue;
                }
                dist.insert(next, here + 1);
                parent.insert(next, cur);
                open -= goals.iter().filter(|goal| **goal == next).count();
                queue.push_back(next);
            }
        }
        (
            goals.iter().map(|goal| dist.get(goal).copied()).collect(),
            parent,
        )
    }

    /// Expected Settler steps the held stands add over `base`, capped per
    /// site at [`BLOCK_VALUE`].
    fn stands_value(base: &[Option<i32>], with: &[Option<i32>], sites: &[(Pos, f64)]) -> f64 {
        sites
            .iter()
            .enumerate()
            .map(|(index, (_, share))| match (base[index], with[index]) {
                (Some(before), Some(after)) => share * f64::from(after - before).min(BLOCK_VALUE),
                (Some(_), None) => share * BLOCK_VALUE,
                _ => 0.0,
            })
            .sum()
    }

    /// The walk from the Settler to `goal`, Settler's tile excluded, goal
    /// included, from the flood's parents.
    fn walk_to(parents: &BTreeMap<Pos, Pos>, from: Pos, goal: Pos) -> Vec<Pos> {
        let mut walk = vec![goal];
        let mut cur = goal;
        while let Some(prev) = parents.get(&cur) {
            if *prev == from {
                break;
            }
            walk.push(*prev);
            cur = *prev;
        }
        walk.reverse();
        walk
    }

    /// Stands for one Settler: greedy over the union of this turn's reaches
    /// of every nearby unit, best stand first, recon first on a tie.
    fn screen_one_settler(
        &self,
        g: &Game,
        pid: usize,
        settler: &Unit,
        sites: &[(Pos, f64)],
        major_war: bool,
        plan: &mut ReconPlan,
    ) {
        let rival = settler.owner;
        let goals: Vec<Pos> = sites.iter().map(|site| site.0).collect();
        let window = (settler.pos, WALK_WINDOW);
        // Stands already held for another Settler this turn are in place.
        let mut held: BTreeSet<Pos> = plan
            .screens
            .values()
            .filter_map(|order| match order {
                ScreenOrder::Stand { at, .. } => Some(*at),
                ScreenOrder::Pursue { .. } => None,
            })
            .collect();
        // The units in range, recon first, nearest first.
        let mut units: Vec<(bool, i32, u32)> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| !plan.sent.contains(uid))
            .filter_map(|uid| {
                let recon = self.disruption_unit(g, pid, uid)?;
                if !recon && major_war {
                    return None;
                }
                let distance = g.wdist(g.units[&uid].pos, settler.pos);
                (distance <= SCREEN_UNIT_RANGE).then_some((!recon, distance, uid))
            })
            .collect();
        units.sort_unstable();
        if units.is_empty() {
            return;
        }
        // Their tiles are open ground to the walk: whoever takes a stand
        // leaves its tile, and the base must not count our own wall twice.
        let movers: BTreeSet<Pos> = units.iter().map(|(_, _, uid)| g.units[uid].pos).collect();
        let (mut base, parents) =
            Self::settler_walks(g, rival, settler.pos, &goals, window, &held, &movers);
        if base.iter().all(Option::is_none) {
            return;
        }
        // Ground beside the predicted walks is where a stand can matter.
        let mut near: BTreeSet<Pos> = BTreeSet::new();
        for (index, goal) in goals.iter().enumerate() {
            if base[index].is_none() {
                continue;
            }
            for step in Self::walk_to(&parents, settler.pos, *goal) {
                near.extend(g.wdisk(step, STAND_NEAR_WALK));
            }
        }
        near.remove(&settler.pos);
        // Each reach once.
        let reaches: Vec<(u32, bool, BTreeSet<Pos>)> = units
            .iter()
            .map(|(other, _, uid)| {
                let ends: BTreeSet<Pos> = g
                    .approach_reach(*uid)
                    .into_keys()
                    .filter(|end| near.contains(end) && Self::stand_open(g, *end))
                    .collect();
                (*uid, !*other, ends)
            })
            .collect();
        let mut assigned = 0usize;
        let mut others = 0usize;
        let mut sent: BTreeSet<u32> = BTreeSet::new();
        while assigned < SCREEN_UNITS_MAX {
            // Every stand some unsent unit reaches, priced once.
            let mut priced: Vec<(f64, Pos)> = reaches
                .iter()
                .filter(|(uid, _, _)| !sent.contains(uid))
                .flat_map(|(_, _, ends)| ends.iter().copied())
                .filter(|stand| !held.contains(stand))
                .collect::<BTreeSet<Pos>>()
                .into_iter()
                .map(|stand| {
                    let mut with = held.clone();
                    with.insert(stand);
                    let (costs, _) =
                        Self::settler_walks(g, rival, settler.pos, &goals, window, &with, &movers);
                    (Self::stands_value(&base, &costs, sites), stand)
                })
                .collect();
            priced.sort_by(|a, b| {
                b.0.partial_cmp(&a.0)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| a.1.cmp(&b.1))
            });
            let mut choice: Option<(u32, bool, Pos, f64)> = None;
            for (value, stand) in priced {
                if value < SCREEN_MIN_GAIN_RECON {
                    break;
                }
                // Recon first, then the nearest other unit if one may go.
                let unit = reaches
                    .iter()
                    .filter(|(uid, recon, ends)| {
                        !sent.contains(uid)
                            && ends.contains(&stand)
                            && (*recon
                                || (others < SCREEN_OTHERS_MAX && value >= SCREEN_MIN_GAIN_OTHER))
                    })
                    .max_by_key(|(uid, recon, _)| (*recon, std::cmp::Reverse(*uid)))
                    .map(|(uid, recon, _)| (*uid, *recon));
                if let Some((uid, recon)) = unit {
                    choice = Some((uid, recon, stand, value));
                    break;
                }
            }
            let Some((uid, recon, stand, _value)) = choice else {
                break;
            };
            plan.screens.insert(
                uid,
                ScreenOrder::Stand {
                    at: stand,
                    settler: settler.id,
                },
            );
            sent.insert(uid);
            plan.sent.insert(uid);
            held.insert(stand);
            assigned += 1;
            if !recon {
                others += 1;
            }
            base = Self::settler_walks(g, rival, settler.pos, &goals, window, &held, &movers).0;
        }
        // A recon unit with no stand in reach walks toward the likeliest
        // walk, two steps ahead of the Settler, to be in reach next turn.
        if assigned >= SCREEN_UNITS_MAX {
            return;
        }
        let ahead = goals
            .iter()
            .enumerate()
            .find(|(index, _)| base[*index].is_some())
            .map(|(_, goal)| Self::walk_to(&parents, settler.pos, *goal))
            .and_then(|walk| walk.get(1).or_else(|| walk.first()).copied());
        let Some(ahead) = ahead else {
            return;
        };
        for (uid, recon, _) in &reaches {
            if assigned >= SCREEN_UNITS_MAX {
                break;
            }
            if !recon || sent.contains(uid) {
                continue;
            }
            let distance = g.wdist(g.units[uid].pos, settler.pos);
            if distance <= 2 || distance > PURSUIT_RANGE {
                continue;
            }
            plan.screens.insert(
                *uid,
                ScreenOrder::Pursue {
                    toward: ahead,
                    settler: settler.id,
                },
            );
            sent.insert(*uid);
            plan.sent.insert(*uid);
            assigned += 1;
        }
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
            .filter(|uid| {
                !plan.screens.contains_key(uid) && self.disruption_unit(g, pid, *uid) == Some(true)
            })
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

    // ------------------------------------------------------------------
    // the step
    // ------------------------------------------------------------------

    /// Walk one legal step at a time along this turn's reach to `at`.
    /// Returns whether the unit moved and whether it stands at `at`.
    fn walk_to_stand(&self, g: &mut Game, pid: usize, uid: u32, at: Pos) -> (bool, bool) {
        let Some((_, path)) = g.approach_reach(uid).remove(&at) else {
            return (false, false);
        };
        // A stand on the far side of our own screen is reached by crossing it,
        // and `MoveTo` is the only action that may execute such a walk — see
        // `Game::entry_at`, and note that stepping it one `Move` at a time
        // stops dead on the friendly tile. An ordinary walk stays one recorded
        // step at a time, which is what the order ledger reads.
        if path.iter().any(|step| !g.can_stop(uid, *step)) {
            let moved = self.base.path_walk_to(g, pid, uid, at);
            let arrived = g.units.get(&uid).is_some_and(|unit| unit.pos == at);
            return (moved, arrived);
        }
        let mut moved = false;
        for step in path {
            if !g.can_move(uid, step) || !self.base.tactical_apply_move(g, pid, uid, step) {
                break;
            }
            moved = true;
        }
        let arrived = g.units.get(&uid).is_some_and(|unit| unit.pos == at);
        (moved, arrived)
    }

    /// This unit's recon order for the turn. `Some(true)` when it moved,
    /// `Some(false)` when it holds its stand or post, `None` when the plan
    /// has nothing for it and the ordinary peacetime step follows.
    pub(super) fn recon_disruption_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !(self.settler_screen || self.pass_picket) || self.recon_disruption.turn != Some(g.turn)
        {
            return None;
        }
        let here = g.units.get(&uid)?.pos;
        if let Some(order) = self.recon_disruption.screens.get(&uid).copied() {
            match order {
                ScreenOrder::Stand { at, settler } => {
                    if here == at {
                        // Holding: the Settler has to walk round this tile.
                        return Some(false);
                    }
                    let (moved, arrived) = self.walk_to_stand(g, pid, uid, at);
                    if moved {
                        think!(self.journal(), Military, Detail,
                               "{} {uid} screens Settler {settler} from {at:?}", g.units[&uid].kind;
                               "a tile a foreign unit holds cannot be entered at peace; the stand \
                                adds the most expected steps to the Settler's likeliest walks \
                                ({})", if arrived { "holding it" } else { "still walking" };
                               at);
                        return Some(true);
                    }
                    // The stand is no longer in reach (a friend crossed the
                    // path, the board moved): release the order for the turn.
                }
                ScreenOrder::Pursue { toward, settler } => {
                    if let Some(next) = g
                        .route_step(uid, toward, 1)
                        .filter(|next| g.can_move(uid, *next))
                    {
                        if self.base.path_move(g, pid, uid, next) {
                            think!(self.journal(), Military, Detail,
                                   "{} {uid} pursues Settler {settler} toward {toward:?}", g.units[&uid].kind;
                                   "no stand on its walk was in reach this turn; from beside \
                                    the walk there will be one next turn";
                                   toward);
                            return Some(true);
                        }
                    }
                }
            }
        }
        if self.pass_picket {
            if let Some(post) = self.recon_disruption.pickets.get(&uid).copied() {
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
mod tests {
    use super::super::genes::GENES;
    use super::super::AdvancedAi;
    use super::*;
    use crate::game::{Action, Game};
    use crate::name;

    fn opt_in_off_in_both_controllers(tag: &str, read: fn(&AdvancedAi) -> bool) {
        assert!(!read(&AdvancedAi::new()), "{tag} must be off in new()");
        assert!(
            !read(&AdvancedAi::legacy()),
            "{tag} must be off in legacy()"
        );
        let gene = GENES
            .iter()
            .find(|gene| gene.tag == tag)
            .expect("the gene is published for gene_screen");
        assert!(gene.opt_in() && gene.screenable() && !gene.live());
        let mut ai = AdvancedAi::new();
        (gene.enable)(&mut ai);
        assert!(read(&ai));
        (gene.disable)(&mut ai);
        assert!(!read(&ai));
    }

    #[test]
    fn settler_screen_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("settler-screen", |ai| ai.settler_screen);
    }

    #[test]
    fn pass_picket_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("pass-picket", |ai| ai.pass_picket);
    }

    /// A flat two-major board at peace with every starting unit cleared, the
    /// map explored by both, no terrain anywhere. Returns the game and a
    /// patch of open neutral ground far from both capitals.
    fn peace_field(seed: u64) -> (Game, Pos) {
        let mut game = Game::new_full(2, 28, 18, seed, 1_000, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .expect("each fixture major starts with a settler");
            game.found_city_for(pid, game.units[&settler].pos, None);
        }
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        game.barb_camps.clear();
        game.barb_naval_camps.clear();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
        }
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
        for pid in 0..2 {
            game.players[pid]
                .explored
                .extend(game.map.tiles.keys().copied());
        }
        game.turn = 60;
        game.current = 0;
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        let rival = game.cities[&game.player_city_ids(1)[0]].pos;
        let field = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|position| {
                game.wdist(*position, home) >= 8
                    && game.wdist(*position, home) <= SCREEN_RANGE - 2
                    && game.wdist(*position, rival) >= 8
                    && game.map.tiles[position].owner_city.is_none()
                    && game.wdisk(*position, 5).len() == game.wdisk(home, 5).len()
            })
            .min()
            .expect("the fixture board has open neutral ground near our capital");
        assert!(!game.is_at_war(0, 1));
        (game, field)
    }

    /// Start of turn for one unit: full movement, nothing spent.
    fn fresh(game: &mut Game, uid: u32) {
        let moves = game.unit_max_moves(uid);
        let unit = game.units.get_mut(&uid).unwrap();
        unit.moves_left = moves;
        unit.attacks_left = 1;
        unit.moved = false;
        unit.acted = false;
    }

    fn mountain(game: &mut Game, pos: Pos) {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = name!("mountain");
        tile.hills = false;
    }

    fn screening_ai() -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.enable_settler_screen();
        ai
    }

    #[test]
    fn a_rival_settler_inside_a_ring_of_mountains_is_walled_in_at_the_gap() {
        let (mut game, field) = peace_field(31);
        // A ring of mountains two tiles out with one gap; the ground inside
        // is desert so no site inside the ring competes with the ones
        // outside it, all of which are reached through the gap alone.
        let ring = game.wring(field, 2);
        let gap = *ring.iter().min().unwrap();
        for pos in &ring {
            if *pos != gap {
                mountain(&mut game, *pos);
            }
        }
        for pos in game.wdisk(field, 1) {
            game.map.tiles.get_mut(&pos).unwrap().terrain = name!("desert");
        }
        let settler = game.spawn_test_unit("settler", 1, field);
        // Our Scout inside the ring, beside the Settler and beside the gap.
        let inside = game
            .nbrs(field)
            .into_iter()
            .find(|pos| game.wdist(*pos, gap) == 1)
            .expect("a tile beside both the Settler and the gap");
        let scout = game.spawn_test_unit("scout", 0, inside);
        fresh(&mut game, scout);
        fresh(&mut game, settler);
        assert!(game.player_can_see(0, field));

        let mut ai = screening_ai();
        ai.recon_disruption_plan(&game, 0);
        assert_eq!(
            ai.recon_disruption.screen(scout),
            Some(&ScreenOrder::Stand { at: gap, settler }),
            "the gap is the one tile every walk out of the ring crosses"
        );
        assert_eq!(ai.recon_disruption_step(&mut game, 0, scout), Some(true));
        assert_eq!(game.units[&scout].pos, gap);
        assert_eq!(
            ai.recon_disruption_step(&mut game, 0, scout),
            Some(false),
            "on the stand the unit holds"
        );
        // The engine agrees: the Settler cannot enter the held gap, and no
        // tile outside the ring is in its reach this turn.
        assert!(!game.can_move(settler, gap));
        assert!(game
            .reachable(settler)
            .into_iter()
            .all(|pos| game.wdist(pos, field) <= 1));
    }

    #[test]
    fn two_units_take_two_distinct_stands_ahead_of_the_settler() {
        let (mut game, field) = peace_field(32);
        let settler = game.spawn_test_unit("settler", 1, field);
        // A previous sighting one step behind the Settler: it is heading the
        // other way, so the likeliest sites lie ahead of it.
        let behind = game.nbrs(field).into_iter().min().unwrap();
        let scout = game.spawn_test_unit("scout", 0, behind);
        let warrior = game.spawn_test_unit(
            "warrior",
            0,
            game.nbrs(field)
                .into_iter()
                .find(|pos| *pos != behind && game.wdist(*pos, behind) == 1)
                .unwrap(),
        );
        fresh(&mut game, scout);
        fresh(&mut game, warrior);
        fresh(&mut game, settler);

        let mut ai = screening_ai();
        ai.recon_disruption.sightings.insert(settler, (behind, 59));
        ai.recon_disruption_plan(&game, 0);
        let orders: Vec<ScreenOrder> = [scout, warrior]
            .into_iter()
            .filter_map(|uid| ai.recon_disruption.screen(uid).copied())
            .collect();
        assert_eq!(
            orders.len(),
            2,
            "both units in reach take a stand: {orders:?}"
        );
        let stands: BTreeSet<Pos> = orders.iter().map(ScreenOrder::tile).collect();
        assert_eq!(stands.len(), 2, "the stands are distinct");
        for order in &orders {
            assert!(matches!(order, ScreenOrder::Stand { .. }));
            // Ahead: beside the Settler and two steps from where it came
            // from — the three forward tiles, not the two flanking its trail.
            let at = order.tile();
            assert_eq!(
                game.wdist(at, field),
                1,
                "a stand {at:?} is beside the Settler"
            );
            assert_eq!(
                game.wdist(at, behind),
                2,
                "a stand {at:?} lies ahead of the Settler"
            );
        }
        // Walk both; the engine then refuses the Settler both tiles.
        for uid in [scout, warrior] {
            assert_eq!(ai.recon_disruption_step(&mut game, 0, uid), Some(true));
            assert!(stands.contains(&game.units[&uid].pos));
        }
        for stand in &stands {
            assert!(!game.can_move(settler, *stand));
        }
    }

    #[test]
    fn the_screen_is_off_by_default_and_draws_no_orders_when_off() {
        let (mut game, field) = peace_field(33);
        let settler = game.spawn_test_unit("settler", 1, field);
        let scout = game.spawn_test_unit("scout", 0, game.nbrs(field).into_iter().min().unwrap());
        fresh(&mut game, scout);
        fresh(&mut game, settler);
        let mut ai = AdvancedAi::new();
        ai.recon_disruption_plan(&game, 0);
        assert_eq!(ai.recon_disruption.turn, None);
        assert_eq!(ai.recon_disruption_step(&mut game, 0, scout), None);
        assert_eq!(
            game.units[&scout].pos,
            game.nbrs(field).into_iter().min().unwrap()
        );
    }

    #[test]
    fn a_settler_of_a_civ_at_war_or_out_of_sight_is_not_screened() {
        let (mut game, field) = peace_field(34);
        let settler = game.spawn_test_unit("settler", 1, field);
        let scout = game.spawn_test_unit("scout", 0, game.nbrs(field).into_iter().min().unwrap());
        fresh(&mut game, scout);
        fresh(&mut game, settler);
        let mut ai = screening_ai();
        game.at_war.insert((0, 1));
        ai.recon_disruption_plan(&game, 0);
        assert!(
            ai.recon_disruption.screen(scout).is_none(),
            "at war the raid owns the Settler"
        );
        game.at_war.clear();
        // Out of sight: the Scout far away, nothing else of ours near.
        let far = game
            .map
            .tiles
            .keys()
            .copied()
            .find(|pos| game.wdist(*pos, field) == 6 && game.city_at(*pos).is_none())
            .unwrap();
        game.relocate(scout, far);
        assert!(!game.player_can_see(0, field));
        ai.recon_disruption_plan(&game, 0);
        assert!(
            ai.recon_disruption.screen(scout).is_none(),
            "an unseen Settler is not read"
        );
    }

    /// Two capitals joined by a one-tile corridor through mountains, every
    /// other tile of the board a mountain: every corridor tile is a pass.
    fn corridor_board() -> (Game, Vec<Pos>, Pos, Pos) {
        let (mut game, _) = peace_field(35);
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        let away = game.cities[&game.player_city_ids(1)[0]].pos;
        let flat = |pos: Pos| game.map.get(pos).is_some();
        let walk = AdvancedAi::land_walk(&game, away, home, &flat, None).expect("a flat walk");
        let keep: BTreeSet<Pos> = game
            .wdisk(home, 3)
            .into_iter()
            .chain(game.wdisk(away, 3))
            .chain(walk.iter().copied())
            .collect();
        for pos in game.map.tiles.keys().copied().collect::<Vec<_>>() {
            if !keep.contains(&pos) {
                mountain(&mut game, pos);
            }
        }
        (game, walk, home, away)
    }

    #[test]
    fn an_idle_scout_holds_the_pass_toward_the_neighbour() {
        let (mut game, walk, home, away) = corridor_board();
        let scout = game.spawn_test_unit("scout", 0, home);
        fresh(&mut game, scout);
        let mut ai = AdvancedAi::new();
        ai.enable_pass_picket();
        ai.recon_disruption_plan(&game, 0);
        let post = *ai
            .recon_disruption
            .post(1)
            .expect("the neighbour has a post");
        assert!(post.pass, "every corridor tile cuts the walk");
        assert!(walk.contains(&post.at));
        assert!(game.wdist(post.at, away) >= PICKET_MIN_FROM_CITY);
        assert!(
            game.map.tiles[&post.at]
                .owner_city
                .and_then(|city| game.cities.get(&city))
                .is_none_or(|city| city.owner == 0),
            "the post is never inside the rival's borders"
        );
        // The forward-most pass: the post cuts the walk and no standable
        // tile nearer the rival city does (the open ground round the city).
        let passable = |pos: Pos| {
            game.map
                .get(pos)
                .is_some_and(|tile| !game.rules.is_water(tile) && game.rules.is_passable(tile))
        };
        assert!(AdvancedAi::land_walk(&game, away, home, &passable, Some(post.at)).is_none());
        for pos in walk.iter().copied().filter(|pos| {
            game.wdist(*pos, away) < game.wdist(post.at, away)
                && game.wdist(*pos, away) >= PICKET_MIN_FROM_CITY
                && AdvancedAi::post_stands(&game, 0, *pos)
        }) {
            assert!(
                AdvancedAi::land_walk(&game, away, home, &passable, Some(pos)).is_some(),
                "{pos:?} nearer the rival city does not cut the walk"
            );
        }
        assert_eq!(ai.recon_disruption.picket(scout), Some(post.at));
        // Nothing to explore: the Scout walks to the post over the turns and
        // then holds it.
        let mut turns = 0;
        loop {
            fresh(&mut game, scout);
            game.turn += 1;
            ai.recon_disruption_plan(&game, 0);
            let step = ai.recon_disruption_step(&mut game, 0, scout);
            if game.units[&scout].pos == post.at {
                assert_eq!(ai.recon_disruption_step(&mut game, 0, scout), Some(false));
                break;
            }
            assert_eq!(step, Some(true), "the Scout walks toward its post");
            turns += 1;
            assert!(turns < 30, "the post is reached within thirty turns");
        }
        assert_eq!(
            ai.recon_disruption.post(1).map(|post| post.at),
            Some(post.at)
        );
    }

    #[test]
    fn the_border_watch_is_the_first_tile_outside_the_border_when_nothing_cuts_the_walk() {
        let (mut game, _) = peace_field(36);
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        let away = game.cities[&game.player_city_ids(1)[0]].pos;
        let scout = game.spawn_test_unit("scout", 0, home);
        fresh(&mut game, scout);
        let mut ai = AdvancedAi::new();
        ai.enable_pass_picket();
        ai.recon_disruption_plan(&game, 0);
        let post = *ai
            .recon_disruption
            .post(1)
            .expect("the neighbour has a post");
        assert!(
            !post.pass,
            "an open field has no single tile that cuts the walk"
        );
        assert!(
            game.wdist(post.at, away) < game.wdist(post.at, home),
            "the watch stands on their side"
        );
        assert!(game.map.tiles[&post.at]
            .owner_city
            .and_then(|city| game.cities.get(&city))
            .is_none_or(|city| city.owner == 0));
        assert!(game.wdist(post.at, away) >= PICKET_MIN_FROM_CITY);
    }

    /// The claim is checked in a real game: on a small four-major board with
    /// both genes on for every seat, the plan draws screen stands and picket
    /// posts — including a pass — before the turn limit.
    #[test]
    fn both_genes_draw_orders_in_a_real_game() {
        use crate::ai::Ai;
        let mut game = Game::new_full(4, 44, 28, 26_082_413, 140, 2, true);
        // This pins the two recon genes on the board they were written
        // against; the barbarian seat's rung moved to Immortal by default on
        // 2026-08-24 and is not what is under test here.
        game.set_barbarian_difficulty("prince").unwrap();
        game.set_fog_memory(false);
        game.set_war_ledger(false);
        let mut ais: Vec<AdvancedAi> = (0..game.players.len())
            .map(|_| {
                let mut ai = AdvancedAi::new();
                ai.enable_settler_screen();
                ai.enable_pass_picket();
                ai
            })
            .collect();
        let (mut stands, mut pursuits, mut pickets, mut passes) = (0usize, 0usize, 0usize, 0usize);
        while game.winner.is_none() && game.turn <= game.max_turns {
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            let plan = &ais[pid].recon_disruption;
            if plan.turn == Some(game.turn) {
                for order in plan.screens.values() {
                    match order {
                        ScreenOrder::Stand { .. } => stands += 1,
                        ScreenOrder::Pursue { .. } => pursuits += 1,
                    }
                }
                pickets += plan.pickets.len();
                passes += plan.posts.values().filter(|post| post.pass).count();
            }
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }
        assert!(
            stands > 0,
            "a stand was taken (stands {stands}, pursuits {pursuits})"
        );
        assert!(pickets > 0, "a recon unit was sent to a post");
        assert!(
            passes > 0,
            "a pass was found on some walk ({pickets} picket-turns)"
        );
    }

    #[test]
    fn a_settler_screen_outranks_the_picket_for_the_same_scout() {
        let (mut game, field) = peace_field(37);
        let settler = game.spawn_test_unit("settler", 1, field);
        let scout = game.spawn_test_unit("scout", 0, game.nbrs(field).into_iter().min().unwrap());
        fresh(&mut game, scout);
        fresh(&mut game, settler);
        let mut ai = screening_ai();
        ai.enable_pass_picket();
        ai.recon_disruption
            .sightings
            .insert(settler, (game.nbrs(field).into_iter().max().unwrap(), 59));
        ai.recon_disruption_plan(&game, 0);
        assert!(ai.recon_disruption.screen(scout).is_some());
        assert!(
            ai.recon_disruption.picket(scout).is_none(),
            "a screening unit is not a picket"
        );
    }
}
