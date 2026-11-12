//! `hopeless-war-brings-the-army-home`: when the war cannot be won in the
//! field, the army goes home and holds there.
//!
//! **Measured (PR #5216, 24 paired whole worlds a shape, both arms with
//! `defend-force-holds-the-city`): it fails its bar.** On the out-teched
//! Immortal shape units lost moved +1.67 ± 3.80 a world (+0.50 ± 4.13 with
//! `siege-goes-on-the-forecast`), cities lost +0.08 ± 0.13, cities taken
//! +0.12 ± 0.09, units killed +2.96 ± 1.25. It was in force 63 turns a world.
//! The brought-home units that died, died on the way: 26 of 29 (38 of 40
//! with the stand-down), mostly within two tiles of an enemy. Opt-in, not
//! armed.
//!
//! **Why (PR #5211, 24 paired whole worlds a shape).**
//! - Hopeless sieges kill their units. On the out-teched Immortal shape, 138
//!   of the 168 deaths last ordered by `siege_doctrine` that had a forecast
//!   reading were in sieges read under 0.1. The train stood in Stage on 95%
//!   of its siege-turns, mustering before cities it could not take.
//! - Standing those sieges down (`siege-goes-on-the-forecast`) cut that
//!   bin's deaths by about 30 and did not cut units lost. The same units
//!   then died under rotation, healing and the battle planner, because
//!   nothing told them where to be.
//! - The live census gives the place: not one of 4,918 enemy blows landed on
//!   a unit standing in one of our City Centers. `death_chance`
//!   (`priced_risk.rs`) says which other tiles no visible enemy can reach.
//!
//! **When** ([`AdvancedAi::plan_army_home`], once a turn):
//! - we are at war with a major;
//! - no siege is worth pressing: every enemy city the siege train holds a
//!   record for reads P(capture within the forecast's horizon) under
//!   `HOPELESS_P` for our whole land army
//!   (`assault_forecast::assault_capture_curve`), or there is none;
//! - we are outclassed in the field: the strongest visible enemy soldier of a
//!   major at war is at least `OBSOLETE_STRENGTH_GAP` above our strongest
//!   land soldier (fighting strength, the reading `risk-is-priced` prices
//!   obsolescence with).
//!
//! It comes into force after [`ENTER_TURNS`] such turns in a row. It lifts
//! when a siege reads [`LIFT_P`] or more, or the war with every major ends.
//!
//! **What, while in force.** Every land soldier that is not posted by
//! `defend-force-holds-the-city`, not within `HOLD_RADIUS` of a city it
//! holds, not a Defend or Relieve member, not an escort or a settler's
//! guard, not one the battle planner ordered, and not claimed by another
//! pass this turn, is given a home:
//! 1. an EMPTY centre of one of our cities or Encampments, nearest the enemy
//!    first, a shooter first;
//! 2. otherwise a free tile inside our borders within [`SAFE_RING`] of the
//!    [`FRONT_CITIES`] cities nearest the enemy, where no visible enemy's
//!    strike reaches and no walled enemy city strikes (death chance zero);
//! 3. otherwise it walks toward the nearest of those cities.
//!
//! A soldier already in one of our centres keeps it. On the way and at home,
//! a shooter fires at anything in range, and any unit takes a kill it
//! survives at the top of the roll that does not carry it off our ground or
//! into reach. Then it fortifies, or heals. One journal line a turn while in
//! force says how many units went where.
//!
//! **Coexistence.** It reads `defend-force-holds-the-city`'s plan and never
//! takes a unit that gene posts, nor one near a city it holds; with that gene
//! off, nothing here depends on it. Its step stands right after the hold's in
//! the ladder. Off by default; nothing is read or written with the gene off.

use super::assault_forecast::{HOLD_P, HOPELESS_P};
use super::battle_planner::DangerField;
use super::defend_force_holds_the_city::HOLD_RADIUS;
use super::objective_board::ObjectiveKey;
use super::priced_risk::{fighting_strength, OBSOLETE_STRENGTH_GAP};
use super::*;
use crate::game::expected_damage;

/// Hopeless turns in a row before the army comes home.
pub(crate) const ENTER_TURNS: u32 = 2;
/// A siege reading at or above this lifts it: the forecast's own
/// invested-train hold (`HOLD_P`).
pub(crate) const LIFT_P: f32 = HOLD_P;
/// The cities nearest the enemy whose ground takes the soldiers no centre
/// holds.
pub(crate) const FRONT_CITIES: usize = 3;
/// How far from a front city a home tile may be.
pub(crate) const SAFE_RING: i32 = 2;

/// Where a soldier goes home to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HomeRole {
    /// One of our City Centers or Encampments.
    Centre,
    /// A tile inside our borders out of every visible enemy's reach.
    Safe,
    /// No home was free: toward the nearest front city.
    Toward,
}

/// The gene's state across turns, and this turn's homes.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ArmyHome {
    /// Hopeless turns in a row, while not yet in force.
    streak: u32,
    /// In force since this turn.
    pub(crate) since: Option<u32>,
    /// The turn the homes below were drawn.
    pub(crate) turn: u32,
    /// Each soldier's home this turn.
    pub(crate) posts: BTreeMap<u32, (Pos, HomeRole)>,
}

/// What one turn's reading of the war said.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WarReading {
    pub(crate) at_war: bool,
    /// The best siege's P(capture within the horizon) for the whole army;
    /// zero with no siege.
    pub(crate) best: f32,
    pub(crate) sieges: usize,
    pub(crate) theirs: f64,
    pub(crate) ours: f64,
}

impl WarReading {
    pub(crate) fn hopeless(&self) -> bool {
        self.at_war && self.best < HOPELESS_P && self.theirs - self.ours >= OBSOLETE_STRENGTH_GAP
    }
}

/// How a blow from home ranks: a kill, the damage, then the tile.
type BlowRank = (bool, i64, std::cmp::Reverse<Pos>);

/// A land tile one of our soldiers may stand on: passable, and no soldier
/// on it but `uid`.
fn free_for(g: &Game, pid: usize, uid: u32, pos: Pos) -> bool {
    g.map
        .get(pos)
        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        && g.unit_ids_at(pos).iter().all(|id| {
            let other = &g.units[id];
            *id == uid || (other.owner == pid && g.rules.units[other.kind].class != "military")
        })
}

impl AdvancedAi {
    /// Whether the gene is in force this turn.
    pub(super) fn army_home_in_force(&self, g: &Game) -> bool {
        self.hopeless_war_brings_the_army_home
            && self.hopeless_war_home.since.is_some()
            && self.hopeless_war_home.turn == g.turn
    }

    /// Our land soldiers, the force a siege of the whole army would bring.
    fn whole_army(g: &Game, pid: usize) -> Vec<u32> {
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| Self::centre_soldier(g, pid, *uid))
            .collect()
    }

    /// This turn's reading: the war, the best siege, the strength gap.
    pub(crate) fn read_hopeless_war(&self, g: &Game, pid: usize) -> WarReading {
        let at_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        });
        let army = Self::whole_army(g, pid);
        let mut sieges = 0;
        let mut best = 0.0_f32;
        if at_war {
            for cid in self.sieges.keys() {
                let Some(city) = g.cities.get(cid) else {
                    continue;
                };
                if city.owner == pid || !g.is_at_war(pid, city.owner) {
                    continue;
                }
                sieges += 1;
                if let Some(forecast) = self.assault_capture_curve(g, pid, *cid, &army) {
                    best = best.max(forecast.p());
                }
            }
        }
        let theirs = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                let owner = &g.players[unit.owner];
                unit.owner != pid
                    && !owner.is_minor
                    && !owner.is_barbarian
                    && g.is_at_war(pid, unit.owner)
                    && spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && g.unit_visible_to(unit.id, pid)
            })
            .map(|unit| fighting_strength(g, unit.id))
            .fold(0.0_f64, f64::max);
        let ours = army
            .iter()
            .map(|uid| fighting_strength(g, *uid))
            .fold(0.0_f64, f64::max);
        WarReading {
            at_war,
            best,
            sieges,
            theirs,
            ours,
        }
    }

    /// Whether `uid` may be sent home: a land soldier nobody else holds.
    fn home_candidate(&self, g: &Game, pid: usize, uid: u32, claimed: &BTreeSet<u32>) -> bool {
        if !Self::centre_soldier(g, pid, uid)
            || claimed.contains(&uid)
            || self.battle_planner_claims(uid)
            || self.builder_guard_reserved(uid)
            || self.guard_is_bound_to_any_settler(uid)
            || self.guard_is_reserved_for_civilian(uid)
        {
            return false;
        }
        // `defend-force-holds-the-city`: its posts, and every soldier near a
        // city it holds, stay its own.
        if self.defend_force_holds_the_city && self.defend_hold_plan.turn == g.turn {
            if self.defend_hold_plan.posts.contains_key(&uid) {
                return false;
            }
            let pos = g.units[&uid].pos;
            let near_held = self.defend_hold_plan.cities.iter().any(|cid| {
                g.cities
                    .get(cid)
                    .is_some_and(|city| g.wdist(city.pos, pos) <= HOLD_RADIUS)
            });
            if near_held {
                return false;
            }
        }
        !self.objective_board_state.forces.iter().any(|force| {
            matches!(
                force.objective_key,
                ObjectiveKey::Defend(_) | ObjectiveKey::Relieve(_)
            ) && force.units.contains(&uid)
        })
    }

    /// Once a turn, before the unit loop: read the war, update the
    /// hysteresis, and while in force give every candidate its home. Nothing
    /// with the gene off.
    pub(super) fn plan_army_home(&mut self, g: &mut Game, pid: usize, claimed: &BTreeSet<u32>) {
        if !self.hopeless_war_brings_the_army_home {
            return;
        }
        let turn = g.turn;
        if self.hopeless_war_home.turn == turn {
            return;
        }
        self.hopeless_war_home.turn = turn;
        self.hopeless_war_home.posts.clear();
        let reading = self.read_hopeless_war(g, pid);
        match self.hopeless_war_home.since {
            None => {
                if reading.hopeless() {
                    self.hopeless_war_home.streak += 1;
                } else {
                    self.hopeless_war_home.streak = 0;
                }
                if self.hopeless_war_home.streak >= ENTER_TURNS {
                    self.hopeless_war_home.since = Some(turn);
                    self.hopeless_war_home.streak = 0;
                }
            }
            Some(since) => {
                if !reading.at_war || reading.best >= LIFT_P {
                    self.hopeless_war_home.since = None;
                    think!(self.journal(), Military, Decision,
                    "The army goes back to the field";
                    "hopeless-war-brings-the-army-home: in force since turn {}; {}",
                    since,
                    if reading.at_war {
                        format!("a siege reads {:.0}% (at least {:.0}%)", reading.best * 100.0, LIFT_P * 100.0)
                    } else {
                        "the war with every major has ended".to_string()
                    });
                    return;
                }
            }
        }
        if self.hopeless_war_home.since.is_none() {
            return;
        }
        self.draw_homes(g, pid, claimed, &reading);
    }

    /// The homes: empty centres nearest the enemy (shooters first), then
    /// tiles no visible enemy reaches near the front cities.
    fn draw_homes(&mut self, g: &Game, pid: usize, claimed: &BTreeSet<u32>, reading: &WarReading) {
        let enemies: Vec<Pos> = g
            .units
            .values()
            .filter(|unit| {
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && g.rules.units[unit.kind].class == "military"
                    && g.unit_visible_to(unit.id, pid)
            })
            .map(|unit| unit.pos)
            .chain(
                g.cities
                    .values()
                    .filter(|city| city.owner != pid && g.is_at_war(pid, city.owner))
                    .map(|city| city.pos),
            )
            .collect();
        let to_enemy = |pos: Pos| {
            enemies
                .iter()
                .map(|enemy| g.wdist(*enemy, pos))
                .min()
                .unwrap_or(i32::MAX)
        };
        let mut candidates: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| self.home_candidate(g, pid, *uid, claimed))
            .collect();
        candidates.sort_by_key(|uid| (to_enemy(g.units[uid].pos), *uid));
        let held: BTreeSet<u32> = if self.defend_force_holds_the_city {
            self.defend_hold_plan.cities.iter().copied().collect()
        } else {
            BTreeSet::new()
        };
        // Our centres and Encampments, nearest the enemy first.
        let mut centres: Vec<(i32, Pos)> = Vec::new();
        for cid in g.player_city_ids(pid) {
            if held.contains(&cid) {
                continue;
            }
            let at = g.cities[&cid].pos;
            centres.push((to_enemy(at), at));
            if let Some(camp) = g
                .wdisk(at, 3)
                .into_iter()
                .find(|pos| g.encampment_at(*pos) == Some(cid))
            {
                centres.push((to_enemy(camp), camp));
            }
        }
        centres.sort();
        let mut posts: BTreeMap<u32, (Pos, HomeRole)> = BTreeMap::new();
        // A soldier already in a centre keeps it.
        for uid in &candidates {
            let pos = g.units[uid].pos;
            if centres.iter().any(|(_, at)| *at == pos) {
                posts.insert(*uid, (pos, HomeRole::Centre));
            }
        }
        let soldier_on = |pos: Pos| {
            g.unit_ids_at(pos).iter().any(|id| {
                let unit = &g.units[id];
                unit.owner == pid && g.rules.units[unit.kind].class == "military"
            })
        };
        for (_, at) in &centres {
            if soldier_on(*at) || posts.values().any(|(pos, _)| pos == at) {
                continue;
            }
            let pick = |shooters: bool| {
                candidates
                    .iter()
                    .copied()
                    .filter(|uid| !posts.contains_key(uid))
                    .filter(|uid| !shooters || g.rules.units[g.units[uid].kind].has_ranged_attack())
                    .min_by_key(|uid| (g.wdist(g.units[uid].pos, *at), *uid))
            };
            let chosen = pick(true).or_else(|| pick(false));
            if let Some(uid) = chosen {
                posts.insert(uid, (*at, HomeRole::Centre));
            }
        }
        // The front cities, and the tiles near them no enemy reaches.
        let mut fronts: Vec<(i32, Pos)> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| {
                let at = g.cities[&cid].pos;
                (to_enemy(at), at)
            })
            .collect();
        fronts.sort();
        fronts.truncate(FRONT_CITIES);
        let field = DangerField::with_reach(g, pid, true);
        let walled: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| city.owner != pid && g.is_at_war(pid, city.owner) && city.wall_hp > 0)
            .map(|city| city.pos)
            .collect();
        let mut safe: Vec<Pos> = fronts
            .iter()
            .flat_map(|(_, at)| g.wdisk(*at, SAFE_RING))
            .filter(|pos| {
                g.city_at(*pos).is_none()
                    && g.map.get(*pos).is_some_and(|tile| {
                        tile.owner_city
                            .and_then(|cid| g.cities.get(&cid))
                            .is_some_and(|city| city.owner == pid)
                    })
                    && !field.reaches_tile(*pos)
                    && walled.iter().all(|city| g.wdist(*city, *pos) > 2)
                    && crate::ai::BasicAi::movement_hazard_damage(g, pid, *pos) <= 0.0
            })
            .collect();
        safe.sort();
        safe.dedup();
        for uid in &candidates {
            if posts.contains_key(uid) {
                continue;
            }
            let pos = g.units[uid].pos;
            let tile = safe
                .iter()
                .copied()
                .filter(|tile| {
                    (*tile == pos || free_for(g, pid, *uid, *tile))
                        && !posts.values().any(|(taken, _)| taken == tile)
                })
                .min_by_key(|tile| (g.wdist(pos, *tile), *tile));
            let home = match tile {
                Some(tile) => (tile, HomeRole::Safe),
                None => match fronts.iter().min_by_key(|(_, at)| (g.wdist(pos, *at), *at)) {
                    Some((_, at)) => (*at, HomeRole::Toward),
                    None => continue,
                },
            };
            posts.insert(*uid, home);
        }
        let count = |role: HomeRole| posts.values().filter(|(_, r)| *r == role).count();
        let (centre, safe_n, toward) = (
            count(HomeRole::Centre),
            count(HomeRole::Safe),
            count(HomeRole::Toward),
        );
        let since = self.hopeless_war_home.since.unwrap_or(g.turn);
        think!(self.journal(), Military, Decision,
            "The army comes home: {} soldiers, {} to centres, {} to safe tiles, {} toward the front cities",
            posts.len(), centre, safe_n, toward;
            "hopeless-war-brings-the-army-home: in force since turn {since}; the best of {} siege(s) reads {:.0}% for the whole army (under {:.0}%); strongest enemy {:.0} against our {:.0}",
            reading.sieges, reading.best * 100.0, HOPELESS_P * 100.0, reading.theirs, reading.ours);
        self.hopeless_war_home.posts = posts;
    }

    /// One soldier's turn at home: walk there, strike what the doctrine
    /// allows, fortify. `None` with the gene off, out of force, or for a
    /// soldier with no home this turn.
    pub(super) fn army_home_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        if !self.army_home_in_force(g) {
            return None;
        }
        let (home, role) = *self.hopeless_war_home.posts.get(&uid)?;
        let unit = g.units.get(&uid)?;
        let mut acted = false;
        if unit.pos != home && unit.moves_left > 0.0 {
            // As far toward home as this turn reaches, no visible enemy's
            // reach first.
            let field = DangerField::with_reach(g, pid, true);
            let here = unit.pos;
            let stop = if role == HomeRole::Toward { 1 } else { 0 };
            let mut reachable = g.reachable(uid);
            reachable.push(here);
            if let Some(to) = reachable
                .into_iter()
                .filter(|tile| *tile == home || free_for(g, pid, uid, *tile))
                .min_by_key(|tile| {
                    (
                        (g.wdist(*tile, home) - stop).max(0),
                        field.reaches_tile(*tile),
                        g.wdist(here, *tile),
                        *tile,
                    )
                })
            {
                if to != here {
                    acted = self.base.path_walk_to(g, pid, uid, to);
                }
            }
        }
        if g.units.contains_key(&uid) {
            if let Some(struck) = self.home_blow(g, pid, uid) {
                acted |= struck;
            }
        }
        if g.units.contains_key(&uid) {
            acted |= self.base.fortify_or_stop(g, pid, uid);
        }
        Some(acted)
    }

    /// A shooter's best shot in range (a kill first, then the most damage),
    /// or a melee kill the unit survives at the top of the roll onto a tile
    /// of ours no visible enemy reaches. `Some(true)` when it struck.
    fn home_blow(&mut self, g: &mut Game, pid: usize, uid: u32) -> Option<bool> {
        let unit = g.units.get(&uid)?;
        if unit.attacks_left <= 0 || unit.moves_left <= 0.0 {
            return None;
        }
        let spec = &g.rules.units[unit.kind];
        let here = unit.pos;
        let hp = f64::from(unit.hp);
        let roll = crate::ai::COMBAT_ROLL_MAX;
        let enemy_on = |tile: Pos| -> Option<u32> {
            g.unit_ids_at(tile).iter().copied().find(|id| {
                let enemy = &g.units[id];
                let espec = &g.rules.units[enemy.kind];
                enemy.owner != pid
                    && g.is_at_war(pid, enemy.owner)
                    && espec.class == "military"
                    && espec.domain.as_deref() != Some("air")
            })
        };
        let mut best: Option<(BlowRank, Action)> = None;
        if spec.has_ranged_attack() {
            let frame = g.player_vision_frame(pid);
            let viewers = g.visibility_viewers(pid);
            for tile in g.wdisk(here, g.unit_attack_range(uid)) {
                if g.city_at(tile).is_some()
                    || !g.ranged_order_is_legal(pid, uid, tile, frame.as_ref(), &viewers)
                {
                    continue;
                }
                let Some(did) = enemy_on(tile) else {
                    continue;
                };
                let Some((att, def)) = g.ranged_strike_strengths(uid, did, tile) else {
                    continue;
                };
                let dealt = expected_damage(att, def);
                let key = (
                    dealt >= f64::from(g.units[&did].hp),
                    (dealt * 100.0) as i64,
                    std::cmp::Reverse(tile),
                );
                if best.as_ref().is_none_or(|(k, _)| key > *k) {
                    best = Some((
                        key,
                        Action::Ranged {
                            unit: uid,
                            target: tile,
                        },
                    ));
                }
            }
        } else if spec.is_melee_capable() {
            let field = DangerField::with_reach(g, pid, true);
            for tile in g.nbrs(here) {
                if g.city_at(tile).is_some() || !g.melee_order_is_legal(pid, uid, tile) {
                    continue;
                }
                let Some(did) = enemy_on(tile) else {
                    continue;
                };
                let Some((att, def)) = g.melee_exchange_strengths(uid, did) else {
                    continue;
                };
                let dealt = expected_damage(att, def);
                let taken = expected_damage(def, att);
                // A kill (at the bottom of the roll) it survives at the top,
                // onto ground of ours no other visible enemy reaches.
                let kills = dealt / roll >= f64::from(g.units[&did].hp);
                let ours = g.map.get(tile).is_some_and(|t| {
                    t.owner_city
                        .and_then(|cid| g.cities.get(&cid))
                        .is_some_and(|city| city.owner == pid)
                });
                if !kills || taken * roll >= hp || !ours || field.reaches_tile(tile) {
                    continue;
                }
                let key = (true, (dealt * 100.0) as i64, std::cmp::Reverse(tile));
                if best.as_ref().is_none_or(|(k, _)| key > *k) {
                    best = Some((
                        key,
                        Action::Attack {
                            unit: uid,
                            target: tile,
                        },
                    ));
                }
            }
        }
        let (_, action) = best?;
        Some(g.apply(pid, &action).is_ok())
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod world_tests;
