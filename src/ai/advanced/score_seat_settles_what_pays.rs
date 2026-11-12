//! `score-seat-settles-what-pays`: a Score seat has no city target. It
//! settles every free seat whose city pays its Settler back in points before
//! the game ends, and stops only in the last turns.
//!
//! **Why (live Rome, Deity, 6-player Small Continents, Online, turn limit
//! 170, Score target, 2026-11-12 19:00-20:15Z).** Four finished games ended
//! with 10, 9, 9 and 7 cities and 529-726 points, sixth of six; the winners
//! held 12 or more cities. `no-city-limit`, `holdable-land-keeps-settling`
//! and `early-settler-floor` were armed, yet the journal read "0 cities of 10
//! wanted" at t1 and "already at the city target ... against a target of
//! 10.0" from t110. Three clocks built for a Science or Culture race stopped
//! the Settlers, none of them a city count:
//!
//! - `no-city-limit`'s pay test prices a city in Science and Culture, ends
//!   the game at 78% of its turn limit (t132.6 of 170) and waits 40 standard
//!   turns from the decision to the first yield, so no seat pays from about
//!   t106 and few before. Its lifted target then falls back to the plan's
//!   ten, and the `no-city-limit:` line never appeared in a Rome journal.
//! - `holdable-land-keeps-settling` closes its window 100 standard turns
//!   before the turn limit: t103 of 170 at Online.
//! - The delegated governor's Settler gate stops at `settler_stop_turn`
//!   (150), the "turn 144 of 150" in the journal.
//!
//! A city is worth points by itself: 5 for the city and 1 for each citizen,
//! 2 for each district and 1 for each building it has at the end. The late
//! cities of those games, founded at t115-163, scored 7-20 points by t171
//! (7 at 10 turns old, 12 at 23, 20 at 46-56), about
//! [`SCORE_SEAT_CITY_BASE_POINTS`] plus 0.17 a turn; the Settler then cost
//! 145-190 Production.
//!
//! **The gene.** While the active victory target is Score:
//!
//! 1. The pay test is in points ([`AdvancedAi::score_seat_pay_floor`]). A
//!    Settler started now founds its city
//!    [`SCORE_SEAT_LEAD_STANDARD_TURNS`] standard turns later, and the city
//!    is worth [`SCORE_SEAT_CITY_BASE_POINTS`] plus
//!    [`SCORE_SEAT_CITY_POINTS_PER_STANDARD_TURN`] for each standard turn it
//!    stands before the end, times the share of Settlers that found
//!    (`NO_CITY_LIMIT_FOUNDS`). It pays when that is at least the Settler's
//!    Production at [`SCORE_SEAT_PRODUCTION_PER_POINT`] a point, plus the
//!    citizen it takes. Every seat the Settler gates already accept then
//!    pays (the site floor stays: the seats never founded on the gate's own
//!    evidence were worth under 25); otherwise none does.
//! 2. No Settler starts in the last [`SCORE_SEAT_LAST_TURNS`] turns, the
//!    turns `score-seat-plays-for-points` keeps for things that finish. Until
//!    then `holdable-land-keeps-settling`'s window stays open and the
//!    delegated governor's Settler gate runs to that turn.
//! 3. Everything else is `no-city-limit`'s: the free seats it counts (clear
//!    of charted rival cities, outside their sphere, inside the walker's own
//!    Loyalty forecast, apart from each other), the Settlers in flight at
//!    once (one, plus one for every two cities while there is a soldier for
//!    every city), the far seats past the young cities, and its pricing of
//!    the Government Plaza and the Ancestral Hall while seats are free. The
//!    gene rides `no-city-limit`: with that off, nothing here runs.
//!
//! Journal (grep `score-seat-settles-what-pays:`): the seats that pay and
//! the points expected, every few turns while counting; each Settler the
//! claims start; and the reason when none pays. Off, or for another victory
//! target, nothing changes.

use super::no_city_limit::NO_CITY_LIMIT_FOUNDS;
use super::*;

/// Turns at the end of the game in which no Settler is started:
/// `score-seat-plays-for-points` keeps them for what finishes in time.
pub(crate) const SCORE_SEAT_LAST_TURNS: u32 = 15;

/// Standard turns from starting a Settler to its city: about five Online
/// turns to train it and five to walk.
pub(crate) const SCORE_SEAT_LEAD_STANDARD_TURNS: f64 = 15.0;

/// Points a founded city is worth at once: 5 for the city and its first
/// citizen.
pub(crate) const SCORE_SEAT_CITY_BASE_POINTS: f64 = 6.0;

/// Points a city adds each standard turn it stands before the end
/// (citizens, buildings, districts): 0.17 each Online turn in the late Rome
/// cities.
pub(crate) const SCORE_SEAT_CITY_POINTS_PER_STANDARD_TURN: f64 = 0.17 / 1.5;

/// Production the cities already held spend for a point of score: a building
/// (1 point) or half a district (2 points) at the mid-game Online prices.
pub(crate) const SCORE_SEAT_PRODUCTION_PER_POINT: f64 = 60.0;

/// Points of the citizen a Settler takes from its city.
pub(crate) const SCORE_SEAT_CITIZEN_POINTS: f64 = 1.0;

/// What a Settler started now is expected to found and cost, in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct ScoreSeatSettler {
    /// The turn the city is expected to stand.
    pub(super) founded: f64,
    /// The points it is expected to be worth at the end, founding odds in.
    pub(super) worth: f64,
    /// The Settler's price in points: its Production and the citizen.
    pub(super) price: f64,
}

impl AdvancedAi {
    /// The gene is on and the seat plays for Score.
    pub(super) fn score_seat_settles(&self, g: &Game) -> bool {
        self.score_seat_settles_what_pays
            && self.no_city_limit
            && self.active_victory_target(g) == Some(VictoryTarget::Score)
    }

    /// The last turn a Score seat starts a Settler: [`SCORE_SEAT_LAST_TURNS`]
    /// before the turn limit. `None` with the gene off or for another target.
    pub(super) fn score_seat_last_settler_turn(&self, g: &Game) -> Option<u32> {
        self.score_seat_settles(g)
            .then(|| g.max_turns.saturating_sub(SCORE_SEAT_LAST_TURNS))
    }

    /// Standard turns in each game turn at this speed.
    fn score_seat_standard_per_turn(g: &Game) -> f64 {
        90.0 / f64::from(g.standard_duration(90).max(1))
    }

    /// What a Settler costing `cost` Production, started this turn, founds
    /// and costs in points. See the module.
    pub(super) fn score_seat_settler(g: &Game, cost: f64) -> ScoreSeatSettler {
        let per_turn = Self::score_seat_standard_per_turn(g);
        let founded = f64::from(g.turn) + SCORE_SEAT_LEAD_STANDARD_TURNS / per_turn;
        let standing = (f64::from(g.max_turns) - founded).max(0.0) * per_turn;
        let worth = NO_CITY_LIMIT_FOUNDS
            * (SCORE_SEAT_CITY_BASE_POINTS + SCORE_SEAT_CITY_POINTS_PER_STANDARD_TURN * standing);
        let price = cost.max(0.0) / SCORE_SEAT_PRODUCTION_PER_POINT + SCORE_SEAT_CITIZEN_POINTS;
        ScoreSeatSettler {
            founded,
            worth,
            price,
        }
    }

    /// Why no Settler pays on a Score seat this turn, if none does.
    pub(super) fn score_seat_stops(&self, g: &Game, cost: f64) -> Option<String> {
        let last = self.score_seat_last_settler_turn(g)?;
        if g.turn > last {
            return Some(format!(
                "turn {} is inside the last {SCORE_SEAT_LAST_TURNS} turns of {}",
                g.turn, g.max_turns
            ));
        }
        let settler = Self::score_seat_settler(g, cost);
        if settler.founded >= f64::from(g.max_turns) {
            return Some(format!(
                "a city started now stands at about t{:.0}, after the last turn {}",
                settler.founded, g.max_turns
            ));
        }
        (settler.worth < settler.price).then(|| {
            format!(
                "a city founded by t{:.0} is worth {:.1} points against its Settler's {:.1}",
                settler.founded, settler.worth, settler.price
            )
        })
    }

    /// `score-seat-settles-what-pays`: the seat floor `no-city-limit`'s pay
    /// test hands the Settler paths on a Score seat, for a Settler costing
    /// `cost`: zero while a city pays its Settler back in points (the
    /// ordinary site floor then decides), infinite when none does. `None`
    /// with the gene off or for another target.
    pub(super) fn score_seat_pay_floor(&self, g: &Game, cost: f64) -> Option<f64> {
        self.score_seat_settles(g).then(|| {
            if self.score_seat_stops(g, cost).is_some() {
                f64::INFINITY
            } else {
                0.0
            }
        })
    }

    /// The journal line `no-city-limit`'s count writes on a Score seat:
    /// the seats that pay and what a city is worth, or why none pays.
    pub(super) fn explain_score_seat_count(
        &self,
        g: &Game,
        pid: usize,
        cities: usize,
        walkers: usize,
        free: usize,
    ) {
        if !self.score_seat_settles(g) || !self.journal().wants(crate::reasoning::Level::Detail) {
            return;
        }
        let Some(cost) = Self::cheapest_settler_cost(g, pid) else {
            return;
        };
        match self.score_seat_stops(g, cost) {
            Some(why) => {
                think!(self.journal(), Expansion, Detail,
                       "Score seat: no Settler pays";
                       "score-seat-settles-what-pays: {cities} cities and {walkers} walking; no \
                        Settler pays: {why}");
            }
            None => {
                let settler = Self::score_seat_settler(g, cost);
                think!(self.journal(), Expansion, Detail,
                       "Score seat: {free} free seat(s) pay";
                       "score-seat-settles-what-pays: {cities} cities and {walkers} walking, \
                        {free} free seat(s) pay; a city founded by t{:.0} is worth {:.1} points \
                        by t{} against its Settler's {:.1} ({cost:.0} Production)",
                       settler.founded, settler.worth, g.max_turns, settler.price);
            }
        }
    }

    /// The journal line for a Settler a claim started in `cid` on a Score
    /// seat.
    pub(super) fn explain_score_seat_settler(&self, g: &Game, pid: usize, cid: u32) {
        if !self.score_seat_settles(g) {
            return;
        }
        let settler_item = Self::early_settler_item();
        let cost = g.item_cost_for_city(pid, cid, &settler_item);
        let settler = Self::score_seat_settler(g, cost);
        let (walkers, _) = Self::early_settler_census(g, pid);
        let name = &g.cities[&cid].name;
        think!(self.journal(), Expansion, Decision,
               "{name} trains a Settler for points";
               "score-seat-settles-what-pays: {name} trains a Settler with {} cities and {walkers} \
                walking; its city, founded by t{:.0}, is worth {:.1} points by t{} against {:.1} \
                ({cost:.0} Production)",
               g.player_city_ids(pid).len(), settler.founded, settler.worth, g.max_turns,
               settler.price);
    }
}

#[cfg(test)]
mod tests;
