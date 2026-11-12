//! `open-city-is-the-objective`: a war in hand takes the open city our
//! takers can take.
//!
//! ## Why (live Deity Domination, Gran Colombia, 45 games of 2026-11-12)
//!
//! Between turns 40 and 110 the seat was at war with a major that owned an
//! unwalled city within twelve tiles of one of ours, its defence within ten
//! of our best fielded melee, on 408 war-turns in 26 games (44 war
//! episodes). One of those episodes ended in the capture of the open city.
//! On 143 of the war-turns the land soldiers within eight tiles of the city
//! would take it within five turns by the combat formula (52 with melee
//! alone), and on 84 of those 143 the campaign was aimed elsewhere: a walled
//! city of the same owner, or a rival we were not at war with. On 105 of the
//! 143 the seat offered that enemy peace the same turn:
//! `unchosen-war-takes-the-first-peace` 45, "this is not the war the
//! recovery plan is fighting" 17, "one war at a time" 16, freeing the army
//! for a victory threat 9, the board's "no siege is feasible" 9, a rout 6,
//! outmatched 3. Fronts that did fight for an open city lost 7.9 soldiers
//! and 0.22 cities in the next 20 turns, against the day's base rate of 6.7
//! and 0.23. Live civvis-20261112T110026Z at war with Persia aimed at
//! Canada's Quebec City, a second front at peace, on turns 105-107 while
//! three takers stood within eight tiles of Hagmatana (no walls, defence
//! 54, our Line Infantry 65); Persia reached Steel and walled it at 108.
//!
//! ## What the gene does
//!
//! A city of a major at war with us, for any reason (a conquest, a faith
//! war, their declaration), is an open city our takers can take when its
//! walls are down and our land melee within [`OPEN_CITY_MARCH`] tiles and
//! [`OPEN_CITY_ROAD_STEPS`] land steps, marching at their own moves, would
//! bring it to nothing within [`OPEN_CITY_TURNS`] turns: blows by
//! `expected_damage`, each taker paying the centre's return blow and
//! stopping under [`OPEN_CITY_STRIKE_HP`], the city healing
//! `FIRST_CAPITAL_CITY_HEAL` a turn. It also needs the centre within
//! `OPEN_CITY_TIER_MARGIN` of our best taker, and the owner's soldiers in
//! sight within [`OPEN_CITY_DEFENDER_REACH`] tiles no stronger than the
//! takers. With the gene on, on the Domination lane or a Score seat (a
//! captured city is five score with its citizens, districts and buildings;
//! the gene reads only wars already in hand and declares none):
//!
//! - **The objective.** The best such city is the campaign's objective,
//!   ahead of a walled committed or ranked city or a rival at peace (a
//!   second front, a counter not yet declared), unless a home emergency, a
//!   city of ours falling, the capital that completes Domination, a war
//!   already running on another rival's urgent victory clock, or a city
//!   already being reduced holds the plan.
//! - **The front.** A faith war on its owner is a front
//!   (`faith-war-is-no-front` does not exempt it), and the one-war front
//!   turns to its owner while the current front offers no such city.
//! - **No peace.** Neither our peace offer nor their white peace is made
//!   while the capture stands, unless a city of ours is falling, our power
//!   is under [`OPEN_CITY_PEACE_POWER`] times theirs, or the one-war rule
//!   closes the war for a victory threat, a settled capital or a rout.
//! - **The go order.** The siege of such a city counted with the train's
//!   own members does not wait at the muster or the Stage for the bill or
//!   the damage budget, and is not aborted on them while the capture holds.
//!
//! Exact no-op with the gene off.

use super::first_capital_strikes_the_open_window::FIRST_CAPITAL_CITY_HEAL;
use super::open_city_front::{OPEN_CITY_MARCH, OPEN_CITY_ROAD_STEPS, OPEN_CITY_TIER_MARGIN};
use super::soft_city_window::SOFT_CITY_GARRISON_BONUS;
use super::{one_war, AdvancedAi, CITY_MAX_HP};
use crate::game::{effective_strength, expected_damage, Game};
use crate::think;

/// Turns, the march included, within which the takers must bring the city
/// to nothing. Live melee captures of open cities took one to four turns
/// once the takers stood in reach (the brief's five-turn bar).
pub(crate) const OPEN_CITY_TURNS: u32 = 5;

/// The health a land melee unit needs to count as a taker.
pub(crate) const OPEN_CITY_TAKER_HP: i32 = 50;

/// A taker under this health strikes no more in the reckoning.
pub(crate) const OPEN_CITY_STRIKE_HP: f64 = 35.0;

/// How far from the city the owner's soldiers in sight answer for it.
pub(crate) const OPEN_CITY_DEFENDER_REACH: i32 = 4;

/// The stock `outmatched` ratio: under this share of the owner's power the
/// peace desk's offer stands whatever the open city reads.
pub(crate) const OPEN_CITY_PEACE_POWER: f64 = 0.62;

/// An open city our takers can take, and how.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OpenCityTake {
    /// The owner, at war with us.
    pub(crate) rival: usize,
    /// The open city.
    pub(crate) city: u32,
    /// The takers counted.
    pub(crate) takers: usize,
    /// The turn the reckoning brings it to nothing.
    pub(crate) turns: u32,
    /// The centre as it will fight.
    pub(crate) defence: f64,
    /// Its health.
    pub(crate) health: i32,
}

/// Turns until `takers` take a centre of `defence` and `health`, each taker
/// given as (attack strength, health, first turn it can strike). Every
/// taker at [`OPEN_CITY_STRIKE_HP`] or more strikes once a turn from its
/// first turn and pays the centre's return blow; the centre heals
/// `FIRST_CAPITAL_CITY_HEAL` at the end of a turn it survives. `None` when it
/// stands after `horizon` turns.
pub(crate) fn open_city_take_turns(
    takers: &[(f64, i32, u32)],
    defence: f64,
    health: i32,
    horizon: u32,
) -> Option<u32> {
    let mut hp: Vec<f64> = takers.iter().map(|taker| f64::from(taker.1)).collect();
    let mut city = f64::from(health.max(0));
    if city <= 0.0 {
        return (!takers.is_empty()).then_some(1);
    }
    for turn in 1..=horizon {
        for (i, (strength, _, first)) in takers.iter().enumerate() {
            if turn < *first || hp[i] < OPEN_CITY_STRIKE_HP {
                continue;
            }
            let attack = effective_strength(*strength, hp[i].round() as i32);
            city -= expected_damage(attack, defence);
            hp[i] -= expected_damage(defence, attack);
            if city <= 0.0 {
                return Some(turn);
            }
        }
        city = (city + f64::from(FIRST_CAPITAL_CITY_HEAL)).min(f64::from(CITY_MAX_HP));
    }
    None
}

impl AdvancedAi {
    /// Whether the gene acts for this seat: on, and on the Domination lane or
    /// a Score seat ([`AdvancedAi::conquest_or_score_seat`]: a captured city
    /// is five score and its citizens, districts and buildings). It only
    /// reads wars already in hand.
    fn open_city_objective_on(&self, g: &Game) -> bool {
        self.open_city_is_the_objective && self.conquest_or_score_seat(g)
    }

    /// The capture reading of `cid` by our takers within the march, or by
    /// `members` alone when given. `None` unless it is an open city of a
    /// living major at war with us, not stood down or deferred, under the
    /// tier floor and the local check, that the takers bring to nothing
    /// within [`OPEN_CITY_TURNS`]. Not gated on the gene: callers are.
    pub(crate) fn open_city_take(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        members: Option<&[u32]>,
    ) -> Option<OpenCityTake> {
        let city = g.cities.get(&cid)?;
        let owner = g.players.get(city.owner)?;
        if city.owner == pid
            || !owner.alive
            || owner.is_minor
            || owner.is_barbarian
            || !g.is_at_war(pid, city.owner)
            || city.wall_hp > 0
            || self.capture_stood_down_holds(g, cid)
            || Self::should_defer_city_capture(g, pid, cid)
        {
            return None;
        }
        let steps = Self::open_city_land_steps(g, city.pos, OPEN_CITY_ROAD_STEPS);
        let guards = self.all_reserved_civilian_guards();
        let candidates: Vec<u32> = match members {
            Some(members) => members.to_vec(),
            None => g.player_unit_ids(pid),
        };
        let mut takers: Vec<(u32, f64, i32, u32)> = candidates
            .into_iter()
            .filter(|uid| !guards.contains(uid))
            .filter_map(|uid| g.units.get(&uid))
            .filter(|unit| {
                unit.owner == pid
                    && unit.hp >= OPEN_CITY_TAKER_HP
                    && !g.is_embarked(unit)
                    && Self::conquest_melee_body(&g.rules.units[unit.kind])
                    && g.wdist(unit.pos, city.pos) <= OPEN_CITY_MARCH
            })
            .filter_map(|unit| {
                let road = *steps.get(&unit.pos)?;
                let moves = g.unit_max_moves(unit.id).max(1.0);
                let first = (f64::from(road) / moves).ceil().max(1.0) as u32;
                Some((unit.id, g.unit_strength(unit, false), unit.hp, first))
            })
            .collect();
        if takers.is_empty() {
            return None;
        }
        takers.sort_by_key(|taker| (taker.3, taker.0));
        let best = takers.iter().map(|taker| taker.1).fold(f64::MIN, f64::max);
        let visible = self.battlefront_visibility(g, pid);
        let defenders: Vec<&crate::game::Unit> = g
            .units
            .values()
            .filter(|unit| unit.owner == city.owner)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .filter(|unit| g.wdist(unit.pos, city.pos) <= OPEN_CITY_DEFENDER_REACH)
            .filter(|unit| g.sees(&visible, unit.pos) && g.unit_visible_to(unit.id, pid))
            .collect();
        let garrison = defenders
            .iter()
            .filter(|unit| g.wdist(unit.pos, city.pos) <= 2)
            .map(|unit| g.unit_strength(unit, true) + SOFT_CITY_GARRISON_BONUS)
            .fold(0.0, f64::max);
        let defence = g.city_strength(cid).max(garrison);
        if defence > best + OPEN_CITY_TIER_MARGIN {
            return None;
        }
        let theirs: f64 = defenders
            .iter()
            .map(|unit| effective_strength(g.unit_strength(unit, true), unit.hp))
            .sum();
        let ids: Vec<u32> = takers.iter().map(|taker| taker.0).collect();
        if theirs > Self::campaign_strength_of(g, &ids) {
            return None;
        }
        let reckoned: Vec<(f64, i32, u32)> = takers
            .iter()
            .map(|taker| (taker.1, taker.2, taker.3))
            .collect();
        let turns = open_city_take_turns(&reckoned, defence, city.hp, OPEN_CITY_TURNS)?;
        Some(OpenCityTake {
            rival: city.owner,
            city: cid,
            takers: takers.len(),
            turns,
            defence,
            health: city.hp,
        })
    }

    /// The best open city of `rival` our takers can take: fewest turns, then
    /// the lowest id. `None` with the gene off or off the Domination lane.
    pub(crate) fn open_city_take_of(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<OpenCityTake> {
        if !self.open_city_objective_on(g) || !g.is_at_war(pid, rival) {
            return None;
        }
        g.player_city_ids(rival)
            .into_iter()
            .filter_map(|cid| self.open_city_take(g, pid, cid, None))
            .min_by_key(|take| (take.turns, take.city))
    }

    /// The best open city of any major at war with us. `None` with the gene
    /// off.
    pub(crate) fn open_city_take_best(&self, g: &Game, pid: usize) -> Option<OpenCityTake> {
        if !self.open_city_objective_on(g) {
            return None;
        }
        self.one_war_enemies(g, pid)
            .into_iter()
            .filter_map(|rival| self.open_city_take_of(g, pid, rival))
            .min_by_key(|take| (take.turns, take.city))
    }

    /// `assess`: the objective the gene names in place of `target_city`, or
    /// `None` to leave the plan as it stands. `held(owner)` is true when a
    /// home emergency, the capital that completes Domination or a running
    /// war on an urgent rival clock other than `owner` already holds the
    /// plan.
    pub(super) fn open_city_objective(
        &self,
        g: &Game,
        pid: usize,
        target_city: Option<u32>,
        held: impl Fn(usize) -> bool,
    ) -> Option<OpenCityTake> {
        if !self.open_city_objective_on(g) || Self::our_cities_falling(g, pid) {
            return None;
        }
        let take = self.open_city_take_best(g, pid)?;
        if held(take.rival) || target_city == Some(take.city) {
            return None;
        }
        // A city already being reduced finishes first.
        if let Some(current) = target_city.and_then(|cid| g.cities.get(&cid)) {
            let walls = g.city_max_wall_hp(current);
            if current.owner != pid
                && g.is_at_war(pid, current.owner)
                && (current.hp < CITY_MAX_HP || (current.wall_hp > 0 && current.wall_hp < walls))
            {
                return None;
            }
        }
        if let Some(city) = g.cities.get(&take.city) {
            let prior = target_city.and_then(|cid| g.cities.get(&cid)).map_or_else(
                || "no city objective".to_string(),
                |prior| {
                    if prior.owner == take.rival {
                        format!("{} behind {} walls", prior.name, prior.wall_hp.max(0))
                    } else {
                        format!("{}'s {}", g.players[prior.owner].civ, prior.name)
                    }
                },
            );
            think!(self.journal(), Strategy, Strategy,
                   "Campaign takes open {}", city.name;
                   "open-city-is-the-objective: at war with {}; {} has no walls and {} taker(s) within {} tiles take it in {} turn(s) against a centre of {:.0} at {} health; it outranks {prior}",
                   g.players[take.rival].civ, city.name, take.takers, OPEN_CITY_MARCH,
                   take.turns, take.defence, take.health;
                   city.pos);
        }
        Some(take)
    }

    /// `faith-war-is-no-front`: a war whose rival has an open city our takers
    /// can take is a front. `false` with the gene off.
    pub(crate) fn open_city_war_is_a_front(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.open_city_take_of(g, pid, rival).is_some()
    }

    /// `one_war_choose_front`: the enemy whose open city our takers can
    /// take, while `current` offers none. `None` with the gene off.
    pub(crate) fn open_city_front_swap(
        &self,
        g: &Game,
        pid: usize,
        current: Option<usize>,
        enemies: &[usize],
    ) -> Option<usize> {
        if !self.open_city_objective_on(g)
            || Self::our_cities_falling(g, pid)
            || current.is_some_and(|front| self.open_city_take_of(g, pid, front).is_some())
        {
            return None;
        }
        let take = enemies
            .iter()
            .filter_map(|rival| self.open_city_take_of(g, pid, *rival))
            .min_by_key(|take| (take.turns, take.city))?;
        if current != Some(take.rival) {
            if let Some(city) = g.cities.get(&take.city) {
                think!(self.journal(), Strategy, Strategy,
                       "Front turns to {}", g.players[take.rival].civ;
                       "open-city-is-the-objective: {} has no walls and {} taker(s) take it in {} turn(s); {}",
                       city.name, take.takers, take.turns,
                       current.map_or_else(
                           || "no front was chosen".to_string(),
                           |front| format!("{} offers no open city in reach", g.players[front].civ));
                       city.pos);
            }
        }
        Some(take.rival)
    }

    /// The peace desk: whether the war with `rival` is kept for its open
    /// city — the capture stands, no city of ours is falling and our power
    /// is at least [`OPEN_CITY_PEACE_POWER`] times theirs. `false` with the
    /// gene off.
    pub(crate) fn open_city_keeps_the_war(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.open_city_objective_on(g)
            && !Self::our_cities_falling(g, pid)
            && g.military_power(pid) >= OPEN_CITY_PEACE_POWER * g.military_power(rival)
            && self.open_city_take_of(g, pid, rival).is_some()
    }

    /// Whether a one-war peace reason still closes a war the gene keeps: a
    /// victory threat, a settled capital and a rout do.
    pub(crate) fn open_city_yields_to(peace: Option<one_war::OneWarPeace>) -> bool {
        matches!(
            peace,
            Some(
                one_war::OneWarPeace::VictoryThreat
                    | one_war::OneWarPeace::CapitalSecured
                    | one_war::OneWarPeace::CapitalElsewhere
                    | one_war::OneWarPeace::Rout
            )
        )
    }

    /// The journal line of a peace the gene withholds or refuses.
    pub(crate) fn journal_open_city_keeps_the_war(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        their_offer: bool,
    ) {
        if !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        let Some(take) = self.open_city_take_of(g, pid, rival) else {
            return;
        };
        let Some(city) = g.cities.get(&take.city) else {
            return;
        };
        let what = if their_offer {
            "their peace offer is refused"
        } else {
            "no peace is offered"
        };
        think!(self.journal(), Diplomacy, Decision,
               "Keeping the war on {}", g.players[rival].civ;
               "open-city-is-the-objective: {} has no walls and {} taker(s) take it in {} turn(s); {:.0} power against their {:.0} and no city of ours is falling, so {what}",
               city.name, take.takers, take.turns, g.military_power(pid), g.military_power(rival);
               city.pos);
    }

    /// The siege: the capture reading of `cid` counted with the train's own
    /// members. `None` with the gene off, off the lane, or while a city of
    /// ours is falling.
    pub(crate) fn open_city_siege_go(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        members: &[u32],
    ) -> Option<OpenCityTake> {
        if !self.open_city_objective_on(g) || Self::our_cities_falling(g, pid) {
            return None;
        }
        self.open_city_take(g, pid, cid, Some(members))
    }
}

#[cfg(test)]
mod tests;
