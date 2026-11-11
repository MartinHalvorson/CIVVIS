//! `first-capital-before-the-walls`: a Domination seat opens the early
//! conquest on the nearest rival capital it can walk to, while that capital
//! still stands behind Ancient Walls or none.
//!
//! ## Why
//!
//! Deity domination has never won here, and the first capital is where it is
//! lost. The eight Gran Colombia Deity domination games of 11-09 took one
//! capital between them (about t160-175); their wars started t119-151, when
//! rival capitals stood behind 300-400 wall HP and our guns were a tier
//! behind. Over the surviving Deity runs (same setup, current code) the
//! early-conquest opening never opened: 0 openings in 72 runs and 301
//! refusals, 183 of them "C is 11-17 tiles from the capital, past the 10 an
//! opening reaches" (`conquest-opening-stays-near`) and 118 the armed
//! `early-conquest-stands-down`, whose evidence is Emperor Gran Colombia
//! archer openings. Its naming deadline, `standard_duration(60)`, is turn 40
//! on Online speed.
//!
//! What the board offers in that window, from the same runs: at t50, 38 of 40
//! rival capitals had walls of 100 or less and a defence near 40; the
//! nearest rival capital on our landmass stood a median 15 tiles away
//! (p75 17, p90 19). Where that capital was charted by t55 within 20 tiles
//! (42 games), its walls read 200 from a median of about t80, and as early
//! as t46 (Buda, 081626Z).
//!
//! ## What the gene changes
//!
//! Only on a seat assigned the Domination lane, and only through the armed
//! `early-conquest-opening` machinery (reservation, rally, assembly, bill
//! preview, pinned campaign):
//!
//! 1. **The target** is the nearest rival ORIGINAL capital that is met and
//!    legal, charted (`explored`), within [`FIRST_CAPITAL_REACH_TILES`], walled
//!    to at most [`FIRST_CAPITAL_WALL_CEILING`] (Ancient), or to
//!    [`FIRST_CAPITAL_TOWER_CEILING`] (Medieval) once we know the Siege
//!    Tower's technology, and joined to our capital by
//!    a walk over charted, dry, passable ground our units may enter
//!    ([`AdvancedAi::first_capital_walk_is_charted`]). It replaces the
//!    shipped scan; the rival's city count is not capped.
//! 2. **No stand-down** on that target: `early-conquest-stands-down` does
//!    not apply.
//! 3. **The clock**: a capital can be named until standard turn
//!    [`FIRST_CAPITAL_NAMING_DEADLINE`] (63 on Online), and an opening gets
//!    [`FIRST_CAPITAL_PREPARATION_TURNS`] standard turns (30 on Online) of
//!    preparation from the turn it is named, or from the turn the empire
//!    holds [`FIRST_CAPITAL_SETTLER_CITIES`] cities if that is later: the
//!    capital trains the first two Settlers before its production is
//!    reserved. The real deadline is the walls, item 5. The armed
//!    `conquest-opening-needs-the-production` estimate (the capital alone
//!    training every missing body) is journalled, not a refusal: the other
//!    cities and the standing army fill the force too.
//! 4. **The force** leads with melee: [`FIRST_CAPITAL_MELEE`] melee bodies
//!    and [`FIRST_CAPITAL_RANGED`] shooters (the shipped total of six), so
//!    the unique melee line (Hypaspist, Hetairoi) carries the assault. The
//!    shipped breaker reservation (one, from the capital) buys a Battering
//!    Ram against Ancient Walls and, under the gene, a Siege Tower against
//!    Medieval Walls, which a Ram does not affect. Each
//!    of our cities keeps the land body standing on or beside it as its
//!    guard: the force never takes a city's last garrison, and the
//!    reservation trains the force on top of the guards.
//! 5. **Stand-down on the walls**: an opening that has taken nothing is
//!    released (or, once at war, asks for terms) when the target's walls go
//!    past Medieval, or past Ancient while we neither hold, queue nor can
//!    train a Siege Tower.
//! 6. **The declaration** keeps every shipped veto (bill preview, legality,
//!    one war at a time, treasury) but not `declaration-needs-production-
//!    parity`: a Deity neighbour out-produces us about four to one at t50
//!    (median 0.24 over the same runs), so parity would refuse every war the
//!    window allows. The local bill is the question here.
//! 7. **After the capital falls** the campaign goes on to the rival's next
//!    known city whatever its kills per loss, with the surviving force and
//!    the field army within [`FIRST_CAPITAL_REINFORCE_RADIUS`] of the
//!    captured city, and the between-captures peace desk stays shut. It
//!    still asks for terms when the whole force is gone.
//!
//! 8. **No friendship with the target**: our own declared-friendship offer
//!    (`befriend-the-strongest`) is withheld from the rival whose capital
//!    the opening stands on or would name, since a friendship forbids the
//!    declaration while it lasts.
//!
//! Everything else (Recovery and a threatened city still hold the march,
//! `one-war-at-a-time`, the treasury, the bill) is the shipped opening.
//! Inert with the flag off, without `early-conquest-opening`, and on any
//! other lane.

use super::early_conquest::{ConquestOpening, CONQUEST_MELEE, CONQUEST_RANGED};
use super::{AdvancedAi, EmpireCounts, VictoryTarget};
use crate::game::{Game, Item};
use crate::think;
use crate::Pos;
use std::collections::{BTreeSet, VecDeque};

/// How far from our capital the target capital may stand, in the wrapped
/// world distance the opening measures in. The nearest rival capital on our
/// landmass in the surviving Deity Small Continents runs: p50 15, p75 17,
/// p90 19 tiles (93 games); twenty covers 85 of them.
pub(crate) const FIRST_CAPITAL_REACH_TILES: i32 = 20;

/// The most wall HP the target may hold when it is named, and while the
/// opening stands, without a Siege Tower to hand: Ancient Walls (100) or
/// none.
pub(crate) const FIRST_CAPITAL_WALL_CEILING: i32 = 100;

/// The most wall HP a Siege Tower still opens: Medieval Walls (200).
pub(crate) const FIRST_CAPITAL_TOWER_CEILING: i32 = 200;

/// The last standard turn a first capital can be named: 63 on Online. The
/// nearest land capital was first charted by t40 in 35 of 93 games, by t50
/// in 40 and by t60 in 45.
pub(crate) const FIRST_CAPITAL_NAMING_DEADLINE: u32 = 95;

/// Standard turns of preparation from the turn the target is named (30 on
/// Online), in place of the shipped thirty (20 on Online). A melee-led force
/// of six walks fifteen tiles in about eight Online turns.
pub(crate) const FIRST_CAPITAL_PREPARATION_TURNS: u32 = 45;

/// Shooters in the reserved force. See [`FIRST_CAPITAL_MELEE`].
pub(crate) const FIRST_CAPITAL_RANGED: usize = 2;

/// Melee bodies in the reserved force: the bodies that strike the walls
/// beside the Ram and take the centre. Four and two keep the shipped total.
pub(crate) const FIRST_CAPITAL_MELEE: usize = 4;

const _: () =
    assert!(FIRST_CAPITAL_RANGED + FIRST_CAPITAL_MELEE == CONQUEST_RANGED + CONQUEST_MELEE);

/// The cities the empire holds before the capital's production is reserved
/// for the force and its Settlers deferred: three, not the shipped two, so the
/// capital still trains the first two Settlers of a Deity start.
pub(crate) const FIRST_CAPITAL_SETTLER_CITIES: usize = 3;

/// A body standing this close to one of our cities may be that city's guard.
pub(crate) const FIRST_CAPITAL_GUARD_RADIUS: i32 = 1;

/// How far past the reach the charted walk may wander, so a walk around a
/// lake or a mountain still counts.
pub(crate) const FIRST_CAPITAL_WALK_SLACK: i32 = 6;

/// After a capture, field-army bodies this close to the captured city join
/// the force that carries the campaign on.
pub(crate) const FIRST_CAPITAL_REINFORCE_RADIUS: i32 = 8;

impl AdvancedAi {
    /// Whether this seat plays the gene now: the flag, the opening it rides
    /// on, and the Domination lane.
    pub(super) fn first_capital_active(&self) -> bool {
        self.first_capital_before_the_walls
            && self.early_conquest_opening
            && self.victory_target == Some(VictoryTarget::Domination)
    }

    /// The reserved force as `(ranged, melee)`. The stepping stone of
    /// `city-state-before-the-first-capital` reserves its own.
    pub(super) fn conquest_quota(&self) -> (usize, usize) {
        if self.city_state_stone_stands() {
            (
                super::city_state_before_the_first_capital::STONE_RANGED,
                super::city_state_before_the_first_capital::STONE_MELEE,
            )
        } else if self.first_capital_active() {
            (FIRST_CAPITAL_RANGED, FIRST_CAPITAL_MELEE)
        } else {
            (CONQUEST_RANGED, CONQUEST_MELEE)
        }
    }

    /// The bodies in the reserved force: the shipped six, or the stepping
    /// stone's five.
    pub(super) fn conquest_force_size(&self) -> usize {
        let (ranged, melee) = self.conquest_quota();
        ranged + melee
    }

    /// The cities the empire holds before the capital's production is
    /// reserved and its Settlers deferred.
    pub(super) fn conquest_first_settler_cities(&self) -> usize {
        if self.city_state_stone_stands() {
            super::city_state_before_the_first_capital::STONE_CITIES
        } else if self.first_capital_active() {
            FIRST_CAPITAL_SETTLER_CITIES
        } else {
            super::early_conquest::CONQUEST_FIRST_SETTLER_CITIES
        }
    }

    /// The standard-turn naming deadline the opening reads.
    pub(super) fn conquest_naming_floor(&self) -> u32 {
        if self.first_capital_active() {
            FIRST_CAPITAL_NAMING_DEADLINE
        } else {
            super::early_conquest::CONQUEST_COMMIT_DEADLINE
        }
    }

    /// The preparation window, in turns at this game's speed. The stepping
    /// stone keeps the shipped window.
    pub(super) fn conquest_preparation(&self, g: &Game) -> u32 {
        g.standard_duration(if self.first_capital_active() && !self.city_state_stone_stands() {
            FIRST_CAPITAL_PREPARATION_TURNS
        } else {
            super::early_conquest::CONQUEST_MIN_PREPARATION_TURNS
        })
    }

    /// Our capital, or `None` before it is founded.
    fn first_capital_home(g: &Game, pid: usize) -> Option<Pos> {
        g.player_city_ids(pid)
            .into_iter()
            .map(|cid| &g.cities[&cid])
            .find(|city| city.is_capital)
            .map(|city| city.pos)
    }

    /// `rival`'s original capital while they still hold it.
    fn first_capital_of(g: &Game, rival: usize) -> Option<u32> {
        g.player_city_ids(rival)
            .into_iter()
            .find(|cid| g.cities[cid].is_capital && g.cities[cid].original_owner == rival)
    }

    /// `rival`'s capital when it is a target, or the first reason it is not.
    /// The order is the order the reasons are checked in, so a journal line
    /// names the first one that holds.
    fn first_capital_check(
        &self,
        g: &Game,
        pid: usize,
        home: Pos,
        rival: usize,
    ) -> Result<u32, &'static str> {
        let player = &g.players[rival];
        if rival == pid || !player.alive || player.is_minor || player.is_barbarian {
            return Err("not a major rival");
        }
        if !g.has_met(pid, rival) || !self.campaign_target_legal(g, pid, rival) {
            return Err("not met or not a legal target");
        }
        let Some(city) = Self::first_capital_of(g, rival) else {
            return Err("they no longer hold their original capital");
        };
        let pos = g.cities[&city].pos;
        if !g.players[pid].explored.contains(&pos) {
            return Err("their capital is not charted");
        }
        if g.wdist(home, pos) > FIRST_CAPITAL_REACH_TILES {
            return Err("their capital is past the reach");
        }
        let walls = g.city_max_wall_hp(&g.cities[&city]);
        if walls > FIRST_CAPITAL_TOWER_CEILING {
            return Err("their capital stands behind more than Medieval Walls");
        }
        if walls > FIRST_CAPITAL_WALL_CEILING && !Self::first_capital_tower_known(g, pid) {
            return Err(
                "their capital stands behind Medieval Walls and we cannot train a Siege Tower",
            );
        }
        if !Self::first_capital_walk_is_charted(g, pid, rival, home, pos) {
            return Err("no charted land walk reaches their capital");
        }
        Ok(city)
    }

    /// ⭐ The target: the nearest rival capital that passes every test in
    /// [`Self::first_capital_check`], ties to the lower city id.
    pub(super) fn first_capital_target(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        let home = Self::first_capital_home(g, pid)?;
        (0..g.players.len())
            .filter_map(|rival| {
                let city = self.first_capital_check(g, pid, home, rival).ok()?;
                Some((g.wdist(home, g.cities[&city].pos), city, rival))
            })
            .min()
            .map(|(_, city, rival)| (rival, city))
    }

    /// Whether a walk from `home` reaches a tile beside `target` over ground
    /// this seat has charted, dry and passable, crossing only unowned land,
    /// our own, `rival`'s, a civilization's we are at war with, or one whose
    /// borders are open to us (the `denial-needs-a-road` rule), and staying
    /// within [`FIRST_CAPITAL_REACH_TILES`] plus [`FIRST_CAPITAL_WALK_SLACK`]
    /// of home. Uncharted ground does not count: on the live mirror it reads
    /// as default land, so a capital across a strait would otherwise pass.
    pub(super) fn first_capital_walk_is_charted(
        g: &Game,
        pid: usize,
        rival: usize,
        home: Pos,
        target: Pos,
    ) -> bool {
        let explored = &g.players[pid].explored;
        let bound = FIRST_CAPITAL_REACH_TILES + FIRST_CAPITAL_WALK_SLACK;
        let walkable = |pos: Pos| {
            if pos != home && !explored.contains(&pos) {
                return false;
            }
            if g.wdist(home, pos) > bound {
                return false;
            }
            let Some(tile) = g.map.get(pos) else {
                return false;
            };
            if !g.rules.is_passable(tile) || g.rules.is_water(tile) {
                return false;
            }
            match tile
                .owner_city
                .and_then(|cid| g.cities.get(&cid))
                .map(|city| city.owner)
            {
                None => true,
                Some(owner) => {
                    owner == pid
                        || owner == rival
                        || g.is_at_war(pid, owner)
                        || g.has_open_borders(pid, owner)
                }
            }
        };
        let mut seen: BTreeSet<Pos> = BTreeSet::from([home]);
        let mut queue: VecDeque<Pos> = VecDeque::from([home]);
        while let Some(pos) = queue.pop_front() {
            if g.wdist(pos, target) <= 1 {
                return true;
            }
            for next in g.nbrs(pos) {
                if seen.insert(next) && walkable(next) {
                    queue.push_back(next);
                }
            }
        }
        false
    }

    /// Journal, once every few turns, why no first capital is named: the
    /// nearest charted rival capital and the first test it fails.
    pub(super) fn first_capital_note_no_target(&mut self, g: &Game, pid: usize) {
        let note_due = self.conquest_near_noted.is_none_or(|noted| {
            g.turn
                >= noted
                    + g.standard_duration(super::early_conquest::CONQUEST_FEASIBILITY_NOTE_TURNS)
        });
        if !note_due {
            return;
        }
        let Some(home) = Self::first_capital_home(g, pid) else {
            return;
        };
        let nearest = (0..g.players.len())
            .filter(|rival| *rival != pid && g.players[*rival].alive && !g.players[*rival].is_minor)
            .filter_map(|rival| {
                let city = Self::first_capital_of(g, rival)?;
                let pos = g.cities[&city].pos;
                g.players[pid]
                    .explored
                    .contains(&pos)
                    .then(|| (g.wdist(home, pos), city, rival))
            })
            .min();
        self.conquest_near_noted = Some(g.turn);
        match nearest {
            None => {
                think!(self.journal(), Military, Strategy,
                       "First capital before the walls: no target yet";
                       "no rival capital is charted; the search goes on");
            }
            Some((distance, city, rival)) => {
                let why = self
                    .first_capital_check(g, pid, home, rival)
                    .err()
                    .unwrap_or("another opening stands");
                think!(self.journal(), Military, Strategy,
                       "First capital before the walls: not {}", g.cities[&city].name;
                       "{} tiles from our capital, walls {}: {why}",
                       distance, g.city_max_wall_hp(&g.cities[&city]));
            }
        }
    }

    /// Each of our cities' guard: the field-army body (a land shooter or
    /// melee body) on or beside it, nearest first, a shooter before a melee
    /// body, then the lower id. One body guards one city. Empty with the
    /// gene off (and under `city-state-before-the-first-capital`'s stepping
    /// stone too).
    pub(super) fn first_capital_home_guards(&self, g: &Game, pid: usize) -> BTreeSet<u32> {
        let mut guards = BTreeSet::new();
        if !self.first_capital_active() && !self.city_state_stone_stands() {
            return guards;
        }
        let escorts = self.all_reserved_civilian_guards();
        let army: Vec<u32> = self
            .campaign_field_army(g, pid)
            .into_iter()
            .filter(|uid| !escorts.contains(uid))
            .filter(|uid| {
                let spec = &g.rules.units[g.units[uid].kind];
                Self::conquest_ranged_body(spec) || Self::conquest_melee_body(spec)
            })
            .collect();
        for cid in g.player_city_ids(pid) {
            let pos = g.cities[&cid].pos;
            let guard = army
                .iter()
                .copied()
                .filter(|uid| !guards.contains(uid))
                .filter(|uid| g.wdist(g.units[uid].pos, pos) <= FIRST_CAPITAL_GUARD_RADIUS)
                .min_by_key(|uid| {
                    let spec = &g.rules.units[g.units[uid].kind];
                    (
                        g.wdist(g.units[uid].pos, pos),
                        !Self::conquest_ranged_body(spec),
                        *uid,
                    )
                });
            guards.extend(guard);
        }
        guards
    }

    /// The reserved bodies still missing, as `(ranged, melee)`, with the
    /// city guards taken out of the census first, so the force is trained on
    /// top of them.
    pub(super) fn first_capital_shortfall(
        &self,
        g: &Game,
        pid: usize,
        counts: &EmpireCounts,
    ) -> (usize, usize) {
        let (mut ranged_guards, mut melee_guards) = (0, 0);
        for uid in self.first_capital_home_guards(g, pid) {
            let spec = &g.rules.units[g.units[&uid].kind];
            if Self::conquest_ranged_body(spec) {
                ranged_guards += 1;
            } else {
                melee_guards += 1;
            }
        }
        let melee = counts
            .melee
            .saturating_sub(counts.scouts)
            .saturating_sub(melee_guards);
        let ranged = counts.ranged.saturating_sub(ranged_guards);
        let (want_ranged, want_melee) = self.conquest_quota();
        (
            want_ranged.saturating_sub(ranged),
            want_melee.saturating_sub(melee),
        )
    }

    /// Whether we know the technology that trains a Siege Tower.
    pub(super) fn first_capital_tower_known(g: &Game, pid: usize) -> bool {
        g.rules
            .units
            .get("siege_tower")
            .and_then(|spec| spec.tech)
            .is_some_and(|tech| g.players[pid].techs.contains(&tech))
    }

    /// Whether our own friendship offer to `other` is withheld: a declared
    /// friendship forbids a war declaration while it lasts, and on live Deity
    /// boards 3 of 13 charted land capitals within 20 tiles at t45-55 were
    /// already Declared Friends of the seat (London, Aduatuca, Seoul). True
    /// while an opening stands on `other`, or while none stands, the window
    /// is open, and `other`'s capital is the target the scan would name.
    /// `false` with the gene off.
    pub(super) fn first_capital_spares_friendship(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
    ) -> bool {
        if !self.first_capital_active() {
            return false;
        }
        // `city-state-before-the-first-capital`: the stepping stone is not
        // the capital opening; the capital it precedes is still spared.
        if let Some(opening) = self
            .conquest_opening
            .as_ref()
            .filter(|_| !self.city_state_stone_stands())
        {
            return opening.target == other;
        }
        !self.conquest_closed
            && g.turn < g.standard_duration(FIRST_CAPITAL_NAMING_DEADLINE)
            && self
                .first_capital_target(g, pid)
                .is_some_and(|(rival, _)| rival == other)
    }

    /// Siege Towers we hold or have queued.
    pub(super) fn first_capital_towers_held(g: &Game, pid: usize) -> usize {
        let standing = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && unit.kind.as_str() == "siege_tower")
            .count();
        let queued = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                g.cities[cid].queue.iter().any(
                    |item| matches!(item, Item::Unit { unit } if unit.as_str() == "siege_tower"),
                )
            })
            .count();
        standing + queued
    }

    /// Why an opening that has taken nothing stands down on the walls, or
    /// `None`: past Medieval Walls, or past Ancient Walls with no Siege
    /// Tower held or queued. `None` with the gene off.
    pub(super) fn first_capital_walled_out(
        &self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
    ) -> Option<&'static str> {
        if !self.first_capital_active() || opening.taken > 0 {
            return None;
        }
        let walls = g.city_max_wall_hp(g.cities.get(&opening.city)?);
        if walls <= FIRST_CAPITAL_WALL_CEILING {
            return None;
        }
        if walls > FIRST_CAPITAL_TOWER_CEILING {
            return Some("the target walled up past Medieval Walls before it fell");
        }
        (Self::first_capital_towers_held(g, pid) == 0 && !Self::first_capital_tower_known(g, pid))
            .then_some(
                "the target walled up past Ancient Walls and we can neither field nor train a \
                 Siege Tower",
            )
    }

    /// After a capture, the surviving force plus the field army (not a city
    /// guard, not an escort) within [`FIRST_CAPITAL_REINFORCE_RADIUS`] of the
    /// captured city, nearest first, up to the reserved total.
    pub(super) fn first_capital_reinforced_force(
        &self,
        g: &Game,
        pid: usize,
        survivors: &BTreeSet<u32>,
        taken: Pos,
    ) -> BTreeSet<u32> {
        let (ranged, melee) = self.conquest_quota();
        let mut force = survivors.clone();
        let guards = self.first_capital_home_guards(g, pid);
        let escorts = self.all_reserved_civilian_guards();
        let mut army: Vec<u32> = self
            .campaign_field_army(g, pid)
            .into_iter()
            .filter(|uid| !force.contains(uid) && !guards.contains(uid) && !escorts.contains(uid))
            .filter(|uid| g.wdist(g.units[uid].pos, taken) <= FIRST_CAPITAL_REINFORCE_RADIUS)
            .collect();
        army.sort_by_key(|uid| (g.wdist(g.units[uid].pos, taken), *uid));
        for uid in army {
            if force.len() >= ranged + melee {
                break;
            }
            force.insert(uid);
        }
        force
    }
}

#[cfg(test)]
mod tests;
