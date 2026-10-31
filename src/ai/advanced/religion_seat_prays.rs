//! `religion-seat-prays-for-the-prophet`: on a seat aiming at a Religious
//! victory that has not founded, an idle, unthreatened city with a standing
//! Holy Site runs Holy Site Prayers while our Prophet points are short of the
//! Prophet's price and a Prophet is still open.
//!
//! **Why (live 6p Immortal, 2026-10-31).** `religion-seat-founds-first` put
//! the Holy Site down early on its first live carrier, and the seat still lost
//! the race by two points.
//! - civvis-20261031T140402Z (Cyrus, Religion): Astrology at t7, the founding
//!   Holy Site started in the one-city capital at t11 and stood by t16, a
//!   decade ahead of the Religion seats before it. Prophet points ran at one
//!   a turn: the pantheon was Religious Settlements (Divine Spark had gone),
//!   and the capital's idle queue at t16 went to a Settler, an Archer and a
//!   Monument. The race Shrine started at t23-24, lost its queue to the early
//!   Settler floor at t26 and to the capital's Monument claim at t33, and
//!   stood at t35. The race closed at t40 with our seat on 28 of 30 points.
//! - The two seats before the gene went the same way at the same rate: 6 of
//!   30 points at t35 (103516Z) and 12 of 30 (120406Z), at two a turn each.
//! - Prayers is the fast price: 10 Prophet points on completion for 25
//!   Production (`data/projects.json`), about four turns of a 7-Production
//!   capital. The Shrine's +1 a turn costs 70 and pays 10 points only after
//!   twenty turns. Two Prayers from 140402Z's idle t16 queue would have put
//!   it near 29 points by t25.
//!
//! **What the gene does.** For a Religion-target seat with no religion, no
//! Prophet in hand and a Prophet still open (`prophet_race_open_for`), while
//! our Prophet points plus the current rate up to the race deadline
//! ([`PRAYER_DEADLINE_STANDARD`] at Immortal and Deity,
//! [`PRAYER_DEADLINE_STANDARD_LOW_TIER`] below) fall short of the Prophet's price, the idle,
//! unthreatened city that produces most among those with a standing Holy Site
//! starts Holy Site Prayers. It runs right after `religion-seat-founds-first`'s
//! claim, before the opening book's branch, and never takes a busy queue. A
//! seat already on pace (Divine Spark, a Shrine) does not pray at all; at one
//! point a turn from t16 it prays twice. The Shrine and everything after the
//! founding stay the existing path's.
//!
//! **Why the deadline (calibrated probe, 2026-10-31).** Praying until the
//! points covered the price founded by t30 in 14 of 20 seats against 0 of 11,
//! but held the capital off its Settlers: cities at t80 5 against 7. The
//! deadline keeps the Prayers to the race's actual shortfall.
//!
//! Off, or for a seat not aiming at Religion, nothing changes.

use super::*;

/// Standard turns by which the founding Prophet must be paid for at Immortal
/// and Deity: the live 6p Immortal race closed at t37-51 Online (about 55-77
/// Standard), so t36 Online.
pub(super) const PRAYER_DEADLINE_STANDARD: u32 = 54;

/// The same deadline below Immortal. Live 6p Emperor seats founded the fourth
/// religion at a median t65.5 Online (quartiles 56-83, n = 4) against t48.5 at
/// Immortal (n = 54), so t48 Online.
pub(super) const PRAYER_DEADLINE_STANDARD_LOW_TIER: u32 = 72;

impl AdvancedAi {
    /// The Prayer deadline on this board, in raw game turns.
    pub(super) fn prayer_deadline(g: &Game) -> u32 {
        let standard = if matches!(g.difficulty.as_str(), "immortal" | "deity") {
            PRAYER_DEADLINE_STANDARD
        } else {
            PRAYER_DEADLINE_STANDARD_LOW_TIER
        };
        g.standard_duration(standard)
    }

    fn holy_site_prayers() -> Item {
        Item::Project {
            project: crate::name!("holy_site_prayers"),
        }
    }

    /// The idle Holy Site city whose Prayers the race wants now. `None` with
    /// the gene off, for another target, once founded or a Prophet is in
    /// hand, with the race closed, once the points and the rate up to the
    /// deadline cover the price, or when no idle, unthreatened Holy Site city
    /// can pray.
    pub(super) fn religion_seat_prayer_city(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<u32> {
        if !self.religion_seat_prays_for_the_prophet
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
        {
            return None;
        }
        let player = &g.players[pid];
        if player.religion.is_some()
            || player.prophet_pending
            || !self.prophet_race_open_for(g, pid)
        {
            return None;
        }
        let points = player.gpp.get("prophet").copied().unwrap_or(0.0);
        let rate = g
            .great_person_points_per_turn(pid)
            .get("prophet")
            .copied()
            .unwrap_or(0.0);
        let left = Self::prayer_deadline(g).saturating_sub(g.turn);
        if points + rate * f64::from(left) >= g.gp_cost(pid, "prophet") {
            return None;
        }
        let prayers = Self::holy_site_prayers();
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                let city = &g.cities[cid];
                city.queue.is_empty()
                    && plan.threatened_city != Some(*cid)
                    && !(city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                    && g.city_has_district_family(city, crate::name!("holy_site"))
                    && g.can_produce(pid, *cid, &prayers)
            })
            .max_by(|a, b| {
                g.city_yields(*a)
                    .production
                    .total_cmp(&g.city_yields(*b).production)
                    .then_with(|| b.cmp(a))
            })
    }

    /// See the module. Exact no-op with the gene off or for another target.
    pub(super) fn claim_religion_seat_prayers(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some(cid) = self.religion_seat_prayer_city(g, pid, plan) else {
            return;
        };
        let name = g.cities[&cid].name.clone();
        let points = g.players[pid].gpp.get("prophet").copied().unwrap_or(0.0);
        let cost = g.gp_cost(pid, "prophet");
        let deadline = Self::prayer_deadline(g);
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: Self::holy_site_prayers(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Faith, Decision,
                   "{name} prays for the Prophet";
                   "religion-seat-prays-for-the-prophet: {points:.0} of {cost:.0} Prophet points, \
                    short of the price by the t{deadline} deadline; {} of {} religions founded, \
                    an idle queue",
                   g.religions_founded(), g.max_religions());
        }
    }
}

#[cfg(test)]
mod tests;
