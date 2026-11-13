//! `score-seat-expands-hard`: a Score seat puts its early Production into
//! Settlers in every city that can train one, not only the capital, and takes
//! the Settler bonuses first: Magnus with Provision, then the Ancestral Hall.
//!
//! ## Why (38 live Deity Rome Score games, 2026-11-12 19:00Z to 11-13 04:30Z)
//!
//! Operator, 2026-11-13 04:32Z: *"we should expand harder and faster as rome
//! ... push outward hard, claiming great city locations."*
//!
//! - **We trail the field from turn 20.** Over the 30 games that reached turn
//!   150 the seat held 1.7 / 2.4 / 3.4 / 3.9 / 4.7 / 6.2 / 6.9 cities at turns
//!   20 / 30 / 40 / 50 / 60 / 80 / 100 against the met rivals' mean of 2.9 /
//!   3.4 / 4.2 / 4.7 / 5.5 / 6.4 / 7.5 (their leader: 3.0 / 3.6 / 4.5 / 5.3 /
//!   6.1 / 7.3 / 8.9). City 2 to city 8 were founded at a median turn 18, 28,
//!   38, 55, 65, 75 and 85: one city every ten turns.
//! - **The score follows the early count.** Cities at turn 60 against the
//!   score at the tally: r = 0.54, 56 points a city (three or fewer at turn
//!   60: 358 points; four or five: 550; six or seven: 638).
//! - **The capital is the only Settler source.** Fourteen games, turns 10 to
//!   80: the journal logged 27 Settler starts in the capital and one in every
//!   other city together. The other cities started a first Campus (44),
//!   Archers (11), Walls (10), Scouts (8).
//! - **Why the others train none.** The wide Settler claims of
//!   `no-city-limit` passed a city over 694 times in those turns: "its queue
//!   keeps its item" 243, "too small" 103, "at the threatened city" 94, "no
//!   holdable seat" 83, "a raider in its ring" 37, "too slow" 34. The opening
//!   floor may displace a building, an unstarted district, a Scout or a
//!   Trader and nothing else, and it closes at turn 60; the claim that runs
//!   after it asks population 3.
//! - **The capital stays small.** Each Settler takes a citizen: Rome stood at
//!   population 2 to 4 from turn 10 to turn 60 in both games read turn by
//!   turn (041706Z, 035921Z), making 7 to 13 Production. Magnus's Provision
//!   (a Settler takes no citizen in his city) came at turn 53, after Pingala
//!   took the first three titles (turns 20, 27, 30).
//! - **The Plaza serves a war we do not fight.** The Plaza stood at turn 28
//!   and took the Warlord's Throne at turns 45 to 50. The Ancestral Hall
//!   (`settler_production_pct` 50 in its city, a free Builder in every new
//!   city) was never built.
//!
//! ## What the gene does
//!
//! Only on a seat whose assigned victory target is Score, only with
//! `no-city-limit` on (the wide claims are its machinery), and only while a
//! city founded now still has [`HOLDABLE_LAND_PAYBACK_STANDARD_TURNS`]
//! standard turns to play (turn 94 of 160 Online). Off, an exact no-op.
//!
//! 1. **The opening floor stays open.** `early-settler-floor`'s claim, which
//!    asks the host's population 2 and no seat of its own, runs through the
//!    whole window instead of closing at turn 60. Its target is still
//!    `no-city-limit`'s: every free seat that pays.
//! 2. **More may give way to the Settler**
//!    ([`AdvancedAi::score_expands_may_displace`]): a district at most half
//!    paid (its progress is kept; never the empire's first Campus), a Builder
//!    at most half paid, and a soldier while no major is at war with us, the
//!    army already stands at one body a city, and no charted rival city lies
//!    within [`SCORE_EXPANDS_FRONTIER_TILES`] of the city. Walls, wonders,
//!    the Ancestral Hall and anything finishing next turn keep their queue.
//! 3. **A wider pipeline** ([`AdvancedAi::score_expands_width_for`]): one
//!    Settler more than the empire has cities, to six, walking or in
//!    production; two while the army is short of a body a city. The capital
//!    queues its next Settler while the first still walks.
//! 4. **Magnus first.** The first two Governor titles seat Magnus and give
//!    him Provision; Pingala's Librarian and Researcher follow. Magnus's seat
//!    already favours the city with a Settler in its queue.
//! 5. **The Ancestral Hall first.** The Plaza's tier list reads the Hall,
//!    then the Grand Master's Chapel, and the Hall takes the head of its
//!    city's queue once the Plaza stands
//!    ([`AdvancedAi::claim_score_expansion_hall`]).
//!
//! Journal reasons start `score-seat-expands-hard:`.
//!
//! ⚠ Not here: where a Settler walks and how long it shelters (the walker's
//! own rules and `score-seat-founds-early`), seats within nine tiles of a
//! rival city, and the Settlers of the last third of the game, which the
//! sim arm of `score-seat-settles-what-pays` (#5213) found cost what they
//! return.

use super::holdable_land_keeps_settling::HOLDABLE_LAND_DISTRICT_DISPLACE_SHARE;
use super::*;

/// The most Settlers walking or in production at once.
pub(crate) const SCORE_EXPANDS_MAX_WIDTH: usize = 6;

/// The width while the army is short of one body a city: the capital's
/// back-to-back Settlers and nothing more.
pub(crate) const SCORE_EXPANDS_THIN_WIDTH: usize = 2;

/// The most of a Builder's cost that may already be paid for a Settler to
/// take its queue.
pub(crate) const SCORE_EXPANDS_BUILDER_SHARE: f64 = 0.5;

/// The most of a Settler's cost that may already be paid for the Ancestral
/// Hall to go ahead of it.
pub(crate) const SCORE_EXPANDS_HALL_OVER_SETTLER_SHARE: f64 = 0.25;

/// A city attacked within this many turns keeps its queue.
const SCORE_EXPANDS_ATTACK_MEMORY: u32 = 4;

/// A charted rival major's city this near one of ours makes it a frontier
/// city: its soldier never gives way to a Settler. Rome fell in 5 of 14 live
/// games, each after a neighbour declared (session -c2).
pub(crate) const SCORE_EXPANDS_FRONTIER_TILES: i32 = 8;

impl AdvancedAi {
    /// The gene is on, `no-city-limit` carries it, and the seat aims at
    /// Score.
    pub(super) fn score_expands_hard(&self) -> bool {
        self.score_seat_expands_hard
            && self.no_city_limit
            && self.victory_target == Some(VictoryTarget::Score)
    }

    /// [`Self::score_expands_hard`] inside the settling window.
    pub(super) fn score_expands_hard_now(&self, g: &Game) -> bool {
        self.score_expands_hard() && self.holdable_land_window(g)
    }

    /// The Settlers that may be walking or in production at once. See the
    /// module, item 3.
    pub(super) fn score_expands_width_for(cities: usize, soldiers: usize) -> usize {
        if cities == 0 {
            1
        } else if soldiers < cities {
            SCORE_EXPANDS_THIN_WIDTH
        } else {
            (1 + cities).min(SCORE_EXPANDS_MAX_WIDTH)
        }
    }

    /// Whether `cid`'s queue head may give way to a Settler beyond what
    /// `early_settler_may_displace` already allows. `army_stands`: no major is
    /// at war with us and the army holds one body a city. See the module,
    /// item 2.
    pub(super) fn score_expands_may_displace(
        g: &Game,
        pid: usize,
        cid: u32,
        army_stands: bool,
    ) -> bool {
        let Some(item) = g.cities[&cid].queue.first() else {
            return true;
        };
        let cost = g.item_cost_for_city(pid, cid, item);
        let invested = g.item_invested_production(cid, item);
        if cost - invested <= g.city_yields(cid).production {
            return false;
        }
        match item {
            Item::District { district, .. } => {
                let campus = crate::name!("campus");
                let first_campus = g.district_family(*district) == campus
                    && !g.player_city_ids(pid).iter().any(|other| {
                        g.cities[other]
                            .districts
                            .keys()
                            .any(|built| g.district_family(*built) == campus)
                    });
                !first_campus && invested <= HOLDABLE_LAND_DISTRICT_DISPLACE_SHARE * cost
            }
            Item::Unit { unit } if *unit == "builder" => {
                invested <= SCORE_EXPANDS_BUILDER_SHARE * cost
            }
            Item::Unit { unit } => {
                army_stands
                    && g.rules.units.get(unit).is_some_and(|spec| {
                        spec.class == "military" && spec.domain.as_deref() != Some("sea")
                    })
            }
            _ => false,
        }
    }

    /// Whether a city at `pos` stands within [`SCORE_EXPANDS_FRONTIER_TILES`]
    /// of a charted rival major's city (`holdable_land_threats`' list).
    pub(super) fn score_expands_on_the_frontier(g: &Game, rival_cities: &[Pos], pos: Pos) -> bool {
        rival_cities
            .iter()
            .any(|rival| g.wdist(*rival, pos) <= SCORE_EXPANDS_FRONTIER_TILES)
    }

    /// Whether the head of `cid`'s queue is the Ancestral Hall, which no
    /// Settler displaces: it is what makes the next ones cheap.
    pub(super) fn score_expands_keeps_the_hall(g: &Game, cid: u32) -> bool {
        matches!(
            g.cities[&cid].queue.first(),
            Some(Item::Building { building }) if *building == "ancestral_hall"
        )
    }

    /// Whether the next Governor titles seat Magnus and give him Provision
    /// before Pingala's. See the module, item 4.
    pub(super) fn score_expands_wants_magnus(&self, g: &Game, pid: usize) -> bool {
        self.score_expands_hard_now(g)
            && !g.players[pid]
                .governor_roster
                .get("magnus")
                .is_some_and(|state| state.promotions.contains("provision"))
    }

    /// See the module, item 5: once the Plaza stands, the Ancestral Hall takes
    /// the head of its city's queue, ahead of routine work and of a Settler
    /// at most a quarter paid. Exact no-op with the gene off.
    pub(super) fn claim_score_expansion_hall(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.score_expands_hard_now(g) {
            return;
        }
        let hall = Item::Building {
            building: crate::name!("ancestral_hall"),
        };
        let Some(cid) = g
            .player_city_ids(pid)
            .into_iter()
            .find(|cid| g.can_produce(pid, *cid, &hall))
        else {
            return;
        };
        let city = &g.cities[&cid];
        if city.queue.first() == Some(&hall)
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= SCORE_EXPANDS_ATTACK_MEMORY)
        {
            return;
        }
        let gives_way = match city.queue.first() {
            None => true,
            Some(head @ Item::Unit { unit }) if *unit == "settler" => {
                let cost = g.item_cost_for_city(pid, cid, head);
                g.item_invested_production(cid, head)
                    <= SCORE_EXPANDS_HALL_OVER_SETTLER_SHARE * cost
            }
            Some(_) => {
                Self::early_settler_may_displace(g, pid, cid)
                    || Self::score_expands_may_displace(g, pid, cid, true)
            }
        };
        if !gives_way {
            return;
        }
        let displaced = city.queue.first().cloned();
        let name = city.name.clone();
        let cost = g.item_cost_for_city(pid, cid, &hall);
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: hall,
            },
        )
        .is_ok()
        {
            think!(self.journal(), Expansion, Decision,
                "{name} raises the Ancestral Hall before its next Settler";
                "score-seat-expands-hard: {cost:.0} Production for half again as much toward \
                 every Settler here and a free Builder in every new city{}",
                displaced.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
