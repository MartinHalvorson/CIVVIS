//! `no-city-limit`: the empire has no city target of its own. It settles
//! every seat it can hold for as long as the city founded there is expected
//! to earn more than the same Production spent at home, and it trains
//! several Settlers at once to get there.
//!
//! ## Why
//!
//! Operator, 2026-11-11: *"Remove the civvis city limit. We should build
//! cities whenever there is good land to settle and we expect the value
//! produced by the city to exceed whatever the opportunity cost is. Try to
//! build 15-20 cities per game + whatever cities we can take. Go wide and
//! really push the benefits to going wide to the extremes."*
//!
//! Three constants made the limit. [`SHARED_CITY_TARGET_FLOOR`] is the
//! ceiling in practice under `rapid-city-expansion-2`: 208 of 242 live Deity
//! boards at the cap read exactly ten wanted (2026-11-08/09).
//! `good-land-lifts-the-city-target` stops at twelve, and the Science
//! contract at twelve. Behind the target, the Settler claims
//! (`early-settler-floor`, `holdable-land-keeps-settling`) train one Settler
//! at a time, none at all while any city is threatened or any major is at
//! war with us, and the opening floor stops at six cities: the live Deity
//! seat held 5.7 cities at turn 100 against 8.9 for the best rival it had
//! met, and a city trained a Settler on 29 of the 80 turns from 21 to 100.
//!
//! ## The test: does the city pay
//!
//! A Settler is worth training when the city it founds is expected to add
//! more Science and Culture before the game ends than the same Production
//! would add spent on the cities the empire already holds
//! ([`AdvancedAi::settler_pay_floor`]). Every figure is from the site's own
//! worlds (4 majors, Prince, Online, the deployment genome; 2026-11-11), where
//! two majors played a first cut of this gene with a looser test and two
//! played stock:
//!
//! - **The alternative.** Each Online turn, a Settler's Production spent at
//!   home returns [`NO_CITY_LIMIT_ALTERNATIVE_RETURN`] of itself (4%) in
//!   Science or Culture: a University is 125 Production for about 5 Science.
//!   The wide majors trained eleven more Settlers by turn 150 and their five
//!   oldest cities made 17.7 Science each against 34.6 for the stock majors'
//!   (7.4 buildings against 12.3): about 85 Science a turn, the eleven
//!   Settlers' Production at that rate.
//! - **The city.** It adds nothing for
//!   [`NO_CITY_LIMIT_DELAY_STANDARD_TURNS`] standard turns and then
//!   [`NO_CITY_LIMIT_YIELD_SLOPE`] more each standard turn (half a yield a
//!   turn, each Online turn). A stock major's sixth to tenth city made 5
//!   Science at age 30 and 23 at age 80; the wide majors' sixteenth and later
//!   cities, founded after turn 125, made 3 by turn 150 and 70% of them were
//!   short of Amenities.
//! - **The seat.** A seat worth nothing earns
//!   [`NO_CITY_LIMIT_BARE_SHARE`] of that slope and one worth
//!   [`NO_CITY_LIMIT_PAR_WORTH`] all of it, to
//!   [`NO_CITY_LIMIT_WORTH_CAP`].
//! - **The risk and the road.** [`NO_CITY_LIMIT_FOUNDS`] of Settlers found
//!   their city (5.35 of 6.4 born in the live Deity games), and it stands
//!   [`NO_CITY_LIMIT_LEAD_STANDARD_TURNS`] standard turns after the decision.
//!   The citizen the Settler takes costs [`NO_CITY_LIMIT_CITIZEN_YIELD`] a
//!   turn for [`NO_CITY_LIMIT_CITIZEN_STANDARD_TURNS`] standard turns.
//! - **The horizon.** The game is expected to end at
//!   [`NO_CITY_LIMIT_EXPECTED_END_SHARE`] of its turn limit.
//!
//! The test has no city count in it. The limit it leaves is the one Civ VI
//! sets: every Settler costs more than the last, so each city must be founded
//! sooner than the one before, on a better seat, or with cheaper Settlers.
//! At Online speed the sixth Settler (115 Production) pays on any seat the
//! ordinary gate accepts until turn 125, the eleventh (190) until turn 107
//! and the sixteenth (265) until turn 90, and none pays from turns 135, 120
//! and 110. Colonization or the Ancestral Hall makes a Settler half again
//! cheaper and moves each of those fifteen to twenty turns later. The first cut paid for nine cities a game after turn 100 and
//! its majors won 5 of 16 worlds against the stock pair.
//!
//! ## What the gene does
//!
//! 1. **Target.** After `assess` settles the plan's own target,
//!    [`AdvancedAi::note_no_city_limit_target`] counts the free seats that
//!    pay: on the Settler gates' site list from every city we hold, worth the
//!    gate floor and the pay floor, acceptable to the walker, clear of every
//!    charted rival city by the holdable-land distance, inside the walker's
//!    own Loyalty forecast, [`SETTLEMENT_SPACING`] apart. The settlement
//!    target every Settler path reads (`settlement_target`) becomes the
//!    cities held, plus walking Settlers, plus those seats: never below the
//!    plan's target and with no ceiling.
//! 2. **The plan keeps its own number.** `plan.desired_cities` is unchanged,
//!    so the grand strategy leaves its expansion half when it did before and
//!    a war lane goes to war on time; the Settlers keep coming from the
//!    claims below under whatever strategy is in force.
//! 3. **Pipeline.** [`AdvancedAi::no_city_limit_width`] Settlers may be
//!    walking or in production at once: one, plus one for every
//!    [`NO_CITY_LIMIT_CITIES_PER_SETTLER`] cities while the empire fields a
//!    soldier for every city, to at most [`NO_CITY_LIMIT_MAX_WIDTH`]. Both
//!    Settler claims and the ordinary pipeline read it.
//! 4. **The claims stay open.** The opening floor fills to the whole target
//!    instead of six. The mid-game claim runs from the first turn it has a
//!    paying seat and passes over only the threatened city and the cities
//!    within [`NO_CITY_LIMIT_FRONT_TILES`] of it rather than the whole
//!    empire. A barbarian raider in a city's home ring stops its Settler
//!    only while the city has no garrison
//!    ([`AdvancedAi::no_city_limit_raider_holds`]): in the live Deity games
//!    a raider in the ring was the top reason a city trained no Settler
//!    before turn 60. Recovery still stops every claim.
//! 5. **Through a war.** The claims keep training while the army has a
//!    soldier for every city, in the cities with no visible enemy soldier
//!    within [`NO_CITY_LIMIT_WAR_CITY_TILES`], for seats no enemy soldier
//!    stands near. A Settler trained beside the front would only shelter in
//!    its city (`settler-shelters-from-the-war`): in the first paired games
//!    two front-line cities held sheltering Settlers for 891 unit-turns.
//!
//! 6. **The Ancestral Hall.** While seats that pay are free, the Government
//!    Plaza and the Ancestral Hall are priced as `expansion-hall` and
//!    `expansion-hall-district` price them for a land grab: the Hall's +50%
//!    toward Settlers in its own city moves every bar above fifteen to
//!    twenty turns later for the Settlers trained there, and its free
//!    Builder in each new city is what a young city lacks.
//!
//! A calibrated chair (`rival-pace`, `--seat-pace`) keeps its curve: the lift
//! is not asked there.
//!
//! Off (the default) nothing here runs.
use super::holdable_land_keeps_settling::HOLDABLE_LAND_RIVAL_CITY_TILES;
use super::settler_admission_journal::SETTLER_ADMISSION_JOURNAL_STRIDE;
use super::*;

/// The share of the turn limit at which the game is expected to end: Online
/// turn 195 of 250. The site's worlds end near 193 and Deity rivals win near
/// 185.
pub(crate) const NO_CITY_LIMIT_EXPECTED_END_SHARE: f64 = 0.78;

/// Standard turns from the decision to the founding: training the Settler and
/// walking it, 12 turns at Online.
pub(crate) const NO_CITY_LIMIT_LEAD_STANDARD_TURNS: f64 = 18.0;

/// Standard turns a founded city takes before it adds Science or Culture: 15
/// at Online.
pub(crate) const NO_CITY_LIMIT_DELAY_STANDARD_TURNS: f64 = 22.0;

/// What a city on a seat worth [`NO_CITY_LIMIT_PAR_WORTH`] adds to its
/// Science and Culture a turn, each standard turn after the delay: half a
/// yield a turn each Online turn.
pub(crate) const NO_CITY_LIMIT_YIELD_SLOPE: f64 = 0.5 / 2.25;

/// The seat worth the slope is measured at.
pub(crate) const NO_CITY_LIMIT_PAR_WORTH: f64 = 60.0;

/// The share of the slope a seat worth nothing still earns; the rest scales
/// with the seat's worth against [`NO_CITY_LIMIT_PAR_WORTH`].
pub(crate) const NO_CITY_LIMIT_BARE_SHARE: f64 = 0.75;

/// Seat worth past this adds nothing more.
pub(crate) const NO_CITY_LIMIT_WORTH_CAP: f64 = 120.0;

/// The share of Settlers that found their city.
pub(crate) const NO_CITY_LIMIT_FOUNDS: f64 = 0.8;

/// What a Settler's Production returns in Science or Culture each standard
/// turn, as a share of itself, when spent on the cities the empire already
/// holds: 4% each Online turn.
pub(crate) const NO_CITY_LIMIT_ALTERNATIVE_RETURN: f64 = 0.04 / 1.5;

/// The Science and Culture a turn of the citizen a Settler takes.
pub(crate) const NO_CITY_LIMIT_CITIZEN_YIELD: f64 = 2.0;

/// Standard turns the training city takes to grow the citizen back.
pub(crate) const NO_CITY_LIMIT_CITIZEN_STANDARD_TURNS: f64 = 12.0;

/// Cities held for each Settler in flight past the first.
pub(crate) const NO_CITY_LIMIT_CITIES_PER_SETTLER: usize = 2;

/// The most Settlers walking or in production at once.
pub(crate) const NO_CITY_LIMIT_MAX_WIDTH: usize = 6;

/// A city this near the plan's threatened city keeps its queue.
pub(crate) const NO_CITY_LIMIT_FRONT_TILES: i32 = 6;

/// At war, a city with a visible enemy soldier this near trains no Settler:
/// the walker would shelter inside it (`settler-shelters-from-the-war` holds
/// a Settler within six tiles of one) instead of walking.
pub(crate) const NO_CITY_LIMIT_WAR_CITY_TILES: i32 = 9;

/// A soldier of ours this near a city's centre is its garrison: a barbarian
/// raider in the home ring then no longer stops the city training a Settler.
pub(crate) const NO_CITY_LIMIT_GARRISON_TILES: i32 = 1;

/// The site-list radius, the one `settler_site_gate` and
/// `holdable-land-keeps-settling` read.
const NO_CITY_LIMIT_SITE_RADIUS: i32 = 11;

/// The most free seats counted: the scan stops there.
const NO_CITY_LIMIT_SEATS_COUNTED: usize = 24;

/// Turns a count of free seats stands before the board is scanned again,
/// while the empire's cities and walking Settlers are unchanged.
const NO_CITY_LIMIT_RECOUNT_TURNS: u32 = 3;

/// The lifted settlement target, with the turn it was noted on, and the
/// count of free seats behind it with the board that count was taken on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct NoCityLimitTarget {
    pub(crate) turn: u32,
    pub(crate) cities: usize,
    counted_turn: u32,
    counted_held: usize,
    counted_walkers: usize,
    free: usize,
}

impl AdvancedAi {
    /// The least a seat may be worth for the city founded on it to out-earn
    /// the Settler's Production spent at home: `turns_left` game turns to the
    /// expected end, `standard_per_turn` standard turns in each of them, and
    /// a Settler costing `cost` Production after the city's own modifiers.
    /// Infinite when no seat pays. See the module.
    pub(super) fn settler_pay_floor(turns_left: f64, standard_per_turn: f64, cost: f64) -> f64 {
        if turns_left <= 0.0 || standard_per_turn <= 0.0 {
            return f64::INFINITY;
        }
        let left = turns_left * standard_per_turn;
        // Standard turns the city spends earning before the end.
        let earning = left - NO_CITY_LIMIT_LEAD_STANDARD_TURNS - NO_CITY_LIMIT_DELAY_STANDARD_TURNS;
        if earning <= 0.0 {
            return f64::INFINITY;
        }
        // What a city on a par seat earns in all: yields that climb in a
        // line from nothing.
        let par = NO_CITY_LIMIT_FOUNDS * 0.5 * NO_CITY_LIMIT_YIELD_SLOPE * earning * earning;
        let alternative = cost.max(0.0) * NO_CITY_LIMIT_ALTERNATIVE_RETURN * left;
        let citizen = NO_CITY_LIMIT_CITIZEN_YIELD * NO_CITY_LIMIT_CITIZEN_STANDARD_TURNS.min(left);
        // The share of the par slope the seat must earn.
        let needed = (alternative + citizen) / par;
        let best = NO_CITY_LIMIT_BARE_SHARE
            + (1.0 - NO_CITY_LIMIT_BARE_SHARE) * NO_CITY_LIMIT_WORTH_CAP / NO_CITY_LIMIT_PAR_WORTH;
        if needed > best {
            f64::INFINITY
        } else {
            ((needed - NO_CITY_LIMIT_BARE_SHARE) / (1.0 - NO_CITY_LIMIT_BARE_SHARE)
                * NO_CITY_LIMIT_PAR_WORTH)
                .max(0.0)
        }
    }

    /// [`Self::settler_pay_floor`] on this board for the next Settler `pid`
    /// trains: its cost, after the city's Settler modifiers, where it is
    /// cheapest. Zero with the gene off.
    pub(super) fn no_city_limit_pay_floor(&self, g: &Game, pid: usize) -> f64 {
        if !self.no_city_limit {
            return 0.0;
        }
        let settler = Self::early_settler_item();
        // The Production a city really spends: Colonization, the Ancestral
        // Hall and the like make every point count for more.
        let Some(cost) = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| {
                g.item_cost_for_city(pid, cid, &settler)
                    / g.item_prod_mult(pid, cid, Some(&settler)).max(0.1)
            })
            .min_by(|a, b| a.total_cmp(b))
        else {
            return f64::INFINITY;
        };
        let end = f64::from(g.max_turns) * NO_CITY_LIMIT_EXPECTED_END_SHARE;
        let standard_per_turn = 90.0 / f64::from(g.standard_duration(90).max(1));
        Self::settler_pay_floor(end - f64::from(g.turn), standard_per_turn, cost)
    }

    /// The Settlers that may be walking or in production at once for an
    /// empire of `cities` cities fielding `soldiers` soldiers. See the
    /// module.
    pub(super) fn no_city_limit_width_for(cities: usize, soldiers: usize) -> usize {
        if cities < 2 || soldiers < cities {
            1
        } else {
            (1 + cities / NO_CITY_LIMIT_CITIES_PER_SETTLER).min(NO_CITY_LIMIT_MAX_WIDTH)
        }
    }

    /// [`Self::no_city_limit_width_for`] on this board, `None` with the gene
    /// off.
    pub(super) fn no_city_limit_width(&self, g: &Game, pid: usize) -> Option<usize> {
        self.no_city_limit.then(|| {
            Self::no_city_limit_width_for(
                g.player_city_ids(pid).len(),
                self.counts(g, pid).military,
            )
        })
    }

    /// See the module: count the free seats that pay and note the lifted
    /// settlement target for this turn. `wanted` is the plan's own target and
    /// is returned to no caller changed. Exact no-op with the gene off.
    pub(super) fn note_no_city_limit_target(
        &self,
        g: &Game,
        pid: usize,
        wanted: usize,
        cities: &[u32],
    ) {
        if !self.no_city_limit {
            return;
        }
        let last = self.no_city_limit_target.get();
        let mut noted = NoCityLimitTarget {
            turn: g.turn,
            cities: 0,
            ..last
        };
        if self.base.minor
            || self.base.barb
            || cities.is_empty()
            || self.rival_city_pace.is_some()
            || self.seat_city_pace
        {
            self.no_city_limit_target.set(noted);
            return;
        }
        let (walkers, _) = Self::early_settler_census(g, pid);
        // The scan is the dear part of `assess`: a count stands for
        // `NO_CITY_LIMIT_RECOUNT_TURNS` while nothing was founded, lost or
        // trained.
        let fresh = last.counted_turn > 0
            && g.turn >= last.counted_turn
            && g.turn < last.counted_turn + NO_CITY_LIMIT_RECOUNT_TURNS
            && last.counted_held == cities.len()
            && last.counted_walkers == walkers;
        let mut counted: Option<(f64, Option<(Pos, f64)>)> = None;
        if !fresh {
            let pay_floor = self.no_city_limit_pay_floor(g, pid);
            let floor = pay_floor.max(Self::settler_site_gate_floor(cities.len()));
            let (free, best) = if floor.is_finite() {
                let origins: Vec<Pos> = cities.iter().map(|cid| g.cities[cid].pos).collect();
                self.no_city_limit_free_seats(g, pid, &origins, floor, walkers)
            } else {
                (0, None)
            };
            noted.counted_turn = g.turn.max(1);
            noted.counted_held = cities.len();
            noted.counted_walkers = walkers;
            noted.free = free;
            counted = Some((floor, best));
        }
        let free = noted.free;
        let lifted = wanted.max(cities.len() + walkers + free);
        noted.cities = lifted;
        self.no_city_limit_target.set(noted);
        if let Some((floor, best)) = counted {
            if lifted > wanted
                && g.turn.is_multiple_of(SETTLER_ADMISSION_JOURNAL_STRIDE)
                && self.journal().wants(crate::reasoning::Level::Detail)
            {
                let best = best.map_or(String::new(), |(site, worth)| {
                    format!(", the best {site:?} worth {worth:.1}")
                });
                think!(self.journal(), Expansion, Detail,
                    "No city limit: {lifted} cities pay";
                    "no-city-limit: {} cities and {walkers} walking against {wanted} planned; \
                     {free} free seat(s) we can hold worth {floor:.0} or more, the least a \
                     city must be worth to out-earn its Settler{best}",
                    cities.len());
            }
        }
    }

    /// The free seats that pay within reach of `origins`, and the best of
    /// them: worth `floor` or more on the Settler gates' site list, clear of
    /// every charted rival city, acceptable to the walker and inside its
    /// Loyalty forecast, [`SETTLEMENT_SPACING`] from each other and from
    /// every target a live Settler holds, less one for each of the `walkers`
    /// still without a target. The dear tests run only on a site that would
    /// otherwise be seated, best-first, and the count stops at
    /// [`NO_CITY_LIMIT_SEATS_COUNTED`].
    pub(super) fn no_city_limit_free_seats(
        &self,
        g: &Game,
        pid: usize,
        origins: &[Pos],
        floor: f64,
        walkers: usize,
    ) -> (usize, Option<(Pos, f64)>) {
        let claimed: Vec<Pos> = self
            .settler_targets
            .iter()
            .filter(|(uid, _)| {
                g.units
                    .get(uid)
                    .is_some_and(|unit| unit.owner == pid && unit.kind == "settler")
            })
            .map(|(_, target)| *target)
            .collect();
        let unseated = walkers.saturating_sub(claimed.len());
        let rival_cities = Self::holdable_land_threats(g, pid).rival_cities;
        let mut worth_at: BTreeMap<Pos, f64> = BTreeMap::new();
        let mut score_cache = BTreeMap::new();
        let mut shared = SettlementScanShared::default();
        for origin in origins {
            for (site, worth) in self.settle_sites_scanning_shared(
                g,
                pid,
                *origin,
                NO_CITY_LIMIT_SITE_RADIUS,
                None,
                Some(&mut score_cache),
                false,
                false,
                Some(&mut shared),
            ) {
                if worth >= floor {
                    let entry = worth_at.entry(site).or_insert(worth);
                    *entry = entry.max(worth);
                }
            }
        }
        let mut sites: Vec<(Pos, f64)> = worth_at.into_iter().collect();
        sites.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let mut seats: Vec<(Pos, f64)> = Vec::new();
        for (site, worth) in sites {
            if seats.len() >= NO_CITY_LIMIT_SEATS_COUNTED + unseated {
                break;
            }
            if claimed
                .iter()
                .chain(seats.iter().map(|(seat, _)| seat))
                .any(|taken| g.wdist(*taken, site) < SETTLEMENT_SPACING)
            {
                continue;
            }
            if rival_cities
                .iter()
                .any(|city| g.wdist(*city, site) < HOLDABLE_LAND_RIVAL_CITY_TILES)
                || self.exhaustion_site_unpriceable(g, pid, site).is_some()
                || Self::inside_rival_sphere(g, pid, site)
                || self.holdable_land_loyalty_refuses(g, pid, site)
            {
                continue;
            }
            seats.push((site, worth));
        }
        (seats.len().saturating_sub(unseated), seats.first().copied())
    }

    /// Whether a barbarian raider in `city`'s home ring holds its queue: only
    /// while no soldier of ours stands within
    /// [`NO_CITY_LIMIT_GARRISON_TILES`] of the centre. In the live Deity
    /// games a raider in the ring was the top reason a city trained no
    /// Settler before turn 60, and the Settler it would have trained was
    /// eight or more turns from leaving.
    pub(super) fn no_city_limit_raider_holds(g: &Game, pid: usize, city: Pos) -> bool {
        Self::early_settler_raider_near(g, city)
            && !g.player_unit_ids(pid).into_iter().any(|uid| {
                let unit = &g.units[&uid];
                Self::holdable_land_soldier(g, unit)
                    && g.wdist(unit.pos, city) <= NO_CITY_LIMIT_GARRISON_TILES
            })
    }

    /// Every visible land soldier of a major at war with `pid`.
    pub(super) fn no_city_limit_war_soldiers(g: &Game, pid: usize) -> Vec<Pos> {
        let visible = g.player_vision_frame(pid);
        g.units
            .values()
            .filter(|unit| {
                g.players.get(unit.owner).is_some_and(|owner| {
                    owner.id != pid
                        && owner.alive
                        && !owner.is_minor
                        && !owner.is_barbarian
                        && g.is_at_war(pid, owner.id)
                }) && Self::holdable_land_soldier(g, unit)
                    && g.sees(&visible, unit.pos)
                    && g.unit_visible_to(unit.id, pid)
            })
            .map(|unit| unit.pos)
            .collect()
    }

    /// Whether the war is at `city`'s gates: one of `war_soldiers` within
    /// [`NO_CITY_LIMIT_WAR_CITY_TILES`].
    pub(super) fn no_city_limit_war_at_the_gates(
        g: &Game,
        war_soldiers: &[Pos],
        city: Pos,
    ) -> bool {
        war_soldiers
            .iter()
            .any(|soldier| g.wdist(*soldier, city) <= NO_CITY_LIMIT_WAR_CITY_TILES)
    }

    /// The city target a published plan report shows (`plan_report`, the
    /// site's "Cities N · wants M" row): the plan's own, or the lifted
    /// settlement target when the gene counted more on the plan's turn. No
    /// decision reads it; without it a major holding eighteen cities would
    /// read "wants 10".
    pub(super) fn reported_city_target(&self, plan: &StrategicPlan) -> usize {
        let noted = self.no_city_limit_target.get();
        if self.no_city_limit && noted.turn == plan.assessed_turn {
            plan.desired_cities.max(noted.cities)
        } else {
            plan.desired_cities
        }
    }

    /// Whether `cid` may train a Settler while the plan names a threatened
    /// city: not that city, and not one within
    /// [`NO_CITY_LIMIT_FRONT_TILES`] of it.
    pub(super) fn no_city_limit_city_is_clear(g: &Game, plan: &StrategicPlan, cid: u32) -> bool {
        let Some(threatened) = plan
            .threatened_city
            .and_then(|threatened| g.cities.get(&threatened))
        else {
            return true;
        };
        g.cities.get(&cid).is_some_and(|city| {
            city.id != threatened.id
                && g.wdist(city.pos, threatened.pos) > NO_CITY_LIMIT_FRONT_TILES
        })
    }
}

#[cfg(test)]
mod tests;
