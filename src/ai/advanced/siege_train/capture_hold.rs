//! `capture-holds-the-ring`: a captured city is taken only when it can be
//! held, the army around it holds its ring for the capture turn and the next,
//! and a capture whose loyalty runs out first sends the taking force on the
//! city whose pressure drives it.
//!
//! Live King, October 5: 8 of 31 captures in ten games were lost the very
//! next turn. G105 (civvis-20261005T061801Z) took Vancouver at turn 151 with
//! "pike and shot 85 walks in from 1 tiles" while 10 of 22 units were staged
//! and 2 of 6 ring tiles held, and Canada took it back at 152. G112 held
//! Xanadu five turns at -13.5 loyalty a turn while Ulaanbaatar, walls down,
//! drove the pressure; taking Ulaanbaatar would have turned Xanadu to +13. G114
//! read "stage" for Curitiba at walls 0 and health 1 while its body stood six
//! to twelve tiles out.

use super::*;

/// `elimination-waits-on-the-clock`: the religion lane's early warning, as
/// progress on the victory screen (half the majors).
pub(crate) const ELIMINATION_FAITH_BAR: i32 = 50;
/// `elimination-waits-on-the-clock`: at most this many holdouts, the victim
/// among them, before an elimination is held.
pub(crate) const ELIMINATION_FAITH_HOLDOUTS: usize = 3;

/// The ring holds on the capture turn and this many turns after it.
pub(super) const CAPTURE_HOLD_TURNS: u32 = 1;
/// The ring holds only while the captured city's loyalty lasts this many
/// turns at its current rate; under it the force marches on the pressure.
pub(super) const CAPTURE_RUNWAY_TURNS: f64 = 6.0;
/// How far a city's loyalty pressure reaches (Civilization VI: nine tiles).
pub(super) const LOYALTY_REACH: i32 = 9;
/// A city with no walls standing at or under this health (a quarter of 200)
/// is a breach any body can finish: its siege never reads Stage.
pub(super) const DYING_CITY_HP: i32 = 50;
/// How many of our units, besides the taker, must be able to end the turn
/// beside a capture that a hostile could retake.
pub(super) const CAPTURE_ESCORTS: usize = 2;

impl AdvancedAi {
    /// `elimination-waits-on-the-clock`: whether taking city `cid` is held
    /// because it is its owner's last city and the owner's elimination would
    /// crown a rival faith (`elimination_crowns_a_faith`). Every capture path
    /// of the train reads it: `capture_holdable` (the taker, the swarm, the
    /// air-led capture) and the breach assault.
    pub(crate) fn elimination_holds_city(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let Some(owner) = g.cities.get(&cid).map(|city| city.owner) else {
            return false;
        };
        if g.player_city_ids(owner).len() != 1 {
            return false;
        }
        let Some(rival) = self.elimination_crowns_a_faith(g, pid, owner) else {
            return false;
        };
        think!(self.journal(), Military, Decision,
            "Holding the capture of {}, {}'s last city", g.cities[&cid].name, g.players[owner].civ;
            "{} still holds out against {}'s faith, among its last {} holdouts; taking it would leave that faith one conversion short of a Religious Victory",
            g.players[owner].civ, g.players[rival].civ, ELIMINATION_FAITH_HOLDOUTS;
            g.cities[&cid].pos);
        true
    }

    /// `elimination-waits-on-the-clock`: the rival whose founded faith would
    /// stand one conversion from a Religious Victory if `victim`, a major
    /// holding out against it, were eliminated. `Some` when a living rival's
    /// faith is at the religion lane's early warning
    /// ([`ELIMINATION_FAITH_BAR`]) or at match point, `victim` does not follow
    /// it and did not found it, and it has at most
    /// [`ELIMINATION_FAITH_HOLDOUTS`] holdouts (living majors other than its
    /// founder that do not follow it, ourselves included) with `victim` among
    /// them. The Religious Victory asks the faith of every OTHER living
    /// major, so the elimination removes a holdout outright.
    ///
    /// Live Emperor civvis-20261008T160451Z (game 415): Poland, Catholic,
    /// was one of Ethiopian Orthodoxy's last three holdouts with Sumeria and
    /// ourselves; we eliminated Poland at turn 104 and lost to Orthodoxy at
    /// 132. Holdouts count only living majors, so this reads true only with
    /// `defeated-majors-leave-the-board` retiring the dead seats.
    pub(crate) fn elimination_crowns_a_faith(
        &self,
        g: &Game,
        pid: usize,
        victim: usize,
    ) -> Option<usize> {
        if !self.elimination_waits_on_the_clock {
            return None;
        }
        let living: Vec<usize> = g
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        living.iter().copied().find(|rival| {
            if *rival == pid || *rival == victim {
                return false;
            }
            let Some(faith) = g.players[*rival].religion.as_deref() else {
                return false;
            };
            if g.players[victim].religion.as_deref() == Some(faith)
                || g.civ_follows_religion(victim, faith)
            {
                return false;
            }
            let warned = self.lane_progress_table(g, *rival)[2] >= ELIMINATION_FAITH_BAR
                || self.faith_at_match_point(g, *rival);
            if !warned {
                return false;
            }
            let holdouts: Vec<usize> = living
                .iter()
                .copied()
                .filter(|other| *other != *rival && !g.civ_follows_religion(*other, faith))
                .collect();
            holdouts.len() <= ELIMINATION_FAITH_HOLDOUTS && holdouts.contains(&victim)
        })
    }

    /// `capture-holds-the-ring` (a): whether `taker` may take city `cid` this
    /// turn. Yes when no hostile unit that can take a city stands within two
    /// tiles of it, or when [`CAPTURE_ESCORTS`] of our other land units can end
    /// the turn beside it. Always yes with the gene off.
    pub(crate) fn capture_holdable(&self, g: &Game, pid: usize, cid: u32, taker: u32) -> bool {
        // `elimination-waits-on-the-clock`: see `elimination_holds_city`.
        if self.elimination_holds_city(g, pid, cid) {
            return false;
        }
        if !self.capture_holds_the_ring {
            return true;
        }
        let Some(city) = g.cities.get(&cid) else {
            return true;
        };
        let at = city.pos;
        let retaker = g.units.values().any(|unit| {
            let spec = &g.rules.units[unit.kind];
            unit.owner != pid
                && g.is_at_war(pid, unit.owner)
                && spec.class == "military"
                && spec.is_melee_capable()
                && spec.domain.as_deref() != Some("air")
                && unit.pos != at
                && g.wdist(unit.pos, at) <= 2
                && g.unit_visible_to(unit.id, pid)
        });
        if !retaker {
            return true;
        }
        let escorts = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && unit.id != taker
                    && spec.class == "military"
                    && spec.domain.as_deref() != Some("air")
                    && !g.is_embarked(unit)
            })
            .filter(|unit| {
                g.wdist(unit.pos, at) == 1
                    || (unit.moves_left > 0.0 && {
                        let reach = g.reachable(unit.id);
                        g.nbrs(at)
                            .into_iter()
                            .any(|pos| reach.contains(&pos) && g.unit_ids_at(pos).is_empty())
                    })
            })
            .count();
        escorts >= CAPTURE_ESCORTS
    }

    /// `capture-holds-the-ring`: the turns the captured city at `cid` holds
    /// at its current loyalty rate; unbounded while it is not falling.
    pub(super) fn capture_runway(&self, g: &Game, cid: u32) -> f64 {
        let Some(city) = g.cities.get(&cid) else {
            return f64::INFINITY;
        };
        let rate = g.city_loyalty_per_turn(city);
        if rate >= 0.0 {
            return f64::INFINITY;
        }
        city.loyalty / (-rate).max(1.0)
    }

    /// `capture-holds-the-ring` (c): the hostile city at war within
    /// [`LOYALTY_REACH`] of `cid` exerting the most pressure on it, read as
    /// population weighted by nearness.
    pub(super) fn loyalty_pressure_source(&self, g: &Game, pid: usize, cid: u32) -> Option<u32> {
        let at = g.cities.get(&cid)?.pos;
        g.cities
            .values()
            .filter(|city| {
                city.owner != pid
                    && g.is_at_war(pid, city.owner)
                    && g.wdist(city.pos, at) <= LOYALTY_REACH
            })
            .max_by_key(|city| {
                (
                    city.pop * (LOYALTY_REACH + 1 - g.wdist(city.pos, at)),
                    Reverse(city.id),
                )
            })
            .map(|city| city.id)
    }

    /// `capture-holds-the-ring`: record each captured city of ours the first
    /// turn it is seen, and which of them run out of loyalty first.
    pub(super) fn note_capture_holds(&mut self, g: &Game, pid: usize) {
        if !self.capture_holds_the_ring {
            return;
        }
        let turn = g.turn;
        let held: Vec<(Pos, u32)> = g
            .cities
            .values()
            .filter(|city| city.owner == pid && city.occupied_from.is_some())
            .map(|city| (city.pos, city.id))
            .collect();
        for (pos, _) in &held {
            self.capture_hold_seen.entry(*pos).or_insert(turn);
        }
        self.capture_hold_seen
            .retain(|pos, _| held.iter().any(|(at, _)| at == pos));
        self.capture_loyalty_prey.clear();
        for (pos, cid) in held {
            if self.capture_runway(g, cid) >= CAPTURE_RUNWAY_TURNS {
                continue;
            }
            if let Some(prey) = self.loyalty_pressure_source(g, pid, cid) {
                self.capture_loyalty_prey.insert(pos, prey);
            }
        }
    }

    /// `capture-holds-the-ring` (c): the city a group near a captured city
    /// whose loyalty runs out besieges instead of its own objective.
    pub(super) fn loyalty_prey_for(&self, g: &Game, pid: usize, anchor: Pos) -> Option<u32> {
        if !self.capture_holds_the_ring {
            return None;
        }
        self.capture_loyalty_prey
            .iter()
            .filter(|(pos, _)| g.wdist(anchor, **pos) <= OBJECTIVE_REACH)
            .map(|(_, cid)| *cid)
            .find(|cid| {
                g.cities
                    .get(cid)
                    .is_some_and(|city| city.owner != pid && g.is_at_war(pid, city.owner))
            })
    }

    /// `capture-holds-the-ring` (b): on the capture turn and the next, a unit
    /// within [`STAGING_FAR`] of a captured city whose loyalty holds keeps it:
    /// melee step onto its ring, ranged units strike the hostile nearest it,
    /// and nothing walks on to the next objective. `None` when no such hold
    /// applies to this unit.
    pub(super) fn capture_ring_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        if !self.capture_holds_the_ring {
            return None;
        }
        let unit = g.units.get(&uid)?.clone();
        if unit.owner != pid || g.is_embarked(&unit) {
            return None;
        }
        let turn = g.turn;
        let hold = self
            .capture_hold_seen
            .iter()
            .filter(|(pos, seen)| {
                turn <= **seen + CAPTURE_HOLD_TURNS
                    && !self.capture_loyalty_prey.contains_key(*pos)
                    && g.wdist(unit.pos, **pos) <= STAGING_FAR
            })
            .min_by_key(|(pos, _)| (g.wdist(unit.pos, **pos), **pos))
            .map(|(pos, _)| *pos)?;
        let name = g
            .city_at(hold)
            .map(|cid| g.cities[&cid].name.clone())
            .unwrap_or_default();
        match arm_of(g, uid) {
            Arm::Melee => {
                if g.wdist(unit.pos, hold) > 1 && unit.moves_left > 0.0 {
                    let reach = g.reachable(uid);
                    let step = g
                        .nbrs(hold)
                        .into_iter()
                        .filter(|pos| {
                            reach.contains(pos)
                                && g.unit_ids_at(*pos).is_empty()
                                && g.map.get(*pos).is_some_and(|tile| {
                                    g.rules.is_passable(tile) && !g.rules.is_water(tile)
                                })
                        })
                        .min_by_key(|pos| (g.wdist(unit.pos, *pos), *pos));
                    if let Some(step) = step {
                        if self.base.path_walk_to(g, pid, uid, step) {
                            self.force_groups_dirty = true;
                            think!(self.journal(), Military, Detail,
                                "The {} holds the ring of {name}", unit.kind;
                                "the city was taken this turn or the last; the next objective waits a turn";
                                hold);
                            return Some(true);
                        }
                    }
                }
                Some(self.base.fortify_or_stop(g, pid, uid))
            }
            Arm::Siege | Arm::Shooter => {
                if unit.attacks_left > 0 && unit.moves_left > 0.0 {
                    let range = g.unit_attack_range(uid).max(1);
                    let frame = g.player_vision_frame(pid);
                    let viewers = g.visibility_viewers(pid);
                    let mut threats: Vec<(i32, u32, Pos)> = g
                        .units
                        .values()
                        .filter(|other| {
                            let spec = &g.rules.units[other.kind];
                            other.owner != pid
                                && g.is_at_war(pid, other.owner)
                                && spec.class == "military"
                                && g.city_at(other.pos).is_none()
                                && g.unit_visible_to(other.id, pid)
                                && g.wdist(unit.pos, other.pos) <= range
                        })
                        .map(|other| (g.wdist(other.pos, hold), other.id, other.pos))
                        .collect();
                    threats.sort_unstable();
                    let target = threats.into_iter().map(|(_, _, pos)| pos).find(|pos| {
                        g.ranged_order_is_legal(pid, uid, *pos, frame.as_ref(), &viewers)
                    });
                    if let Some(target) = target {
                        if g.apply(pid, &Action::Ranged { unit: uid, target }).is_ok() {
                            self.force_groups_dirty = true;
                            think!(self.journal(), Military, Detail,
                                "The {} covers the ring of {name}", unit.kind;
                                "it strikes the hostile nearest the city it just took";
                                hold);
                            return Some(true);
                        }
                    }
                }
                Some(self.base.fortify_or_stop(g, pid, uid))
            }
            Arm::Other => None,
        }
    }

    /// `capture-holds-the-ring` (d): a city with no walls standing at or under
    /// [`DYING_CITY_HP`] is a breach to finish, never a train to stage for.
    pub(super) fn dying_open_city(&self, city: &CityView) -> bool {
        self.capture_holds_the_ring && city.wall_hp <= 0 && city.hp <= DYING_CITY_HP
    }
}
