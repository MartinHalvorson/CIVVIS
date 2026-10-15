//! `open-city-draws-the-front`: a siege held on a walled city turns to an
//! open city of the same owner the field army can reach, not only one a
//! taker already stands beside.
//!
//! The campaign's one narrow change of objective, the open foothold
//! (`AdvancedAi::capture_opportunity_city`), wants a Conquest plan, a prior
//! objective at full walls, and a healthy land taker already within three
//! tiles of the open city. Over 102 Emperor games of 10-13/14 the siege sat
//! on a walled (or "inf turns") objective while the same owner held an
//! unwalled or breached city within twelve tiles of the force, reachable by
//! land, on 1,019 siege-turns in 60 games; that open city was never named in
//! 165 of 289 episodes. The three-tile taker clause failed on 82% of those
//! turns (the only failing clause on 358), the Conquest clause on 44%
//! (Recovery 304, Expansion 147), the untouched-prior clause on 19%. Live
//! Emperor 235353Z held Solikamsk and then St. Petersburg while Tver stood
//! unwalled at one tile from the army's centre for 33 siege-turns
//! (turns 75-118), with Russia's army at 0-3; no healthy melee stood within
//! three tiles of Tver on any of them.
//!
//! With the gene on, while the siege commitment is already on that owner:
//!
//! - a healthy land melee of the FIELD ARMY within [`OPEN_CITY_MARCH`] tiles
//!   and a land road of [`OPEN_CITY_ROAD_STEPS`] steps counts as the taker,
//!   since it arrives with the army;
//! - a Recovery or Expansion plan may turn too, not only Conquest;
//! - a prior objective whose walls are lightly chipped (at least three
//!   quarters standing) still yields.
//!
//! Guards: an open city whose defence exceeds our best field melee by more
//! than [`OPEN_CITY_TIER_MARGIN`] is not worth the march (live melee blows on
//! cities at a defence margin of 15-20 dealt a median 11 and took 56, under
//! the city's heal); once the front turns, the open city holds as the
//! objective until it is walled or captured, and is given up (stood down)
//! after [`OPEN_CITY_STALL_TURNS`] turns without a new low of health; a city
//! the front turned away from is not turned back to for
//! [`OPEN_CITY_COOLDOWN_TURNS`].

use super::{AdvancedAi, CITY_MAX_HP};
use crate::game::{City, Game};
use crate::think;
use crate::Pos;
use std::collections::{BTreeMap, VecDeque};

/// Tiles from the open city a field-army taker may stand.
pub(super) const OPEN_CITY_MARCH: i32 = 8;
/// Land steps (units and borders ignored: the owner is at war with us) a
/// field-army taker may need to reach the open city.
pub(super) const OPEN_CITY_ROAD_STEPS: i32 = 10;
/// How far an open city's defence may stand above our best field melee.
/// Calibrated on 206 live melee blows on cities (10-13/14): a defence margin
/// of 0-10 dealt a median 21-24 for 38 taken, 10-15 dealt 15 for 52, 15-20
/// dealt 11 for 56, 20-30 dealt 11-14 for 74-95.
pub(super) const OPEN_CITY_TIER_MARGIN: f64 = 15.0;
/// Standard turns an open city may stand at the front without a new low of
/// health before the front gives it up.
pub(super) const OPEN_CITY_STALL_TURNS: u32 = 8;
/// Standard turns a city the front turned away from (or gave up) stays out
/// of the open-city turn.
pub(super) const OPEN_CITY_COOLDOWN_TURNS: u32 = 20;

/// The open city the front turned to, and its progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OpenCityHold {
    /// The open city the front holds.
    pub(crate) city: u32,
    /// The walled objective it turned away from.
    pub(crate) from: u32,
    /// The lowest health the open city has shown since the turn.
    pub(crate) best_hp: i32,
    /// The turn of that low.
    pub(crate) best_turn: u32,
}

impl AdvancedAi {
    /// `open-city-draws-the-front` (b): with the gene on, a Recovery or
    /// Expansion plan may turn the front as a Conquest plan does. The caller
    /// passes only a committed objective, which needs the siege commitment
    /// already on an at-war major.
    pub(super) fn open_city_strategy_turns(&self, strategy: super::GrandStrategy) -> bool {
        self.open_city_draws_the_front
            && matches!(
                strategy,
                super::GrandStrategy::Recovery | super::GrandStrategy::Expansion
            )
    }

    /// `open-city-draws-the-front` (c): a prior objective still yields while
    /// at full health with at least three quarters of its walls standing.
    pub(super) fn open_city_prior_yields(g: &Game, prior: &City) -> bool {
        let max = g.city_max_wall_hp(prior);
        prior.hp >= CITY_MAX_HP && max > 0 && prior.wall_hp > 0 && 4 * prior.wall_hp >= 3 * max
    }

    /// Land steps from `from` to every passable land tile within `reach`,
    /// ignoring units and borders. Empty when `from` is not on land.
    fn open_city_land_steps(g: &Game, from: Pos, reach: i32) -> BTreeMap<Pos, i32> {
        let land = |pos: Pos| {
            g.map
                .get(pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        };
        let mut steps = BTreeMap::new();
        if !land(from) {
            return steps;
        }
        steps.insert(from, 0);
        let mut queue = VecDeque::from([from]);
        while let Some(pos) = queue.pop_front() {
            let here = steps[&pos];
            if here >= reach {
                continue;
            }
            for next in g.nbrs(pos) {
                if !steps.contains_key(&next) && land(next) {
                    steps.insert(next, here + 1);
                    queue.push_back(next);
                }
            }
        }
        steps
    }

    /// A healthy land melee of our field army able to take `city`: one that
    /// can fight in melee, not ranged, at `min_hp` health and `min_strength`
    /// or more. Yields `(tiles, land steps)` of the nearest such taker within
    /// [`OPEN_CITY_MARCH`] tiles and [`OPEN_CITY_ROAD_STEPS`] land steps.
    pub(super) fn open_city_taker(
        &self,
        g: &Game,
        pid: usize,
        city: &City,
        min_hp: i32,
        min_strength: f64,
    ) -> Option<(i32, i32)> {
        let steps = Self::open_city_land_steps(g, city.pos, OPEN_CITY_ROAD_STEPS);
        self.campaign_field_army(g, pid)
            .into_iter()
            .filter_map(|uid| g.units.get(&uid))
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                spec.is_melee_capable()
                    && !spec.has_ranged_attack()
                    && unit.hp >= min_hp
                    && g.unit_strength(unit, false) >= min_strength
            })
            .filter_map(|unit| {
                let tiles = g.wdist(unit.pos, city.pos);
                (tiles <= OPEN_CITY_MARCH)
                    .then(|| steps.get(&unit.pos).map(|road| (tiles, *road)))
                    .flatten()
            })
            .min()
    }

    /// Our best field melee strength, attacking: the yardstick of the tier
    /// floor.
    pub(super) fn open_city_best_melee(&self, g: &Game, pid: usize) -> f64 {
        self.campaign_field_army(g, pid)
            .into_iter()
            .filter_map(|uid| g.units.get(&uid))
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                spec.is_melee_capable() && !spec.has_ranged_attack()
            })
            .map(|unit| g.unit_strength(unit, false))
            .fold(0.0, f64::max)
    }

    /// `open-city-draws-the-front` tier floor: the open city's defence is at
    /// most [`OPEN_CITY_TIER_MARGIN`] above our best field melee.
    pub(super) fn open_city_within_reach_of_our_tier(
        &self,
        g: &Game,
        pid: usize,
        city: &City,
    ) -> bool {
        g.city_strength(city.id) <= self.open_city_best_melee(g, pid) + OPEN_CITY_TIER_MARGIN
    }

    /// `open-city-draws-the-front`: whether the front turned away from this
    /// city (or gave it up) recently enough that it may not turn back.
    pub(super) fn open_city_recently_left(&self, g: &Game, city: u32) -> bool {
        self.open_city_left
            .get(&city)
            .is_some_and(|until| g.turn < *until)
    }

    /// `open-city-draws-the-front`: the journal line of a turn to an open
    /// city, in place of the open foothold's.
    pub(super) fn journal_open_city_turn(&self, g: &Game, pid: usize, prior: u32, open: u32) {
        let (Some(prior), Some(open)) = (g.cities.get(&prior), g.cities.get(&open)) else {
            return;
        };
        let taker = self
            .open_city_taker(g, pid, open, 60, 25.0)
            .map(|(tiles, _)| tiles.to_string())
            .unwrap_or_else(|| "on the ring".to_string());
        let walled = if prior.wall_hp > 0 { "walled" } else { "holds" };
        think!(self.journal(), Strategy, Strategy,
               "Campaign turns to open {}", open.name;
               "open-city-draws-the-front: {} {walled} {}; {} has no walls, our taker is {taker} tiles out",
               g.players[open.owner].civ, prior.name, open.name;
               open.pos);
    }

    /// `open-city-draws-the-front`: once a turn, track the open city the
    /// front turned to. It holds until walled or captured; after
    /// [`OPEN_CITY_STALL_TURNS`] without a new low of health it is stood
    /// down; a city the front leaves stays out of the turn for
    /// [`OPEN_CITY_COOLDOWN_TURNS`]. Exact no-op with the gene off.
    pub(crate) fn open_city_observe(&mut self, g: &Game, pid: usize) {
        if !self.open_city_draws_the_front {
            return;
        }
        let target = self.plan.as_ref().and_then(|plan| plan.target_city);
        let last = self.open_city_last_target;
        self.open_city_last_target = target;
        let cooldown = g.turn + g.standard_duration(OPEN_CITY_COOLDOWN_TURNS);
        if let Some(hold) = self.open_city_hold.clone() {
            let city = g.cities.get(&hold.city);
            let captured_or_gone = city.is_none_or(|city| city.owner == pid);
            if target != Some(hold.city) {
                // The front moved on: the open city stays out of the turn.
                if !captured_or_gone {
                    self.open_city_left.insert(hold.city, cooldown);
                }
                self.open_city_hold = None;
            } else if captured_or_gone || city.is_some_and(|city| city.wall_hp > 0) {
                // Captured, or walled: the hold ends; the ordinary commitment
                // keeps or moves the objective from here.
                self.open_city_hold = None;
            } else if let Some(city) = city {
                let mut hold = hold;
                if city.hp < hold.best_hp {
                    hold.best_hp = city.hp;
                    hold.best_turn = g.turn;
                }
                if g.turn.saturating_sub(hold.best_turn)
                    >= g.standard_duration(OPEN_CITY_STALL_TURNS)
                {
                    think!(self.journal(), Strategy, Strategy,
                           "Campaign gives up open {}", city.name;
                           "open-city-draws-the-front: {} turns without a new low of health (it stands at {}); the siege looks elsewhere than {} and {}",
                           g.turn.saturating_sub(hold.best_turn), city.hp, city.name,
                           g.cities.get(&hold.from).map_or("the walled city it left", |from| from.name.as_str());
                           city.pos);
                    self.capture_stood_down.insert(hold.city, cooldown);
                    self.open_city_left.insert(hold.city, cooldown);
                    self.open_city_hold = None;
                } else {
                    self.open_city_hold = Some(hold);
                }
            }
        }
        if self.open_city_hold.is_none() {
            if let (Some(now), Some(before)) = (target, last) {
                if now != before {
                    if let (Some(open), Some(walled)) = (g.cities.get(&now), g.cities.get(&before))
                    {
                        if open.owner == walled.owner
                            && open.owner != pid
                            && open.wall_hp <= 0
                            && walled.wall_hp > 0
                        {
                            self.open_city_hold = Some(OpenCityHold {
                                city: now,
                                from: before,
                                best_hp: open.hp,
                                best_turn: g.turn,
                            });
                        }
                    }
                }
            }
        }
        self.open_city_left.retain(|_, until| g.turn < *until);
    }
}

#[cfg(test)]
mod tests;
