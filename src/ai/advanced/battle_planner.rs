//! The battle planner: a force's turn planned jointly — the danger field,
//! the kill plan, the heal rotation.
//!
//! The deployed controller plays its units one at a time down a ladder
//! (`advanced_military_step_with_decline`): each unit prices its own attack
//! from where it stands, on an exact clone, with the force's `focus_target`
//! as the one shared hint. `fire-plan` (`fire_plan.rs`) orders that loop so
//! the shooters that can finish a target go first, and `swap-rotation`
//! (`swap_rotation.rs`) trades a wounded front-liner for the fresh unit
//! behind it. Both leave the unit-at-a-time decision in place. What no
//! controller here has done is plan the turn as one decision: which blows,
//! from which tiles, in which order, and which units sit the turn out to
//! heal — priced against what the enemy can do back next turn.
//!
//! This gene does that, in three parts, each its own function with its own
//! tests, and each built on the engine's own arithmetic rather than a copy:
//!
//! 1. **The danger field.** `danger(tile, unit)` is the damage every visible
//!    hostile that can reach and attack `tile` next turn would do to our unit
//!    standing there unfortified, plus the strike of every walled enemy city
//!    or Encampment within two tiles. The reach is `Game::attack_reach`; the
//!    blows are `melee_exchange_strengths` / `ranged_strike_strengths` and
//!    `expected_damage`, evaluated on one speculative probe of the board with
//!    our unit relocated onto the tile — so matchup, terrain, river, support
//!    and promotions are the engine's, not a re-derivation. Cached per
//!    (tile, unit) for the frame.
//! 2. **The kill plan.** Every legal strike each of our units could make —
//!    from its own tile, or after a move to a reachable tile (never a siege
//!    unit that moved) — is a candidate priced with the exact pair. A beam
//!    search (width `BEAM_WIDTH`, at most `MAX_SHOOTERS` shooters and
//!    `MAX_TARGETS` targets) finds the ordered sequence maximising kill value
//!    minus return damage, minus the danger of every end tile, minus a
//!    penalty for a target left at 1–30 hit points with a finisher to spare.
//!    A kill counts only when the cumulative expected damage reaches the hit
//!    points times `KILL_MARGIN`; ranged blows go before a melee finisher;
//!    the value scale is `tactical_attack_result_in`'s, so the gene's
//!    numbers read against the ladder's. Three vetoes: no blow whose return
//!    kills the attacker unless it completes a kill worth more than the
//!    attacker; no blow from a unit under `WOUNDED_STRIKER_HP` unless it is
//!    the finishing blow and its end tile is safe; and no melee into a walled
//!    city — cities are not targets here at all (see below). The chosen
//!    sequence is then replayed on ONE speculative clone through
//!    `tactical_attack_result_in` and blows the clone refuses are dropped,
//!    before the survivors are applied in order, moves first.
//! 3. **The heal rotation.** A unit under `ROTATE_HP`, or standing where the
//!    danger exceeds its hit points minus `ROTATE_DANGER_MARGIN`, that is
//!    not a planned striker moves to the nearest reachable tile with no
//!    danger — preferring a City Center or district, friendly ground, and a
//!    tile beside an `adjacent_heal` support unit — and fortifies. It is then
//!    kept out of the kill plan until it is back at `RETURN_HP`
//!    (`battle_planner_recovering`, cleared on death). Where a fresh friend
//!    stands adjacent, further from the enemy and `Action::Swap` is legal,
//!    the swap is preferred so the tile stays held.
//!
//! Units the plan has ordered are marked (`battle_planner_ordered`) and the
//! per-unit ladder leaves them alone for the turn; everyone else plays the
//! ladder exactly as before. `coordinated_tactical_step` — the movement of
//! units that are not striking — is untouched in this change; the module is
//! shaped so a `positions_plan` can join the three parts later.
//!
//! **Cities are not kill-plan targets.** A blow on a City Center is priced
//! by `city_take_damage` — walls absorb a share that depends on the wall
//! fraction and the attacker's class — and its value is progress, not a
//! kill. The ladder already prices that on an exact clone; a siege unit
//! within range of an enemy city is therefore left to the ladder rather than
//! spent on a unit shot, and the walls veto is satisfied by construction.
//! Pricing the assault jointly is the follow-up this module leaves room for.
//!
//! **Arenas that do not heal.** `healing_step` refuses recovery on a
//! Tactics board with healing off, because nothing recovers there and a
//! unit pulled out never comes back. The rotation follows the same rule: on
//! such a board only the lethal trigger fires — a unit the field says would
//! be removed next turn still steps out of reach — and nothing is
//! remembered, so a unit is never walked out of reach one turn and back
//! into it the next.
//!
//! `Kind::OptIn`, off in `AdvancedAi::new()` and `legacy()`, byte-identical
//! when off: `plan_battle` returns before it reads the board, the ordered
//! set is empty, and the ladder's own `prioritize_immediate_kills` and
//! `plan_fire` run as they always have. Priced on the arena first
//! (`battle_bench`, `doctrine_arena --a advanced+battle-planner`); the
//! whole-game screen is the no-harm check (`docs/DOCTRINE_ARENA.md`, "The
//! gate for a tactical gene").

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{AdvancedAi, AppliedAttack, StrategicPlan};
use crate::game::{effective_strength, expected_damage, Action, Game};
use crate::think;
use crate::Pos;

/// Planned damage over the target's hit points before a kill is counted.
/// The engine's roll is uniform on 0.8–1.2 of the centre, so a plan at the
/// centre fails half the time; fifteen percent is the same margin
/// `fire_plan` chose, and the same reason.
pub(super) const KILL_MARGIN: f64 = 1.15;
/// States kept at each depth of the sequence search.
pub(super) const BEAM_WIDTH: usize = 32;
/// Shooters a plan may spend, and the search depth.
pub(super) const MAX_SHOOTERS: usize = 12;
/// Targets a plan considers at once.
pub(super) const MAX_TARGETS: usize = 8;
/// Stands kept per (shooter, target): the unit's own tile, then the safest.
const FROM_TILES_PER_PAIR: usize = 3;
/// How much of next turn's expected damage a striker's end tile costs, on
/// the ladder's attacker-loss scale. Half: the enemy has to choose to spend
/// its blows there, and the ladder's own reply search prices a forcing reply
/// at `trade_caution`, not at par.
const DANGER_WEIGHT: f64 = 0.5;
/// A unit under this strikes only to finish a kill, and only from safety.
pub(super) const WOUNDED_STRIKER_HP: i32 = 50;
/// A unit under this is rotated out to heal where the board heals.
pub(super) const ROTATE_HP: i32 = 50;
/// A rotated unit rejoins the kill plan at this.
pub(super) const RETURN_HP: i32 = 80;
/// Danger above `hp - this` rotates a unit whatever its hit points.
const ROTATE_DANGER_MARGIN: i32 = 20;
/// A target left at or under this, with a finisher to spare, is a plan that
/// chipped: the penalty says finish it or leave it whole.
const UNFINISHED_HP: f64 = 30.0;
const UNFINISHED_PENALTY_SHARE: f64 = 0.35;
/// The relief in a swap has to be this much healthier; `swap_rotation`'s
/// `ROTATION_HP_MARGIN`, for the same reason.
const RELIEF_HP_MARGIN: i32 = 25;
/// Danger this small is none: the field sums exact blows, so zero is zero.
const NO_DANGER: f64 = 1e-9;

/// A kill's worth, on `tactical_attack_result_in`'s scale.
fn unit_value(cost: f64, strength: f64, siege: bool, captures: bool) -> f64 {
    190.0
        + cost * 0.45
        + strength * 1.8
        + if siege { 65.0 } else { 0.0 }
        + if captures { 30.0 } else { 0.0 }
}

/// Damage that leaves the defender standing, on the same scale.
fn damage_value(damage: f64, strength: f64, siege: bool, captures: bool) -> f64 {
    damage * (1.0 + strength / 100.0)
        + if siege { 18.0 } else { 0.0 }
        + if captures { 6.0 } else { 0.0 }
}

/// Hit points our attacker gives up, on the same scale.
fn loss_value(hp_lost: f64, cost: f64) -> f64 {
    hp_lost * (1.25 + cost / 800.0)
}

/// Our attacker removed, on the same scale.
fn death_value(cost: f64) -> f64 {
    230.0 + cost * 0.65
}

/// The term `effective_strength` takes off a wounded unit. Used only to
/// recover a strength's base from the engine's effective reading, so the
/// same base can be re-read at a planned hit-point total through
/// `effective_strength` itself.
fn wounded_penalty(hp: i32) -> f64 {
    (10.0 - hp.clamp(0, 100) as f64 / 10.0).round()
}

/// Each source's expected blow on one of our units at one tile: `None` is a
/// city or Encampment strike; a unit source is named so a plan that kills it
/// can leave its blow out.
type Blows = Arc<Vec<(Option<u32>, f64)>>;

/// The danger field for one frame of the board: what every visible hostile
/// could do to one of our units on a tile next turn.
///
/// Owns a speculative probe of the board so a unit can be stood on a tile,
/// unfortified, and priced there by the engine's own pair functions, then
/// put back. The same probe stands a shooter on a candidate tile for the
/// kill plan's legality and strength reads.
pub(super) struct DangerField {
    pid: usize,
    probe: Game,
    /// Every visible hostile that can fight, with the tiles it can strike
    /// next turn (`Game::attack_reach`: full movement, read through units),
    /// ascending and distinct so membership is a binary search.
    reaches: Vec<(u32, Vec<Pos>)>,
    /// (tile, our unit) → each source's expected blow there.
    cache: BTreeMap<(Pos, u32), Blows>,
}

impl DangerField {
    pub(super) fn new(g: &Game, pid: usize) -> Self {
        let mut reaches = Vec::new();
        for unit in g.units.values() {
            let spec = &g.rules.units[unit.kind];
            if unit.owner == pid
                || !g.is_at_war(pid, unit.owner)
                || spec.class != "military"
                || !(spec.is_melee_capable() || spec.has_ranged_attack())
                || !g.unit_visible_to(unit.id, pid)
            {
                continue;
            }
            let reach = g.attack_reach(unit.id);
            if !reach.is_empty() {
                reaches.push((unit.id, reach));
            }
        }
        reaches.sort_by_key(|(id, _)| *id);
        DangerField {
            pid,
            probe: g.speculative_clone(),
            reaches,
            cache: BTreeMap::new(),
        }
    }

    /// Every blow that would land on `uid` standing unfortified on `tile`
    /// next turn, by source.
    pub(super) fn contributions(&mut self, tile: Pos, uid: u32) -> Blows {
        if let Some(hit) = self.cache.get(&(tile, uid)) {
            return Arc::clone(hit);
        }
        let Some(saved) = self.probe.units.get(&uid).cloned() else {
            return Arc::new(Vec::new());
        };
        let mut out = Vec::new();
        // A garrison is not a combat target: blows on a City Center or an
        // Encampment damage the district, not the unit inside it.
        let garrisoned =
            self.probe.city_at(tile).is_some() || self.probe.encampment_at(tile).is_some();
        if !garrisoned {
            self.probe.relocate(uid, tile);
            if let Some(unit) = self.probe.units.get_mut(&uid) {
                unit.fortified = false;
                unit.fortify_turns = 0;
            }
            for (enemy, reach) in &self.reaches {
                if reach.binary_search(&tile).is_err() {
                    continue;
                }
                let Some(attacker) = self.probe.units.get(enemy) else {
                    continue;
                };
                let pair = if self.probe.rules.units[attacker.kind].has_ranged_attack() {
                    self.probe.ranged_strike_strengths(*enemy, uid, tile)
                } else {
                    self.probe.melee_exchange_strengths(*enemy, uid)
                };
                if let Some((att, def)) = pair {
                    out.push((Some(*enemy), expected_damage(att, def)));
                }
            }
            // A walled city or a standing Encampment strikes within two tiles
            // on its next turn whether or not it fired this one. The defence
            // is `do_city_strike`'s: the unit's own strength on the tile plus
            // the tile's defence.
            let defence = {
                let standing = &self.probe.units[&uid];
                effective_strength(
                    self.probe.unit_strength(standing, true) + self.probe.tile_defense_bonus(tile),
                    standing.hp,
                )
            };
            let mut cities = BTreeSet::new();
            let mut encampments = BTreeSet::new();
            for pos in self.probe.wdisk(tile, 2) {
                if let Some(cid) = self.probe.city_at(pos) {
                    let city = &self.probe.cities[&cid];
                    if city.owner != self.pid
                        && self.probe.is_at_war(self.pid, city.owner)
                        && city.wall_hp > 0
                        && cities.insert(cid)
                    {
                        out.push((
                            None,
                            expected_damage(self.probe.city_ranged_strength(cid), defence),
                        ));
                    }
                }
                if let Some(cid) = self.probe.encampment_at(pos) {
                    let city = &self.probe.cities[&cid];
                    if city.owner != self.pid
                        && self.probe.is_at_war(self.pid, city.owner)
                        && self.probe.encampment_can_strike(city)
                        && encampments.insert(cid)
                    {
                        out.push((
                            None,
                            expected_damage(self.probe.city_ranged_strength(cid), defence),
                        ));
                    }
                }
            }
            self.probe.relocate(uid, saved.pos);
            if let Some(unit) = self.probe.units.get_mut(&uid) {
                *unit = saved;
            }
        }
        let out = Arc::new(out);
        self.cache.insert((tile, uid), Arc::clone(&out));
        out
    }

    /// The whole field at one tile.
    pub(super) fn danger(&mut self, tile: Pos, uid: u32) -> f64 {
        self.contributions(tile, uid)
            .iter()
            .map(|(_, blow)| blow)
            .sum()
    }

    /// The field with the named enemies already dead.
    fn danger_without(&mut self, tile: Pos, uid: u32, dead: &BTreeSet<u32>) -> f64 {
        self.contributions(tile, uid)
            .iter()
            .filter(|(source, _)| source.is_none_or(|id| !dead.contains(&id)))
            .map(|(_, blow)| blow)
            .sum()
    }

    /// Stand `uid` on `to` by an actual `MoveTo` on the probe, read the board
    /// there, and put it back. `None` when the move is refused or stops
    /// short — then the stand is not one this unit can take this turn. The
    /// unit's remaining movement, `moved` flag and zone-of-control state on
    /// the probe are the engine's, so the legality predicates read there are
    /// the ones the real move would face.
    fn at_stand<R>(&mut self, uid: u32, to: Pos, read: impl FnOnce(&Game) -> R) -> Option<R> {
        let saved = self.probe.units.get(&uid).cloned()?;
        if saved.pos == to {
            return Some(read(&self.probe));
        }
        let moved = self
            .probe
            .apply(self.pid, &Action::MoveTo { unit: uid, to })
            .is_ok()
            && self
                .probe
                .units
                .get(&uid)
                .is_some_and(|unit| unit.pos == to);
        let out = moved.then(|| read(&self.probe));
        // Put the unit back whatever happened: a refused move may still have
        // walked part of the way.
        if let Some(unit) = self.probe.units.get(&uid) {
            if unit.pos != saved.pos {
                self.probe.relocate(uid, saved.pos);
            }
        }
        if let Some(unit) = self.probe.units.get_mut(&uid) {
            *unit = saved;
        }
        out
    }
}

/// The danger field at one tile for one of our units, on a fresh frame.
/// `DangerField` is the cached form the planner uses; this is the reading
/// alone, for tests and explainers.
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn danger(g: &Game, pid: usize, tile: Pos, uid: u32) -> f64 {
    DangerField::new(g, pid).danger(tile, uid)
}

/// One of our units that could strike this turn.
#[derive(Clone, Debug)]
struct Shooter {
    uid: u32,
    pos: Pos,
    hp: i32,
    cost: f64,
    /// What we would lose if it died, on the kill scale — the bar a kill has
    /// to clear before this unit is spent for it.
    value: f64,
}

/// One hostile unit the plan could finish: the strongest military defender
/// on its tile, as the engine will resolve the blow.
#[derive(Clone, Debug)]
struct Target {
    uid: u32,
    pos: Pos,
    hp: i32,
    /// Its defending strength, unwounded, for the value scale.
    strength: f64,
    kill_value: f64,
    siege: bool,
    captures: bool,
}

/// One legal blow: a shooter, from a stand, at a target, with the exact
/// pair read there. `att` is our attacker's effective strength (its hit
/// points do not move within the plan); `def_base` is the defender's
/// strength before the wound, re-read at each planned hit-point total.
#[derive(Clone, Debug)]
struct Candidate {
    shooter: usize,
    target: usize,
    from: Pos,
    ranged: bool,
    att: f64,
    def_base: f64,
}

/// A blow the plan has chosen, in the words the board needs.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Blow {
    pub unit: u32,
    pub from: Pos,
    pub target: Pos,
    pub defender: u32,
    pub ranged: bool,
    /// The centre-roll damage the plan expects from this blow.
    pub expected: f64,
    /// Whether the plan expects this blow to finish the defender.
    pub finishes: bool,
}

/// One partial sequence in the beam.
#[derive(Clone, Debug)]
struct BeamState {
    blows: Vec<usize>,
    spent: u32,
    /// End tiles taken by movers and by melee finishers stepping into the
    /// tile they emptied.
    occupied: Vec<Pos>,
    dealt: Vec<f64>,
    killed: u32,
    /// Targets a melee blow has landed on: no ranged blow follows it there.
    melee_on: u32,
    score: f64,
}

impl BeamState {
    fn empty(targets: usize) -> Self {
        BeamState {
            blows: Vec::new(),
            spent: 0,
            occupied: Vec::new(),
            dealt: vec![0.0; targets],
            killed: 0,
            melee_on: 0,
            score: 0.0,
        }
    }

    /// The targets a kill mask has finished, by unit id.
    fn dead_of(killed: u32, targets: &[Target]) -> BTreeSet<u32> {
        targets
            .iter()
            .enumerate()
            .filter(|(index, _)| killed & (1 << index) != 0)
            .map(|(_, target)| target.uid)
            .collect()
    }

    /// The sequence with `candidate` appended, or `None` where a veto or a
    /// conflict refuses it.
    fn extend(
        &self,
        index: usize,
        candidate: &Candidate,
        shooters: &[Shooter],
        targets: &[Target],
        field: &mut DangerField,
    ) -> Option<BeamState> {
        let shooter = &shooters[candidate.shooter];
        let target = &targets[candidate.target];
        let shooter_bit = 1u32 << candidate.shooter;
        let target_bit = 1u32 << candidate.target;
        if self.spent & shooter_bit != 0 || self.killed & target_bit != 0 {
            return None;
        }
        if candidate.ranged && self.melee_on & target_bit != 0 {
            return None;
        }
        let moves = candidate.from != shooter.pos;
        if moves && self.occupied.contains(&candidate.from) {
            return None;
        }
        let remaining = (f64::from(target.hp) - self.dealt[candidate.target]).max(1.0);
        let def = effective_strength(candidate.def_base, remaining.round() as i32);
        let damage = expected_damage(candidate.att, def);
        let dealt = self.dealt[candidate.target] + damage;
        let kill = dealt >= f64::from(target.hp) * KILL_MARGIN;
        let gain = if kill {
            target.kill_value
        } else {
            damage_value(
                damage.min(remaining),
                target.strength,
                target.siege,
                target.captures,
            )
        };
        let mut cost = 0.0;
        let mut returned = 0.0;
        let mut dies = false;
        if !candidate.ranged {
            returned = expected_damage(def, candidate.att);
            if returned >= f64::from(shooter.hp) {
                // The return kills the attacker: only for a kill worth more
                // than the unit, and then at the price of the unit.
                if !(kill && target.kill_value > shooter.value) {
                    return None;
                }
                dies = true;
                cost += death_value(shooter.cost);
            } else {
                cost += loss_value(returned, shooter.cost);
            }
        }
        let mut killed = self.killed;
        if kill {
            killed |= target_bit;
        }
        // A melee finisher steps into the tile it emptied; everyone else
        // ends where it stood to strike.
        let end = if !candidate.ranged && kill {
            target.pos
        } else {
            candidate.from
        };
        let mut danger = 0.0;
        if !dies {
            let dead = Self::dead_of(killed, targets);
            danger = field.danger_without(end, shooter.uid, &dead);
            if danger >= f64::from(shooter.hp) - returned {
                cost += death_value(shooter.cost);
            } else {
                cost += DANGER_WEIGHT * loss_value(danger, shooter.cost);
            }
        }
        // A wounded unit strikes only to finish, only from safety, and
        // never as a trade of itself.
        if shooter.hp < WOUNDED_STRIKER_HP && (dies || !(kill && danger <= NO_DANGER)) {
            return None;
        }
        let mut next = self.clone();
        next.blows.push(index);
        next.spent |= shooter_bit;
        if moves || (!candidate.ranged && kill) {
            next.occupied.push(end);
        }
        next.dealt[candidate.target] = dealt;
        next.killed = killed;
        if !candidate.ranged {
            next.melee_on |= target_bit;
        }
        next.score = self.score + gain - cost;
        Some(next)
    }

    /// The sequence's worth once it stops: its running score less the
    /// penalty for every target it left low with a finisher still to spare.
    fn terminal(&self, shooters: &[Shooter], targets: &[Target], candidates: &[Candidate]) -> f64 {
        let mut score = self.score;
        for (index, target) in targets.iter().enumerate() {
            let bit = 1u32 << index;
            if self.killed & bit != 0 || self.dealt[index] <= 0.0 {
                continue;
            }
            let remaining = f64::from(target.hp) - self.dealt[index];
            if remaining <= 0.0 || remaining > UNFINISHED_HP {
                continue;
            }
            let finisher_spare = candidates.iter().any(|candidate| {
                candidate.target == index
                    && self.spent & (1 << candidate.shooter) == 0
                    && shooters[candidate.shooter].hp >= WOUNDED_STRIKER_HP
            });
            if finisher_spare {
                score -= UNFINISHED_PENALTY_SHARE * target.kill_value;
            }
        }
        score
    }
}

/// The ordered sequence of candidate indices with the best terminal score,
/// and that score. Empty, at zero, when no sequence beats doing nothing.
fn search_kill_sequence(
    shooters: &[Shooter],
    targets: &[Target],
    candidates: &[Candidate],
    field: &mut DangerField,
) -> (Vec<usize>, f64) {
    let mut beam = vec![BeamState::empty(targets.len())];
    let mut best: (Vec<usize>, f64) = (Vec::new(), 0.0);
    for _depth in 0..MAX_SHOOTERS.min(shooters.len()) {
        let mut next: Vec<BeamState> = Vec::new();
        for state in &beam {
            for (index, candidate) in candidates.iter().enumerate() {
                if let Some(extended) = state.extend(index, candidate, shooters, targets, field) {
                    next.push(extended);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        // Best first; equal scores by the sequence itself, so the plan is a
        // function of the board alone.
        next.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.blows.cmp(&b.blows))
        });
        next.truncate(BEAM_WIDTH);
        for state in &next {
            let terminal = state.terminal(shooters, targets, candidates);
            if terminal > best.1 + 1e-9
                || (terminal > best.1 - 1e-9 && !best.0.is_empty() && state.blows < best.0)
            {
                best = (state.blows.clone(), terminal);
            }
        }
        beam = next;
    }
    best
}

impl AdvancedAi {
    /// Whether the battle plan has already ordered this unit this turn, so
    /// the per-unit ladder leaves it where the plan put it.
    pub(super) fn battle_planner_claims(&self, uid: u32) -> bool {
        self.battle_planner_ordered.contains(&uid)
    }

    /// Plan and play the force's turn: the kill plan, then the heal
    /// rotation. `true` when a blow landed, so the caller rebuilds its force
    /// picture. Nothing is read with the gene off.
    pub(super) fn plan_battle(&mut self, g: &mut Game, pid: usize, plan: &StrategicPlan) -> bool {
        self.battle_planner_ordered.clear();
        if !self.battle_planner {
            return false;
        }
        self.battle_planner_recovering.retain(|uid| {
            g.units
                .get(uid)
                .is_some_and(|unit| unit.owner == pid && unit.hp < RETURN_HP)
        });
        let mut field = DangerField::new(g, pid);
        let blows = self.kill_sequence_in(g, pid, &mut field);
        let mut struck = false;
        let mut strikers = BTreeSet::new();
        if !blows.is_empty() {
            let planned_kills = blows.iter().filter(|blow| blow.finishes).count() as u32;
            self.census.battle_plan_kills += planned_kills;
            let (verified, verified_kills, dropped) = self.verify_blows(g, pid, plan, &blows);
            self.census.battle_plan_verified_kills += verified_kills;
            self.census.battle_plan_dropped_blows += dropped;
            self.journal_kill_plan(g, &verified);
            strikers.extend(verified.iter().map(|blow| blow.unit));
            struck = self.apply_blows(g, pid, &verified);
        }
        if struck {
            field = DangerField::new(g, pid);
        }
        let rotations = self.rotate_wounded(g, pid, &mut field, &strikers);
        self.census.battle_plan_rotations += rotations;
        struck
    }

    /// The kill plan alone — the ordered blows the search chose, before any
    /// clone has seen them. Pure: for tests and explainers.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn kill_sequence(&self, g: &Game, pid: usize) -> Vec<Blow> {
        let mut field = DangerField::new(g, pid);
        self.kill_sequence_in(g, pid, &mut field)
    }

    fn kill_sequence_in(&self, g: &Game, pid: usize, field: &mut DangerField) -> Vec<Blow> {
        let (shooters, targets, candidates) = self.strike_candidates(g, pid, field);
        if candidates.is_empty() {
            return Vec::new();
        }
        let (sequence, score) = search_kill_sequence(&shooters, &targets, &candidates, field);
        if score <= 0.0 || sequence.is_empty() {
            return Vec::new();
        }
        let mut dealt = vec![0.0; targets.len()];
        let mut blows = Vec::with_capacity(sequence.len());
        for index in sequence {
            let candidate = &candidates[index];
            let target = &targets[candidate.target];
            let remaining = (f64::from(target.hp) - dealt[candidate.target]).max(1.0);
            let def = effective_strength(candidate.def_base, remaining.round() as i32);
            let expected = expected_damage(candidate.att, def);
            dealt[candidate.target] += expected;
            blows.push(Blow {
                unit: shooters[candidate.shooter].uid,
                from: candidate.from,
                target: target.pos,
                defender: target.uid,
                ranged: candidate.ranged,
                expected,
                finishes: dealt[candidate.target] >= f64::from(target.hp) * KILL_MARGIN,
            });
        }
        blows
    }

    /// Every legal blow each eligible unit could make this turn, from its
    /// own tile or after a move, priced with the engine's pair on the probe.
    fn strike_candidates(
        &self,
        g: &Game,
        pid: usize,
        field: &mut DangerField,
    ) -> (Vec<Shooter>, Vec<Target>, Vec<Candidate>) {
        // Targets: the strongest hostile military unit on each visible tile
        // that is not a City Center or an Encampment — the defender the
        // engine will resolve against, as `fire_plan` reads it.
        let mut by_tile: BTreeMap<Pos, u32> = BTreeMap::new();
        for unit in g.units.values() {
            let spec = &g.rules.units[unit.kind];
            if unit.owner == pid
                || !g.is_at_war(pid, unit.owner)
                || spec.class != "military"
                || !g.unit_visible_to(unit.id, pid)
                || g.city_at(unit.pos).is_some()
                || g.encampment_at(unit.pos).is_some()
            {
                continue;
            }
            let strength = |id: u32| {
                let other = &g.units[&id];
                effective_strength(g.unit_strength(other, true), other.hp)
            };
            match by_tile.get(&unit.pos) {
                Some(current)
                    if strength(*current) > strength(unit.id)
                        || (strength(*current) == strength(unit.id) && *current < unit.id) => {}
                _ => {
                    by_tile.insert(unit.pos, unit.id);
                }
            }
        }
        let mut targets: Vec<Target> = by_tile
            .into_iter()
            .map(|(pos, uid)| {
                let unit = &g.units[&uid];
                let spec = &g.rules.units[unit.kind];
                let strength = g.unit_strength(unit, true);
                Target {
                    uid,
                    pos,
                    hp: unit.hp,
                    strength,
                    kill_value: unit_value(
                        spec.cost,
                        strength,
                        spec.siege,
                        spec.is_melee_capable(),
                    ),
                    siege: spec.siege,
                    captures: spec.is_melee_capable(),
                }
            })
            .collect();
        if targets.is_empty() {
            return (Vec::new(), Vec::new(), Vec::new());
        }
        let enemy_city_near = |pos: Pos, range: i32| {
            g.cities.values().any(|city| {
                city.owner != pid && g.is_at_war(pid, city.owner) && g.wdist(city.pos, pos) <= range
            })
        };
        let mut ids = g.player_unit_ids(pid);
        ids.sort_unstable();
        let frame = g.player_vision_frame(pid);
        let viewers = g.visibility_viewers(pid);
        let mut shooters: Vec<Shooter> = Vec::new();
        let mut candidates: Vec<Candidate> = Vec::new();
        for uid in ids {
            let unit = &g.units[&uid];
            let spec = &g.rules.units[unit.kind];
            if spec.class != "military"
                || spec.domain.as_deref() == Some("air")
                || unit.linked_to.is_some()
                || unit.attacks_left <= 0
                || unit.moves_left <= 0.0
                || !(spec.is_melee_capable() || spec.has_ranged_attack())
                || self.battle_planner_recovering.contains(&uid)
                || self.guard_is_bound_to_any_settler(uid)
            {
                continue;
            }
            let range = if spec.has_ranged_attack() {
                g.unit_attack_range(uid).max(1)
            } else {
                1
            };
            // A siege unit within range of an enemy city is the ladder's:
            // its shot is the wall, not a unit.
            if spec.siege && enemy_city_near(unit.pos, range + g.unit_max_moves(uid).ceil() as i32)
            {
                continue;
            }
            let shooter_index = shooters.len();
            let value = unit_value(
                spec.cost,
                g.unit_strength(unit, true),
                spec.siege,
                spec.is_melee_capable(),
            );
            let mut own: Vec<Candidate> = Vec::new();
            let mut stands: Vec<Pos> = vec![unit.pos];
            stands.extend(g.reachable(uid));
            for stand in stands {
                if !targets
                    .iter()
                    .any(|target| g.wdist(stand, target.pos) <= range)
                {
                    continue;
                }
                let read = |board: &Game| -> Vec<Candidate> {
                    let mut found = Vec::new();
                    for (target_index, target) in targets.iter().enumerate() {
                        if g.wdist(stand, target.pos) > range {
                            continue;
                        }
                        if spec.has_ranged_attack()
                            && board.ranged_order_is_legal(
                                pid,
                                uid,
                                target.pos,
                                frame.as_ref(),
                                &viewers,
                            )
                        {
                            if let Some((att, def)) =
                                board.ranged_strike_strengths(uid, target.uid, target.pos)
                            {
                                found.push(Candidate {
                                    shooter: shooter_index,
                                    target: target_index,
                                    from: stand,
                                    ranged: true,
                                    att,
                                    def_base: def + wounded_penalty(target.hp),
                                });
                            }
                        }
                        if spec.is_melee_capable()
                            && board.melee_order_is_legal(pid, uid, target.pos)
                        {
                            if let Some((att, def)) =
                                board.melee_exchange_strengths(uid, target.uid)
                            {
                                found.push(Candidate {
                                    shooter: shooter_index,
                                    target: target_index,
                                    from: stand,
                                    ranged: false,
                                    att,
                                    def_base: def + wounded_penalty(target.hp),
                                });
                            }
                        }
                    }
                    found
                };
                let found = if stand == unit.pos {
                    read(g)
                } else {
                    field.at_stand(uid, stand, read).unwrap_or_default()
                };
                own.extend(found);
            }
            if own.is_empty() {
                continue;
            }
            // Per target keep the unit's own tile and the safest stands
            // after it, so the search is not spent choosing among tiles
            // that differ only in exposure.
            let mut kept: Vec<Candidate> = Vec::new();
            for (target_index, target) in targets.iter().enumerate() {
                let mut on_target: Vec<&Candidate> = own
                    .iter()
                    .filter(|candidate| candidate.target == target_index)
                    .collect();
                if on_target.is_empty() {
                    continue;
                }
                let mut keyed: Vec<(bool, f64, f64, Pos, bool, &Candidate)> = on_target
                    .drain(..)
                    .map(|candidate| {
                        let danger = field.danger(candidate.from, uid);
                        let damage = expected_damage(
                            candidate.att,
                            effective_strength(candidate.def_base, target.hp),
                        );
                        (
                            candidate.from != unit.pos,
                            danger,
                            -damage,
                            candidate.from,
                            !candidate.ranged,
                            candidate,
                        )
                    })
                    .collect();
                keyed.sort_by(|a, b| {
                    a.0.cmp(&b.0)
                        .then_with(|| a.1.total_cmp(&b.1))
                        .then_with(|| a.2.total_cmp(&b.2))
                        .then_with(|| a.3.cmp(&b.3))
                        .then_with(|| a.4.cmp(&b.4))
                });
                let mut stands_kept: Vec<Pos> = Vec::new();
                for (_, _, _, from, _, candidate) in keyed {
                    if !stands_kept.contains(&from) {
                        if stands_kept.len() >= FROM_TILES_PER_PAIR {
                            continue;
                        }
                        stands_kept.push(from);
                    }
                    kept.push(candidate.clone());
                }
            }
            shooters.push(Shooter {
                uid,
                pos: unit.pos,
                hp: unit.hp,
                cost: spec.cost,
                value,
            });
            candidates.extend(kept);
        }
        if candidates.is_empty() {
            return (Vec::new(), Vec::new(), Vec::new());
        }
        // The most finishable targets, and the shooters with the heaviest
        // blow, within the search's bounds.
        let mut target_order: Vec<usize> = (0..targets.len())
            .filter(|index| {
                candidates
                    .iter()
                    .any(|candidate| candidate.target == *index)
            })
            .collect();
        target_order.sort_by(|a, b| {
            targets[*a]
                .hp
                .cmp(&targets[*b].hp)
                .then_with(|| targets[*b].kill_value.total_cmp(&targets[*a].kill_value))
                .then_with(|| targets[*a].uid.cmp(&targets[*b].uid))
        });
        target_order.truncate(MAX_TARGETS);
        let heaviest = |shooter: usize| {
            candidates
                .iter()
                .filter(|candidate| {
                    candidate.shooter == shooter && target_order.contains(&candidate.target)
                })
                .map(|candidate| {
                    expected_damage(
                        candidate.att,
                        effective_strength(candidate.def_base, targets[candidate.target].hp),
                    )
                })
                .fold(0.0, f64::max)
        };
        let mut shooter_order: Vec<usize> =
            (0..shooters.len()).filter(|s| heaviest(*s) > 0.0).collect();
        shooter_order.sort_by(|a, b| {
            heaviest(*b)
                .total_cmp(&heaviest(*a))
                .then_with(|| shooters[*a].uid.cmp(&shooters[*b].uid))
        });
        shooter_order.truncate(MAX_SHOOTERS);
        let mut target_map: BTreeMap<usize, usize> = BTreeMap::new();
        let mut shooter_map: BTreeMap<usize, usize> = BTreeMap::new();
        let mut kept_targets: Vec<Target> = Vec::new();
        let mut kept_shooters: Vec<Shooter> = Vec::new();
        let mut order = target_order;
        order.sort_unstable();
        for index in order {
            target_map.insert(index, kept_targets.len());
            kept_targets.push(targets[index].clone());
        }
        shooter_order.sort_unstable();
        for index in shooter_order {
            shooter_map.insert(index, kept_shooters.len());
            kept_shooters.push(shooters[index].clone());
        }
        let kept: Vec<Candidate> = candidates
            .into_iter()
            .filter_map(|mut candidate| {
                let shooter = *shooter_map.get(&candidate.shooter)?;
                let target = *target_map.get(&candidate.target)?;
                candidate.shooter = shooter;
                candidate.target = target;
                Some(candidate)
            })
            .collect();
        targets = kept_targets;
        (kept_shooters, targets, kept)
    }

    /// Replay the plan on one speculative clone, through the same exact
    /// forward model the ladder scores with, and keep the blows it accepts.
    /// Returns them with the kills the clone saw and the blows it dropped.
    fn verify_blows(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        blows: &[Blow],
    ) -> (Vec<Blow>, u32, u32) {
        let mut clone = g.speculative_clone();
        let mut verified = Vec::with_capacity(blows.len());
        let mut kills = 0u32;
        let mut dropped = 0u32;
        for blow in blows {
            let Some(unit) = clone.units.get(&blow.unit) else {
                dropped += 1;
                continue;
            };
            if unit.pos != blow.from {
                let walked = clone
                    .apply(
                        pid,
                        &Action::MoveTo {
                            unit: blow.unit,
                            to: blow.from,
                        },
                    )
                    .is_ok()
                    && clone
                        .units
                        .get(&blow.unit)
                        .is_some_and(|unit| unit.pos == blow.from);
                if !walked {
                    dropped += 1;
                    continue;
                }
            }
            let action = if blow.ranged {
                Action::Ranged {
                    unit: blow.unit,
                    target: blow.target,
                }
            } else {
                Action::Attack {
                    unit: blow.unit,
                    target: blow.target,
                }
            };
            let (result, applied) =
                Self::tactical_attack_result_in(&mut clone, pid, blow.unit, &action, plan);
            match applied {
                AppliedAttack::Applied if result.eliminates_enemy_unit || result.value >= 0.0 => {
                    if result.eliminates_enemy_unit {
                        kills += 1;
                    }
                    verified.push(blow.clone());
                }
                _ => {
                    dropped += 1;
                }
            }
        }
        (verified, kills, dropped)
    }

    /// Land the verified blows on the real board, in order, each mover
    /// walking first. Units that struck are the plan's for the turn.
    fn apply_blows(&mut self, g: &mut Game, pid: usize, blows: &[Blow]) -> bool {
        let mut struck = false;
        for blow in blows {
            let Some(unit) = g.units.get(&blow.unit) else {
                continue;
            };
            if unit.pos != blow.from
                && (!self.base.path_walk_to(g, pid, blow.unit, blow.from)
                    || g.units
                        .get(&blow.unit)
                        .is_none_or(|unit| unit.pos != blow.from))
            {
                self.census.battle_plan_dropped_blows += 1;
                continue;
            }
            let action = if blow.ranged {
                Action::Ranged {
                    unit: blow.unit,
                    target: blow.target,
                }
            } else {
                Action::Attack {
                    unit: blow.unit,
                    target: blow.target,
                }
            };
            if g.apply(pid, &action).is_ok() {
                struck = true;
                self.battle_planner_ordered.insert(blow.unit);
            } else {
                self.census.battle_plan_dropped_blows += 1;
            }
        }
        if struck {
            self.force_groups_dirty = true;
        }
        struck
    }

    /// One "Military/Decision" line per planned kill.
    fn journal_kill_plan(&self, g: &Game, blows: &[Blow]) {
        if !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        for blow in blows.iter().filter(|blow| blow.finishes) {
            if !seen.insert(blow.defender) {
                continue;
            }
            let on_target: Vec<&Blow> = blows
                .iter()
                .filter(|other| other.defender == blow.defender)
                .collect();
            let expected: f64 = on_target.iter().map(|other| other.expected).sum();
            let ranged = on_target.iter().filter(|other| other.ranged).count();
            let (kind, hp) = g
                .units
                .get(&blow.defender)
                .map(|unit| (unit.kind.to_string(), unit.hp))
                .unwrap_or_else(|| ("unit".to_string(), 0));
            think!(self.journal(), Military, Decision,
                "Battle plan: {} blow(s) finish the {kind} at {:?}", on_target.len(), blow.target;
                "expected {expected:.0} damage on {hp} hp (kill margin {KILL_MARGIN:.2}); \
                 {ranged} ranged first, {} melee last", on_target.len() - ranged);
        }
    }

    /// The fresh friend a wounded unit should trade places with, if any:
    /// adjacent, ours, melee-capable, healthier by `RELIEF_HP_MARGIN`,
    /// further from the nearest hostile than the wounded unit, and standing
    /// where the wounded unit would be less exposed.
    fn rotation_relief(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        field: &mut DangerField,
        here: f64,
    ) -> Option<u32> {
        let unit = g.units.get(&uid)?;
        let hostile_distance = |pos: Pos| {
            g.units
                .values()
                .filter(|other| {
                    other.owner != pid
                        && g.is_at_war(pid, other.owner)
                        && g.rules.units[other.kind].class == "military"
                        && g.unit_visible_to(other.id, pid)
                })
                .map(|other| g.wdist(pos, other.pos))
                .min()
        };
        let ours = hostile_distance(unit.pos)?;
        let mut best: Option<(i32, u32)> = None;
        for pos in g.nbrs(unit.pos) {
            for other_id in g.unit_ids_at(pos) {
                let Some(other) = g.units.get(other_id) else {
                    continue;
                };
                let spec = &g.rules.units[other.kind];
                if other.owner != pid
                    || other.id == uid
                    || other.linked_to.is_some()
                    || other.moves_left <= 0.0
                    || spec.class != "military"
                    || !spec.is_melee_capable()
                    || other.hp < unit.hp + RELIEF_HP_MARGIN
                    || self.battle_planner_ordered.contains(other_id)
                    || self.battle_planner_recovering.contains(other_id)
                {
                    continue;
                }
                if hostile_distance(other.pos).unwrap_or(i32::MAX) <= ours {
                    continue;
                }
                if field.danger(other.pos, uid) >= here - NO_DANGER {
                    continue;
                }
                if best.is_none_or(|(hp, id)| other.hp > hp || (other.hp == hp && other.id < id)) {
                    best = Some((other.hp, other.id));
                }
            }
        }
        best.map(|(_, id)| id)
    }

    /// Pull the wounded and the exposed out of reach and fortify them.
    /// Returns how many actually moved or swapped.
    fn rotate_wounded(
        &mut self,
        g: &mut Game,
        pid: usize,
        field: &mut DangerField,
        strikers: &BTreeSet<u32>,
    ) -> u32 {
        let heals = !g.is_arena() || g.tactics.heal;
        let mut ids = g.player_unit_ids(pid);
        ids.sort_unstable();
        let mut rotations = 0u32;
        for uid in ids {
            let Some(unit) = g.units.get(&uid).cloned() else {
                continue;
            };
            let spec = &g.rules.units[unit.kind];
            if strikers.contains(&uid)
                || self.battle_planner_ordered.contains(&uid)
                || spec.class != "military"
                || spec.domain.as_deref() == Some("air")
                || unit.linked_to.is_some()
                || unit.moves_left <= 0.0
                || !(spec.is_melee_capable() || spec.has_ranged_attack())
                || g.city_at(unit.pos).is_some()
                || g.encampment_at(unit.pos).is_some()
                || self.guard_is_bound_to_any_settler(uid)
            {
                continue;
            }
            let here = field.danger(unit.pos, uid);
            let wounded =
                heals && (unit.hp < ROTATE_HP || self.battle_planner_recovering.contains(&uid));
            // Where nothing heals, a unit is pulled out only when it would
            // otherwise be removed: without a recovery to remember, a margin
            // would walk it out of reach one turn and back into it the next.
            let margin = if heals { ROTATE_DANGER_MARGIN } else { 0 };
            let exposed = here > f64::from(unit.hp - margin);
            if !(wounded || exposed) {
                continue;
            }
            let mut tiles = vec![unit.pos];
            tiles.extend(g.reachable(uid));
            let mut safe: Vec<(i32, i32, Pos)> = tiles
                .into_iter()
                .filter(|tile| field.danger(*tile, uid) <= NO_DANGER)
                .map(|tile| {
                    (
                        -heal_preference(g, pid, tile),
                        g.wdist(unit.pos, tile),
                        tile,
                    )
                })
                .collect();
            safe.sort_unstable();
            let relief = self.rotation_relief(g, pid, uid, field, here);
            let mut moved = false;
            if let Some(relief) = relief {
                moved = g
                    .apply(
                        pid,
                        &Action::Swap {
                            unit: uid,
                            other: relief,
                        },
                    )
                    .is_ok();
            }
            if !moved {
                let Some(&(_, _, best)) = safe.first() else {
                    continue;
                };
                if best != unit.pos {
                    if !self.base.path_walk_to(g, pid, uid, best) {
                        continue;
                    }
                    moved = true;
                }
            }
            if moved {
                rotations += 1;
            }
            self.base.fortify_or_stop(g, pid, uid);
            self.battle_planner_ordered.insert(uid);
            if heals {
                self.battle_planner_recovering.insert(uid);
            }
            if let Some(now) = g.units.get(&uid) {
                think!(self.journal(), Military, Decision,
                    "Battle plan: the {} at {:?} {} to heal", now.kind, now.pos,
                    if moved { "rotates out" } else { "holds and fortifies" };
                    "{} hp, danger {here:.0} where it stood, {} where it stands",
                    now.hp, field.danger(now.pos, uid));
            }
        }
        rotations
    }
}

/// How good a tile is to heal on: the engine's own location rate — a
/// district 20, friendly ground 15 — plus the best `adjacent_heal` support
/// unit of ours beside it.
fn heal_preference(g: &Game, pid: usize, tile: Pos) -> i32 {
    let location = g.healing_location(pid, tile).rate();
    let support = g
        .nbrs(tile)
        .into_iter()
        .flat_map(|pos| g.unit_ids_at(pos).iter().copied())
        .filter(|id| g.units[id].owner == pid)
        .map(|id| g.promotion_effect(&g.units[&id], "adjacent_heal"))
        .fold(0.0, f64::max);
    location + support.round() as i32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctrine::{build, position};
    use crate::hex;

    fn open_field() -> Game {
        let mut g = build(position("the_reserve").expect("known"), 3).expect("buildable");
        let seeded: Vec<u32> = (0..2).flat_map(|pid| g.player_unit_ids(pid)).collect();
        for uid in seeded {
            g.remove_unit(uid);
        }
        g
    }

    fn at(col: i32, row: i32) -> Pos {
        hex::offset_to_axial(col, row)
    }

    fn wound(g: &mut Game, uid: u32, hp: i32) {
        g.units.get_mut(&uid).expect("unit").hp = hp;
    }

    fn fortify(g: &mut Game, uid: u32) {
        let unit = g.units.get_mut(&uid).expect("unit");
        unit.fortified = true;
        unit.fortify_turns = 2;
    }

    fn conquest(g: &Game) -> StrategicPlan {
        StrategicPlan {
            strategy: super::super::GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: g.turn,
            rush: false,
        }
    }

    #[test]
    fn the_gene_ships_off_and_is_registered() {
        let ai = AdvancedAi::new();
        assert!(!ai.battle_planner, "an opt-in ships off");
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.opt_in() && gene.field == "battle_planner"));
        let mut on = AdvancedAi::new();
        on.enable_battle_planner();
        assert!(on.battle_planner);
        on.disable_battle_planner();
        assert!(!on.battle_planner);
        super::super::test_support::opt_in_off_in_both_controllers("battle-planner", |ai| {
            ai.battle_planner
        });
    }

    /// Off, the plan reads nothing and orders nothing, whatever the board.
    #[test]
    fn off_the_plan_is_empty() {
        let mut g = open_field();
        g.spawn_unit("archer", 0, at(9, 6));
        g.spawn_unit("archer", 0, at(9, 8));
        let victim = g.spawn_unit("warrior", 1, at(10, 7));
        wound(&mut g, victim, 40);
        let mut ai = AdvancedAi::new();
        let plan = conquest(&g);
        assert!(!ai.plan_battle(&mut g, 0, &plan));
        assert!(g.units.contains_key(&victim), "nothing struck");
        assert!(ai.battle_planner_ordered.is_empty());
    }

    /// Two archers and a warrior beside a wounded, fortified enemy warrior
    /// that no single blow — and no pair of them — finishes with margin:
    /// the plan spends all three, the two shots first and the melee blow
    /// last, and the victim is gone from the board.
    #[test]
    fn two_archers_and_a_warrior_finish_the_wounded_fortified_enemy_melee_last() {
        let mut g = open_field();
        let archer_a = g.spawn_unit("archer", 0, at(9, 6));
        let archer_b = g.spawn_unit("archer", 0, at(9, 8));
        let warrior = g.spawn_unit("warrior", 0, at(9, 7));
        let victim = g.spawn_unit("warrior", 1, at(10, 7));
        fortify(&mut g, victim);
        let mut ai = AdvancedAi::new();
        ai.enable_battle_planner();
        // Find a wound where two shots cannot finish with margin but three
        // blows can: the plan then has to sequence all three.
        let mut chosen = None;
        for hp in (40..=90).rev() {
            wound(&mut g, victim, hp);
            let blows = ai.kill_sequence(&g, 0);
            if blows.len() == 3 && blows.iter().any(|blow| blow.finishes) {
                chosen = Some((hp, blows));
                break;
            }
        }
        let (hp, blows) = chosen.expect("some wound needs exactly three blows");
        wound(&mut g, victim, hp);
        assert!(
            blows.iter().all(|blow| blow.target == at(10, 7)),
            "{blows:?}"
        );
        assert!(
            blows[0].ranged && blows[1].ranged,
            "the shots go first: {blows:?}"
        );
        assert!(
            !blows[2].ranged && blows[2].unit == warrior,
            "the melee blow is last: {blows:?}"
        );
        assert!(blows[2].finishes && !blows[0].finishes, "{blows:?}");
        let shooters: BTreeSet<u32> = blows.iter().map(|blow| blow.unit).collect();
        assert_eq!(
            shooters,
            [archer_a, archer_b, warrior].into_iter().collect()
        );
        let plan = conquest(&g);
        assert!(ai.plan_battle(&mut g, 0, &plan), "a blow landed");
        assert!(
            !g.units.contains_key(&victim),
            "the victim is finished on the real board"
        );
        assert_eq!(ai.census.battle_plan_kills, 1);
        assert_eq!(ai.census.battle_plan_verified_kills, 1);
        assert_eq!(ai.census.battle_plan_dropped_blows, 0);
        for uid in [archer_a, archer_b, warrior] {
            assert!(
                ai.battle_planner_claims(uid),
                "the ladder leaves a planned striker alone"
            );
        }
    }

    /// A 20-hit-point warrior beside a healthy fortified swordsman: the
    /// return would kill it and it finishes nothing, so no blow is planned
    /// and the unit is not spent.
    #[test]
    fn a_suicidal_attack_is_vetoed() {
        let mut g = open_field();
        let ours = g.spawn_unit("warrior", 0, at(9, 7));
        let enemy = g.spawn_unit("swordsman", 1, at(10, 7));
        fortify(&mut g, enemy);
        wound(&mut g, ours, 20);
        let mut ai = AdvancedAi::new();
        ai.enable_battle_planner();
        let (att, def) = g.melee_exchange_strengths(ours, enemy).expect("a pair");
        assert!(
            expected_damage(def, att) >= 20.0,
            "the fixture's return blow is lethal: {}",
            expected_damage(def, att)
        );
        assert!(ai.kill_sequence(&g, 0).is_empty());
        // And a healthy warrior in the same place is not suicidal but is not
        // a kill either: the exchange is a loss, so nothing is planned.
        wound(&mut g, ours, 100);
        assert!(ai.kill_sequence(&g, 0).is_empty());
    }

    /// A 30-hit-point warrior beside a healthy enemy steps to a tile the
    /// enemy cannot strike next turn and fortifies; the healthy warrior on
    /// the same board does neither.
    ///
    /// Beside, not two tiles off: stepping into a tile adjacent to an enemy
    /// ends a move here (zone of control), so a melee unit two tiles away
    /// cannot close and strike in one turn and the field reads zero there —
    /// which is the engine's fact, and the reason the fixture is adjacent.
    #[test]
    fn a_wounded_unit_rotates_to_a_zero_danger_tile_and_fortifies_while_a_healthy_one_does_not() {
        let mut g = open_field();
        g.tactics.heal = true;
        let hurt = g.spawn_unit("warrior", 0, at(10, 6));
        let healthy = g.spawn_unit("warrior", 0, at(9, 8));
        let enemy = g.spawn_unit("warrior", 1, at(11, 6));
        wound(&mut g, hurt, 30);
        let mut ai = AdvancedAi::new();
        ai.enable_battle_planner();
        let before = danger(&g, 0, at(10, 6), hurt);
        assert!(
            before > 10.0,
            "the wounded unit is exposed where it stands: {before}"
        );
        let plan = conquest(&g);
        ai.plan_battle(&mut g, 0, &plan);
        let after = &g.units[&hurt];
        assert_ne!(after.pos, at(10, 6), "it left the exposed tile");
        assert!(after.fortified, "and fortified where it went");
        assert!(g.units.contains_key(&enemy));
        let now = danger(&g, 0, after.pos, hurt);
        assert!(now <= NO_DANGER, "its new tile is out of reach: {now}");
        assert!(ai.battle_planner_recovering.contains(&hurt));
        assert_eq!(ai.census.battle_plan_rotations, 1);
        let fresh = &g.units[&healthy];
        assert_eq!(fresh.pos, at(9, 8), "the healthy unit is not rotated");
        assert!(!fresh.fortified);
        assert!(!ai.battle_planner_recovering.contains(&healthy));
    }

    /// The danger field at a tile in range of two enemy archers is exactly
    /// the sum of their expected shots at our unit standing there.
    #[test]
    fn the_danger_field_sums_the_archers_expected_damage() {
        let mut g = open_field();
        let ours = g.spawn_unit("warrior", 0, at(10, 6));
        let left = g.spawn_unit("archer", 1, at(8, 6));
        let right = g.spawn_unit("archer", 1, at(12, 6));
        let tile = at(10, 6);
        for archer in [left, right] {
            assert!(g.wdist(g.units[&archer].pos, tile) <= 2, "in range");
            assert!(g.attack_reach(archer).contains(&tile));
        }
        let expected: f64 = [left, right]
            .into_iter()
            .map(|archer| {
                let (att, def) = g
                    .ranged_strike_strengths(archer, ours, tile)
                    .expect("a pair");
                expected_damage(att, def)
            })
            .sum();
        assert!(expected > 0.0);
        let read = danger(&g, 0, tile, ours);
        assert!((read - expected).abs() < 1e-9, "{read} v {expected}");
        // The same unit fortified reads the same: the field prices it
        // unfortified, as it would stand after moving there.
        fortify(&mut g, ours);
        let read = danger(&g, 0, tile, ours);
        assert!((read - expected).abs() < 1e-9, "{read} v {expected}");
        // And a tile beyond both archers' reach reads zero.
        let far = at(10, 12);
        assert!([left, right]
            .iter()
            .all(|archer| !g.attack_reach(*archer).contains(&far)));
        assert_eq!(danger(&g, 0, far, ours), 0.0);
    }
}
