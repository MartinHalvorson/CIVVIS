//! `siege-guns-take-a-reachable-post`: a siege gun that has held one firing
//! post for [`GUN_POST_PATIENCE_TURNS`] assessments without standing on it
//! gives that post up for one it can reach this turn or next; a soldier of
//! ours (not a gun, not the taker) standing on such a post steps off it; and
//! once one of our guns stands within its range of the city, the Stage
//! escorts stop holding at the muster line "while the train cannot close".
//!
//! **Why (live Immortal civvis-20261115T230117Z, Gilgamesh, turns 89-125).**
//! At war with the Mapuche at two to four times their strength, with up to
//! four Bombards and four Field Cannons, the seat never took Puel Mapu (axial
//! (19, 12)). `siege_posts_keeping` drew a firing post only on a tile no unit
//! stands on, and at turn 106 our Pikemen and Crossbowmen held the free
//! near-side tiles of the range-2 band, (17, 13), (18, 14) and (20, 13); the
//! last two Bombards were posted to (20, 10) and (21, 10), across the city,
//! behind the Roraima tiles and the city's own first ring, which the train's
//! router never crosses. The kept-post rule then held (20, 10) from turn 106
//! to 111 because some route to it existed, however long: the why log reads
//! "the post (20, 10) 8 away", 7, 6, 6, 5 and 7 tiles away again while the
//! Bombard stood three to six tiles from the city, though (17, 13) was free on
//! turns 107-108 and (18, 14) on 108 and 110. 47 of the 72 "stops short of
//! its post" lines over turns 104-123 read "out of moves" on such a walk. The
//! siege fell back to Stage at turn 113 with the walls at 311/400, and the
//! peace at turn 126 cost one of our own cities.
//!
//! Off, nothing here runs: the stalled set is empty, so the post search is
//! the old one, nobody is asked to step aside and the muster gate is read
//! as before.

use super::*;

/// Turns a gun may hold one firing post without standing on it before the
/// post is drawn again among tiles it can reach. A post on the gun's own
/// side of the band is one or two tiles from the staging ring
/// ([`STAGING_NEAR`] to [`STAGING_FAR`] against a range-2 band), so a
/// two-move gun stands on it within two turns of walking; a post it still
/// has not reached by then lies across the city or behind our own column.
/// Live 230117Z held (20, 10) for six.
pub(in crate::ai::advanced) const GUN_POST_PATIENCE_TURNS: u32 = 2;

/// A gun's firing post as the train last drew it: the tile, the turn it was
/// first drawn there, and whether this gene drew it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::ai::advanced) struct HeldPost {
    pub(in crate::ai::advanced) post: Pos,
    pub(in crate::ai::advanced) since: u32,
    pub(in crate::ai::advanced) repicked: bool,
}

/// The guns of last turn's `posts` that have held the same post for
/// [`GUN_POST_PATIENCE_TURNS`] turns or more without standing on it, and
/// those still walking to a post this gene drew: their search runs again each
/// turn until they stand on it, so a post one of ours has not yet stepped off
/// is not dropped for the old far one meanwhile.
pub(super) fn stalled_guns(
    g: &Game,
    posts: &BTreeMap<u32, Pos>,
    held: &BTreeMap<u32, HeldPost>,
    turn: u32,
) -> BTreeSet<u32> {
    posts
        .iter()
        .filter(|(uid, post)| {
            arm_of(g, **uid) == Arm::Siege
                && g.units.get(uid).is_some_and(|unit| unit.pos != **post)
                && held.get(uid).is_some_and(|held| {
                    held.post == **post
                        && (held.repicked
                            || turn.saturating_sub(held.since) >= GUN_POST_PATIENCE_TURNS)
                })
        })
        .map(|(uid, _)| *uid)
        .collect()
}

/// Record this turn's gun posts for `force`. A gun on its post, or without
/// one, is forgotten; a post drawn on a new tile starts its clock.
pub(super) fn note_posts(
    g: &Game,
    force: &[u32],
    posts: &BTreeMap<u32, Pos>,
    repicked: &BTreeSet<u32>,
    held: &mut BTreeMap<u32, HeldPost>,
    turn: u32,
) {
    held.retain(|uid, _| g.units.contains_key(uid));
    for uid in force {
        if arm_of(g, *uid) != Arm::Siege {
            continue;
        }
        let Some(post) = posts
            .get(uid)
            .copied()
            .filter(|post| g.units.get(uid).is_some_and(|unit| unit.pos != *post))
        else {
            held.remove(uid);
            continue;
        };
        let again = repicked.contains(uid);
        // The same post, not drawn again this turn, keeps its clock.
        if !again && held.get(uid).is_some_and(|known| known.post == post) {
            continue;
        }
        held.insert(
            *uid,
            HeldPost {
                post,
                since: turn,
                repicked: again,
            },
        );
    }
}

/// Whether the soldiers on `pos` would step off a gun's post: every one is
/// ours, a land soldier, neither a gun nor the siege's taker.
fn clears_for_a_gun(g: &Game, pid: usize, pos: Pos, taker: Option<u32>) -> bool {
    let here = g.unit_ids_at(pos);
    !here.is_empty()
        && here.iter().all(|other| {
            g.units.get(other).is_some_and(|unit| unit.owner == pid)
                && matches!(arm_of(g, *other), Arm::Melee | Arm::Shooter)
                && Some(*other) != taker
        })
}

/// Turns (0 this turn, 1 next) for `uid` to stand on `pos`. A free tile is
/// reached this turn when it lies in the gun's movement, else by the
/// router's walk while every stand on it is dry and off the city's first
/// ring; a tile one of ours holds is reached this turn from beside it and
/// next turn from a tile beside it the gun reaches now. `None` beyond that.
fn reach_turns(
    g: &Game,
    uid: u32,
    pos: Pos,
    city: Pos,
    held_by_ours: bool,
    this_turn: &BTreeSet<Pos>,
) -> Option<u32> {
    let here = g.units.get(&uid)?.pos;
    if held_by_ours {
        if g.wdist(here, pos) == 1 {
            return Some(0);
        }
        return g
            .nbrs(pos)
            .into_iter()
            .any(|beside| this_turn.contains(&beside) && g.wdist(beside, city) > 1)
            .then_some(1);
    }
    if this_turn.contains(&pos) {
        return Some(0);
    }
    let walk = g.route_preview(uid, pos)?;
    let (_, turn) = *walk.last()?;
    let clear = walk
        .iter()
        .all(|(step, _)| dry_stand(g, uid, *step) && (*step == pos || g.wdist(*step, city) > 1));
    (clear && turn <= 1).then_some(turn)
}

/// Ranking of a post that [`repick`] can reach: out of every other hostile
/// city's strike, the farthest band, a free tile before one of ours steps
/// off it, this turn before next, then `siege_posts_keeping`'s own order.
type PostRank = (bool, Reverse<i32>, bool, u32, bool, bool, usize, i32, Pos);

/// A firing post for a stalled gun among the tiles it can stand on this turn
/// or next: within `range` of the city with a line to it, dry open land, off
/// the taker's corridor and every post already drawn, free or held only by
/// soldiers of ours that step off it (see [`clears_for_a_gun`]). Ranked as
/// [`PostRank`] reads. `None` when no such tile exists.
#[allow(clippy::too_many_arguments)]
pub(super) fn repick(
    g: &Game,
    pid: usize,
    uid: u32,
    city: &CityView,
    taker: Option<u32>,
    range: i32,
    corridor: Option<Pos>,
    taken: (&BTreeSet<Pos>, &BTreeSet<Pos>),
    hostiles: &[Pos],
    third: &[Pos],
) -> Option<Pos> {
    let (fire_taken, ring_taken) = taken;
    let here = g.units.get(&uid)?.pos;
    let this_turn: BTreeSet<Pos> = g.reachable(uid).into_iter().collect();
    let mut best: Option<PostRank> = None;
    for distance in 1..=range {
        for pos in g.wring(city.pos, distance) {
            let open_land = g
                .map
                .get(pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile));
            if pos == here
                || Some(pos) == corridor
                || fire_taken.contains(&pos)
                || ring_taken.contains(&pos)
                || !open_land
                || !g.unit_can_traverse(uid, pos)
                || !g.unit_has_line_of_sight_from(uid, pos, city.pos)
            {
                continue;
            }
            let held_by_ours = !g.unit_ids_at(pos).is_empty();
            if held_by_ours && !clears_for_a_gun(g, pid, pos, taker) {
                continue;
            }
            let Some(turns) = reach_turns(g, uid, pos, city.pos, held_by_ours, &this_turn) else {
                continue;
            };
            let third_exposed = third
                .iter()
                .any(|source| g.wdist(*source, pos) <= CITY_STRIKE_RANGE);
            let far_side = g.wdist(here, pos) > g.wdist(here, city.pos);
            let behind = ring_taken.iter().any(|held| g.wdist(*held, pos) == 1);
            let exposure = hostiles.iter().filter(|h| g.wdist(**h, pos) <= 2).count();
            let rank = (
                third_exposed,
                Reverse(distance),
                held_by_ours,
                turns,
                far_side,
                !behind,
                exposure,
                g.wdist(here, pos),
                pos,
            );
            if best.as_ref().is_none_or(|known| rank < *known) {
                best = Some(rank);
            }
        }
    }
    best.map(|rank| rank.8)
}

/// One of our guns stands within its range of the city.
pub(super) fn gun_stands_in_range(g: &Game, pid: usize, city: Pos) -> bool {
    g.units.values().any(|unit| {
        unit.owner == pid
            && arm_of(g, unit.id) == Arm::Siege
            && g.wdist(unit.pos, city) <= g.unit_attack_range(unit.id).max(1)
    })
}

impl AdvancedAi {
    /// `siege-guns-take-a-reachable-post`: `uid` stands on a post this gene
    /// drew for one of our guns, and steps off it to the safest dry tile
    /// beside it off the city's first ring and every other post. `None` with
    /// the gene off, for a gun or the taker, off every such post, or when no
    /// tile beside it is free.
    pub(super) fn clear_gun_post(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        city: &CityView,
    ) -> Option<bool> {
        if !self.siege_guns_take_a_reachable_post {
            return None;
        }
        let siege = self.sieges.get(&city.id)?;
        let here = g.units.get(&uid)?.pos;
        if g.units[&uid].moves_left <= 0.0 || !clears_for_a_gun(g, pid, here, siege.taker) {
            return None;
        }
        let gun = siege.posts.iter().find_map(|(gun, post)| {
            (*post == here
                && *gun != uid
                && arm_of(g, *gun) == Arm::Siege
                && self
                    .siege_gun_posts_held
                    .get(gun)
                    .is_some_and(|held| held.repicked && held.post == here))
            .then_some(*gun)
        })?;
        let posts: BTreeSet<Pos> = siege.posts.values().copied().collect();
        // Out of the gun's way: the least danger, then the tile farthest
        // from the gun (a stand on its walk in would block it again), then
        // the nearest the city.
        let from = g.units.get(&gun)?.pos;
        let mut stands: Vec<(f64, Reverse<i32>, i32, Pos)> = g
            .reachable(uid)
            .into_iter()
            .filter(|pos| {
                g.wdist(*pos, here) == 1
                    && g.wdist(*pos, city.pos) > 1
                    && !posts.contains(pos)
                    && dry_stand(g, uid, *pos)
            })
            .map(|pos| {
                (
                    super::super::battle_planner::strike_danger(g, pid, pos, uid),
                    Reverse(g.wdist(pos, from)),
                    g.wdist(pos, city.pos),
                    pos,
                )
            })
            .collect();
        stands.sort_by(|a, b| {
            a.0.total_cmp(&b.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(&b.2))
                .then_with(|| a.3.cmp(&b.3))
        });
        let (danger, _, _, to) = *stands.first()?;
        if !self.base.path_walk_to(g, pid, uid, to) {
            return None;
        }
        let name = g
            .cities
            .get(&city.id)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        think!(self.journal(), Military, Detail,
            "siege-guns-take-a-reachable-post: the {} steps off the {}'s post before {name}", g.units[&uid].kind, g.units[&gun].kind;
            "{here:?} is the gun's post, {} tiles from the city; it steps to {to:?}, reading {danger:.0} danger",
            g.wdist(here, city.pos);
            city.pos);
        Some(true)
    }

    /// `siege-guns-take-a-reachable-post`: before gun `uid` walks to `post`,
    /// the soldier of ours standing on it steps off (see
    /// [`Self::clear_gun_post`]). Nothing with the gene off.
    pub(super) fn clear_post_for_gun(
        &mut self,
        g: &mut Game,
        pid: usize,
        post: Pos,
        city: &CityView,
    ) {
        if !self.siege_guns_take_a_reachable_post {
            return;
        }
        let blockers: Vec<u32> = g.unit_ids_at(post).to_vec();
        for blocker in blockers {
            self.clear_gun_post(g, pid, blocker, city);
        }
    }

    /// `siege-guns-take-a-reachable-post`: journal the posts this turn's
    /// assessment drew again for stalled guns.
    pub(super) fn note_repicked_posts(
        &self,
        g: &Game,
        city: &CityView,
        repicked: &[(u32, Pos, Pos)],
    ) {
        if repicked.is_empty() || !self.journal().wants(crate::reasoning::Level::Detail) {
            return;
        }
        let name = g
            .cities
            .get(&city.id)
            .map(|c| c.name.clone())
            .unwrap_or_default();
        for (uid, old, new) in repicked {
            let Some(unit) = g.units.get(uid) else {
                continue;
            };
            let held = g
                .unit_ids_at(*new)
                .iter()
                .filter_map(|other| g.units.get(other))
                .map(|other| other.kind.to_string())
                .collect::<Vec<_>>();
            let note = if held.is_empty() {
                String::new()
            } else {
                format!("; our {} steps off it", held.join(", "))
            };
            think!(self.journal(), Military, Detail,
                "siege-guns-take-a-reachable-post: the {} takes a post it can reach before {name}", unit.kind;
                "{GUN_POST_PATIENCE_TURNS} turns on {old:?} ({} tiles away) without closing; {new:?} is {} tiles away and {} from the city{note}",
                g.wdist(unit.pos, *old), g.wdist(unit.pos, *new), g.wdist(*new, city.pos);
                city.pos);
        }
    }
}
