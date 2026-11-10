//! `religion-seat-charts-the-rivals`: a Religion-target founder keeps charting
//! until it knows enough of every rival's cities to hold a majority of them,
//! and takes Cartography sooner.
//!
//! **Why (live Deity Russia, 6 majors, Continents, Online, 2026-11-10).** A
//! Religious victory asks for a strict majority of cities in every living
//! major, and a spreader can only be sent to a city the seat has seen. Run
//! civvis-20261110T012017Z held four of five majors at t167 (Portugal 5 of 9
//! cities, America 7 of 13, Australia 4 of 7, Gran Colombia 10 of 15) and lost
//! to America's Science at t185 with Byzantium at 0 of 12: Byzantium was met
//! at t108, seven turns after Cartography (t101), and the seat knew 2 of its
//! 12 cities from t125 to t175 with a Caravel and two Galleys afloat. Nine
//! long games of 2026-11-09/10 met their overseas rivals at t102-136, knew
//! Cartography at t79-113 with Shipbuilding at t51-72, and at t100 knew 2-14
//! rival cities of the 14-37 their met rivals held; 005727Z had seen 18 of 54
//! at t199 and one rival's cities never.
//!
//! `contact-fleet` already holds a hull, lays one down and sails for foreign
//! landfall, but its gap closes on the first city of each major, which is
//! what a Spy or a ballot needs and one twelfth of what this seat needs.
//!
//! **What the gene does**, for a seat aiming at Religion that has founded:
//! - **Gap.** A met major is *under-charted* while the seat knows fewer of its
//!   cities than a strict majority of the host's public count plus
//!   [`CHARTED_SPARE_CITIES`] (never more than it has). While one is, the
//!   contact gap stays open ([`BasicAi::contact_gap_open`]), so every part of
//!   `contact-fleet` goes on: the hull hold, the sea scout's queue claim and
//!   the ocean-crossing landfall routing. It needs `contact-fleet` armed.
//! - **Research.** While a living major is unmet, unlocated or under-charted
//!   and the empire touches water, each technology on the way to Cartography
//!   is worth [`CARTOGRAPHY_VALUE`] more
//!   ([`AdvancedAi::religion_seat_cartography_value`]). In Gathering Storm
//!   that is Shipbuilding and the Currency line to Mathematics, then Buttress
//!   (`TECH_CARTOGRAPHY` requires `TECH_BUTTRESS`, `Expansion2`), which the
//!   live seats reached 28-47 turns after Shipbuilding.
//!
//! - **The hull.** The seat's last rivals are overseas, and the turn it
//!   meets them bounds the turn it can win: civvis-20261110T112233Z met its
//!   last three at t130-142 and lost at t193 holding four of five majors,
//!   its corps one to three tiles from the fifth's cities. Two rules:
//!   - *It keeps the head.* `sea-scout-claims-the-queue` lays the hull down
//!     and other queue claims take the head back. Over today's 17 long live
//!     games the journal has 73 such lines (Campus slot 29, bound Granary
//!     25, Campus building 7, backlog Builder 6, producer Industrial Zone
//!     5, Aqueduct 1); civvis-20261110T100022Z knew Cartography at t89,
//!     claimed at t91, t95, t97 and t101, lost the head 15 times and
//!     floated its first ship at t107. Those six claims now leave the
//!     committed hull alone ([`AdvancedAi::charting_hull_holds`], the
//!     `contact-fleet` commitment for this seat): one city, the hull that
//!     finishes soonest, until a ship of ours is charting.
//!   - *A technology early.* The claim waits for a ship that can cross
//!     Ocean, which is the turn Cartography lands, so the ship is a build
//!     behind. With Buttress known (`TECH_CARTOGRAPHY` requires
//!     `TECH_BUTTRESS`; 7 to 13 turns apart in four live games) the claim
//!     may lay the hull down
//!     ([`AdvancedAi::religion_seat_hull_before_cartography`]); Cartography
//!     gives every ship already afloat the Ocean.
//! - **Spreaders.** A Scout cannot enter a rival's closed borders and a
//!   religious unit can. In civvis-20261110T054430Z, with the gap and the
//!   research parts armed, the seat bought fifteen Apostles between t75 and
//!   t150 and sent no spread in those 75 turns: it knew 2-3 of Rome's 8-12
//!   cities, 0-2 of Byzantium's 6-9 and none of Arabia's 7-8, every one it
//!   knew already followed its faith, and 20 to 27 religious units stood idle
//!   while one last-charge Missionary scouted. So a spreader of ours with
//!   nothing left to convert walks to the nearest unexplored dry ground
//!   within [`CHARTING_RADIUS`] of a known city of an under-charted rival
//!   ([`AdvancedAi::religion_seat_spreader_charts`]), where its other cities
//!   stand, and when a rival under-charted has no known city at all it takes
//!   the long expedition `missionary-last-charge-explores-2` gives a
//!   last-charge Missionary. It keeps its charges; the next city it reveals
//!   is a target the turn it appears.
//! - **Scouts.** A corps that always has work never idles: in
//!   civvis-20261110T073607Z 60 of 102 spread charges went to our own cities
//!   under a neighbour's pressure, and at t157 the seat knew 0 of Australia's
//!   9 cities, 2 of France's 16, 2 of England's 8 and 4 of Brazil's 9. So
//!   while a met rival is under-charted, one spreader for each such rival, up
//!   to one in every [`SCOUT_SHARE`] of the corps, charts before it spreads
//!   ([`AdvancedAi::religion_seat_scout_charts`]): the spreaders with the
//!   fewest charges left, whose next spread is worth least.
//! - **A kept goal.** A goal chosen afresh each turn is a different tile
//!   each turn. In civvis-20261110T110341Z the journal has the corps
//!   charting on 1,638 lines between t75 and t150, and its units circled
//!   home: one walked 58 tiles between t78 and t112 for a net 16, another 52
//!   for 13; the seat knew 7 of 17 rival cities at t100, sent 8 spread
//!   orders from t50 to t125 with 28 religious units, and held its first
//!   major at t160. So a charting unit keeps its tile until it is seen or
//!   cannot be reached (`AdvancedAi::charting_goals`), and a new one is the
//!   unexplored ground nearest the under-charted rival city nearest the
//!   unit, the corps spread over the first [`CHARTING_ROUTE_TRIES`] of them.
//! - **Surplus.** A corps far larger than its known work is the same
//!   problem at scale. civvis-20261110T100022Z bought twenty Apostles
//!   between t75 and t125 and held 26 to 36 religious units while it knew 3
//!   of the 15 cities of the rivals it had met; its one priced target asked
//!   five charges and stood behind rival Apostles, and two or three scouts
//!   charted. It held no major until t144, four of five at t203, and lost to
//!   a rival's Science that turn eight Apostle charges short. So where the
//!   host's rows price the known work
//!   (`AdvancedAi::campaign_charges_known`), every spreader beyond
//!   [`SURPLUS_COVER`] charges for each charge that work asks charts too.
//!
//! Off, for a seat aiming elsewhere, or before the founding, nothing changes.

use super::*;

/// Cities beyond a strict majority the seat wants to know of each rival, so
/// one city that will not flip does not end the campaign against its owner.
pub(crate) const CHARTED_SPARE_CITIES: usize = 1;

/// How far from a known city of an under-charted rival its unexplored ground
/// is searched. Rival cities stand four to six tiles apart and a city's
/// pressure reaches ten (`RELIGION_SPREAD_ADJACENT_CITY_DISTANCE`).
pub(super) const CHARTING_RADIUS: i32 = 9;

/// The unexplored tiles tried for a route, nearest the rival's city first.
/// A unit takes one of them by its id, so a corps fans out instead of
/// queueing for one tile.
const CHARTING_ROUTE_TRIES: usize = 8;

/// One spreader in this many may be a scout. A corps of three sends none.
pub(super) const SCOUT_SHARE: usize = 4;

/// The charges the corps keeps for each charge the cities it can see still
/// ask of it, before the rest chart. Two: a spread can be lost to a death on
/// the way or undone by a rival's visit.
pub(super) const SURPLUS_COVER: u32 = 2;

/// Charges a fresh spreader carries, for turning surplus charges into units.
const CHARGES_PER_SPREADER: u32 = 3;

/// Added to a technology on the way to Cartography. The shipped water goal
/// pays 150-230 for the same step when it applies (`research_value`).
pub(super) const CARTOGRAPHY_VALUE: f64 = 300.0;

impl BasicAi {
    /// The cities of `rival` that `pid` knows: on the board on a tile it has
    /// explored, or in its city memory and no longer on the board.
    pub(crate) fn rival_cities_known(g: &Game, pid: usize, rival: usize) -> usize {
        let Some(seat) = g.players.get(pid) else {
            return 0;
        };
        g.cities
            .values()
            .filter(|city| city.owner == rival && seat.explored.contains(&city.pos))
            .count()
            + seat
                .remembered_cities
                .values()
                .filter(|city| city.owner == rival && !g.cities.contains_key(&city.id))
                .count()
    }

    /// Whether `pid` knows too few of `rival`'s cities to convert a majority
    /// of them. The count is the host's public figure, because a live board
    /// holds only the cities this seat has seen; a simulated board holds all.
    pub(crate) fn rival_is_under_charted(g: &Game, pid: usize, rival: usize) -> bool {
        let count = g
            .observed_public_empire_stats
            .get(&rival)
            .and_then(|stats| stats.city_count)
            .unwrap_or_else(|| g.player_city_ids(rival).len());
        let wanted = (count / 2 + 1 + CHARTED_SPARE_CITIES).min(count);
        Self::rival_cities_known(g, pid, rival) < wanted
    }

    /// Living majors `pid` has met and knows too few cities of.
    pub(crate) fn under_charted_majors(g: &Game, pid: usize) -> usize {
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
                    && Self::rival_is_under_charted(g, pid, rival.id)
            })
            .count()
    }

    /// The contact gap as this seat reads it: `contact-fleet`'s own, or, for
    /// a seat `religion-seat-charts-the-rivals` acts for, a met major still
    /// under-charted. See the module.
    pub(crate) fn contact_gap_open(&self, g: &Game, pid: usize) -> bool {
        Self::contact_fleet_gap(g, pid)
            || (self.charts_rival_majorities && Self::under_charted_majors(g, pid) > 0)
    }
}

impl AdvancedAi {
    /// Whether the gene acts for this seat: on, aiming at Religion, founded.
    pub(super) fn religion_seat_charts_the_rivals_active(&self, g: &Game, pid: usize) -> bool {
        self.religion_seat_charts_the_rivals
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && g.players[pid].religion.is_some()
    }

    /// Whether the hull at the head of `cid` is this seat's committed contact
    /// ship, which other queue claims leave to finish: the gene acts for the
    /// seat and `contact-fleet` holds the hull
    /// ([`Self::naval_eye_commitment`]). See the module.
    pub(super) fn charting_hull_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        self.religion_seat_charts_the_rivals_active(g, pid)
            && self.naval_eye_commitment(g, pid, cid, plan, item)
    }

    /// Whether the sea scout's queue claim may lay a hull down before a ship
    /// can cross Ocean: the gene acts for this seat and Buttress, the last
    /// technology before Cartography, is known. See the module.
    pub(super) fn religion_seat_hull_before_cartography(&self, g: &Game, pid: usize) -> bool {
        self.religion_seat_charts_the_rivals_active(g, pid)
            && g.players[pid].techs.contains(&crate::name!("buttress"))
    }

    /// What the gene adds to `tech` in the research score: zero, or
    /// [`CARTOGRAPHY_VALUE`] for a step toward Cartography while a living
    /// major is unmet, unlocated or under-charted and the empire is coastal.
    pub(super) fn religion_seat_cartography_value(&self, g: &Game, pid: usize, tech: &str) -> f64 {
        if !self.religion_seat_charts_the_rivals_active(g, pid)
            || g.players[pid].techs.contains(&crate::name!("cartography"))
            || !self.tech_leads_to(g, tech, "cartography")
            || !BasicAi::empire_is_coastal(g, pid)
        {
            return 0.0;
        }
        let (unmet, unlocated) = BasicAi::contact_fleet_missing_majors(g, pid);
        if unmet + unlocated + BasicAi::under_charted_majors(g, pid) == 0 {
            return 0.0;
        }
        CARTOGRAPHY_VALUE
    }
}

impl AdvancedAi {
    /// A spreader of ours with nothing left to convert walks into an
    /// under-charted rival's ground: unexplored dry ground within
    /// [`CHARTING_RADIUS`] of the rival city nearest it, outside every
    /// visible raider's reach, kept from turn to turn until it is seen. With
    /// no known city of an under-charted rival to start from it takes the
    /// long expedition instead. False when the gene does not act for this
    /// seat, off the offensive, for another faith's spreader, when every met
    /// rival is charted, or when no such ground can be reached.
    pub(super) fn religion_seat_spreader_charts(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        religion: &str,
        offensive: bool,
    ) -> bool {
        let Some(goal) = self.religion_seat_charting_goal(g, pid, uid, religion, offensive) else {
            return false;
        };
        think!(self.journal(), Faith, Decision,
            "{} spreader charts a rival's ground", religion;
            "religion-seat-charts-the-rivals: nothing we know is left to convert and a met rival holds cities we have not seen; a religious unit crosses closed borders, so it walks {} tiles to unexplored ground beside them",
            g.wdist(g.units[&uid].pos, goal));
        self.religious_step_toward_range(g, pid, uid, goal, 0)
    }

    /// Whether `uid` is one of the corps' scouts this turn and has ground to
    /// chart, in which case it walks there instead of spreading. The scouts
    /// are the spreaders of our faith with the fewest charges left (then the
    /// oldest), one for each met rival still under-charted, capped at one in
    /// every [`SCOUT_SHARE`] of the corps.
    pub(super) fn religion_seat_scout_charts(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        religion: &str,
        offensive: bool,
    ) -> bool {
        if !offensive
            || !self.religion_seat_charts_the_rivals_active(g, pid)
            || g.players[pid].religion.as_deref() != Some(religion)
        {
            return false;
        }
        let under_charted = BasicAi::under_charted_majors(g, pid);
        if under_charted == 0 {
            return false;
        }
        let mut corps: Vec<(i32, u32)> = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && matches!(unit.kind.as_str(), "missionary" | "apostle")
                    && unit.charges > 0
                    && unit.religion.as_deref() == Some(religion)
            })
            .map(|unit| (unit.charges, unit.id))
            .collect();
        corps.sort();
        let supply: u32 = corps.iter().map(|(charges, _)| *charges as u32).sum();
        let surplus = Self::campaign_charges_known(g, pid, religion).map_or(0, |demand| {
            (supply.saturating_sub(SURPLUS_COVER * demand) / CHARGES_PER_SPREADER) as usize
        });
        let scouts = under_charted.min(corps.len() / SCOUT_SHARE).max(surplus);
        if !corps.iter().take(scouts).any(|(_, id)| *id == uid) {
            return false;
        }
        self.religion_seat_spreader_charts(g, pid, uid, religion, offensive)
    }

    /// Where [`AdvancedAi::religion_seat_spreader_charts`] sends `uid`, if
    /// anywhere.
    pub(super) fn religion_seat_charting_goal(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        religion: &str,
        offensive: bool,
    ) -> Option<Pos> {
        if !offensive
            || !self.religion_seat_charts_the_rivals_active(g, pid)
            || g.players[pid].religion.as_deref() != Some(religion)
        {
            return None;
        }
        let origin = g.units[&uid].pos;
        let threats = self.religious_raider_threats(g, pid);
        let open = |position: Pos| {
            position != origin
                && !g.players[pid].explored.contains(&position)
                && !Self::religious_tile_in_reach(g, pid, position, &threats)
        };
        // A goal already taken is kept while it is still worth the walk.
        let kept = self.charting_goals.borrow().get(&uid).copied();
        if let Some(goal) = kept.filter(|goal| open(*goal) && g.route_step(uid, *goal, 0).is_some())
        {
            return Some(goal);
        }
        let mut unlocated = false;
        let mut candidates: Vec<(i32, i32, Pos)> = Vec::new();
        {
            let seat = &g.players[pid];
            for rival in g.players.iter().filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !rival.is_free_city
                    && !g.same_team(pid, rival.id)
                    && g.has_met(pid, rival.id)
                    && BasicAi::rival_is_under_charted(g, pid, rival.id)
            }) {
                let anchors: Vec<Pos> = g
                    .cities
                    .values()
                    .filter(|city| city.owner == rival.id && seat.explored.contains(&city.pos))
                    .map(|city| city.pos)
                    .chain(
                        seat.remembered_cities
                            .values()
                            .filter(|city| {
                                city.owner == rival.id && !g.cities.contains_key(&city.id)
                            })
                            .map(|city| city.pos),
                    )
                    .collect();
                unlocated |= anchors.is_empty();
                for anchor in anchors {
                    let reach = g.wdist(origin, anchor);
                    candidates.extend(
                        g.wdisk(anchor, CHARTING_RADIUS)
                            .into_iter()
                            .filter(|position| open(*position))
                            .filter(|position| {
                                g.map.get(*position).is_some_and(|tile| {
                                    g.rules.is_passable(tile) && !g.rules.is_water(tile)
                                })
                            })
                            .map(|position| (reach, g.wdist(anchor, position), position)),
                    );
                }
            }
        }
        // The rival city nearest this unit first, then the unexplored ground
        // nearest that city: into the rival's land, not round the edge of ours.
        candidates.sort();
        let mut seen = BTreeSet::new();
        let near: Vec<Pos> = candidates
            .iter()
            .map(|(_, _, position)| *position)
            .filter(|position| seen.insert(*position))
            .filter(|position| g.route_step(uid, *position, 0).is_some())
            .take(CHARTING_ROUTE_TRIES)
            .collect();
        let goal = if near.is_empty() {
            unlocated
                .then(|| Self::missionary_expedition_goal(g, pid, uid, &threats))
                .flatten()
        } else {
            Some(near[uid as usize % near.len()])
        };
        let mut goals = self.charting_goals.borrow_mut();
        goals.retain(|other, _| g.units.contains_key(other));
        match goal {
            Some(goal) => {
                goals.insert(uid, goal);
            }
            None => {
                goals.remove(&uid);
            }
        }
        goal
    }
}

#[cfg(test)]
mod tests;
