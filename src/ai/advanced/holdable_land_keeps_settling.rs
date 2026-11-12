//! `holdable-land-keeps-settling`: past the opening, while the plan still
//! wants more cities and a seat we can very likely hold lies within reach,
//! the unthreatened city that trains a Settler soonest keeps putting one at
//! the head of its queue, ahead of the routine Granary, Builder, Aqueduct,
//! Industrial Zone and Campus claims.
//!
//! ## Why (52 live Deity Korea Online games, 2026-11-04 21:03Z → 11-05 03:20Z)
//!
//! Operator, 11-05: *"we should be doing a better job of expanding into any
//! territory we can very likely hold. more good cities is more good prod and
//! science over the long run."*
//!
//! - **The Settler stops after the opening.** In 79-88% of turns from t20 to
//!   t60 a Settler is walking or queued. That falls to 58% at t60-80 and 41%
//!   at t80-100. Settler starts across about 39 games: 70 in t40-59, then 8 in
//!   t60-79.
//! - **The plan still wants the cities.** `desired_cities` reads 8.9-9.0 over
//!   t60-100 while the empire holds 5.2-5.7. The target is not the limit, and
//!   the recovery hold accounts for only ~13% of the Settler-free turns.
//! - **The land is there.** Packing decent free sites (≥8 good tiles within
//!   three, legal, ≤9 tiles from one of our cities) finds about 4.7 a game at
//!   t60. Forty turns later 56% are still free, 19% went to a rival and 20% to
//!   us.
//! - **The field settles past us.** By the rivals' own public city counts, the
//!   median rival holds 7.2 cities to our 6.0 at t100, and the leader holds 8.4.
//!
//! The mechanism: [`super::early_settler_floor`] queues the opening's Settlers
//! but closes at standard turn 90, which is t60 Online (the 66% Online scale;
//! the live line reads "before turn 60"). After that the only
//! Settler path that reaches an idle queue early is the delegated governor's
//! `capital-settler-after-completion`, and by then the routine claims have
//! filled the capital's queue first. The `expansion-best-idle-city`
//! reservation runs after every claim and fired 17 times in about 50 games.
//!
//! ## What the gene does
//!
//! From the turn the early floor closes until a city founded now would have
//! fewer than [`HOLDABLE_LAND_PAYBACK_STANDARD_TURNS`] standard turns left,
//! the gene queues one Settler, in the city that trains it soonest, when all
//! of the following hold:
//!
//! - the empire's cities plus walking Settlers are short of the plan's
//!   settlement target;
//! - no city has a Settler queued and at most one Settler is walking;
//! - the plan is not Recovery and names no threatened city, and we are at war
//!   with no major;
//! - the city has population [`HOLDABLE_LAND_MIN_POP`] or more, was not
//!   attacked in the last four turns, has no barbarian raider in its home
//!   ring, and can train the Settler within the early floor's build cap;
//! - the city's queue head may give way: by the early floor's own test, or
//!   because it is a district at most half built (its progress is kept and
//!   `paused-district-finishes` resumes it), except the empire's first
//!   Campus while no Campus-family district stands.
//!
//! **Holdable** is the existing `settler-site-gate` verdict (an unclaimed
//! seat, outside every visible rival's Loyalty sphere, worth at least the
//! gate floor), run on the city's site list after three more filters:
//!
//! - no charted city of a living rival major within
//!   [`HOLDABLE_LAND_RIVAL_CITY_TILES`]. Peer census of 173 live second and
//!   third cities lost 56% at 5-6 tiles, 40% at 7-8, 23% at 9-11 and 8% at
//!   12+;
//! - no visible land soldier of a major fielding [`HOLDABLE_LAND_STRONG_RATIO`]
//!   times our military power within [`HOLDABLE_LAND_STRONG_SOLDIER_TILES`],
//!   at peace or not. Deity rivals park armies beside our cities and then
//!   declare (Austria 2 tiles from Gwangju at t44, declared t45);
//! - no visible barbarian or Free City soldier within
//!   [`HOLDABLE_LAND_RAIDER_TILES`].
//! - and the walker's own Loyalty forecast passes it, asked exactly as the
//!   walker asks it before walking (`settle_site_loyalty_verdict` on a
//!   Science seat or under the rate alarm, the frontier verdict otherwise).
//!   Without it the first armed games queued Settlers for seats the walker
//!   then refused: 083100Z stranded one for 26 turns and queued a second.
//!
//! The Settler it queues is protected from the routine claims through
//! `early_settler_floor_holds`, the same way the opening floor's Settler is.
//!
//! ⚠ The walker still chooses its own site once it exists. The gate decides
//! only *when* a Settler is worth building: when a holdable seat exists. The
//! walker's own sphere, threat and stranding rules still run.
//!
//! Off (the default) nothing here runs.

use super::early_settler_floor::{EARLY_SETTLER_FLOOR_STANDARD_TURN, EARLY_SETTLER_MAX_BUILD};
use super::settler_admission_journal::SETTLER_ADMISSION_JOURNAL_STRIDE;
use super::*;

/// Standard turns a new city must have left before the game ends for the
/// floor to queue its Settler: 50 turns at Online. A city founded later rarely
/// repays its Settler and the citizen it costs.
pub(crate) const HOLDABLE_LAND_PAYBACK_STANDARD_TURNS: u32 = 100;

/// The smallest city the floor trains a Settler in. The Settler costs one
/// citizen, and a city at the host's floor of two would be left at one.
pub(crate) const HOLDABLE_LAND_MIN_POP: i32 = 3;

/// A charted rival major's city this near a seat makes it not holdable.
pub(crate) const HOLDABLE_LAND_RIVAL_CITY_TILES: i32 = 9;

/// A stronger major's visible soldier this near a seat makes it not holdable.
pub(crate) const HOLDABLE_LAND_STRONG_SOLDIER_TILES: i32 = 6;

/// How much stronger than us a major must be for its soldiers to count. The
/// `garrison-before-the-surprise` bar.
pub(crate) const HOLDABLE_LAND_STRONG_RATIO: f64 = 1.5;

/// A visible barbarian or Free City soldier this near a seat makes it not
/// holdable.
pub(crate) const HOLDABLE_LAND_RAIDER_TILES: i32 = 3;

/// A city attacked within this many turns keeps its queue.
const HOLDABLE_LAND_ATTACK_MEMORY: u32 = 4;

/// The most of a district's cost that may already be paid for the floor's
/// Settler to take its queue. The host keeps a switched item's progress and
/// `paused-district-finishes` resumes it, so a young district loses only
/// time; one past half built finishes first.
pub(crate) const HOLDABLE_LAND_DISTRICT_DISPLACE_SHARE: f64 = 0.5;

/// The site-list radius, the one `settler_site_gate` reads.
const HOLDABLE_LAND_SITE_RADIUS: i32 = 11;

/// See [`AdvancedAi::holdable_land_threats`].
#[derive(Debug, Default, Clone)]
pub(crate) struct HoldableLandThreats {
    pub(crate) rival_cities: Vec<Pos>,
    pub(crate) strong_soldiers: Vec<Pos>,
    pub(crate) raiders: Vec<Pos>,
}

impl AdvancedAi {
    /// Whether the floor's window is open this turn: on, past the opening
    /// floor's window, and early enough for a new city to repay.
    pub(super) fn holdable_land_window(&self, g: &Game) -> bool {
        // `no-city-limit`: open from the first turn, beside the opening
        // floor; the pay test closes it. See `advanced/no_city_limit.rs`.
        // `score-seat-settles-what-pays`: a Score seat settles until its last
        // turns; the pay test in points closes it sooner when no city pays.
        let open_to_the_end = match self.score_seat_last_settler_turn(g) {
            Some(last) => g.turn <= last,
            None => {
                g.turn + g.standard_duration(HOLDABLE_LAND_PAYBACK_STANDARD_TURNS) < g.max_turns
            }
        };
        (self.no_city_limit
            || (self.holdable_land_keeps_settling
                && g.turn >= g.standard_duration(EARLY_SETTLER_FLOOR_STANDARD_TURN)))
            && open_to_the_end
    }

    /// Whether `cid`'s queued Settler is the floor's and should be kept: the
    /// window is open, the queue head is a Settler and the empire is still
    /// short of its target.
    pub(super) fn holdable_land_settler_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.holdable_land_window(g)
            || !matches!(
                g.cities[&cid].queue.first(),
                Some(Item::Unit { unit }) if *unit == "settler"
            )
        {
            return false;
        }
        let (walkers, queued) = Self::early_settler_census(g, pid);
        // `no-city-limit`: the other Settlers in production count too, so
        // the last seat is not trained for twice.
        let others = if self.no_city_limit {
            queued.saturating_sub(1)
        } else {
            0
        };
        g.player_city_ids(pid).len() + walkers + others < self.settlement_target(plan)
    }

    /// Whether `cid`'s queue head may give way to the floor's Settler: the
    /// early floor's test, or a district at most
    /// [`HOLDABLE_LAND_DISTRICT_DISPLACE_SHARE`] built with more than
    /// `paused-district-finishes`'s two turns of work left, except the
    /// empire's first Campus while none stands. In the live census an
    /// invested district was the queue that kept an otherwise eligible city
    /// 173 times on boards where no Settler was queued; Builders and soldiers
    /// still keep their queues.
    pub(super) fn holdable_land_may_displace(g: &Game, pid: usize, cid: u32) -> bool {
        if Self::early_settler_may_displace(g, pid, cid) {
            return true;
        }
        let Some(item @ Item::District { district, .. }) = g.cities[&cid].queue.first() else {
            return false;
        };
        // The empire's first Campus (Korea's Seowon) is its early science
        // jump and keeps its queue until one stands (-04).
        let campus = crate::name!("campus");
        if g.district_family(*district) == campus
            && !g.player_city_ids(pid).iter().any(|other| {
                g.cities[other]
                    .districts
                    .keys()
                    .any(|built| g.district_family(*built) == campus)
            })
        {
            return false;
        }
        let cost = g.item_cost_for_city(pid, cid, item);
        let invested = g.item_invested_production(cid, item);
        // More than `paused-district-finishes`'s two turns of work left, or
        // that gene puts the district straight back at the head of the queue
        // and the Settler claim is spent for nothing (3 of 75 census claims).
        let per_turn = g.city_yields(cid).production.max(0.5);
        cost > 0.0
            && invested <= HOLDABLE_LAND_DISTRICT_DISPLACE_SHARE * cost
            && (cost - invested) / per_turn
                > super::paused_district_finish::PAUSED_DISTRICT_FINISH_TURNS
    }

    /// The land soldier test shared by both soldier filters: a unit that can
    /// fight on land and is not recon.
    pub(super) fn holdable_land_soldier(g: &Game, unit: &crate::game::Unit) -> bool {
        let spec = &g.rules.units[unit.kind];
        spec.class == "military"
            && spec.domain.as_deref().is_none_or(|domain| domain == "land")
            && spec.promotion_class != "recon"
            && (spec.is_melee_capable() || spec.has_ranged_attack())
    }

    /// What the seat can see that makes land unholdable: charted cities of
    /// living rival majors, visible land soldiers of majors fielding
    /// [`HOLDABLE_LAND_STRONG_RATIO`] times our power (at peace or not), and
    /// visible barbarian and Free City soldiers. Read once per claim.
    pub(super) fn holdable_land_threats(g: &Game, pid: usize) -> HoldableLandThreats {
        let mut threats = HoldableLandThreats::default();
        let Some(seat) = g.players.get(pid) else {
            return threats;
        };
        let major = |owner: usize| {
            g.players.get(owner).is_some_and(|player| {
                player.alive && !player.is_minor && !player.is_barbarian && !player.is_free_city
            })
        };
        threats.rival_cities = g
            .cities
            .values()
            .filter(|city| {
                city.owner != pid && seat.explored.contains(&city.pos) && major(city.owner)
            })
            .map(|city| city.pos)
            .collect();
        let ours = g.military_power(pid).max(1.0);
        let strong: BTreeSet<usize> = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid
                    && major(player.id)
                    && g.military_power(player.id) >= HOLDABLE_LAND_STRONG_RATIO * ours
            })
            .map(|player| player.id)
            .collect();
        // Only what the seat sees this turn: the mirror carries foreign units
        // under current visibility, and the headless board must not read
        // through the fog.
        let visible = g.player_vision_frame(pid);
        for unit in g.units.values() {
            if unit.owner == pid
                || !Self::holdable_land_soldier(g, unit)
                || !g.sees(&visible, unit.pos)
                || !g.unit_visible_to(unit.id, pid)
            {
                continue;
            }
            if strong.contains(&unit.owner) {
                threats.strong_soldiers.push(unit.pos);
            } else if Some(unit.owner) == g.barb_pid
                || g.players
                    .get(unit.owner)
                    .is_some_and(|owner| owner.is_barbarian || owner.is_free_city)
            {
                threats.raiders.push(unit.pos);
            }
        }
        threats
    }

    /// Why `site` is not holdable, or `None` when it is.
    pub(super) fn holdable_land_refusal(
        g: &Game,
        threats: &HoldableLandThreats,
        site: Pos,
    ) -> Option<&'static str> {
        let near =
            |positions: &[Pos], tiles: i32| positions.iter().any(|at| g.wdist(*at, site) <= tiles);
        if near(&threats.rival_cities, HOLDABLE_LAND_RIVAL_CITY_TILES - 1) {
            Some("a rival city is near")
        } else if near(&threats.strong_soldiers, HOLDABLE_LAND_STRONG_SOLDIER_TILES) {
            Some("a stronger major's soldier is near")
        } else if near(&threats.raiders, HOLDABLE_LAND_RAIDER_TILES) {
            Some("a raider is near")
        } else {
            None
        }
    }

    /// Whether the walker's own Loyalty forecast refuses `site`, asked the way
    /// `settling.rs` asks it before the walk: `settle_site_loyalty_verdict`
    /// on a Science seat or under the rate alarm, the frontier verdict under
    /// `frontier_loyalty`, and no forecast at all otherwise.
    pub(super) fn holdable_land_loyalty_refuses(&self, g: &Game, pid: usize, site: Pos) -> bool {
        let science_targeted = self.active_victory_target(g) == Some(VictoryTarget::Science);
        // `score-seat-settles-what-pays`: a Score seat asks the full forecast,
        // its Dark Age test included. False with that gene off.
        if self.base.loyalty_rate_alarm || science_targeted || self.score_seat_settles(g) {
            self.settle_site_loyalty_verdict(g, pid, site).is_some()
        } else if self.frontier_loyalty {
            self.settle_site_frontier_loyalty_verdict(g, pid, site)
                .is_some()
        } else {
            false
        }
    }

    /// The best holdable seat for a Settler trained in a city at `from`: the
    /// `settler-site-gate` verdict over the city's acceptable sites that pass
    /// [`Self::holdable_land_refusal`]. `Err` says why there is none.
    pub(super) fn holdable_land_seat(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        walkers: usize,
        threats: &HoldableLandThreats,
    ) -> Result<(Pos, f64), String> {
        self.holdable_land_seat_within(g, pid, from, walkers, threats, HOLDABLE_LAND_SITE_RADIUS)
    }

    /// [`Self::holdable_land_seat`] over the city's site list at `radius`.
    /// `no-city-limit` asks it a second time, farther out, for a city with
    /// no seat of its own left.
    pub(super) fn holdable_land_seat_within(
        &self,
        g: &Game,
        pid: usize,
        from: Pos,
        walkers: usize,
        threats: &HoldableLandThreats,
        radius: i32,
    ) -> Result<(Pos, f64), String> {
        let mut sites = self.settle_sites(g, pid, from, radius);
        let found = sites.len();
        sites.retain(|(site, _)| {
            self.exhaustion_site_unpriceable(g, pid, *site).is_none()
                && !Self::inside_rival_sphere(g, pid, *site)
        });
        let acceptable = sites.len();
        let mut refused: BTreeMap<&'static str, usize> = BTreeMap::new();
        sites.retain(
            |(site, _)| match Self::holdable_land_refusal(g, threats, *site) {
                Some(why) => {
                    *refused.entry(why).or_default() += 1;
                    false
                }
                None => true,
            },
        );
        // The walker's own Loyalty forecast, asked the way `settling.rs` asks
        // it before the walk, last because it is the dearest test. A seat the
        // walker would refuse is not a seat: live 083100Z queued two Settlers
        // for seats the walker then refused ("beyond the empire's Loyalty
        // reach on ground the seat has not explored", 85 refusals), and one
        // stood stranded for 26 turns while the other was queued beside it.
        sites.retain(|(site, _)| {
            if self.holdable_land_loyalty_refuses(g, pid, *site) {
                *refused
                    .entry("the walker's Loyalty forecast refuses it")
                    .or_default() += 1;
                return false;
            }
            true
        });
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
        let floor = Self::settler_site_gate_floor(g.player_city_ids(pid).len());
        Self::settler_site_gate_verdict(g, &sites, &claimed, walkers, floor).map_err(|hold| {
            let refused: Vec<String> = refused
                .iter()
                .map(|(why, n)| format!("{n} where {why}"))
                .collect();
            format!(
                "{found} site(s), {acceptable} acceptable, {} holdable{}; {}",
                sites.len(),
                if refused.is_empty() {
                    String::new()
                } else {
                    format!(" (refused: {})", refused.join(", "))
                },
                hold.describe()
            )
        })
    }

    /// Journal why the floor queued nothing this turn, every
    /// [`SETTLER_ADMISSION_JOURNAL_STRIDE`] turns. Reads only.
    fn journal_holdable_land_decline(&self, g: &Game, why: &str) {
        if g.turn.is_multiple_of(SETTLER_ADMISSION_JOURNAL_STRIDE)
            && self.journal().wants(crate::reasoning::Level::Detail)
        {
            think!(self.journal(), Expansion, Detail,
                "No Settler for holdable land"; "holdable-land-keeps-settling: {why}");
        }
    }

    /// See the module: queue one Settler for holdable land in the
    /// unthreatened city that trains it soonest. Exact no-op with the gene
    /// off.
    pub(super) fn claim_holdable_land_settler(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.holdable_land_window(g) || self.base.minor || self.base.barb {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        let n_cities = city_ids.len();
        let (walkers, queued) = Self::early_settler_census(g, pid);
        let target = self.settlement_target(plan);
        // `no-city-limit`: several Settlers at once, each with its own seat.
        // `None` with the gene off. See `advanced/no_city_limit.rs`.
        let wide = self.no_city_limit_width(g, pid);
        let in_flight = walkers + queued;
        let full = match wide {
            Some(width) => in_flight >= width || n_cities + in_flight >= target,
            None => queued > 0 || walkers > 1 || n_cities + walkers >= target,
        };
        if n_cities == 0 || full {
            return;
        }
        // `no-city-limit`: Recovery is a war like any other. The soldier for
        // every city, the threatened city's neighbourhood and the enemy
        // soldiers near a city or a seat answer it below.
        if (plan.strategy == GrandStrategy::Recovery || plan.threatened_city.is_some())
            && wide.is_none()
        {
            self.journal_holdable_land_decline(
                g,
                &format!("{n_cities} cities against {target}, but the plan defends a city"),
            );
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.is_at_war(pid, other.id)
        });
        // `no-city-limit`: a war or a Recovery stops the Settlers only while
        // the army is short of a soldier for every city.
        let fighting = at_major_war || plan.strategy == GrandStrategy::Recovery;
        if fighting && (wide.is_none() || self.counts(g, pid).military < n_cities) {
            self.journal_holdable_land_decline(
                g,
                &format!("{n_cities} cities against {target}, but we are at war with a major"),
            );
            return;
        }
        let opening_capital = self
            .conquest_reservation_open(g)
            .then(|| {
                city_ids
                    .iter()
                    .copied()
                    .find(|cid| g.cities[cid].is_capital)
            })
            .flatten();
        let mut threats = Self::holdable_land_threats(g, pid);
        // `no-city-limit`: at war every visible enemy soldier makes a seat
        // unholdable and keeps the cities near it from training.
        let war_soldiers = if wide.is_some() && at_major_war {
            Self::no_city_limit_war_soldiers(g, pid)
        } else {
            Vec::new()
        };
        threats.strong_soldiers.extend(war_soldiers.iter().copied());
        // `no-city-limit`: the seat must be worth what a city needs to
        // out-earn its Settler, at any city count.
        let good_seat_floor = if wide.is_some() {
            self.no_city_limit_pay_floor(g, pid)
        } else {
            self.good_land_seat_floor(n_cities + walkers)
        };
        // Each Settler in flight takes a seat ahead of the one queued now.
        let seated = if wide.is_some() { in_flight } else { walkers };
        let item = Self::early_settler_item();
        let max_turns = g.standard_duration(EARLY_SETTLER_MAX_BUILD) as f64;
        let mut best: Option<(f64, u32, Pos, f64)> = None;
        // Why each city was passed over, for the decline line.
        let mut passed: BTreeMap<&'static str, usize> = BTreeMap::new();
        let mut seat_miss: Option<String> = None;
        for &cid in &city_ids {
            let city = &g.cities[&cid];
            let skip = if Some(cid) == opening_capital {
                Some("reserved for the conquest opening")
            } else if wide.is_some() && !Self::no_city_limit_city_is_clear(g, plan, cid) {
                Some("at the threatened city")
            } else if Self::no_city_limit_war_at_the_gates(g, &war_soldiers, city.pos) {
                Some("the war is at its gates")
            } else if wide.is_some()
                && matches!(
                    city.queue.first(),
                    Some(Item::Unit { unit }) if *unit == "settler"
                )
            {
                Some("already training a Settler")
            } else if city.pop < HOLDABLE_LAND_MIN_POP {
                Some("too small")
            } else if city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= HOLDABLE_LAND_ATTACK_MEMORY
            {
                Some("recently attacked")
            } else if match wide {
                // `no-city-limit`: a garrisoned city trains through raiders.
                Some(_) => Self::no_city_limit_raider_holds(g, pid, city.pos),
                None => Self::early_settler_raider_near(g, city.pos),
            } {
                Some("a raider in its ring")
            } else if self.science_campus_holds_repeat_settler(g, pid, cid, plan)
                || !Self::holdable_land_may_displace(g, pid, cid)
            {
                Some("its queue keeps its item")
            } else if !g.can_produce(pid, cid, &item) {
                Some("cannot train a Settler")
            } else {
                None
            };
            if let Some(why) = skip {
                *passed.entry(why).or_default() += 1;
                continue;
            }
            let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                g.item_cost_for_city(pid, cid, &item) / g.city_yields(cid).production.max(0.5)
                    // `no-city-limit`: the city's Settler modifiers count, so
                    // the Ancestral Hall's city and a Colonization empire
                    // train where the Settler is really cheapest.
                    / wide.map_or(1.0, |_| g.item_prod_mult(pid, cid, Some(&item)).max(0.1))
            });
            if turns > max_turns {
                *passed.entry("too slow").or_default() += 1;
                continue;
            }
            if best.as_ref().is_some_and(|(old, old_city, _, _)| {
                turns > *old + 1e-9 || ((turns - old).abs() <= 1e-9 && cid > *old_city)
            }) {
                continue;
            }
            let seat = self.holdable_land_seat(g, pid, city.pos, seated, &threats);
            // `no-city-limit`: a city with no seat of its own left trains for
            // one farther out; the site list prices the longer walk.
            let seat = match (seat, wide) {
                (Err(near), Some(_)) => self
                    .holdable_land_seat_within(
                        g,
                        pid,
                        city.pos,
                        seated,
                        &threats,
                        super::no_city_limit::NO_CITY_LIMIT_FAR_SEAT_RADIUS,
                    )
                    .map_err(|_| near),
                (seat, _) => seat,
            };
            match seat {
                // `good-land-lifts-the-city-target`: past the shared target
                // only a good seat is worth a Settler. Zero with it off.
                Ok((_, worth)) if worth < good_seat_floor => {
                    *passed
                        .entry("no good seat past the shared target")
                        .or_default() += 1;
                }
                Ok((site, worth)) => best = Some((turns, cid, site, worth)),
                Err(why) => {
                    *passed.entry("no holdable seat").or_default() += 1;
                    seat_miss.get_or_insert(format!("{}: {why}", city.name));
                }
            }
        }
        let Some((turns, cid, site, worth)) = best else {
            let passed: Vec<String> = passed.iter().map(|(why, n)| format!("{n} {why}")).collect();
            self.journal_holdable_land_decline(
                g,
                &format!(
                    "{n_cities} cities and {walkers} walking against {target}; cities passed \
                     over: {}{}",
                    passed.join(", "),
                    seat_miss.map_or(String::new(), |miss| format!("; {miss}"))
                ),
            );
            return;
        };
        let displaced = g.cities[&cid].queue.first().cloned();
        if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
            think!(self.journal(), Expansion, Decision,
                "{} starts a Settler for holdable land", g.cities[&cid].name;
                "holdable-land-keeps-settling: {n_cities} cities and {walkers} walking against a \
                 target of {target}; the best holdable seat {site:?} is worth {worth:.1}, \
                 {turns:.1} turns{}",
                displaced.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))); site);
            // `score-seat-settles-what-pays`: what the city is worth in points.
            self.explain_score_seat_settler(g, pid, cid);
        }
    }
}

#[cfg(test)]
mod tests;
