//! `own-flip-faith-war-asks-no-power`: the small-world faith war opened on
//! our own flip, the founder alone on its faith and half our cities leaning
//! to it, opens at any power.
//!
//! **Why (live Deity Online, 4-player Tiny Pangaea, Gran Colombia, no faith
//! founded; 62 games from 2026-11-11 19:57Z to 11-12 11:00Z).** Ten ended
//! on a rival Religious Victory. In eight of them our own majority flipped
//! before the faith held another major while we still held out: those
//! losses ran through the small-world own-flip reading (`faith_war_small_world`: only the founder
//! holds its faith, at least [`SMALL_WORLD_OWN_CITIES`] cities of ours, at
//! least [`SMALL_WORLD_OWN_LEANING`] and [`SMALL_WORLD_OWN_SHARE`]% of them
//! following or converting to it), 3 to 39 turns before our flip. Of the 43
//! games that entered that reading and were decided, 1 of the 14 that
//! opened a faith war on the founder before our flip lost on Religion,
//! against 7 of the 29 that did not; our power did not separate them
//! (median 0.40 times the founder's steady power in the losses, 0.42 in the
//! rest). The war was held on power: four of the 2026-11-12 losses read our
//! power at 0.21-0.37 times the founder's steady power when it was held
//! (civvis-20261112T091845Z Canada 0.21, 094511Z Russia 0.35, 103652Z Cree
//! 0.37, 104039Z Egypt 0.25), under the reading's own floor
//! [`SMALL_WORLD_OWN_POWER`] and version 2's 0.7. The wars the seat did
//! open under 0.7 times the founder's steady power (18 games) lost 0.22
//! cities on average in the next 30 turns, all 18 reached turn 100 and none
//! of the 17 decided ended on Religion; the 37 held under 0.7 times lost
//! 0.38 cities, 30 of 35 reached turn 100 and 6 of 30 ended on Religion.
//!
//! **What the gene does.** Riding `faith-war-on-a-small-world`, the own-flip
//! reading no longer asks [`SMALL_WORLD_OWN_POWER`]
//! (`faith_war_small_world_race`); its city thresholds are unchanged. When
//! that reading alone opens the war (`own_flip_asks_no_power`: the shipped
//! reading, with the floor, would not), the faith war opens without version
//! 2's power floor, `faith-war-needs-the-edge` or the rout ratio, and keeps
//! every other hold: the threatened-city and falling-city holds, the
//! major-war limit, the treasury reserve, the income gate, legality and
//! version 2's cooldown. When the host allows no declaration on the founder
//! yet, the seat denounces it so a Formal War follows (`faith_war_denounce`).
//! The opening, the holds and the keep are journalled under the gene's tag
//! with the power ratio it waived. The declaration desk's counter-war floor
//! (`counter_floor_holds`) does not hold a founder on that reading.
//!
//! **Loosened (live, 2026-11-12 after arming at 12:59Z).** Two Religious
//! defeats followed in the next 18 games, both held by this module's
//! thresholds or holds, and our majority followed the faith within about
//! ten turns. civvis-20261112T142558Z (Ethiopia, lost t111) held two
//! cities at t44, one leaning, a spreader at ours, under 0.7x ("needs 3
//! cities and 2 leaning"): both followed Orthodoxy by t50. 160320Z (Egypt,
//! lost t105) held one leaning city of three with a spreader at t33 on the
//! edge at parity (their steady 118 against our 112), and at t45 held the
//! own-flip war on "a city of ours is threatened": one barbarian unit near
//! Bogota. Over the day's 61 Gran Colombia Deity games, a founder's spreader
//! stood at our cities before any city of ours followed it 136 times. Holds
//! with the own-flip thresholds unmet preceded our flip within 10 turns in
//! 23 of 50 founder-episodes; holds at parity (their steady 0.7-1.1x ours)
//! in 4 of 10; threatened holds in 3 of 6, and every threatened hold that
//! day was barbarians. The 26 faith wars opened under 0.7x lost one city
//! between them in the next 20 turns (median 12 units against 6 in the 20
//! before), our cities on the faith fell in 16 of them, and none ended on
//! that founder's Religious Victory. So, under the gene:
//!
//! - the own-flip reading applies from [`OWN_FLIP_CITIES`] cities, and from
//!   [`OWN_FLIP_LEANING_WITH_A_SPREADER`] leaning city whatever its share
//!   while a spreader of the faith stands at our cities (a small city flips
//!   on one charge); the shipped thresholds stay for the shipped reading.
//!   Every parity hold of the day had a spreader at a leaning city of ours,
//!   so this covers them without a separate parity clause;
//! - a threatened city of ours holds the small-world war only when a
//!   visible soldier of a major at war with us stands within six tiles of
//!   our cities (`threat_is_a_majors_army`); barbarians and city-states do
//!   not. A peaceful neighbour's soldier never counted (`threatened_city`
//!   reads only those at war with us).
//!
//! - a faith war on record is kept (`own_flip_keeps`, read by
//!   `faith_war_keeps_the_war`) while any city of ours follows or is
//!   converting to the faith, or a spreader of it stood at our cities
//!   within the family's clear window, whatever else asks for peace; a city
//!   of ours falling or the one-war rule's rout still ends it, and with no
//!   city on the faith and no spreader for the window the family's exit
//!   does. Of the day's 60 faith wars that ended in peace, 39 ended with a
//!   city of ours following the faith, and in 20 of those half our cities
//!   followed it 20 turns later (7 of 21 without one). In 160812Z (Portugal,
//!   lost t108) the recovery plan's peace closed the war at t80, eleven
//!   turns after it opened, with one city of four following; its spreaders
//!   were back at our cities the next turn and five of six cities followed
//!   it by t88. The family's keep had lapsed on its own terms: no spreader
//!   at ours since t69, the five turns of its exit, and by the seat's
//!   reading no city converting.
//!
//! The major-war limit, the falling-city hold, the treasury and income
//! gates, legality and the cooldown are unchanged.
//!
//! Off: unchanged.

use super::faith_war_before_the_flip::{
    FAITH_WAR_CLEAR_TURNS, SMALL_WORLD_OWN_CITIES, SMALL_WORLD_OWN_LEANING, SMALL_WORLD_OWN_POWER,
    SMALL_WORLD_OWN_SHARE,
};
use super::*;

/// `own-flip-faith-war-asks-no-power`: our cities from which the gene's
/// own-flip reading applies (the shipped reading asks
/// [`SMALL_WORLD_OWN_CITIES`]).
pub(crate) const OWN_FLIP_CITIES: usize = 2;

/// `own-flip-faith-war-asks-no-power`: cities of ours leaning to the faith
/// that open the gene's reading whatever their share while a spreader of
/// it stands within six tiles of our cities.
pub(crate) const OWN_FLIP_LEANING_WITH_A_SPREADER: usize = 1;

impl AdvancedAi {
    /// `own-flip-faith-war-asks-no-power`: the gene's own-flip thresholds on
    /// `rival`'s `faith`: at least [`OWN_FLIP_CITIES`] cities of ours, and
    /// either the shipped share ([`SMALL_WORLD_OWN_LEANING`] and
    /// [`SMALL_WORLD_OWN_SHARE`]%) leaning to it, or
    /// [`OWN_FLIP_LEANING_WITH_A_SPREADER`] leaning while a spreader of it
    /// stands at our cities.
    pub(crate) fn own_flip_thresholds(
        g: &Game,
        pid: usize,
        rival: usize,
        faith: &str,
        cities: usize,
        leaning: usize,
    ) -> bool {
        cities >= OWN_FLIP_CITIES
            && leaning >= OWN_FLIP_LEANING_WITH_A_SPREADER
            && ((leaning >= SMALL_WORLD_OWN_LEANING
                && 100 * leaning >= SMALL_WORLD_OWN_SHARE * cities)
                || !super::religious_interception::visible_home_spreaders(g, pid, rival, faith)
                    .is_empty())
    }

    /// `own-flip-faith-war-asks-no-power`: whether a visible soldier of a
    /// living major at war with us stands within six tiles of a city of
    /// ours: the threat that still holds the small-world war under the
    /// gene. Barbarians and city-states do not count.
    pub(crate) fn threat_is_a_majors_army(&self, g: &Game, pid: usize) -> bool {
        let visible = g.player_vision_frame(pid);
        let cities: Vec<crate::Pos> = g
            .player_city_ids(pid)
            .iter()
            .filter_map(|cid| g.cities.get(cid))
            .map(|city| city.pos)
            .collect();
        g.units.values().any(|unit| {
            unit.owner != pid
                && g.players
                    .get(unit.owner)
                    .is_some_and(|owner| owner.alive && !owner.is_minor && !owner.is_barbarian)
                && g.is_at_war(pid, unit.owner)
                && g.rules.units[unit.kind].class == "military"
                && g.sees(&visible, unit.pos)
                && g.unit_visible_to(unit.id, pid)
                && cities.iter().any(|pos| g.wdist(*pos, unit.pos) <= 6)
        })
    }

    /// `own-flip-faith-war-asks-no-power`: what keeps the faith war on
    /// record with `rival` whatever else asks for peace (the recovery plan,
    /// a stalled war, one war at a time, its own offer): cities of ours
    /// following or converting to its faith, and the standard turns since a
    /// spreader of it last stood at our cities, while either a city leans
    /// or that is under [`FAITH_WAR_CLEAR_TURNS`]. `None` when the one-war
    /// rule reads the last window as a rout (a city of ours falling is the
    /// caller's exit), and always with the gene off.
    pub(crate) fn own_flip_keeps(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<(usize, u32)> {
        if !self.own_flip_faith_war_asks_no_power {
            return None;
        }
        let war = self
            .faith_war_before_the_flip_war
            .filter(|war| war.rival == rival)?;
        let faith = Self::faith_war_faith(g, rival)?;
        let leaning = Self::our_cities_on_the_faith(g, pid, faith);
        let since = g.turn.saturating_sub(war.spreader_seen);
        if leaning == 0 && since >= g.standard_duration(FAITH_WAR_CLEAR_TURNS).max(1) {
            return None;
        }
        if matches!(
            self.one_war_peace(g, pid, rival),
            Some(one_war::OneWarPeace::Rout)
        ) {
            return None;
        }
        Some((leaning, since))
    }

    /// `own-flip-faith-war-asks-no-power`: whether a threatened city of ours
    /// is waived on `rival`'s small-world war: the gene is on, the reading
    /// holds, a city of ours is threatened, and no major's army is the
    /// threat (`threat_is_a_majors_army`).
    pub(crate) fn own_flip_threat_waived(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.own_flip_faith_war_asks_no_power
            && self.faith_war_small_world(g, pid, rival).is_some()
            && self.threatened_city(g, pid).is_some()
            && !self.threat_is_a_majors_army(g, pid)
    }

    /// `own-flip-faith-war-asks-no-power`: whether `rival`'s small-world
    /// reading holds on our own flip whatever our power (the founder's faith
    /// at the own-flip thresholds). `false` with the gene off.
    pub(crate) fn own_flip_reading(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.own_flip_faith_war_asks_no_power
            && self
                .faith_war_small_world_race(g, pid, rival, true)
                .is_some_and(|(_, own_flip)| own_flip)
    }

    /// `own-flip-faith-war-asks-no-power`: our power as a share of `rival`'s
    /// steady power when the gene alone opens the war on it: the own-flip
    /// reading holds and the shipped reading, which asks
    /// [`SMALL_WORLD_OWN_POWER`], does not. `None` otherwise, and always
    /// with the gene off.
    pub(crate) fn own_flip_asks_no_power(&self, g: &Game, pid: usize, rival: usize) -> Option<f64> {
        if !self.own_flip_reading(g, pid, rival)
            || self
                .faith_war_small_world_race(g, pid, rival, false)
                .is_some()
        {
            return None;
        }
        Some(g.military_power(pid) / self.steady_rival_power(g, rival).max(1.0))
    }

    /// The journal's account of the waived power: `ours` against their
    /// steady power, as the ratio and the floors it is under.
    pub(crate) fn own_flip_power_waived(&self, g: &Game, pid: usize, rival: usize) -> String {
        let ours = g.military_power(pid);
        let steady = self.steady_rival_power(g, rival).max(1.0);
        let ratio = ours / steady;
        let under = if ratio < SMALL_WORLD_OWN_POWER {
            format!(", under the {SMALL_WORLD_OWN_POWER:.1}x own-flip floor")
        } else {
            String::new()
        };
        format!("ours {ours:.0} against their steady {steady:.0} ({ratio:.2}x){under}")
    }

    /// The reading the journal names: the founder alone on its faith, how
    /// many of our cities lean to it, and the thresholds that read it.
    pub(crate) fn own_flip_cities(g: &Game, pid: usize, rival: usize) -> String {
        let cities = g.player_city_ids(pid).len();
        let Some(faith) = Self::faith_war_faith(g, rival) else {
            return format!(
                "only its founder follows its faith and our {cities} cities lean to it"
            );
        };
        let leaning = Self::our_cities_on_the_faith(g, pid, faith);
        let shipped = cities >= SMALL_WORLD_OWN_CITIES
            && leaning >= SMALL_WORLD_OWN_LEANING
            && 100 * leaning >= SMALL_WORLD_OWN_SHARE * cities;
        let reading = if shipped {
            format!(
                "at least {SMALL_WORLD_OWN_LEANING} of {SMALL_WORLD_OWN_CITIES} and {SMALL_WORLD_OWN_SHARE}%"
            )
        } else {
            let spreaders =
                super::religious_interception::visible_home_spreaders(g, pid, rival, faith).len();
            format!(
                "from {OWN_FLIP_CITIES} cities, {spreaders} of its spreader(s) at ours; the shipped reading asks {SMALL_WORLD_OWN_LEANING} of {SMALL_WORLD_OWN_CITIES} and {SMALL_WORLD_OWN_SHARE}%"
            )
        };
        format!(
            "only its founder follows its faith and {leaning} of our {cities} cities lean to it ({reading}), so our flip is next"
        )
    }
}

#[cfg(test)]
mod tests;
