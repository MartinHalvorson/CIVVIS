//! The religious corps as a standing army. Two opt-in genes.
//!
//! **Why (live Russia Deity games of 2026-11-09/10).** In four games rival
//! Apostles attacked our religious units 231 times and killed 74 of them. 68
//! of the 74 were at 60 hit points or less when the killing blow landed, and
//! only 4 died from full health: a unit hit once kept spreading beside the
//! Apostle that hit it and died to the next blow. Every death hands the
//! winner's faith 250 pressure in each city within ten tiles and takes 250
//! from ours. Over the same games our Apostles started 56 fights against 231
//! received, because an Apostle walks to a city, spends its three charges and
//! is gone; it fights only when a rival happens to stand beside it.
//!
//! `corps-comes-home-alive`:
//!
//! - a religious unit at [`RETREAT_FLOOR`] or less walks to the nearest Holy
//!   Site of ours and stands in its ring until it is back to [`HEALED`];
//! - a unit already in such a ring below [`HEALED`] finishes healing first
//!   (an Inquisitor only at [`RETREAT_FLOOR`] or less, since it works there);
//! - a fight is started only when ours survives the blow back at the top of
//!   the roll, and a unit at [`RETREAT_FLOOR`] or less takes only a kill;
//! - a tile is unsafe when every rival that can strike it this turn would
//!   together kill the unit (see `apostle_evasion::every_blow_kills`).
//!
//! `last-charge-joins-the-army`, on a Religion seat with its own faith:
//!
//! - an Apostle keeps its last charge. Its strength does not depend on its
//!   charges, so it hunts rival religious units it can beat, and with none in
//!   sight it escorts our spreaders abroad;
//! - an Apostle with Debater hunts before it spreads;
//! - any Apostle of ours hunts a rival religious unit within [`HOME_GROUND`]
//!   tiles of a city of ours before it spreads. Live, 2026-11-10: rival
//!   Apostles killed 7 Missionaries within 4 tiles of our Lavras in one game
//!   while our 8 Apostles started 4 fights;
//! - a Missionary keeps its last charge and walks the fog, which is how the
//!   seat finds the rival cities it has never seen;
//! - neither counts against the corps caps, so the next spreader is still
//!   bought.
//!
//! The first [`ARMY_CEILING`] Apostles and [`SCOUT_CEILING`] Missionaries are
//! kept; later ones spend the charge as before. A city of ours that needs
//! religious support within [`LOCAL_EMERGENCY`] tiles releases the charge.

use super::*;

/// At or below this a single even Apostle's blow can kill at the top of the
/// roll (mean about 45, roll up to 1.25, host edge on top).
pub(super) const RETREAT_FLOOR: i32 = 60;

/// A healing unit goes back to work from here.
const HEALED: i32 = 90;

/// The top of the damage roll, as a factor on the mean blow.
pub(super) const ROLL_MARGIN: f64 = 1.25;

/// How far a hunter looks for prey: two turns of Apostle movement and the
/// ten tiles a kill's pressure carries.
const HUNT_RADIUS: i32 = 10;

/// This turn's reach for an Apostle on open ground.
const HUNT_REACH: i32 = 4;

/// How near a city of ours a rival religious unit is on home ground, where
/// every Apostle hunts it: the range a kill's pressure carries in Civ VI's
/// own data (`RELIGION_SPREAD_RANGE_COMBAT_VICTORY`), so each kill there
/// pays 250 pressure into that city.
const HOME_GROUND: i32 = 6;

/// How many last-charge Apostles stand in the army at once.
const ARMY_CEILING: usize = 8;

/// How many last-charge Missionaries scout at once.
const SCOUT_CEILING: usize = 3;

/// A city of ours needing support this close takes the last charge.
const LOCAL_EMERGENCY: i32 = 6;

/// Turns between searches for fog once a search has found none.
const SCOUT_SEARCH_REST: u32 = 5;

/// `MissionaryExplore::turns` marker: the last search found no fog.
const SCOUT_FOUND_NOTHING: u32 = u32::MAX;

impl AdvancedAi {
    /// The nearest Holy Site district tile of ours whose ring heals `uid`.
    fn nearest_healing_site(g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let from = g.units.get(&uid)?.pos;
        g.player_city_ids(pid)
            .into_iter()
            .flat_map(|cid| g.cities[&cid].owned_tiles.iter().copied())
            .filter(|pos| {
                g.map
                    .get(*pos)
                    .and_then(|tile| tile.district)
                    .is_some_and(|district| g.district_family(district) == "holy_site")
            })
            .filter(|pos| g.unit_heal_rate_at(uid, *pos) > 0)
            .min_by_key(|pos| (g.wdist(from, *pos), *pos))
    }

    /// Whether a city of ours on or beside `uid`'s tile needs its faith back
    /// and `uid` has a charge to give it.
    fn defends_a_city_from_here(g: &Game, pid: usize, uid: u32, religion: &str) -> bool {
        let unit = &g.units[&uid];
        unit.charges > 0
            && g.rules.units[unit.kind].religious_spread > 0.0
            && std::iter::once(unit.pos)
                .chain(g.nbrs(unit.pos))
                .filter_map(|position| g.city_at(position))
                .any(|cid| Self::city_needs_religious_support(g, pid, &g.cities[&cid], religion))
    }

    /// `corps-comes-home-alive`: a wounded religious unit leaves the field.
    /// `Some(false)` holds it in a healing ring, `Some(moved)` walks it to
    /// one, and `None` leaves the turn to the caller: the gene is off, the
    /// unit is healthy, a Guru can heal itself, a city of ours beside it
    /// needs the charge, or we own no Holy Site that heals.
    pub(super) fn wounded_preacher_goes_home(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        if !self.corps_comes_home_alive {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.owner != pid || g.rules.units[unit.kind].class != "religious" || unit.hp >= HEALED {
            return None;
        }
        if unit.kind == "guru" && unit.charges > 0 {
            return None;
        }
        let healing_here = g.unit_heal_rate(uid) > 0;
        // An Inquisitor works at home, where the ring is: holding it there
        // above the floor would keep it from the heresy it was bought for.
        let stays_to_finish = healing_here && unit.kind != "inquisitor";
        if unit.hp > RETREAT_FLOOR && !stays_to_finish {
            return None;
        }
        let religion = unit
            .religion
            .clone()
            .or_else(|| g.players[pid].religion.clone())?;
        if Self::defends_a_city_from_here(g, pid, uid, &religion) {
            return None;
        }
        let (kind, hp) = (unit.kind, unit.hp);
        if healing_here {
            think!(self.journal(), Faith, Detail,
                "{} {} heals at a Holy Site", kind.as_str(), uid;
                "corps-comes-home-alive: {} hit points, and a rival Apostle kills what it finds at {} or less", hp, RETREAT_FLOOR);
            return Some(false);
        }
        let site = Self::nearest_healing_site(g, pid, uid)?;
        think!(self.journal(), Faith, Decision,
            "{} {} walks home to heal", kind.as_str(), uid;
            "corps-comes-home-alive: {} hit points; 68 of 74 units rival Apostles killed were at {} or less, and each death hands the rival faith 250 pressure around it", hp, RETREAT_FLOOR;
            site);
        let stepped = self.religious_step_toward_range(g, pid, uid, site, 1);
        Some(stepped || self.preacher_runs_for_home(g, pid, uid))
    }

    /// `corps-comes-home-alive`: with no safe step left, a religious unit
    /// still does not stand and wait for the blow. It takes this turn's
    /// reachable tile the fewest rivals reach, nearest our healing ring (or
    /// our nearest city when no ring heals), if that betters where it is.
    /// Live, 2026-11-09: two Apostles at 46 and 51 hit points, each with two
    /// rivals in reach, were refused every step and held in the open.
    pub(super) fn preacher_runs_for_home(&self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if !self.corps_comes_home_alive {
            return false;
        }
        let Some((current, moves)) = g.units.get(&uid).map(|unit| (unit.pos, unit.moves_left))
        else {
            return false;
        };
        if moves <= 0.0 {
            return false;
        }
        let Some(home) = Self::nearest_healing_site(g, pid, uid).or_else(|| {
            g.player_city_ids(pid)
                .into_iter()
                .map(|city| g.cities[&city].pos)
                .min_by_key(|pos| (g.wdist(current, *pos), *pos))
        }) else {
            return false;
        };
        let threats = self.theological_threats(g, pid, uid);
        let rank = |pos: Pos| {
            (
                Self::theological_threats_reaching(g, uid, pos, &threats),
                g.wdist(pos, home),
            )
        };
        let here = rank(current);
        let Some((there, goal)) = g
            .reachable(uid)
            .into_iter()
            .map(|pos| (rank(pos), pos))
            .min()
        else {
            return false;
        };
        if there >= here {
            return false;
        }
        let Some(next) = g
            .route_step(uid, goal, 0)
            .filter(|next| g.can_move(uid, *next))
        else {
            return false;
        };
        let kind = g.units[&uid].kind;
        think!(self.journal(), Faith, Decision,
            "{} {} runs for home", kind.as_str(), uid;
            "corps-comes-home-alive: no step is safe, {} rivals reach it here and {} where it is going, and standing still is the one sure way to be found", here.0, there.0;
            goal);
        self.base.path_move(g, pid, uid, next)
    }

    /// `corps-comes-home-alive`: whether `uid` passes up a fight that reads
    /// `(dealt, received, defender_hp)` at the mean. It does when the blow
    /// back could kill it at the top of the roll, or when it is wounded and
    /// the fight is not a kill.
    pub(super) fn corps_declines_the_fight(
        &self,
        g: &Game,
        uid: u32,
        (dealt, received, defender_hp): (f64, f64, i32),
    ) -> bool {
        if !self.corps_comes_home_alive {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let hp = f64::from(unit.hp);
        hp <= received * ROLL_MARGIN || (unit.hp <= RETREAT_FLOOR && dealt < f64::from(defender_hp))
    }

    /// Whether `last-charge-joins-the-army` applies to this seat at all.
    fn army_doctrine_live(&self, g: &Game, pid: usize) -> bool {
        self.last_charge_joins_the_army
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && g.players[pid].religion.is_some()
    }

    /// Our units of `kind` and of our own faith on their last charge, oldest
    /// first, up to `ceiling`.
    fn last_charge_roster(g: &Game, pid: usize, kind: &str, ceiling: usize) -> Vec<u32> {
        let faith = &g.players[pid].religion;
        let mut roster: Vec<u32> = g
            .units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && unit.kind == kind
                    && unit.charges == 1
                    && unit.religion == *faith
            })
            .map(|unit| unit.id)
            .collect();
        roster.sort_unstable();
        roster.truncate(ceiling);
        roster
    }

    /// `last-charge-joins-the-army`: whether `uid` holds its last charge
    /// instead of spending it.
    pub(super) fn keeps_its_last_charge(&self, g: &Game, pid: usize, uid: u32) -> bool {
        if !self.army_doctrine_live(g, pid) {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let ceiling = match unit.kind.as_str() {
            "apostle" => ARMY_CEILING,
            "missionary" => SCOUT_CEILING,
            _ => return false,
        };
        if unit.owner != pid || unit.charges != 1 || unit.religion != g.players[pid].religion {
            return false;
        }
        let Some(religion) = unit.religion.as_deref() else {
            return false;
        };
        let emergency = g.player_city_ids(pid).into_iter().any(|cid| {
            let city = &g.cities[&cid];
            g.wdist(unit.pos, city.pos) <= LOCAL_EMERGENCY
                && Self::city_needs_religious_support(g, pid, city, religion)
        });
        !emergency && Self::last_charge_roster(g, pid, unit.kind.as_str(), ceiling).contains(&uid)
    }

    /// Whether `pos` lies in the land of a player at war with `pid`, where
    /// any soldier condemns a religious unit.
    fn hostile_land(g: &Game, pid: usize, pos: Pos) -> bool {
        g.map
            .get(pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| city.owner != pid && g.is_at_war(pid, city.owner))
    }

    /// Whether `pos` is within [`HOME_GROUND`] tiles of a city of ours.
    fn home_ground(g: &Game, pid: usize, pos: Pos) -> bool {
        g.player_city_ids(pid)
            .into_iter()
            .any(|city| g.wdist(g.cities[&city].pos, pos) <= HOME_GROUND)
    }

    /// The rival religious unit this hunter goes for: visible, of another
    /// faith, within [`HUNT_RADIUS`], outside hostile land and one it beats.
    /// The weakest of those in this turn's reach, else the nearest. A home
    /// guard takes only prey on home ground.
    fn army_prey(&self, g: &Game, pid: usize, uid: u32, home_only: bool) -> Option<Pos> {
        let hunter = g.units.get(&uid)?;
        let faith = hunter.religion.as_deref()?;
        g.units
            .values()
            .filter(|other| {
                other.owner != pid
                    && other.hp > 0
                    && g.rules.units[other.kind].class == "religious"
                    && other.religion.as_deref().is_some_and(|r| r != faith)
                    && g.wdist(hunter.pos, other.pos) <= HUNT_RADIUS
                    && !Self::hostile_land(g, pid, other.pos)
                    && (!home_only || Self::home_ground(g, pid, other.pos))
                    && self.battlefront_unit_visible(g, pid, other.id)
                    && self.theology_fight_is_winnable(g, uid, other.pos)
            })
            .min_by_key(|other| {
                let distance = g.wdist(hunter.pos, other.pos);
                (distance.max(HUNT_REACH), other.hp, distance, other.id)
            })
            .map(|other| other.pos)
    }

    /// Where an army Apostle stands with no prey in sight: beside the nearest
    /// spreader of ours working outside our borders, else at the Holy City.
    fn army_post(g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let hunter = g.units.get(&uid)?;
        let abroad = |pos: Pos| {
            !g.map
                .get(pos)
                .and_then(|tile| tile.owner_city)
                .and_then(|cid| g.cities.get(&cid))
                .is_some_and(|city| city.owner == pid)
        };
        g.units
            .values()
            .filter(|other| {
                other.owner == pid
                    && other.id != uid
                    && other.charges > 1
                    && g.rules.units[other.kind].religious_spread > 0.0
                    && abroad(other.pos)
                    && !Self::hostile_land(g, pid, other.pos)
            })
            .min_by_key(|other| (g.wdist(hunter.pos, other.pos), other.id))
            .map(|other| other.pos)
            .or_else(|| {
                g.players[pid]
                    .holy_city
                    .and_then(|cid| g.cities.get(&cid))
                    .map(|city| city.pos)
            })
    }

    /// `last-charge-joins-the-army`, the Apostle: hunt, else escort. `None`
    /// for any unit the doctrine does not hold, and for an Apostle with
    /// charges to spend and no prey, which spreads as usual: a Debater hunts
    /// anywhere, any other Apostle of our faith only on home ground. An
    /// Apostle on its kept charge always gets `Some`, so it never reaches
    /// the spread.
    pub(super) fn army_apostle_step(&self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        let unit = g.units.get(&uid)?;
        if unit.kind != "apostle" || !self.army_doctrine_live(g, pid) {
            return None;
        }
        let keeps = self.keeps_its_last_charge(g, pid, uid);
        let ours = unit.religion == g.players[pid].religion;
        let debater = ours && unit.promotions.contains(&crate::name!("debater"));
        if !keeps && !ours {
            return None;
        }
        let home_only = !keeps && !debater;
        let here = unit.pos;
        if let Some(prey) = self.army_prey(g, pid, uid, home_only) {
            if g.wdist(here, prey) <= 1 {
                let fought = g
                    .apply(
                        pid,
                        &Action::TheologicalAttack {
                            unit: uid,
                            target: prey,
                        },
                    )
                    .is_ok();
                if fought {
                    think!(self.journal(), Faith, Decision,
                        "Apostle {} strikes the heretic", uid;
                        "last-charge-joins-the-army: the exchange wins, and a kill moves 250 pressure to our faith in every city within ten tiles";
                        prey);
                }
                return Some(fought);
            }
            think!(self.journal(), Faith, Detail,
                "Apostle {} hunts a rival religious unit", uid;
                "last-charge-joins-the-army: an Apostle fights at full strength on any number of charges, and this fight is one it wins";
                prey);
            return Some(self.religious_step_toward_range(g, pid, uid, prey, 1));
        }
        if !keeps {
            return None;
        }
        let Some(post) = Self::army_post(g, pid, uid) else {
            return Some(false);
        };
        if g.wdist(here, post) <= 1 {
            return Some(false);
        }
        think!(self.journal(), Faith, Detail,
            "Apostle {} keeps its last charge and covers the corps", uid;
            "last-charge-joins-the-army: no rival it can beat is in sight, so it stands with the spreaders a rival Apostle would come for";
            post);
        Some(self.religious_step_toward_range(g, pid, uid, post, 1))
    }

    /// `last-charge-joins-the-army`, the Missionary: walk the fog, and wait at
    /// the Holy City once none can be reached. `None` for any other unit.
    pub(super) fn scout_missionary_step(&self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        if g.units.get(&uid)?.kind != "missionary" || !self.keeps_its_last_charge(g, pid, uid) {
            return None;
        }
        let threats = self.religious_raider_threats(g, pid);
        let turn = g.turn;
        let goal = {
            let mut memory = self.missionary_explore.borrow_mut();
            memory.retain(|other, _| g.units.contains_key(other));
            let entry = memory.entry(uid).or_default();
            let explored = &g.players[pid].explored;
            let held = entry
                .goal
                .filter(|goal| !explored.contains(goal) && g.route_step(uid, *goal, 0).is_some());
            let resting = entry.turns == SCOUT_FOUND_NOTHING
                && turn < entry.last_turn.saturating_add(SCOUT_SEARCH_REST);
            let goal = held.or_else(|| {
                if resting {
                    None
                } else {
                    Self::missionary_expedition_goal(g, pid, uid, &threats)
                }
            });
            if goal.is_none() && !resting {
                entry.turns = SCOUT_FOUND_NOTHING;
                entry.last_turn = turn;
            }
            entry.goal = goal;
            goal
        };
        if let Some(goal) = goal {
            think!(self.journal(), Faith, Detail,
                "Missionary {} keeps its last charge and scouts", uid;
                "last-charge-joins-the-army: the charge is the unit's life, it crosses closed borders, and a rival city we have never seen cannot be converted";
                goal);
            return Some(self.religious_step_toward_range(g, pid, uid, goal, 0));
        }
        let home = g.players[pid]
            .holy_city
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.pos);
        Some(home.is_some_and(|home| self.religious_step_toward_range(g, pid, uid, home, 1)))
    }
}

#[cfg(test)]
mod tests;
