//! `good-land-lifts-the-city-target`: the plan's city target rises above the
//! shared ten, to at most twelve, by one city for each good seat the empire
//! can hold, and only while a city founded now would still repay its Settler.
//!
//! ## Why
//!
//! Operator, 2026-11-09: *"lets raise our city target up to 12 cities, but
//! civs should use their discretion. we should expand into good city space
//! while building cities is still beneficial to us. we shouldn't force cities
//! on bad locations."*
//!
//! The shared contract floors the target at [`SHARED_CITY_TARGET_FLOOR`], and
//! under `rapid-city-expansion-2` the era cadence does not pass it until the
//! last fifty turns, so ten is the ceiling in practice: 208 of 242 live Deity
//! boards at the cap read exactly ten wanted (2026-11-08/09).
//!
//! A flat higher target is not the answer. A paired sweep of flat targets 10,
//! 12, 15 and 20 (16 Deity seeds each on the live genome, 2026-11-09) added
//! late cities worth 1-7 Production each against 21 for an average city,
//! moved no economy figure outside its error and won no game. On the seeds
//! where the cap never bound, the higher target sent more Settlers out at
//! once and Production at turn 125 read 8% lower (se 4%).
//!
//! ## What the gene does
//!
//! `assess` asks it after the shared contract and the difficulty widening, so
//! every later cut (`city-target-meets-the-map`, the Science contract, a
//! calibrated chair's pace) still applies. The target becomes the empire's
//! cities, plus its walking Settlers, plus the free good seats, never below
//! what the plan already wanted and never above [`GOOD_LAND_CITY_CEILING`],
//! when both hold:
//!
//! - the plan's own target is below the ceiling and the empire holds all but
//!   [`GOOD_LAND_NEAR_TARGET`] of it. Below that the ordinary target is not
//!   what paces the Settlers, and the opening and its pipeline are unchanged;
//! - a city founded now still repays: the game has more than
//!   [`HOLDABLE_LAND_PAYBACK_STANDARD_TURNS`] standard turns left, the horizon
//!   `holdable-land-keeps-settling` stops queueing Settlers at.
//!
//! A **good seat** is a site on the Settler gates' own list (radius
//! [`GOOD_LAND_SITE_RADIUS`] from one of our cities) that
//!
//! - is worth at least [`GOOD_LAND_SEAT_WORTH`] from that city;
//! - the walker would accept: `exhaustion_site_unpriceable` passes it and it
//!   lies outside every visible rival's Loyalty sphere;
//! - has no charted city of a living rival major within
//!   [`HOLDABLE_LAND_RIVAL_CITY_TILES`], the holdable-land distance;
//! - passes the walker's own Loyalty forecast, asked as the walker asks it.
//!
//! Seats are counted best-first, each [`SETTLEMENT_SPACING`] from every seat
//! already counted and from every target a live Settler holds. A Settler
//! without a target takes one of them.
//!
//! Passing soldiers and raiders are not read here. The target is a horizon;
//! `holdable-land-keeps-settling` reads them on the turn it would queue the
//! Settler, and the walker reads them on the road.
//!
//! Past the shared target the Settler itself must be for a good seat:
//! `holdable-land-keeps-settling` queues one only in a city whose own best
//! holdable seat is worth [`GOOD_LAND_SEAT_WORTH`]
//! ([`AdvancedAi::good_land_seat_floor`]).
//!
//! ## The worth bar (52 live Deity games, 2026-11-08/09)
//!
//! The holdable-land rule journals the worth of the seat it queues a Settler
//! for; 110 distinct seats, each followed to the city founded within two
//! tiles of it in the next forty turns:
//!
//! | seat worth | seats | founded | population 30 turns on | Production 30 turns on |
//! |---|---|---|---|---|
//! | under 80 | 74 | 34 (46%) | 3.5 | 9.8 |
//! | 80 or more | 36 | 22 (61%) | 4.6 | 9.1 |
//!
//! Eighty is twice the ordinary Settler floor (`SETTLER_SITE_GATE_FLOOR`) and
//! the top third of the seats the rule queues for. With seven to nine cities
//! held, the best holdable seat reached it on 5 of 21 occasions, so the lift
//! is the exception and not the rule.
//!
//! Off (the default) nothing here runs.
use super::holdable_land_keeps_settling::{
    HOLDABLE_LAND_PAYBACK_STANDARD_TURNS, HOLDABLE_LAND_RIVAL_CITY_TILES,
};
use super::settler_admission_journal::SETTLER_ADMISSION_JOURNAL_STRIDE;
use super::*;

/// The most cities the lift may ask for.
pub(crate) const GOOD_LAND_CITY_CEILING: usize = 12;

/// The least a seat may be worth, from the city that would settle it, to
/// count as good. See the module's calibration table.
pub(crate) const GOOD_LAND_SEAT_WORTH: f64 = 80.0;

/// The lift is asked only once the empire holds all but this many of the
/// cities the plan already wants.
pub(crate) const GOOD_LAND_NEAR_TARGET: usize = 2;

/// The site-list radius, the one `settler_site_gate` and
/// `holdable-land-keeps-settling` read.
const GOOD_LAND_SITE_RADIUS: i32 = 11;

impl AdvancedAi {
    /// The lifted target for `held` cities, `walkers` Settlers on the map and
    /// `free_seats` good seats no Settler is bound for: never below `wanted`,
    /// never above [`GOOD_LAND_CITY_CEILING`].
    pub(super) fn good_land_lifted_target(
        wanted: usize,
        held: usize,
        walkers: usize,
        free_seats: usize,
    ) -> usize {
        wanted.max((held + walkers + free_seats).min(GOOD_LAND_CITY_CEILING))
    }

    /// How many of `sites` (best-first) can each seat a new city: every seat
    /// [`SETTLEMENT_SPACING`] from every `claimed` target and from every seat
    /// counted before it, less one for each of the `unseated` Settlers that
    /// still needs a seat. Stops counting at `wanted`.
    pub(super) fn good_land_free_seats(
        g: &Game,
        sites: &[(Pos, f64)],
        claimed: &[Pos],
        unseated: usize,
        wanted: usize,
    ) -> usize {
        let mut seats: Vec<Pos> = Vec::new();
        for &(pos, _) in sites {
            if seats.len() >= wanted + unseated {
                break;
            }
            if claimed
                .iter()
                .chain(seats.iter())
                .all(|taken| g.wdist(*taken, pos) >= SETTLEMENT_SPACING)
            {
                seats.push(pos);
            }
        }
        seats.len().saturating_sub(unseated)
    }

    /// Every good seat within reach of `origins`, best-first, at the worth the
    /// nearest-valued origin gives it. See the module for what makes a seat
    /// good.
    pub(super) fn good_land_seats(&self, g: &Game, pid: usize, origins: &[Pos]) -> Vec<(Pos, f64)> {
        let rival_cities = Self::holdable_land_threats(g, pid).rival_cities;
        self.good_land_seats_clear_of(g, pid, origins, &rival_cities)
    }

    /// [`Self::good_land_seats`] against a given list of charted rival
    /// cities.
    pub(super) fn good_land_seats_clear_of(
        &self,
        g: &Game,
        pid: usize,
        origins: &[Pos],
        rival_cities: &[Pos],
    ) -> Vec<(Pos, f64)> {
        let mut best: BTreeMap<Pos, f64> = BTreeMap::new();
        // Every origin scans the same board for the same seat, so the site
        // values and the exclusion disks are shared, as `map_settlement_room`
        // shares them. Only the distance term differs by origin.
        let mut score_cache = BTreeMap::new();
        let mut shared = SettlementScanShared::default();
        for origin in origins {
            for (site, worth) in self.settle_sites_scanning_shared(
                g,
                pid,
                *origin,
                GOOD_LAND_SITE_RADIUS,
                None,
                Some(&mut score_cache),
                false,
                false,
                Some(&mut shared),
            ) {
                if worth >= GOOD_LAND_SEAT_WORTH {
                    let entry = best.entry(site).or_insert(worth);
                    *entry = entry.max(worth);
                }
            }
        }
        // The dearer tests run only on sites already worth the bar.
        let mut seats: Vec<(Pos, f64)> = best
            .into_iter()
            .filter(|(site, _)| {
                self.exhaustion_site_unpriceable(g, pid, *site).is_none()
                    && !Self::inside_rival_sphere(g, pid, *site)
                    && rival_cities
                        .iter()
                        .all(|city| g.wdist(*city, *site) >= HOLDABLE_LAND_RIVAL_CITY_TILES)
                    && !self.holdable_land_loyalty_refuses(g, pid, *site)
            })
            .collect();
        seats.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        seats
    }

    /// See the module: the plan's city target, lifted by the good seats the
    /// empire can hold. Returns `wanted` unchanged with the gene off, for a
    /// minor or barbarian seat, outside the payback horizon, above the
    /// ceiling, and while the empire is not yet near its ordinary target.
    pub(super) fn good_land_city_target(
        &self,
        g: &Game,
        pid: usize,
        wanted: usize,
        cities: &[u32],
    ) -> usize {
        if !self.good_land_lifts_the_city_target
            || self.base.minor
            || self.base.barb
            || cities.is_empty()
            || wanted >= GOOD_LAND_CITY_CEILING
            || cities.len() + GOOD_LAND_NEAR_TARGET < wanted
            || g.turn + g.standard_duration(HOLDABLE_LAND_PAYBACK_STANDARD_TURNS) >= g.max_turns
        {
            return wanted;
        }
        let (walkers, _) = Self::early_settler_census(g, pid);
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
        let origins: Vec<Pos> = cities.iter().map(|cid| g.cities[cid].pos).collect();
        let seats = self.good_land_seats(g, pid, &origins);
        let room = GOOD_LAND_CITY_CEILING.saturating_sub(cities.len() + walkers);
        let free = Self::good_land_free_seats(g, &seats, &claimed, unseated, room);
        let lifted = Self::good_land_lifted_target(wanted, cities.len(), walkers, free);
        if lifted > wanted
            && g.turn.is_multiple_of(SETTLER_ADMISSION_JOURNAL_STRIDE)
            && self.journal().wants(crate::reasoning::Level::Detail)
        {
            // No seat is left to name when the lift only keeps a Settler that
            // set out for one.
            let best = seats.first().map_or(String::new(), |(site, worth)| {
                format!(", the best {site:?} worth {worth:.1}")
            });
            think!(self.journal(), Expansion, Detail,
                "Good land lifts the city target to {lifted}";
                "good-land-lifts-the-city-target: {} cities and {walkers} walking against \
                 {wanted} wanted; {free} free seat(s) worth {GOOD_LAND_SEAT_WORTH:.0} or more \
                 that we can hold{best}",
                cities.len());
        }
        lifted
    }

    /// The least a holdable seat may be worth for
    /// `holdable-land-keeps-settling` to queue a Settler when the empire's
    /// cities and walking Settlers already number `settled`: the good-seat
    /// bar from the shared target on, and zero below it and with the gene off.
    pub(super) fn good_land_seat_floor(&self, settled: usize) -> f64 {
        if self.good_land_lifts_the_city_target && settled >= SHARED_CITY_TARGET_FLOOR {
            GOOD_LAND_SEAT_WORTH
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests;
