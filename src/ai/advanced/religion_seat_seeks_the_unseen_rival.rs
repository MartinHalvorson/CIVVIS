//! `religion-seat-seeks-the-unseen-rival`: a rival the seat has met and holds
//! no city of on its map gets a spreader of its own to look for it, now,
//! while the rest of the corps converts and charts.
//!
//! **Why (live Deity Mali, 6 majors, Continents, Online,
//! civvis-20261118T082214Z).** The seat lost to a rival's Science at t171
//! with 29 spreaders holding 83 unspent charges: the host read our faith as
//! the majority in three of five rivals, Sumeria stood at 6 of 12 cities and
//! Portugal at 4 of 10. Its three overseas rivals were met at t109, t116 and
//! t119 (Cartography at t113) and their first cities were seen at t131, t122
//! and t159. Portugal took forty turns, with our corps on its continent from
//! t128: at t150 the explored edge of the Netherlands ran through Tilburg,
//! four tiles from Viseu, and Portuguese ground was first seen that turn two
//! tiles from Beja. Once seen, Portugal went from none to four cities of
//! ours in eleven turns.
//!
//! `religion-seat-charts-the-rivals` sends its scouts and its idle spreaders
//! to the unexplored ground within nine tiles of a known city of a rival
//! still short of a charted majority, and gives a rival with no known city
//! the long expedition only when no such ground is left. So the unseen rival
//! waits behind every located one: the Netherlands until it was charted
//! (seven of ten cities, t148), then Sumeria (t158), and Portugal's first
//! city came the next turn. A rival charted to its majority also takes its
//! cities out of the search, and the fog on its far side with them.
//!
//! **What the gene does**, where `religion-seat-charts-the-rivals` acts (a
//! founded seat aiming at Religion, on the offensive), while a living major
//! we have met has no city on our map:
//! - **Its own seeker.** One spreader of our faith for each such rival looks
//!   for it instead of spreading or charting a located rival's ground. A
//!   seeker is a Missionary or an Apostle with a charge that
//!   `last-charge-joins-the-army` has not kept and that is not due home to
//!   heal; the one farthest from our capital is taken first (it stands
//!   deepest in foreign ground), and a seeker stays one until its rival is
//!   found. It keeps its charges: the turn the rival's first city is seen it
//!   is a spreader again, beside that city.
//! - **The lead.** Ground a met major owns whose city we have not seen
//!   (`Game::unseen_major_borders`, a live board's reading) is a border with
//!   its city in the fog behind it. The seeker makes for the unexplored tile
//!   nearest it within [`BORDER_LEAD_RADIUS`] of such ground.
//! - **Otherwise it explores.** With no such lead it takes the fog an
//!   explorer of ours would take from where it stands, water included
//!   (`BasicAi::exploration_goal`): into the unexplored interior beside it,
//!   not round the edge of the cities already known. Either goal is kept
//!   until it is seen or cannot be reached, lies outside every visible
//!   raider's reach, and is one this unit has a route to.
//!
//! A seeker with nowhere to go has its ordinary turn. Off, or with
//! `religion-seat-charts-the-rivals` off, or once every met rival has a city
//! on the map, nothing changes.

use super::*;

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

    /// This turn's seekers, one for each met rival with no city on our map:
    /// the units already seeking first, then the spreaders farthest from our
    /// capital (then the lower id). Empty when the gene does not act, off the
    /// offensive, for another faith, or when every met rival is located.
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
        let wanted = Self::unseen_met_rivals(g, pid);
        if wanted == 0 {
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
        corps
            .into_iter()
            .take(wanted)
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
    /// safe and reachable, else the border lead, else an explorer's fog. See
    /// the module.
    fn seeker_goal(&self, g: &Game, pid: usize, uid: u32) -> Option<(Pos, bool)> {
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
        let goal = seekers
            .contains(&uid)
            .then(|| self.seeker_goal(g, pid, uid))
            .flatten();
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
            "{} spreader {} seeks a rival we have never seen", religion, uid;
            "religion-seat-seeks-the-unseen-rival: {} met rival(s) hold no city on our map, and a city we have not seen cannot be converted; this unit looks for them while the corps works, making for {} {} tiles away",
            Self::unseen_met_rivals(g, pid),
            if lead { "the fog behind a border whose city we have not seen" } else { "the fog an explorer would take from here" },
            g.wdist(g.units[&uid].pos, goal);
            goal);
        self.religious_step_toward_range(g, pid, uid, goal, 0)
    }
}

#[cfg(test)]
mod tests;
