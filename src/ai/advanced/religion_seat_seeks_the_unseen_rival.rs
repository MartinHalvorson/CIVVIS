//! `religion-seat-seeks-the-unseen-rival`: every rival whose cities the seat
//! cannot see enough of to hold its majority gets a spreader of its own to
//! look for them, now, while the rest of the corps converts.
//!
//! **Why (three live Deity Religious-target games, 6 majors, Continents,
//! Online, 2026-11-18).** A spreader can only be sent to a city the seat has
//! seen, and each of these games ended with the last rival's cities still
//! coming into view:
//! - Mali civvis-20261118T082214Z lost to a rival's Science at t171 with 29
//!   spreaders holding 83 unspent charges, Sumeria at 6 of 12 cities and
//!   Portugal at 4 of 10. Portugal was met at t119 and its first city seen
//!   at t159, with our corps on its continent from t128: at t150 the
//!   explored edge of the Netherlands ran through Tilburg, four tiles from
//!   Viseu, and Portuguese ground was first seen that turn two tiles from
//!   Beja. Once seen, Portugal went from none to four cities of ours in
//!   eleven turns.
//! - Khmer civvis-20261118T102229Z held four of five rivals at t180 and lost
//!   to the fifth's Science about t190. Ethiopia was met at t134 and one of
//!   its fifteen cities was on the map from that turn to t160 (three at
//!   t170, nine at t180), with 12 to 28 spreaders in the field.
//! - Byzantium civvis-20261118T094102Z (lost at t208) knew four to six of
//!   Japan's ten or eleven cities from t160 to t195 and ten at t200; Japan
//!   followed our faith four turns later.
//!
//! `religion-seat-charts-the-rivals` gives the search to its scouts: the
//! spreaders with the fewest charges left, one for each rival still short of
//! a charted majority, at most one in four of the corps. With
//! `last-charge-joins-the-army` armed beside it those are the units on their
//! last charge, which that gene has already given their own turn (a kept
//! Missionary walks its own expedition, a kept Apostle hunts). Read from the
//! state rows every eight turns, the three games had a rival short of a
//! charted majority on 28 boards and the quota named no unit that reaches
//! the scout's step on 25 of them. What charts then is a spreader with
//! nothing left to convert or the corps' surplus over its known work, and
//! all of it goes to the ground within nine tiles of a known city of a
//! rival, so a rival with no known city waits behind every located one
//! (Portugal behind the Netherlands to t148 and Sumeria to t158).
//!
//! **What the gene does**, where `religion-seat-charts-the-rivals` acts (a
//! founded seat aiming at Religion, on the offensive):
//! - **Its own seeker.** Each living major we have met with no city on our
//!   map has one spreader looking for it, and the rivals we know too few
//!   cities of (`BasicAi::rival_is_under_charted`) have one each as well, up
//!   to one in [`SCOUT_SHARE`] of the corps. A seeker is a Missionary or an
//!   Apostle of our faith with a charge that `last-charge-joins-the-army`
//!   has not kept and that is not due home to heal; the one farthest from
//!   our capital is taken first (it stands deepest in foreign ground), and a
//!   seeker stays one while there is a rival to look for. It keeps its
//!   charges, so the turn a city comes into view it is a spreader beside
//!   it. The corps' last spreader is never taken: a corps of one spreads.
//! - **A rival with no city on the map.** Ground a met major owns whose
//!   city we have not seen (`Game::unseen_major_borders`, a live board's
//!   reading) is a border with its city in the fog behind it: its seeker
//!   makes for the unexplored tile nearest it within [`BORDER_LEAD_RADIUS`]
//!   of such ground. With no such lead it takes the fog an explorer of ours
//!   would take from where it stands, water included
//!   (`BasicAi::exploration_goal`): into the unexplored interior beside it,
//!   not round the edge of the cities already known.
//! - **A rival known by too few cities.** Its seeker takes the charting goal
//!   of `religion-seat-charts-the-rivals` (the unexplored ground beside the
//!   nearest known city of such a rival), and the lead or the explorer's
//!   fog when none of that is left.
//!
//! A goal is kept until it is seen or cannot be reached, lies outside every
//! visible raider's reach, and is one this unit has a route to. A seeker
//! with nowhere to go has its ordinary turn. Off, or with
//! `religion-seat-charts-the-rivals` off, or once every met rival is charted
//! to its majority, nothing changes.

use super::*;
// One spreader in this many may seek a rival that is on the map but short of
// a charted majority: the share `religion-seat-charts-the-rivals` gives its
// own scouts.
use super::religion_seat_charts_the_rivals::SCOUT_SHARE;

/// How far from unattributed major ground the fog is read for the city that
/// owns it: a tile is owned by a city at most a few rings away
/// (`scout_distance::RIVAL_LEAD_RADIUS` reads the same three for a Scout).
const BORDER_LEAD_RADIUS: i32 = 3;

/// How far from the seeker the nearest fog is searched when the explorer's
/// own pick has no route for it.
const SEEK_RADIUS: i32 = 12;

/// The fog tiles tried for a route in that search, nearest first.
const SEEK_ROUTE_TRIES: usize = 8;

impl AdvancedAi {
    /// Whether the gene acts for this seat: on, and
    /// `religion-seat-charts-the-rivals` acts for it.
    fn seeks_the_unseen_rival(&self, g: &Game, pid: usize) -> bool {
        self.religion_seat_seeks_the_unseen_rival
            && self.religion_seat_charts_the_rivals_active(g, pid)
    }

    /// Living majors `pid` has met and holds no city of on its map
    /// (`BasicAi::contact_fleet_city_known`).
    pub(super) fn unseen_met_rivals(g: &Game, pid: usize) -> usize {
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !rival.is_free_city
                    && !g.same_team(pid, rival.id)
                    && g.has_met(pid, rival.id)
                    && !BasicAi::contact_fleet_city_known(g, pid, rival.id)
            })
            .count()
    }

    /// Whether `uid` may seek: a Missionary or an Apostle of our faith with a
    /// charge, one `last-charge-joins-the-army` has not kept (those hunt and
    /// scout on their own turn), above the floor `corps-comes-home-alive`
    /// sends a unit home at.
    fn may_seek(&self, g: &Game, pid: usize, uid: u32, religion: &str) -> bool {
        g.units.get(&uid).is_some_and(|unit| {
            unit.owner == pid
                && matches!(unit.kind.as_str(), "missionary" | "apostle")
                && unit.charges > 0
                && unit.religion.as_deref() == Some(religion)
                && unit.hp > super::apostle_army::RETREAT_FLOOR
        }) && !self.keeps_its_last_charge(g, pid, uid)
    }

    /// This turn's seekers: one for each met rival with no city on our map,
    /// one for each rival short of a charted majority up to one in
    /// [`SCOUT_SHARE`] of the corps, and never the corps' last spreader. The
    /// units already seeking come first, then the spreaders farthest from
    /// our capital (then the lower id); the first are the unseen rivals'.
    /// Empty when the gene does not act, off the offensive, for another
    /// faith, or when every met rival is charted to its majority.
    pub(super) fn unseen_rival_seekers(
        &self,
        g: &Game,
        pid: usize,
        religion: &str,
        offensive: bool,
    ) -> Vec<u32> {
        if !offensive
            || !self.seeks_the_unseen_rival(g, pid)
            || g.players[pid].religion.as_deref() != Some(religion)
        {
            return Vec::new();
        }
        let unseen = Self::unseen_met_rivals(g, pid);
        let located = BasicAi::under_charted_majors(g, pid).saturating_sub(unseen);
        if unseen + located == 0 {
            self.rival_seekers.borrow_mut().clear();
            return Vec::new();
        }
        let home = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| &g.cities[&cid])
            .find(|city| city.is_capital)
            .or_else(|| g.players[pid].holy_city.and_then(|cid| g.cities.get(&cid)))
            .map(|city| city.pos);
        let seeking = self.rival_seekers.borrow();
        let mut corps: Vec<(bool, std::cmp::Reverse<i32>, u32)> = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && self.may_seek(g, pid, unit.id, religion))
            .map(|unit| {
                (
                    !seeking.contains_key(&unit.id),
                    std::cmp::Reverse(home.map_or(0, |home| g.wdist(home, unit.pos))),
                    unit.id,
                )
            })
            .collect();
        corps.sort();
        // One for each rival with no city on the map, one in `SCOUT_SHARE`
        // of the corps for the rivals short of a charted majority, and never
        // the last spreader: a corps of one spreads.
        let seekers =
            (unseen + located.min(corps.len() / SCOUT_SHARE)).min(corps.len().saturating_sub(1));
        corps
            .into_iter()
            .take(seekers)
            .map(|(_, _, id)| id)
            .collect()
    }

    /// The unexplored tile nearest `uid` within [`BORDER_LEAD_RADIUS`] of
    /// ground a met major owns whose city we have not seen, outside every
    /// visible raider's reach and on a route this unit can follow. `None` on
    /// a native board, which has no such ground.
    fn seeker_border_lead(
        g: &Game,
        pid: usize,
        uid: u32,
        threats: &[BarbarianCaptureThreat],
    ) -> Option<Pos> {
        if g.unseen_major_borders.is_empty() {
            return None;
        }
        let origin = g.units[&uid].pos;
        let explored = &g.players[pid].explored;
        let mut considered = BTreeSet::new();
        let mut fog: Vec<(i32, Pos)> = g
            .unseen_major_borders
            .iter()
            .flat_map(|border| g.wdisk(*border, BORDER_LEAD_RADIUS))
            .filter(|position| considered.insert(*position))
            .filter(|position| {
                *position != origin
                    && !explored.contains(position)
                    && g.unit_can_traverse(uid, *position)
                    && !Self::religious_tile_in_reach(g, pid, *position, threats)
            })
            .map(|position| (g.wdist(origin, position), position))
            .collect();
        fog.sort();
        fog.into_iter()
            .map(|(_, position)| position)
            .find(|position| g.route_step(uid, *position, 0).is_some())
    }

    /// Where the seeker `uid` goes: its kept goal while that is unexplored,
    /// safe and reachable; else, for a rival known by too few cities
    /// (`beside`), the charting goal of `religion-seat-charts-the-rivals`;
    /// else the border lead; else an explorer's fog. The flag is whether the
    /// goal is a border lead. See the module.
    fn seeker_goal(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        religion: &str,
        beside: bool,
    ) -> Option<(Pos, bool)> {
        let origin = g.units[&uid].pos;
        let threats = self.religious_raider_threats(g, pid);
        let open = |position: Pos| {
            position != origin
                && !g.players[pid].explored.contains(&position)
                && !Self::religious_tile_in_reach(g, pid, position, &threats)
                && g.route_step(uid, position, 0).is_some()
        };
        let kept = self.rival_seekers.borrow().get(&uid).copied();
        if let Some((goal, lead)) = kept.filter(|(goal, _)| open(*goal)) {
            return Some((goal, lead));
        }
        if beside {
            if let Some(goal) = self.religion_seat_charting_goal(g, pid, uid, religion, true) {
                return Some((goal, false));
            }
        }
        Self::seeker_border_lead(g, pid, uid, &threats)
            .map(|goal| (goal, true))
            .or_else(|| {
                self.base
                    .exploration_goal(g, pid, uid, false)
                    .filter(|goal| open(*goal))
                    // The explorer's pick is not route-checked for a unit
                    // that walks through closed borders: the nearest fog it
                    // can reach, then.
                    .or_else(|| {
                        let mut fog: Vec<(i32, Pos)> = g
                            .wdisk(origin, SEEK_RADIUS)
                            .into_iter()
                            .filter(|position| {
                                !g.players[pid].explored.contains(position)
                                    && g.unit_can_traverse(uid, *position)
                            })
                            .map(|position| (g.wdist(origin, position), position))
                            .collect();
                        fog.sort();
                        fog.into_iter()
                            .map(|(_, position)| position)
                            .filter(|position| {
                                !Self::religious_tile_in_reach(g, pid, *position, &threats)
                            })
                            .take(SEEK_ROUTE_TRIES)
                            .find(|position| open(*position))
                    })
                    .map(|goal| (goal, false))
            })
    }

    /// The seeker's turn: `uid` is one of [`Self::unseen_rival_seekers`] and
    /// has somewhere to look, so it walks there instead of spreading. False
    /// when it is not a seeker or has no goal, which leaves it its ordinary
    /// turn.
    pub(super) fn religion_seat_seeker_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        religion: &str,
        offensive: bool,
    ) -> bool {
        let seekers = self.unseen_rival_seekers(g, pid, religion, offensive);
        if seekers.is_empty() {
            return false;
        }
        // The first seekers are the unseen rivals'; the rest chart beside a
        // known city of a rival short of its majority.
        let unseen = Self::unseen_met_rivals(g, pid);
        let goal = seekers
            .iter()
            .position(|seeker| *seeker == uid)
            .and_then(|rank| self.seeker_goal(g, pid, uid, religion, rank >= unseen));
        {
            let mut seeking = self.rival_seekers.borrow_mut();
            seeking.retain(|other, _| *other != uid && seekers.contains(other));
            if let Some(goal) = goal {
                seeking.insert(uid, goal);
            }
        }
        let Some((goal, lead)) = goal else {
            return false;
        };
        think!(self.journal(), Faith, Decision,
            "{} spreader {} seeks a rival's unseen cities", religion, uid;
            "religion-seat-seeks-the-unseen-rival: {} met rival(s) hold no city on our map and we know too few cities of {} more to hold their majorities; a city we have not seen cannot be converted, so this unit looks for them while the corps works, making for {} {} tiles away",
            unseen,
            BasicAi::under_charted_majors(g, pid).saturating_sub(unseen),
            if lead { "the fog behind a border whose city we have not seen" } else { "unexplored ground" },
            g.wdist(g.units[&uid].pos, goal);
            goal);
        self.religious_step_toward_range(g, pid, uid, goal, 0)
    }
}

#[cfg(test)]
mod tests;
