//! `holdout-keeps-the-faith-war`: no peace with a faith's founder while we
//! are that faith's last holdout and it is already taking our cities.
//!
//! A Religious Victory needs every living major's majority. When the founder's
//! faith holds every other major and we are the one still out, the war on the
//! founder is the only thing slowing its spreaders: our units condemn the
//! Missionaries and Apostles that enter our borders, and the founder sends its
//! spreaders to the front instead of past it. Across the religious losses of
//! 2026-10-13/14 the racer's spreaders reached our cities at 0.26 a turn at
//! war against 1.47 at peace.
//!
//! - Live Emperor 110038Z: at war with the Khmer (Buddhism, already holding
//!   Mali and Nubia, and 3 of our 7 cities) for 33 turns; at turn 114 we
//!   offered "the last window was a rout: 274 power against their 390", the
//!   Khmer took it, and Buddhism won at 116.
//! - Live Immortal 193955Z: peace with Kongo at 110, the religious loss at 120.
//!
//! The gene withholds every peace offer to, and refuses every peace offer
//! from, such a founder, whatever the reason the shipped clauses give. The
//! exit a besieged empire still needs stays: once a city of ours is falling
//! (`our_cities_falling`), peace is allowed again. A founder whose faith holds
//! none of our cities is spared: four match-point peace offers of the same
//! fortnight that the holdout survived had no city of ours on the racer's
//! faith.
//!
//! "Taking our cities" is a city of ours whose majority follows the faith, or
//! one it is converting: the faith at [`CONVERTING_SHARE`] of the city's
//! strongest pressure, the same line `home_conversion_threat` reacts to (on a
//! mirrored board, the host's next faith within its conversion clock).
//!
//! - `holdout-keeps-the-faith-war-2`: version two also keeps the war at match
//!   point whatever our cities' current conversion: while we are the
//!   founder's last holdout and its race reads [`MATCH_POINT_RACE`] or more,
//!   no peace with it is offered or accepted. Live Emperor 034356Z: we
//!   founded Buddhism and held 6 of 6 cities on it from turn 70 to 130 while
//!   Sweden's Protestantism held Macedon and Norway ("religion race reads
//!   75%" at 101 and 126). We were at war with Sweden from 40, offered peace
//!   from 116 ("no siege feasible … urgent Defend row at Quito unserved"),
//!   and peace closed the war at 126. Version one let it through because no
//!   city of ours was converting. Sweden's Apostles then flipped Caracas at
//!   131, Cuenca and Guayaquil at 132 and Quito at 134, and Sweden won on
//!   Religion at 134: a founder's cities fall in three to five turns once its
//!   spreaders walk in at peace. The exits: a city of ours actually falling
//!   (hit within two turns, or under fire on the board, at half health or
//!   less) or our capital under siege (hit within two turns or under fire),
//!   the war that threatens elimination. Short of match point, version two
//!   is version one.
//!
//! Off: unchanged.

use super::*;

/// A faith at this share of a city's strongest pressure is converting it.
const CONVERTING_SHARE: f64 = 0.6;
/// `holdout-keeps-the-faith-war-2`: the founder's race reading (living
/// majors following its faith, as a percentage) at which its last holdout
/// keeps the war whatever its own cities' conversion.
pub(crate) const MATCH_POINT_RACE: usize = 75;
/// A city at this health or less (of 200) that is under fire is falling.
const FALLING_CITY_HP: i32 = 100;
/// A city hit within this many turns is under attack.
const RECENTLY_HIT_TURNS: u32 = 2;

impl AdvancedAi {
    /// Our cities following `faith` or being converted to it.
    pub(super) fn our_cities_on_the_faith(g: &Game, pid: usize, faith: &str) -> usize {
        g.player_city_ids(pid)
            .iter()
            .filter_map(|cid| g.cities.get(cid))
            .filter(|city| {
                if g.city_religion(city) == Some(faith) {
                    return true;
                }
                let top = city.pressure.values().fold(0.0f64, |a, b| a.max(*b));
                top > 0.0
                    && city
                        .pressure
                        .get(faith)
                        .is_some_and(|pressure| *pressure + 1e-9 >= top * CONVERTING_SHARE)
            })
            .count()
    }

    /// `holdout-keeps-the-faith-war`: whether the war with `rival` is kept —
    /// we are its founded faith's last holdout, a city of ours follows or is
    /// converting to that faith, and no city of ours is falling. `false` with
    /// the gene off.
    ///
    /// Version two (`holdout-keeps-the-faith-war-2`) also keeps it at match
    /// point; see [`Self::holdout_match_point`].
    pub(crate) fn holdout_keeps_the_faith_war_with(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        let Some(faith) = self.holdout_faith(g, pid, rival) else {
            return false;
        };
        self.holdout_match_point(g, pid, rival).is_some()
            || (Self::our_cities_on_the_faith(g, pid, faith) > 0
                && !Self::our_cities_falling(g, pid))
    }

    /// `rival`'s founded faith, when either version is on, a Religious
    /// Victory is in play, `rival` is a living major and we are that faith's
    /// last holdout.
    fn holdout_faith<'g>(&self, g: &'g Game, pid: usize, rival: usize) -> Option<&'g str> {
        if !(self.holdout_keeps_the_faith_war || self.holdout_keeps_the_faith_war_2)
            || !g.victory_conditions.religious
        {
            return None;
        }
        let player = g.players.get(rival)?;
        if !player.alive || player.is_minor || player.is_barbarian {
            return None;
        }
        let faith = player.religion.as_deref()?;
        Self::we_are_the_last_holdout(g, pid, rival).then_some(faith)
    }

    /// `holdout-keeps-the-faith-war-2`: the founder's race reading when its
    /// last holdout keeps the war at match point — the reading is at least
    /// [`MATCH_POINT_RACE`], no city of ours is actually falling and our
    /// capital is not under siege. `None` with the version off.
    pub(crate) fn holdout_match_point(&self, g: &Game, pid: usize, rival: usize) -> Option<usize> {
        if !self.holdout_keeps_the_faith_war_2 {
            return None;
        }
        self.holdout_faith(g, pid, rival)?;
        let (converted, living) = self.religious_conversion_tally(g, rival);
        let race = 100 * converted / living.max(1);
        (race >= MATCH_POINT_RACE
            && !self.our_city_actually_falling(g, pid)
            && !self.our_capital_besieged(g, pid))
        .then_some(race)
    }

    /// A city of ours hit within [`RECENTLY_HIT_TURNS`] turns, or under fire
    /// on the board (`city-under-fire-ranks-first`), at [`FALLING_CITY_HP`]
    /// or less.
    fn our_city_actually_falling(&self, g: &Game, pid: usize) -> bool {
        if Self::our_cities_falling(g, pid) {
            return true;
        }
        let visible = g.player_vision_frame(pid);
        g.player_city_ids(pid).into_iter().any(|cid| {
            g.cities
                .get(&cid)
                .is_some_and(|city| city.hp <= FALLING_CITY_HP)
                && self.city_under_fire(g, pid, cid, &visible)
        })
    }

    /// Our capital hit within [`RECENTLY_HIT_TURNS`] turns, or under fire on
    /// the board: the war that threatens elimination.
    fn our_capital_besieged(&self, g: &Game, pid: usize) -> bool {
        let visible = g.player_vision_frame(pid);
        g.player_city_ids(pid).into_iter().any(|cid| {
            g.cities.get(&cid).is_some_and(|city| {
                city.is_capital
                    && ((city.last_attacked > 0
                        && g.turn.saturating_sub(city.last_attacked) <= RECENTLY_HIT_TURNS)
                        || self.city_under_fire(g, pid, cid, &visible))
            })
        })
    }

    /// The journal line for a withheld offer or a refused one.
    pub(crate) fn journal_holdout_keeps_the_faith_war(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        refused: bool,
    ) {
        if !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        if let Some(race) = self.holdout_match_point(g, pid, rival) {
            think!(self.journal(), Diplomacy, Decision,
                   "Refusing peace with {}", g.players[rival].civ;
                   "holdout-keeps-the-faith-war-2: last holdout at match point (race {race}%); its apostles convert a founder's cities in 3-5 turns");
            return;
        }
        let faith = g.players[rival].religion.clone().unwrap_or_default();
        let on_it = Self::our_cities_on_the_faith(g, pid, &faith);
        let cities = g.player_city_ids(pid).len();
        let what = if refused {
            "Refusing peace with"
        } else {
            "Keeping the war with"
        };
        think!(self.journal(), Diplomacy, Decision,
               "{what} {}", g.players[rival].civ;
               "holdout-keeps-the-faith-war: we are {}'s last holdout and {on_it} of our {cities} cities follow it or are converting; at war our soldiers condemn its spreaders",
               faith);
    }
}

#[cfg(test)]
mod tests;
