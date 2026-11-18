//! `science-seat-campus-before-the-claims`: on a Science seat the empire's
//! first Campus goes ahead of the floor's Settler, a quick first Campus in
//! any city goes ahead of the economy claims that keep its queue, and the
//! Government Plaza never takes the last slot of a city with no Campus.
//!
//! **Why (live Deity, 6-player Continents, Online, 250 turns, the nine games
//! of 2026-11-18 04:58Z to 11:16Z: five on Science, four on Diplomacy).** No
//! Science seat won, and none held more than 0.71 of the best rival's techs
//! at t150 (a Science win has needed 0.82). Writing landed at t27-37 in every
//! game. In the cities founded by t70 the Science seats placed the first
//! Campus at a median t79 (37 turns after the host first offered it) against
//! the Diplomatic seats' t73 (21 turns), and the empire's first three
//! Campuses at t55 / t68 / t79 against t38 / t58 / t64. The Science capitals
//! placed theirs at t35, t68, t80, t83 and t104. Lisbon
//! (`civvis-20261118T084012Z`) had a four-to-seven-turn Campus on offer for
//! 77 turns while its queue held a Settler (29 turns), soldiers and siege
//! (19), a Builder (8), a Harbor (7), a Trader (6), Walls (4), a Granary (2)
//! and an Entertainment Complex (2). The Library was not late: it started a
//! mean 2.1 turns after its Campus stood (7.8 on the Diplomatic seats).
//!
//! Over the 1,203 city-turns between a Campus on the host's menu and its
//! placing (31 Science cities), `campus-claims-the-free-slot` journalled what
//! held it on 536: the early floor's Settler 162, the backlog Builder 100,
//! near-rival deterrence 86, a head it may not defer 78 (Walls 55, a Harbor
//! with progress 16), the bound Granary 70, an item finishing that turn 26.
//! Of the Settler's 162, 93 were turns with no Campus anywhere in the empire,
//! at three to five cities, and of the other 69, 24 came at four or five
//! cities and 45 at six or more. The rest wrote no line: on 214 the city's
//! slot had gone to a Government Plaza, a Diplomatic Quarter or a Harbor, on
//! 179 the Campus took over the claim's fifteen-turn budget, 172 fell under
//! the war's hold and 85 in a city of one citizen.
//!
//! **One claim is the Science lane's own.** `plaza-serves-the-scientist-3`
//! (armed, Science aim only; `claim_queue_for_the_plaza_district`) starts the
//! Government Plaza in the capital once its Campus stands, and until then in
//! the most productive other city with routine work at its head. In all five
//! Science games it went, at t35-49, to a second-to-fourth city of one to
//! three citizens, and in four it took that city's only specialty slot:
//! Dumfries (one citizen, t35), Funchal (two, t40), Mbamba (three, t42) and
//! New York (three, t49), whose Campuses were placed 63, 99, 37 and 6 turns
//! later (Germany's Aachen has a second slot). The Plaza, the Diplomatic
//! Quarter and their buildings held 4.9 turns a city of the Campus wait
//! against the Diplomatic seats' 0.9. The generic
//! `plaza-in-the-district-list` has the same hand-off while the capital
//! builds its Campus (Nippur, one citizen, t38, on the one Diplomatic seat
//! that did it; its Campus came at t108); the other three Diplomatic seats
//! placed the Plaza at t49, t82 and t96 in cities of four to seven citizens
//! with a slot to spare.
//!
//! **The rest is not a Science-lane rule.** The claim chain is the same on
//! every seat: the Campus claim runs after the Settler, Builder, Monument, Granary,
//! deterrent, Trader, Aqueduct and Industrial Zone claims
//! (`run_idle_queue_claims`) and yields to each one's head, and the two genes
//! that let it pass them, `first-campus-before-the-settler` and
//! `campus-and-library-before-the-claims`, read
//! [`AdvancedAi::conquest_or_score_seat`], so they do nothing on a Science or
//! a Diplomatic seat. Seven of the nine capitals got their Campus the first
//! turn the claim met an idle queue: t31, t36 and t40 on three Diplomatic
//! seats, t68 to t83 on three Science seats and the fourth Diplomatic one.
//! The Diplomatic seats' holds have the same shape (turns per city: Settler
//! 2.6, Builder 2.4, Granary 1.6, against the Science seats' 5.2, 3.2 and
//! 2.3). What separated the nine games was not the lane. Three of the four
//! Diplomatic games, both wins among them, ran without six early-defence
//! genes that all five Science games carried (Walls, soldiers and siege took
//! 23% of the Science seats' production over t30-100 against those three
//! games' 12%; the fourth Diplomatic game carried them and read Science 40 at
//! t100, below every Science seat). Three of the five Science games were at
//! war with a major before t100 and no Diplomatic game was. The paired
//! simulator, one genome on both lanes, reads the Science lane ahead (Science
//! 79 against 68 at t100, `deity-18/defence-sim`, 40 seeds a lane).
//!
//! **The gene.** While the active victory target is Science, in a city of
//! ours that is not the plan's threatened city, was not attacked within four
//! turns and, while a major is at war with us, stands away from the front:
//!
//! 1. The empire's first Campus goes ahead of the early floor's Settler once
//!    two cities stand, by `first-campus-before-the-settler`'s rule: no war
//!    with a major, no rush, no threatened city, no raider in the ring, a
//!    Campus of at most seven Online turns on the empire's best site. The
//!    Settler keeps its progress and the committed Campus keeps its queue
//!    against both Settler claims until a Campus stands. No later Campus
//!    passes a Settler.
//! 2. Any city's first Campus that `campus-claims-the-free-slot` (or `-2`)
//!    would claim but for a hold, and that finishes within
//!    [`SCIENCE_SEAT_CAMPUS_ONLINE_TURNS`] Online turns (the host's quote),
//!    goes ahead of a Builder (the backlog claims' included), a Granary, an
//!    Aqueduct, an Industrial Zone and a Commercial Hub or its Market. The
//!    displaced item keeps its progress, and the Builder and Granary writers
//!    then leave that Campus alone. A slower Campus waits for them.
//! 3. Both Plaza claims pass by a city that has no Campus standing, founded
//!    or queued, has a site for one, and has one specialty slot left
//!    ([`AdvancedAi::plaza_would_take_the_campus_slot`]). The Plaza then
//!    goes to the first city with a slot to spare: the capital once its
//!    Campus is placed and it holds four citizens, or any city of four
//!    citizens.
//!
//! **What keeps its queue.** Every soldier: unlike the Domination gene this
//! one never passes the near-rival deterrent, a siege reservation's gun or a
//! routine unit, at peace or at war, and the deterrent claim does not spare
//! a city for it. The faith defence's Holy Site and Shrine are not passed
//! either, and its writers are not held off. Walls, the capital's archers
//! and the garrison writers run after the claims and are not read here. So
//! do a wonder, a project, a Trader, any Settler but the one rule 1 names,
//! saved work within two turns of done under `near-done-work-finishes`, and
//! anything finishing this turn. The Library and the University keep
//! `campus-building-claims-the-queue`'s own rules. Off, nothing changes.
//!
//! **Why only a quick Campus, no Library and no later Settler (paired sim,
//! Science lane, Deity, 30 seeds from 67000, the live Science genome with
//! and without the gene; paired differences with their standard errors).**
//! The first build put every city's first Campus within the claim's
//! fifteen-turn budget, and its Library, ahead of those heads and, from four
//! cities, of a Settler. It stood the first three Campuses 4.9, 10.5 and 8.4
//! turns earlier and read Science +2.3 (0.6) at t50, and it cost the empire:
//! cities -0.37 (0.12) at t50, citizens -3.9 (1.2) and Production -9.9 (3.8)
//! at t100, Science -7.0 (4.2) at t125, cities -0.83 (0.39) at the end. On
//! the live boards the Campuses held behind a Granary were a median eleven
//! turns away, those behind a Builder nine and those behind the Settler
//! seven, and the Library was not late. With the seven-turn cap and no
//! Library but the Settler pass from four cities kept, the arm still read
//! cities -0.43 (0.30) and Production -4.9 (4.0) at t100. This rule reads
//! cities +0.03 (0.22), citizens +0.6 (1.0), Production +1.1 (4.0), Science
//! +4.0 (2.4) and techs +0.60 (0.26) at t100, with the first two Campuses 2.6
//! (0.9) and 4.0 (1.1) turns earlier; at t150, which 11 seeds reach, techs
//! -0.2 (0.6) and cities -0.5 (0.4). The simulated seat stands its first
//! Campus at a mean t49 without the gene, where the live Science seats'
//! median was t72, so the sim prices the rule's cost and little of its gain.
//! On the live frames of the five Science games (240 boards, each replayed
//! with and without the gene) it orders the first three Campuses a mean
//! 10.2, 18.8 and 7.4 turns before the live seat placed them.
//!
//! Grep the journal for `science-seat-campus-before-the-claims:`.

use super::*;

/// The journal tag of the gene's lines.
pub(crate) const SCIENCE_SEAT_CAMPUS_TAG: &str = "science-seat-campus-before-the-claims";

/// The longest first Campus, in Online turns, that goes ahead of the economy
/// claims on a Science seat. A slower one waits for the Granary and Builder.
pub(crate) const SCIENCE_SEAT_CAMPUS_ONLINE_TURNS: f64 = 7.0;

impl AdvancedAi {
    /// Whether the gene plays for this seat: it is on and the active victory
    /// target is Science. Always false with the gene off.
    pub(super) fn science_seat_campus_first(&self, g: &Game) -> bool {
        self.science_seat_campus_before_the_claims
            && self.active_victory_target(g) == Some(VictoryTarget::Science)
    }

    /// Whether `item`, a first Campus for `cid`, is quick enough for the gene:
    /// done within [`SCIENCE_SEAT_CAMPUS_ONLINE_TURNS`] Online turns
    /// (cost-scaled at other speeds), by the host's quote where it has one.
    pub(super) fn science_seat_campus_is_quick(
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> bool {
        let turns = g.host_production_turns(cid, item).unwrap_or_else(|| {
            let remaining =
                g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item);
            remaining.max(0.0) / g.city_yields(cid).production.max(0.5)
        });
        let cap = SCIENCE_SEAT_CAMPUS_ONLINE_TURNS * g.game_speed.cost_percent() as f64
            / crate::setup::GameSpeed::Online.cost_percent() as f64;
        turns.is_finite() && turns <= cap
    }

    /// Whether either gene that puts the first Campus and the Library ahead
    /// of the claims is on. Each still asks its own seat
    /// ([`Self::campus_and_library_reach`]).
    pub(super) fn campus_and_library_armed(&self) -> bool {
        self.campus_and_library_before_the_claims || self.science_seat_campus_before_the_claims
    }

    /// Whether a Government Plaza in `cid` would take the last specialty
    /// slot of a city of ours that has no Campus standing, founded or
    /// queued and has a site for one: both Plaza claims pass such a city by,
    /// so the slot stays the Campus's. A city with two slots free may still
    /// hold the Plaza. Always false with the gene off or off a Science seat.
    pub(super) fn plaza_would_take_the_campus_slot(&self, g: &Game, pid: usize, cid: u32) -> bool {
        if !self.science_seat_campus_first(g) {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.owner != pid {
            return false;
        }
        let founded = |specialty_only: bool, family: Option<&str>| {
            city.owned_tiles
                .iter()
                .filter_map(|pos| g.map.get(*pos)?.district_foundation.as_ref())
                .filter(|foundation| {
                    (!specialty_only
                        || g.rules
                            .districts
                            .get(&foundation.district)
                            .is_some_and(|spec| spec.specialty))
                        && family
                            .is_none_or(|family| g.district_family(foundation.district) == family)
                })
                .count()
        };
        let has_campus = g.city_has_district_family(city, crate::name!("campus"))
            || founded(false, Some("campus")) > 0
            || city.queue.iter().any(|item| {
                matches!(item, Item::District { district, .. }
                    if g.district_family(*district) == "campus")
            });
        if has_campus {
            return false;
        }
        let used = city
            .districts
            .keys()
            .filter(|name| {
                g.rules
                    .districts
                    .get(name)
                    .is_some_and(|spec| spec.specialty)
            })
            .count()
            + founded(true, None);
        g.city_specialty_district_capacity(city) <= used + 1
            && !g
                .district_sites(cid, BasicAi::civ_district(g, pid, "campus"))
                .is_empty()
    }

    /// The gene a first Campus or Library put first is journalled under on
    /// this seat.
    pub(super) fn campus_and_library_tag(&self, g: &Game) -> &'static str {
        if self.science_seat_campus_first(g) {
            SCIENCE_SEAT_CAMPUS_TAG
        } else {
            "campus-and-library-before-the-claims"
        }
    }
}

#[cfg(test)]
mod tests;
