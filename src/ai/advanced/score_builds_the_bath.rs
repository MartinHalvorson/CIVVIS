//! `score-seat-builds-the-bath`: a Score seat whose civilization replaces the
//! Aqueduct with a unique district (Rome's Bath) builds it in every city that
//! can hold one, as soon as the city can.
//!
//! ## Why (live Deity Rome, 6-player Continents, Online, turn limit 170)
//!
//! A district is two points at the tally and a unique one four
//! (`Game::score_parts`), and the Bath is the cheapest district there is: 23
//! Production at Online when the host first offers it, against 45 for a
//! Library's single point. That is 17 points a hundred Production where the
//! seat's queues buy about two (`score-seat-plays-for-points`), before its
//! Housing and its Amenity. It does not count against the population limit
//! on districts.
//!
//! Eight games of 2026-11-13 (`civvis-20261113T104009Z` to `T135337Z`, 78
//! cities):
//!
//! - 40 cities held a Bath at the end and 11 more had it in the host's
//!   buildable list and never built it.
//! - A city that built one waited a median 30 turns after the host first
//!   offered it (mean 33.5; over 20 turns in 70% of them). At turn 100 the
//!   empire held 2.4 Baths in 8.4 cities, at turn 160 5.4 in 10.4.
//! - Those cities spent 280 city-turns a game with a Bath on offer and none
//!   built, on a Campus (15%), a University, a Builder, a Granary, a Library,
//!   an Industrial Zone.
//!
//! The Bath is only ever asked for as a housing repair:
//! `aqueduct-claims-the-capped-queue` claims it for a city of three citizens
//! within one of its housing with its Granary built, one city a turn, and
//! leaves every science and culture item ahead of it.
//!
//! ## What the gene does
//!
//! Only on a seat whose assigned victory target is Score, and only for a
//! civilization whose Aqueduct is a unique district. Off, an exact no-op.
//!
//! Every turn, each city of [`SCORE_BATH_MIN_POP`] citizens or more with no
//! Aqueduct, neither threatened nor attacked lately, whose legal Bath
//! finishes within [`SCORE_BATH_MAX_STANDARD`] standard turns, puts it at the
//! head of its queue ([`AdvancedAi::claim_score_bath`]). The head it takes
//! keeps its progress. These keep their queue ([`AdvancedAi::score_bath_may_defer`]):
//! a military unit, Walls, a wonder, a project, a Settler more than
//! [`SCORE_BATH_OVER_SETTLER_SHARE`] paid, another district more than half
//! paid, and anything finishing this turn.
//!
//! The claimed Bath then keeps the head: the routine claims read
//! `early_settler_floor_holds`, and the two Settler claims skip its city
//! ([`AdvancedAi::score_bath_holds`]).
//!
//! Journal reasons start `score-seat-builds-the-bath:`.

use super::*;

/// The longest the claimed district may take, in standard turns (12 at
/// Online): a city too small to finish it by then grows first.
pub(crate) const SCORE_BATH_MAX_STANDARD: u32 = 18;

/// The smallest city that takes it.
pub(crate) const SCORE_BATH_MIN_POP: i32 = 2;

/// The most of a Settler's cost that may already be paid for the district to
/// go ahead of it.
pub(crate) const SCORE_BATH_OVER_SETTLER_SHARE: f64 = 0.25;

/// The most of another district's cost that may already be paid for this one
/// to go ahead of it.
pub(crate) const SCORE_BATH_OVER_DISTRICT_SHARE: f64 = 0.5;

/// A city attacked within this many turns keeps its queue.
const SCORE_BATH_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// The gene is on and the seat aims at Score.
    pub(super) fn score_builds_the_bath(&self) -> bool {
        self.score_seat_builds_the_bath && self.victory_target == Some(VictoryTarget::Score)
    }

    /// The seat's unique replacement of the Aqueduct, when its civilization
    /// has one.
    pub(super) fn score_bath_district(g: &Game, pid: usize) -> Option<Name> {
        let district = BasicAi::civ_district(g, pid, "aqueduct");
        (district != "aqueduct").then_some(district)
    }

    /// The district `cid` would claim and the turns it takes: see the module.
    /// `None` with the gene off.
    pub(super) fn score_bath_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        if !self.score_builds_the_bath() {
            return None;
        }
        let bath = Self::score_bath_district(g, pid)?;
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || city.pop < SCORE_BATH_MIN_POP
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= SCORE_BATH_ATTACK_MEMORY)
            || g.city_has_district_family(city, crate::name!("aqueduct"))
        {
            return None;
        }
        let pos = g
            .district_sites(cid, bath)
            .into_iter()
            .filter(|pos| {
                g.can_produce(
                    pid,
                    cid,
                    &Item::District {
                        district: bath,
                        pos: *pos,
                    },
                )
            })
            .min()?;
        let item = Item::District {
            district: bath,
            pos,
        };
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            let remaining =
                g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item);
            remaining.max(0.0) / g.city_yields(cid).production.max(0.5)
        });
        (turns <= g.standard_duration(SCORE_BATH_MAX_STANDARD) as f64).then_some((item, turns))
    }

    /// Whether the head of `cid`'s queue is the seat's Bath under the gene:
    /// no Settler claim displaces it and the routine claims leave it alone.
    pub(super) fn score_bath_holds(&self, g: &Game, pid: usize, cid: u32) -> bool {
        self.score_builds_the_bath()
            && Self::score_bath_district(g, pid).is_some_and(|bath| {
                matches!(
                    g.cities[&cid].queue.first(),
                    Some(Item::District { district, .. }) if *district == bath
                )
            })
    }

    /// Whether the queue head `item` may wait for the Bath. See the module.
    pub(super) fn score_bath_may_defer(g: &Game, pid: usize, cid: u32, item: &Item) -> bool {
        let cost = g.item_cost_for_city(pid, cid, item);
        let invested = g.item_invested_production(cid, item);
        if cost - invested <= g.city_yields(cid).production {
            return false;
        }
        match item {
            Item::Unit { unit } if *unit == "settler" => {
                invested <= SCORE_BATH_OVER_SETTLER_SHARE * cost
            }
            Item::Unit { unit } => g
                .rules
                .units
                .get(unit)
                .is_some_and(|spec| spec.class != "military"),
            Item::Building { building } => g.rules.buildings.get(building).is_some_and(|spec| {
                !spec.wonder
                    && !matches!(
                        building.as_str(),
                        "walls" | "medieval_walls" | "renaissance_walls"
                    )
            }),
            Item::District { .. } => invested <= SCORE_BATH_OVER_DISTRICT_SHARE * cost,
            _ => false,
        }
    }

    /// See the module: every city with a legal, timely Bath and none built
    /// puts it at the head of its queue, idle or over a head that may wait.
    /// Exact no-op with the gene off.
    pub(super) fn claim_score_bath(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.score_builds_the_bath() {
            return;
        }
        for cid in g.player_city_ids(pid) {
            let Some((bath, turns)) = self.score_bath_item(g, pid, cid, plan) else {
                continue;
            };
            let displaced = match g.cities[&cid].queue.first() {
                Some(head) if *head == bath => continue,
                Some(head) => {
                    if !Self::score_bath_may_defer(g, pid, cid, head) {
                        continue;
                    }
                    Some(head.clone())
                }
                None => None,
            };
            let name = g.cities[&cid].name.clone();
            let cost = g.item_cost_for_city(pid, cid, &bath);
            let label = Self::plain_item(&bath);
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: bath,
                },
            )
            .is_ok()
            {
                think!(self.journal(), Economy, Decision,
                    "{name} builds its {label} for the tally";
                    "score-seat-builds-the-bath: {cost:.0} Production in {turns:.1} turns for \
                     four points, its Housing and an Amenity{}",
                    displaced.map_or(String::new(), |item| format!(
                        "; {} keeps its progress", Self::plain_item(&item))));
            }
        }
    }
}

#[cfg(test)]
mod tests;
