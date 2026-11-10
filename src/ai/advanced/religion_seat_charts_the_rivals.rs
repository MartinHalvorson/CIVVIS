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
//!   ([`AdvancedAi::religion_seat_cartography_value`]), so Cartography follows
//!   Shipbuilding instead of trailing it by 28-47 turns.
//!
//! Off, for a seat aiming elsewhere, or before the founding, nothing changes.

use super::*;

/// Cities beyond a strict majority the seat wants to know of each rival, so
/// one city that will not flip does not end the campaign against its owner.
pub(crate) const CHARTED_SPARE_CITIES: usize = 1;

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

#[cfg(test)]
mod tests;
