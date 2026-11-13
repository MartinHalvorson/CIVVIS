//! `first-campus-before-the-settler`: a Domination seat's first Campus goes
//! ahead of the early Settler floor's Settler once two cities stand.
//!
//! **Why (live Deity Gran Colombia, 4-player Tiny Pangaea, Online, the 38
//! games of 2026-11-11 19:57Z to 2026-11-12 02:00Z).** Our Science was 11.4
//! at t50 against the best rival's 71.5. Writing landed at a median t32, but
//! the first Campus was queued at a median t46 and finished at t55, with 3
//! cities and a capital of 5 citizens. Over the 676 turns between Writing and
//! the first Campus queued (546 of them at peace), `campus-claims-the-free-slot`
//! journalled what held its quickest Campus. At peace the largest single
//! cause was the early Settler floor: 139 turns, against 121 for near-rival
//! deterrence and 60 for the bound Granary. Another 123 peace turns had no
//! Campus to claim because the Prophet race kept the site city's slot. In
//! 136 of the 142 floor-held turns two or more cities already stood, and the
//! held Campus was a median 5 turns away (4 to 7 for 80% of them).
//!
//! **The gene.** While the active victory target is Domination or Score
//! ([`AdvancedAi::conquest_or_score_seat`]: the score counts the techs a
//! Campus brings as much as a city), the empire
//! holds no Campus (standing, a foundation, or queued), at least
//! [`FIRST_CAMPUS_BEFORE_THE_SETTLER_MIN_CITIES`] cities stand and no major is
//! at war with us, the free-slot claim's Campus goes ahead of the floor's
//! Settler in that city. That holds only when the Campus finishes within
//! [`FIRST_CAMPUS_BEFORE_THE_SETTLER_ONLINE_TURNS`] Online turns
//! (cost-scaled at other speeds) and its site yields at least as much as
//! every other city's first Campus. The Settler keeps its progress. The
//! committed Campus then keeps its queue against both Settler claims (the
//! early floor and `holdable-land-keeps-settling`) until a Campus stands,
//! so the floor trains its next Settler in another city or after the
//! Campus. A rush, a threatened or freshly attacked city, a raider in the
//! ring and any war with a major keep the old priority. It rides
//! `campus-claims-the-free-slot` (or `-2`): with those off, nothing runs.
//!
//! **What it costs in cities.** One Settler, once a game, is delayed by at
//! most the Campus's own build, 7 Online turns and a median 5. These games
//! founded 0.068 cities a turn over t45-60 (median 3 cities at t45 and 4 at
//! t60), so a 5-turn delay costs about 0.34 of a city at t60 and the 7-turn
//! cap about 0.48, less when another city trains the Settler meanwhile.
//!
//! Grep the journal for `first-campus-before-the-settler:`.

use super::*;

/// The founded cities that must stand before the first Campus may go ahead
/// of the floor's Settler: the capital has founded one city already.
pub(crate) const FIRST_CAMPUS_BEFORE_THE_SETTLER_MIN_CITIES: usize = 2;

/// The longest first Campus, in Online turns, that may go ahead of the
/// Settler. It bounds the Settler's delay; see the module.
pub(crate) const FIRST_CAMPUS_BEFORE_THE_SETTLER_ONLINE_TURNS: f64 = 7.0;

impl AdvancedAi {
    /// [`FIRST_CAMPUS_BEFORE_THE_SETTLER_ONLINE_TURNS`] at this game's speed:
    /// production quotes scale with the speed's costs.
    pub(super) fn first_campus_before_the_settler_turns(g: &Game) -> f64 {
        FIRST_CAMPUS_BEFORE_THE_SETTLER_ONLINE_TURNS * g.game_speed.cost_percent() as f64
            / crate::setup::GameSpeed::Online.cost_percent() as f64
    }

    /// The gene is on, the seat pursues Domination, two cities stand, there
    /// is no rush, threatened city or war with a major, and `cid` is ours,
    /// not freshly attacked and clear of raiders.
    fn first_campus_before_the_settler_safe(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        self.first_campus_before_the_settler
            // A Score seat too: see [`AdvancedAi::conquest_or_score_seat`].
            && self.conquest_or_score_seat(g)
            && g.player_city_ids(pid).len() >= FIRST_CAMPUS_BEFORE_THE_SETTLER_MIN_CITIES
            && !plan.rush
            && plan.threatened_city.is_none()
            && g.cities.get(&cid).is_some_and(|city| {
                city.owner == pid
                    && (city.last_attacked == 0 || g.turn.saturating_sub(city.last_attacked) > 4)
                    && !Self::early_settler_raider_near(g, city.pos)
            })
            && !g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
    }

    /// Whether any city of `pid` holds a Campus standing, as a foundation, or
    /// anywhere in its queue.
    pub(super) fn empire_has_a_campus(g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid).into_iter().any(|cid| {
            let city = &g.cities[&cid];
            g.city_has_district_family(city, crate::name!("campus"))
                || city.queue.iter().any(|item| {
                    matches!(item, Item::District { district, .. }
                        if g.district_family(*district) == "campus")
                })
                || city.owned_tiles.iter().any(|pos| {
                    g.map
                        .get(*pos)
                        .and_then(|tile| tile.district_foundation.as_ref())
                        .is_some_and(|foundation| {
                            g.district_family(foundation.district) == "campus"
                        })
                })
        })
    }

    /// What a first Campus site yields, the measure the site picker ranks by.
    fn first_campus_site_yield(g: &Game, campus: &Item) -> Option<f64> {
        match campus {
            Item::District { district, pos } => {
                Some(g.district_site_yields(*district, *pos).total())
            }
            _ => None,
        }
    }

    /// Whether `campus` in `cid` sits on a site at least as good as every
    /// other city's first Campus the free-slot claim would pick now.
    fn first_campus_site_is_the_best(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        campus: &Item,
    ) -> bool {
        let Some(here) = Self::first_campus_site_yield(g, campus) else {
            return false;
        };
        g.player_city_ids(pid)
            .into_iter()
            .filter(|other| *other != cid)
            .all(|other| {
                self.free_slot_campus_item(g, pid, other, plan)
                    .and_then(|(item, _)| Self::first_campus_site_yield(g, &item))
                    .is_none_or(|there| there <= here + 1e-9)
            })
    }

    /// `first-campus-before-the-settler`: whether the empire's first
    /// `campus` in `cid`, `turns` from done, goes ahead of the floor's
    /// Settler at the head of its queue. Always false with the gene off.
    pub(super) fn first_campus_precedes_the_settler(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        campus: &Item,
        turns: f64,
    ) -> bool {
        self.first_campus_before_the_settler_safe(g, pid, cid, plan)
            && matches!(campus, Item::District { district, .. }
                if g.district_family(*district) == "campus")
            && matches!(g.cities[&cid].queue.first(), Some(Item::Unit { unit }) if *unit == "settler")
            && turns.is_finite()
            && turns > 0.0
            && turns <= Self::first_campus_before_the_settler_turns(g)
            && !Self::empire_has_a_campus(g, pid)
            && self.first_campus_site_is_the_best(g, pid, cid, plan, campus)
    }

    /// Whether `cid`'s queue head is the first Campus this gene put ahead of
    /// the Settler and should keep its queue against the Settler claims: no
    /// other city has a Campus standing yet. Always false with the gene off.
    pub(super) fn first_campus_holds_the_settler(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        self.first_campus_before_the_settler_safe(g, pid, cid, plan)
            && !g.player_city_ids(pid).into_iter().any(|other| {
                other != cid
                    && g.city_has_district_family(&g.cities[&other], crate::name!("campus"))
            })
            && g.cities[&cid]
                .queue
                .first()
                .is_some_and(|item| self.free_slot_campus_holds(g, pid, cid, plan, item))
    }

    /// The journal line for a claim this gene released.
    pub(super) fn explain_first_campus_before_the_settler(&self, g: &Game, cid: u32, turns: f64) {
        think!(self.journal(), Cities, Decision,
               "{} starts the first Campus before its Settler", g.cities[&cid].name;
               "first-campus-before-the-settler: {turns:.1} turns with {} cities standing and no war; \
                the Settler keeps its progress and the floor trains the next one elsewhere or after it",
               g.player_city_ids(g.cities[&cid].owner).len());
    }
}

#[cfg(test)]
mod tests;
