//! `culture-seat-monuments`: on a seat aiming at a Culture victory, every
//! unthreatened city of ours builds its Monument early, once the second city
//! stands, ahead of a deferrable head.
//!
//! **Why (live Emperor Pericles, 2026-10-30, civvis-20261030T090715Z — the
//! first Emperor Culture seat).**
//! - Six cities at t60 held ONE Monument between them (three at t80, five at
//!   t100), and the empire made 12 Culture a turn at t60 against Egypt's 56,
//!   55 at t100 against 121. The Culture race is our domestic tourists (our
//!   lifetime Culture) against the rivals', and the civics that open the
//!   Culture lane's buildings come from the same output.
//! - `culture-seat-theater-first` put the Acropolis in four of six cities by
//!   t80, but a Theater Square holds no Culture until its buildings and
//!   Great Works arrive; a Monument is two Culture a turn from the turn it is
//!   finished, for the cheapest building on the menu.
//! - `capital-monument-after-the-second-city` covers only the capital, and on
//!   the live seat the early Settler floor takes that queue first.
//!
//! **What the gene does.** For a Culture seat with two cities or more and no
//! major at war with us, a city with no Monument that can finish one within
//! `CAPITAL_MONUMENT_MAX_STANDARD` standard turns claims it: an idle queue,
//! or one over a deferrable head that keeps its progress (the Theater
//! claim's rule: a Settler, Walls, a wonder, a project, the faith defence's
//! buildings and anything finishing this turn keep their place). The heads
//! the other claims hold keep their place too, and the delegated governor's
//! rescoring leaves the claimed Monument alone
//! ([`AdvancedAi::culture_monument_holds`]).
//!
//! Off, or for a seat not aiming at Culture, nothing changes.
//!
//! **Version two, `culture-seat-monuments-2` (live Deity Pericles and
//! Kristina, 28 Culture games since 2026-11-10T19:00Z).** A non-capital city
//! founded by t60 waited a median 48 turns for its Monument, and only 42% of
//! our cities held one at t60. Of 4,810 city-turns with no Monument, the two
//! war gates stopped the claim empire-wide on 1,147: `threatened-war-holds-
//! the-claims` (891) and version one's own return while any major is at war
//! with us (256). In 538 of those the city had no at-war major's soldier
//! within six tiles and could finish within the cap (14 Online turns; the
//! cap itself held only 242). Separately, 1,331
//! city-turns had a Monument half built or three Online turns from done
//! sitting behind another head: civvis-20261111T000134Z's Pharsalos held one
//! with 14 of 30 invested from t33 to t77, and the Granary claim took
//! Argos's at t18.
//! - The war holds the claim only for a city that is itself threatened: the
//!   plan's threatened city, a city attacked within four turns, or one with an
//!   at-war major's land or naval soldier within
//!   [`MONUMENT_THREAT_RADIUS`] tiles. `threatened-war-holds-the-claims`
//!   no longer holds this claim (`advanced/hint_runs_the_claims.rs`).
//! - A Monument within [`MONUMENT_FINISH_STANDARD`] standard turns of done,
//!   or with half its cost invested, goes before any head except a Settler
//!   under the early floor, Walls, a live Great Person commitment, and
//!   anything finishing this turn or no later than the Monument. The claims
//!   that run before this one and took a Monument's queue on those boards
//!   (100 times in 28 games: the bound Granary 20, the early Settler floor
//!   14, the idle-route Trader 11, the Theater 11, the backlog Builder 11,
//!   frontier Walls 8, the sea scout 6, the free-slot Campus 5) leave a
//!   near-done one at the head
//!   ([`AdvancedAi::culture_monument_finishes_first`]), as do the first
//!   Culture buildings, the Faith source and the unique Builders, so no pass
//!   moves it twice. The Settler floor and the Walls keep their place.
//!
//! Version one's holds are otherwise unchanged. With version two off the
//! board plays exactly as version one.
//!
//! **`science-seat-monuments`: the same claim for a Science seat (live
//! Tournament 7, 36 Science seats on Lv 8 Deity, read 2026-11-27).** At t100
//! only 39% of the seats' 258 cities held a Monument and 2% a Theater Square;
//! the seat's Culture ran a median 35 a turn against the best rival's 168.
//! Civics lagged (about 18 at t100; 27 against the rivals' 37 at t150), so 30
//! of 32 seats still governed from Monarchy at t150, Exploration (Merchant
//! Republic) was unlocked in none and Suffrage (Democracy) in one: fewer and
//! later policy slots for the science cards. `monument-first` sits in the
//! basic ladder that the advanced claims (the Settler floor, the Campus
//! claims, the Builders, Traders and Granaries) usually reach first.
//! - For a seat whose active victory target is Science, this module's claim
//!   runs on version one's holds from [`SCIENCE_MONUMENT_MIN_CITIES`] cities.
//!   On top of them, science keeps its queue (a Campus-family district or
//!   one of its buildings: [`AdvancedAi::science_head`], the rule
//!   `science-heads-keep-their-queue` set for the backlog Builder), as does
//!   `trade-district-follows-the-campus`' claimed district or building.
//! - War holds the claim only in a threatened city, by version two's gate:
//!   the plan's threatened city, a city attacked within four turns, or one
//!   with an at-war major's soldier within [`MONUMENT_THREAT_RADIUS`] tiles.
//!   A soldier at the head keeps its queue while any major is at war with us.
//!   On those boards 12% of the seats' turns from t30 to t150 were at war
//!   with a major (14% of their Monument-less city-turns); version one's
//!   empire-wide hold would leave the rear cities unclaimed all that time.
//! - Version two's near-done pass is the Culture seat's alone: on a Science
//!   seat a Monument never goes ahead of science, a wonder or a project.
//!
//! Off, or for any other victory target, nothing changes; a Culture seat
//! plays as before with the gene on.

use super::capital_monument::{CAPITAL_MONUMENT_MAX_STANDARD, CAPITAL_MONUMENT_MIN_CITIES};
use super::*;

/// Version two: an at-war major's soldier this close to a city holds its
/// Monument claim.
pub(crate) const MONUMENT_THREAT_RADIUS: i32 = 6;

/// Version two: a Monument this many standard turns from done (three Online
/// turns: Online durations are 66% of Standard, rounded up) is finished
/// before the city's other heads.
pub(crate) const MONUMENT_FINISH_STANDARD: u32 = 4;

/// `science-seat-monuments`: the city count from which a Science seat claims.
/// The early Settler floor and the first Campus come first in the opening,
/// and `capital-monument-after-the-second-city` already covers the capital at
/// two.
pub(crate) const SCIENCE_MONUMENT_MIN_CITIES: usize = 3;

impl AdvancedAi {
    /// The victory target whose Monument claim this seat plays: Culture with
    /// `culture-seat-monuments` (or `-2`), Science with
    /// `science-seat-monuments`. `None` otherwise.
    fn culture_monument_lane(&self, g: &Game) -> Option<VictoryTarget> {
        if !(self.culture_seat_monuments
            || self.culture_seat_monuments_2
            || self.science_seat_monuments)
        {
            return None;
        }
        match self.active_victory_target(g)? {
            VictoryTarget::Culture
                if self.culture_seat_monuments || self.culture_seat_monuments_2 =>
            {
                Some(VictoryTarget::Culture)
            }
            VictoryTarget::Science if self.science_seat_monuments => Some(VictoryTarget::Science),
            _ => None,
        }
    }

    /// Whether the claim holds at war only in a threatened city (version
    /// two's gate): version two, or a Science seat's claim. Under it the claim
    /// also runs through `threatened-war-holds-the-claims`.
    pub(super) fn culture_monuments_hold_locally(&self, g: &Game) -> bool {
        self.culture_seat_monuments_2
            || self.culture_monument_lane(g) == Some(VictoryTarget::Science)
    }

    /// The Monument `cid` would claim and its build turns: a Culture seat's
    /// unthreatened city of ours with none built, once the second city
    /// stands, or a Science seat's from its third. `None` with the genes off
    /// or for another victory target.
    pub(super) fn culture_monument_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64)> {
        let science = self.culture_monument_lane(g)? == VictoryTarget::Science;
        let min_cities = if science {
            SCIENCE_MONUMENT_MIN_CITIES
        } else {
            CAPITAL_MONUMENT_MIN_CITIES
        };
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || g.player_city_ids(pid).len() < min_cities
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || ((science || self.culture_seat_monuments_2)
                && Self::monument_city_under_arms(g, pid, city.pos))
        {
            return None;
        }
        let monument = BasicAi::civ_building(g, pid, cid, "monument")?;
        let turns = g.host_production_turns(cid, &monument).unwrap_or_else(|| {
            let remaining = g.item_cost_for_city(pid, cid, &monument)
                - g.item_invested_production(cid, &monument);
            remaining.max(0.0) / g.city_yields(cid).production.max(0.5)
        });
        (turns <= g.standard_duration(CAPITAL_MONUMENT_MAX_STANDARD) as f64)
            .then_some((monument, turns))
    }

    /// Whether `item` at the head of `cid` is the Monument this gene claims
    /// and the city still wants it: the delegated governor's rescoring leaves
    /// it alone. Always false with the gene off.
    pub(super) fn culture_monument_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        self.culture_monument_item(g, pid, cid, plan)
            .is_some_and(|(monument, _)| monument == *item)
    }

    /// Version two: whether an at-war major's land or naval soldier stands
    /// within [`MONUMENT_THREAT_RADIUS`] tiles of `pos`.
    fn monument_city_under_arms(g: &Game, pid: usize, pos: Pos) -> bool {
        g.units.values().any(|unit| {
            unit.owner != pid
                && g.players
                    .get(unit.owner)
                    .is_some_and(|owner| !owner.is_minor && !owner.is_barbarian)
                && g.is_at_war(pid, unit.owner)
                && g.rules.units.get(&unit.kind).is_some_and(|spec| {
                    spec.class == "military" && spec.domain.as_deref() != Some("air")
                })
                && g.wdist(pos, unit.pos) <= MONUMENT_THREAT_RADIUS
        })
    }

    /// Version two: whether a Monument this city may claim is near done:
    /// within [`MONUMENT_FINISH_STANDARD`] standard turns, or with half its
    /// cost invested.
    fn culture_monument_near_done(
        g: &Game,
        pid: usize,
        cid: u32,
        monument: &Item,
        turns: f64,
    ) -> bool {
        let invested = g.item_invested_production(cid, monument);
        turns <= g.standard_duration(MONUMENT_FINISH_STANDARD) as f64
            || (invested > 0.0 && invested * 2.0 >= g.item_cost_for_city(pid, cid, monument))
    }

    /// Version two: whether `item` at the head of `cid` is a near-done
    /// Monument this gene claims, so a claim that runs before this one leaves
    /// it there. Always false with version two off, and on a Science seat.
    pub(super) fn culture_monument_finishes_first(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        self.culture_seat_monuments_2
            && self.culture_monument_lane(g) == Some(VictoryTarget::Culture)
            && self
                .culture_monument_item(g, pid, cid, plan)
                .is_some_and(|(monument, turns)| {
                    monument == *item && Self::culture_monument_near_done(g, pid, cid, item, turns)
                })
    }

    /// Version two: whether the head `item` keeps its place ahead of a
    /// near-done Monument `monument_turns` from done: a Settler under the
    /// early floor, Walls, a live Great Person commitment, or anything
    /// finishing this turn or no later than the Monument (a paused district
    /// two turns from done is put back by `paused-district-finishes`).
    fn culture_monument_near_done_yields_to(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
        monument_turns: f64,
    ) -> bool {
        let production = g.city_yields(cid).production;
        let remaining =
            g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item);
        let turns = g
            .host_production_turns(cid, item)
            .filter(|turns| turns.is_finite() && *turns > 0.0)
            .unwrap_or_else(|| remaining.max(0.0) / production.max(0.5));
        self.early_settler_floor_holds(g, pid, cid, plan)
            || matches!(item, Item::Building { building } if matches!(
                building.as_str(), "walls" | "medieval_walls" | "renaissance_walls"))
            || Self::live_gp_cultural_building_commitment(g, pid, cid, item)
            || remaining <= production
            || turns <= monument_turns
    }

    /// See the module: every eligible city of a Culture seat (or, with
    /// `science-seat-monuments`, a Science seat) claims its Monument. Exact
    /// no-op with the genes off or for another target.
    pub(super) fn claim_culture_monuments(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some(lane) = self.culture_monument_lane(g) else {
            return;
        };
        let science = lane == VictoryTarget::Science;
        // Version two's near-done pass is the Culture seat's alone.
        let version_two = self.culture_seat_monuments_2 && !science;
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        if at_major_war && !(self.culture_seat_monuments_2 || science) {
            return;
        }
        for cid in g.player_city_ids(pid) {
            let Some((monument, turns)) = self.culture_monument_item(g, pid, cid, plan) else {
                continue;
            };
            let head = g.cities[&cid].queue.first().cloned();
            if head.as_ref() == Some(&monument) {
                continue;
            }
            let near_done =
                version_two && Self::culture_monument_near_done(g, pid, cid, &monument, turns);
            if let Some(item) = &head {
                if near_done {
                    if self.culture_monument_near_done_yields_to(g, pid, cid, plan, item, turns) {
                        continue;
                    }
                } else if self.early_settler_floor_holds(g, pid, cid, plan)
                    || self.bound_granary_holds(g, pid, cid, plan, item)
                    || self.capital_monument_holds(g, pid, cid, plan, item)
                    || self.culture_theater_holds(g, pid, cid, plan, item)
                    || self.deterrent_holds_against_culture(g, pid, item)
                    || self.idle_route_trader_holds(g, pid, cid, plan, item)
                    || self.backlog_builder_holds(g, pid, cid, plan, item)
                    || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                    // `science-seat-monuments`: science and the Science
                    // lane's trade district keep their queue.
                    || (science
                        && (Self::science_head(g, item)
                            || self.trade_district_claim_holds(g, pid, cid, plan, item)))
                    || !Self::culture_theater_may_defer(g, pid, cid, item, at_major_war)
                {
                    continue;
                }
            }
            let name = g.cities[&cid].name.clone();
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: monument,
                },
            )
            .is_ok()
            {
                let ahead = head.map_or_else(
                    || "an idle queue".to_string(),
                    |item| format!("{} (it keeps its progress)", Self::plain_item(&item)),
                );
                if near_done {
                    think!(self.journal(), Cities, Decision,
                           "{name} finishes its Monument first";
                           "culture-seat-monuments-2: {turns:.1} turns left, ahead of {ahead}");
                } else if science {
                    think!(self.journal(), Cities, Decision,
                           "{name} claims its Monument for the civics";
                           "science-seat-monuments: {turns:.1} turns, ahead of {ahead}");
                } else {
                    think!(self.journal(), Cities, Decision,
                           "{name} claims its Monument for the Culture victory";
                           "culture-seat-monuments: {turns:.1} turns, ahead of {ahead}");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
