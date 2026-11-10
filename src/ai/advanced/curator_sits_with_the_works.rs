//! `curator-sits-with-the-works`: a Culture seat's Pingala takes Curator once
//! the empire holds [`CURATOR_MIN_WORKS`] Writing, Art or Music works, and a
//! Pingala holding Curator sits in the city that holds the most of them.
//!
//! **Why (live Deity Kristina, 2026-11-10).** Curator doubles the Tourism of
//! the Great Works in Pingala's own city (`great_work_tourism_pct` 100), and
//! the seat put him where the works were not:
//! - 144009Z: Curator from t145, Pingala in Göteborg with 0 works at t150 and
//!   t175, while Västerås held a full Art Museum (3) and Stockholm and Uppsala
//!   3 each.
//! - 152217Z: Curator from about t93, Pingala in Göteborg with 0 works at
//!   t125 and t150, while Stockholm held 5 (a full Art Museum among them).
//! `relocate_governors_for_dividends` never moves a Pingala who holds Curator
//! (and does not run at war or under the Recovery plan at all), and under
//! the Recovery plan, which this seat plays from about t150, Pingala's
//! promotion order puts Space Initiative before Curator.
//!
//! **What the gene does**, for a seat whose active victory target is Culture:
//! - **Promotions.** Nothing changes before the empire holds
//!   [`CURATOR_MIN_WORKS`] Writing, Art or Music works: which governor is
//!   appointed, where, and every promotion keep the stock order. From then
//!   on Curator is Pingala's next promotion whenever it is open, and with a
//!   title in hand it is taken before any other title is spent.
//! - **The post.** Every turn, at war or at peace, a Pingala holding Curator
//!   moves to the city of ours that holds the most Writing, Art and Music
//!   works, when that city holds at least [`CURATOR_MARGIN_WORKS`] more than
//!   his own and at least half as many again. A Pingala in a city with works
//!   stays at least [`CURATOR_MIN_STAY_STANDARD`] standard turns (and never
//!   less than the establishment time) before he moves again; one in a city
//!   with none moves as soon as he may. The new city must be safe and
//!   unoccupied by another governor; the host's establishment delay and
//!   Neutralize are respected. Cities are read from the board every turn, so
//!   no live city id is kept between frames.
//!
//! Off, or for another target, nothing changes.

use super::*;

/// Writing, Art and Music works the empire holds before Pingala takes Curator.
pub(super) const CURATOR_MIN_WORKS: usize = 4;
/// Works a city must hold beyond Pingala's own before he moves there.
pub(super) const CURATOR_MARGIN_WORKS: usize = 2;
/// Standard turns a Pingala sitting with works stays before he moves again.
pub(super) const CURATOR_MIN_STAY_STANDARD: u32 = 10;

/// The kinds of work Curator doubles in Pingala's city.
const CURATED_KINDS: [&str; 3] = ["writing", "art", "music"];

impl AdvancedAi {
    /// Whether the gene acts for this seat now: on, an active Culture target,
    /// and the lane still open.
    pub(super) fn curator_seat_active(&self, g: &Game) -> bool {
        self.curator_sits_with_the_works
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
    }

    /// Writing, Art and Music works housed in each of `pid`'s cities, where
    /// the host placed them on a mirrored seat.
    pub(super) fn curated_works_by_city(g: &Game, pid: usize) -> BTreeMap<u32, usize> {
        g.housed_great_works(pid)
            .iter()
            .filter(|(city, _)| g.cities.get(*city).is_some_and(|c| c.owner == pid))
            .map(|(city, kinds)| {
                let works = kinds
                    .iter()
                    .filter(|(kind, _)| CURATED_KINDS.contains(&kind.as_str()))
                    .map(|(_, count)| *count)
                    .sum::<usize>();
                (*city, works)
            })
            .collect()
    }

    /// Whether the empire holds the works that make Curator Pingala's next
    /// promotion. False with the gene off or for another target.
    pub(super) fn curator_comes_next(&self, g: &Game, pid: usize) -> bool {
        self.curator_seat_active(g)
            && Self::curated_works_by_city(g, pid).values().sum::<usize>() >= CURATOR_MIN_WORKS
    }

    /// See the module: with a title in hand, Pingala takes Curator first once
    /// the works stand. True when the promotion was applied.
    pub(super) fn promote_the_curator(&self, g: &mut Game, pid: usize) -> bool {
        if g.governor_titles_available(pid) == 0
            || !self.curator_comes_next(g, pid)
            || !g
                .available_governor_promotions(pid, "pingala")
                .iter()
                .any(|promotion| *promotion == "curator")
        {
            return false;
        }
        let applied = g
            .apply(
                pid,
                &Action::PromoteGovernor {
                    governor: crate::name!("pingala"),
                    promotion: crate::name!("curator"),
                },
            )
            .is_ok();
        if applied {
            think!(self.journal(), Government, Decision,
                   "Pingala takes Curator";
                   "curator-sits-with-the-works: {} Writing, Art and Music works stand",
                   Self::curated_works_by_city(g, pid).values().sum::<usize>());
        }
        applied
    }

    /// The city a Pingala holding Curator should move to now, with its works
    /// and his own city's. `None` with the gene off, without Curator, inside
    /// the stay or the host's delays, or when no city clears the margin.
    pub(super) fn curator_destination(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, usize, usize)> {
        if !self.curator_seat_active(g) {
            return None;
        }
        let state = g.players[pid].governor_roster.get("pingala")?;
        let current = state.city?;
        if !state.promotions.contains("curator")
            || state.disabled_until > g.turn
            || self.governor_requested_this_turn(g, "pingala")
            || !self.base.capital_governor_may_depart(g, pid, "pingala")
        {
            return None;
        }
        let works = Self::curated_works_by_city(g, pid);
        let here = g
            .cities
            .get(&current)
            .filter(|city| city.owner == pid)
            .map_or(0, |_| works.get(&current).copied().unwrap_or(0));
        let settle = g
            .standard_duration(g.rules.governors["pingala"].establish_turns)
            .max(1);
        let stay = g.standard_duration(CURATOR_MIN_STAY_STANDARD).max(settle);
        if here > 0 && self.governor_residence_age(g, pid, "pingala") < stay {
            return None;
        }
        let occupied: BTreeSet<u32> = g.players[pid]
            .governor_roster
            .values()
            .filter_map(|other| other.city)
            .collect();
        works
            .iter()
            .filter(|(city, count)| {
                **city != current
                    && !occupied.contains(*city)
                    && **count >= here + CURATOR_MARGIN_WORKS
                    && **count * 2 >= here * 3
                    && self.governor_dividend_safe_city(g, pid, **city, plan)
            })
            .max_by_key(|(city, count)| (**count, std::cmp::Reverse(g.cities[*city].pos)))
            .map(|(city, count)| (*city, *count, here))
    }

    /// See the module: move a Pingala holding Curator to the works. True when
    /// a reassignment was requested. Exact no-op with the gene off.
    pub(super) fn curator_follows_the_works(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some((city, works, here)) = self.curator_destination(g, pid, plan) else {
            return false;
        };
        if g.apply(
            pid,
            &Action::ReassignGovernor {
                governor: crate::name!("pingala"),
                city,
            },
        )
        .is_err()
        {
            return false;
        }
        self.note_governor_request(g, "pingala");
        think!(self.journal(), Government, Decision,
               "Moving Pingala to {} for its Great Works", g.cities[&city].name;
               "curator-sits-with-the-works: Curator doubles {works} Writing, Art and Music \
                work(s) there against {here} where he sat");
        true
    }
}

#[cfg(test)]
mod tests;
