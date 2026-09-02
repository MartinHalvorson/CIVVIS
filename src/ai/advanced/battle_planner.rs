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
//! 4. **The positions plan** (`battle-planner-2` only). After the kill plan
//!    and the rotation, every advancing or engaging force lays out *slots*
//!    against the enemy contact and the objective — never against the
//!    group's own anchor — and the members that were given no strike and no
//!    rotation are placed in them by a minimum-cost assignment. Front slots
//!    (Vanguard and Mobile) stand at distance one from the nearest contacts,
//!    on our side, scored by `tile_defense_bonus` and a river between slot
//!    and enemy; with no enemy within `CONTACT_BAND` they stand at the
//!    front's depth from the objective instead. Shooter slots stand at
//!    attack range from the most finishable targets with line of sight and a
//!    front slot between them and the enemy, or one tile further back where
//!    no front slot covers them; siege slots in range of the
//!    objective city behind a front slot; support slots beside the most
//!    front slots; and a unit under `HEAL_SLOT_HP`, where the board heals,
//!    takes a heal slot — a tile the field reads as zero, a district or
//!    friendly ground preferred — and enters the recovery. Units go to slots
//!    by the Hungarian method on route turns plus danger past the unit's
//!    spare hit points; a lethal slot is never taken. A unit with a blow to
//!    offer — the kill plan's shooters, spent or declined, and any unit with
//!    an enemy city in its attack reach — is not placed: it stays the
//!    ladder's, which prices the blow on its own clone exactly as version
//!    one does. Every assigned tile is
//!    reserved, each unit's end tile this turn is chosen against the
//!    reservations, and the moves are issued front to rear so a rear unit
//!    can enter the tile a front unit vacates. On the approach — no enemy
//!    within `close_as_a_body::CONTACT_RANGE` of any member — no unit ends
//!    more than the body's pace plus one closer to the objective than the
//!    slowest member stood; in contact nothing is paced. Cohesion here is a
//!    property of the slot layout alone: there is no scored adjacency or
//!    support term, which `docs/TACTICS.md` §4 and §7 measured null and
//!    refuted.
//!
//! Units the plan has ordered are marked (`battle_planner_ordered`) and the
//! per-unit ladder leaves them alone for the turn; everyone else plays the
//! ladder exactly as before. Under version one `coordinated_tactical_step`
//! moves every unit that is not striking; under version two it moves only
//! the units the positions plan did not place — a unit without a group, a
//! scout, a garrison, a member of a holding or mustering force, or one whose
//! every slot was unreachable or lethal — so behaviour changes only where a
//! slot plan exists. One version of the family plays: `enable_battle_planner_2`
//! turns version one off, and the seam reads `battle_planner_on()`.
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

//!
//! **Version three** (`battle-planner-3`, `enable_battle_planner_3` turns
//! versions one and two off; `battle_planner_on()` covers all three) is
//! version two plus three readings of the wider machinery:
//!
//! - **The taker is the siege's.** A unit `siege_train.rs` has reserved
//!   (`unit_is_reserved`) is skipped by the kill plan, the heal rotation and
//!   the positions plan — the Take blow is the siege train's own step, and
//!   the planner never spends the unit that walks in.
//! - **The host's price beats the closed form.** Where
//!   `Game::host_preview` holds the host's own `SimulateAttackInto` reading
//!   for a `(unit, target, ranged)` pair (a `preview` order answered the
//!   frame before), the candidate carries the host's damage both ways in
//!   place of `expected_damage`'s centre roll, and a blow the host says
//!   kills the attacker is vetoed outright — no kill is worth the unit on
//!   the host's word. Native boards hold no previews, so the closed form
//!   stands there.
//! - **Asking for the price.** Before the search, the top
//!   [`MAX_WANTED_PREVIEWS`] candidate pairs by closed-form damage from the
//!   unit's own tile that have no host reading yet are published through
//!   [`AdvancedAi::wanted_previews`]; `civvis_orders` turns them into
//!   `preview` orders ahead of the frame's strikes, and the answers reach
//!   `Game::host_previews` on the turn's next frame.

//!
//! **`strike-reach`** (opt-in, separate from the version family so any
//! version can carry it): the danger field's reach. Read off
//! `Game::attack_reach`, a hostile's reach is the movement flood's, and
//! `flow_past` writes zero movement into a tile that enters our zone of
//! control — so a melee unit two tiles off could not close and strike, and a
//! shooter one step out of range could not step and shoot, and the field read
//! zero at every tile only such a unit could hit. The engine does not resolve
//! a blow that way: a unit stopped by a zone of control keeps its unused
//! movement for the attack (`approach_reach`, `do_attack`), and on the live
//! host it is the ordinary way a unit dies — measured over the seven games of
//! 2026-09-02 with the planner on, the rotation stood 58 units on a tile the
//! field read as zero that were killed there, 36 of the killers in sight at
//! the start of the turn and 34 of those within one move and a blow, 17 of
//! them melee at exactly two tiles. With the gene on, `DangerField` stands
//! each hostile at the start of its own turn on the probe (full movement, no
//! zone-of-control memory), takes `approach_reach` for the tiles it can end
//! on with the movement it keeps there, and counts a melee blow on every
//! neighbour of a stand with movement left and a ranged blow on every tile in
//! range of one (a siege unit only from its own tile, as before). On the
//! mirrored board — `MIRRORED_SEAT` with `Game::host_observed` filled — the
//! ranged blow also drops the per-unit line-of-sight test: a Civilization VI
//! ranged attack needs the target tile visible to the player, not a line from
//! the unit, and the barbarian's other units see for it. Native boards keep
//! the test, which is the engine's own `do_ranged` rule there. The kill plan,
//! the rotation and the positions plan read the same field, so all three
//! move with it; nothing else changes.
//! **`doomed-blow-veto`** (opt-in, separate from the version family): the
//! plan leaves every unit with a legal blow it did not spend to the ladder,
//! which prices the blow on its own clone and takes it more often — and the
//! ladder's reply price reads the movement flood, not the danger field.
//! Twelve of the 145 losses in the 2026-09-02 taxonomy were such an attack:
//! the unit struck, stood beside its target, and was removed on the enemy's
//! turn. With the gene on, `doomed_shooters` reads every candidate blow of
//! every shooter — return damage (a melee blow's) plus the field's danger at
//! the stand — and a shooter with no blow that leaves it above zero is not
//! armed: if it is wounded or exposed where it stands the rotation takes it
//! out of reach (or to the least danger under `safest-stand`); if it is safe
//! where it stands it holds that ground and fortifies — the ladder's attack
//! is the one thing denied it. (The first cut rotated every doomed unit and
//! read −338 ± 84 on the_ridge: a unit safe on its hill was walked off it.)
//! A unit with one survivable blow keeps the ladder's freedom, and
//! the kill plan's own vetoes and values are unchanged. Census
//! `battle_plan_doomed`; one Decision line per plan when it fires.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::close_as_a_body::{BODY_PACE_SLACK, CONTACT_RANGE};
use super::{AdvancedAi, AppliedAttack, ForceGroup, ForcePosture, ForceRole, StrategicPlan};
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
/// `battle-planner-3`: candidate pairs asked of the host a frame, by
/// closed-form damage.
pub(super) const MAX_WANTED_PREVIEWS: usize = 24;
/// A pair the plan wants the host to price: `(unit, target tile, ranged)`.
pub type WantedPreview = (u32, Pos, bool);
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
/// `battle-planner-2`: a visible hostile within this many tiles of a member
/// is a contact, and the slots are laid against the contacts rather than
/// the objective.
pub(super) const CONTACT_BAND: i32 = 3;
/// `battle-planner-2`: a member under this, where the board heals, takes a
/// heal slot instead of a role slot and enters the recovery. Above
/// `ROTATE_HP`, which the rotation already handles, and below the return.
pub(super) const HEAL_SLOT_HP: i32 = 60;
/// `battle-planner-2`: hit points a unit keeps in hand before the danger at
/// a slot starts to cost it turns in the assignment.
const DANGER_FREE_HP: i32 = 30;
/// `battle-planner-2`: route turns one hit point of danger past the spare
/// hit points costs. Twenty points past them weigh a turn of marching.
const DANGER_COST_PER_HP: f64 = 0.05;
/// `battle-planner-2`: members one plan places; the rest keep today's step.
const MAX_PLANNED_UNITS: usize = 20;
/// `battle-planner-2`: a river between a front slot and the enemy it faces,
/// on `tile_defense_bonus`'s scale, where a hill is three.
const RIVER_SLOT_BONUS: f64 = 5.0;
/// `battle-planner-2`: slot tiles are sought this far around each contact,
/// or around the objective.
const SLOT_RADIUS: i32 = 4;
/// `battle-planner-2`: the most finishable contacts shooter slots are laid
/// against.
const SHOOTER_TARGETS: usize = 3;
/// `battle-planner-2`: a pairing the assignment must not make — unreachable,
/// or lethal. Finite so the potentials stay finite.
const UNASSIGNABLE: f64 = 1.0e6;

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

/// Whether `pid` is the live seat on a mirrored board: the one place the host
/// has shown the seat what it can see. Empty everywhere else, including every
/// arena and screen board, so a rule keyed on it is inert there.
fn mirrored_board(g: &Game, pid: usize) -> bool {
    pid == crate::game::MIRRORED_SEAT && !g.host_observed.is_empty()
}

/// `strike-reach`: every tile the hostile `uid` could strike on its next
/// turn, read the way the engine resolves the blow rather than the way the
/// movement flood records it. The hostile is stood at the start of its own
/// turn on `probe` — full movement, no zone-of-control memory from the turn
/// it has just ended — `approach_reach` gives the tiles it can end on with
/// the movement it keeps there (a zone-of-control stop keeps it; `flow_past`
/// zeroes it), and from its own tile and every stand with movement left it
/// strikes each neighbour (melee) or each tile in range (ranged; a siege unit
/// only from its own tile unless it may attack after moving). Ranged blows
/// keep the engine's line-of-sight test on a native board and drop it on the
/// mirrored one, where the host's rule is the player's visibility. Ascending
/// and distinct, like `attack_reach`. The probe is left as it was found.
fn strike_reach_of(probe: &mut Game, pid: usize, uid: u32) -> Vec<Pos> {
    let Some(saved) = probe.units.get(&uid).cloned() else {
        return Vec::new();
    };
    let spec = &probe.rules.units[saved.kind];
    if spec.class != "military" || !(spec.is_melee_capable() || spec.has_ranged_attack()) {
        return Vec::new();
    }
    if spec.domain.as_deref() == Some("air") {
        return probe.attack_reach(uid);
    }
    let max_moves = probe.unit_max_moves(uid);
    if max_moves <= 0.0 {
        return Vec::new();
    }
    let melee = spec.is_melee_capable();
    let ranged = spec.has_ranged_attack();
    let siege = spec.siege;
    let sea = spec.domain.as_deref() == Some("sea");
    if let Some(live) = probe.units.get_mut(&uid) {
        live.moves_left = max_moves;
        live.moved = false;
        live.acted = false;
        live.zoc_stopped = false;
        live.started_turn_in_zoc = false;
    }
    let mut stands: Vec<(Pos, f64)> = vec![(saved.pos, max_moves)];
    stands.extend(
        probe
            .approach_reach(uid)
            .into_iter()
            .map(|(pos, (kept, _path))| (pos, kept)),
    );
    if let Some(live) = probe.units.get_mut(&uid) {
        *live = saved.clone();
    }
    let range = if ranged {
        probe.unit_attack_range(uid).max(1)
    } else {
        0
    };
    let after_move = probe.promotion_effect(&saved, "attack_after_move") > 0.0;
    let host_sight = mirrored_board(probe, pid);
    let mut targets: Vec<Pos> = Vec::new();
    for (from, kept) in stands {
        if kept <= 0.0 {
            continue;
        }
        // A land unit standing on water is embarked there and strikes nothing.
        let embarked = !sea
            && probe
                .map
                .get(from)
                .is_some_and(|tile| probe.rules.is_water(tile));
        if embarked {
            continue;
        }
        if melee {
            for target in probe.nbrs(from) {
                if probe.map.tiles.contains_key(&target)
                    && probe.unit_can_melee_target_domain(uid, target)
                {
                    targets.push(target);
                }
            }
        }
        if ranged && (!siege || from == saved.pos || after_move) {
            for target in probe.wdisk(from, range) {
                if target != from
                    && probe.map.tiles.contains_key(&target)
                    && (host_sight || probe.unit_has_line_of_sight_from(uid, from, target))
                {
                    targets.push(target);
                }
            }
        }
    }
    targets.sort_unstable();
    targets.dedup();
    targets
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
    /// `strike-reach`: hostiles whose strike reach held a tile the movement
    /// flood did not. Zero with the gene off.
    pub(super) widened: u32,
}

impl DangerField {
    /// The field on the movement flood's reach (`Game::attack_reach`).
    pub(super) fn new(g: &Game, pid: usize) -> Self {
        Self::with_reach(g, pid, false)
    }

    /// The field on the flood's reach, or with `strike_reach` on the reach
    /// the engine resolves a blow over (`strike_reach_of`).
    pub(super) fn with_reach(g: &Game, pid: usize, strike_reach: bool) -> Self {
        let mut probe = g.speculative_clone();
        let mut reaches = Vec::new();
        let mut widened = 0u32;
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
            let flood = g.attack_reach(unit.id);
            let reach = if strike_reach {
                let strike = strike_reach_of(&mut probe, pid, unit.id);
                if strike.iter().any(|tile| flood.binary_search(tile).is_err()) {
                    widened += 1;
                }
                strike
            } else {
                flood
            };
            if !reach.is_empty() {
                reaches.push((unit.id, reach));
            }
        }
        reaches.sort_by_key(|(id, _)| *id);
        DangerField {
            pid,
            probe,
            reaches,
            cache: BTreeMap::new(),
            widened,
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

/// The same reading on the strike reach (`strike-reach` on).
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn strike_danger(g: &Game, pid: usize, tile: Pos, uid: u32) -> f64 {
    DangerField::with_reach(g, pid, true).danger(tile, uid)
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
    /// `battle-planner-3`: the host's own reading of this pair from the
    /// unit's tile, `(damage to the defender, damage to the attacker)`,
    /// when `Game::host_preview` holds one. Replaces the closed form.
    host: Option<(f64, f64)>,
}

impl Candidate {
    /// The damage this blow deals against a defender at `def`: the host's
    /// reading when it has one, the centre roll otherwise.
    fn damage_on(&self, def: f64) -> f64 {
        self.host.map_or_else(
            || expected_damage(self.att, def),
            |(to_defender, _)| to_defender,
        )
    }

    /// The damage a melee blow takes back from a defender at `def`.
    fn return_on(&self, def: f64) -> f64 {
        self.host.map_or_else(
            || expected_damage(def, self.att),
            |(_, to_attacker)| to_attacker,
        )
    }
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
        let damage = candidate.damage_on(def);
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
            returned = candidate.return_on(def);
            if returned >= f64::from(shooter.hp) {
                // The host's word that the attacker dies is a veto, whatever
                // the kill is worth (`battle-planner-3`).
                if candidate.host.is_some() {
                    return None;
                }
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

/// `doomed-blow-veto`: the shooters whose every candidate blow would leave
/// them dead on the enemy's next turn — the return damage of the blow (a
/// melee blow's; a ranged blow takes none) plus the danger field at the
/// stand, at or over the unit's hit points. Such a unit has no blow worth
/// the ladder's freedom: the rotation takes it as exposed instead.
fn doomed_shooters(
    shooters: &[Shooter],
    targets: &[Target],
    candidates: &[Candidate],
    field: &mut DangerField,
) -> BTreeSet<u32> {
    let mut doomed = BTreeSet::new();
    for (index, shooter) in shooters.iter().enumerate() {
        let mut any = false;
        let mut survivable = false;
        for candidate in candidates.iter().filter(|c| c.shooter == index) {
            any = true;
            let target = &targets[candidate.target];
            let def = effective_strength(candidate.def_base, target.hp);
            let back = if candidate.ranged {
                0.0
            } else {
                candidate.return_on(def)
            };
            let after = field.danger(candidate.from, shooter.uid);
            if f64::from(shooter.hp) - back - after > 0.0 {
                survivable = true;
                break;
            }
        }
        if any && !survivable {
            doomed.insert(shooter.uid);
        }
    }
    doomed
}

impl AdvancedAi {
    /// Whether the battle plan has already ordered this unit this turn, so
    /// the per-unit ladder leaves it where the plan put it.
    pub(super) fn battle_planner_claims(&self, uid: u32) -> bool {
        self.battle_planner_ordered.contains(&uid)
    }

    /// Plan and play the force's turn: the kill plan, then the heal
    /// rotation, then — version two — the positions plan. `true` when a blow
    /// landed, so the caller rebuilds its force picture. Nothing is read with
    /// both versions off.
    pub(super) fn plan_battle(&mut self, g: &mut Game, pid: usize, plan: &StrategicPlan) -> bool {
        self.battle_planner_ordered.clear();
        if !self.battle_planner_on() {
            return false;
        }
        self.battle_planner_recovering.retain(|uid| {
            g.units
                .get(uid)
                .is_some_and(|unit| unit.owner == pid && unit.hp < RETURN_HP)
        });
        let mut field = DangerField::with_reach(g, pid, self.strike_reach);
        self.census.strike_reach_widened += field.widened;
        if field.widened > 0 {
            think!(self.journal(), Military, Detail,
                "Strike reach: {} hostile(s) can hit tiles the movement flood read as safe", field.widened;
                "a unit stopped by our zone of control keeps its movement for the blow; \
                 the danger field, the rotation and the slots read that reach");
        }
        let (blows, armed, wanted, doomed) = self.kill_sequence_in(g, pid, &mut field);
        self.battle_planner_wanted_previews = wanted;
        self.census.battle_plan_doomed += doomed.len() as u32;
        if !doomed.is_empty() {
            think!(self.journal(), Military, Decision,
                "Battle plan: {} unit(s) have no blow they would survive", doomed.len();
                "return damage plus the danger at every stand reaches their hit points; \
                 the rotation takes them as exposed and the ladder leaves them alone");
        }
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
            field = DangerField::with_reach(g, pid, self.strike_reach);
        }
        let rotations = self.rotate_wounded(g, pid, &mut field, &strikers, &doomed);
        self.census.battle_plan_rotations += rotations;
        // `battle-planner-2` (and three): the positions plan, on a force
        // picture that no longer holds the defenders the kill plan removed.
        // The caller's own rebuild after a strike is the version-one
        // contract and stands.
        if self.positions_plan_on() {
            if struck {
                self.rebuild_force_groups(g, pid, plan);
                self.force_groups_dirty = false;
            }
            self.plan_positions(g, pid, &mut field, &armed);
        }
        struck
    }

    /// The kill plan alone — the ordered blows the search chose, before any
    /// clone has seen them. Pure: for tests and explainers.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn kill_sequence(&self, g: &Game, pid: usize) -> Vec<Blow> {
        let mut field = DangerField::new(g, pid);
        self.kill_sequence_in(g, pid, &mut field).0
    }

    /// `battle-planner-3`: the pairs the kill plan wants the host to price —
    /// `(unit, target tile, ranged)` — as the last plan left them. Empty
    /// with the version off. `civvis_orders` reads it after `take_turn` and
    /// issues a `preview` order per pair.
    pub fn wanted_previews(&self) -> Vec<WantedPreview> {
        self.battle_planner_wanted_previews.clone()
    }

    /// The top [`MAX_WANTED_PREVIEWS`] candidate pairs by closed-form
    /// damage, read from the unit's own tile (where the host would simulate
    /// them), that the host has not priced yet.
    fn wanted_previews_of(
        &self,
        shooters: &[Shooter],
        targets: &[Target],
        candidates: &[Candidate],
    ) -> Vec<WantedPreview> {
        if !self.battle_planner_3 {
            return Vec::new();
        }
        let mut ranked: Vec<(f64, u32, Pos, bool)> = candidates
            .iter()
            .filter(|candidate| {
                candidate.host.is_none() && candidate.from == shooters[candidate.shooter].pos
            })
            .map(|candidate| {
                let target = &targets[candidate.target];
                let damage = candidate.damage_on(effective_strength(candidate.def_base, target.hp));
                (
                    damage,
                    shooters[candidate.shooter].uid,
                    target.pos,
                    candidate.ranged,
                )
            })
            .collect();
        ranked.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| a.1.cmp(&b.1))
                .then_with(|| a.2.cmp(&b.2))
                .then_with(|| a.3.cmp(&b.3))
        });
        let mut wanted: Vec<WantedPreview> = Vec::new();
        for (_, uid, pos, ranged) in ranked {
            if wanted.len() >= MAX_WANTED_PREVIEWS {
                break;
            }
            if !wanted.contains(&(uid, pos, ranged)) {
                wanted.push((uid, pos, ranged));
            }
        }
        wanted
    }

    /// The ordered blows, every unit that had a legal blow to offer, the
    /// pairs version three wants the host to price, and — `doomed-blow-veto`
    /// — the shooters whose every blow would leave them dead next turn.
    fn kill_sequence_in(
        &self,
        g: &Game,
        pid: usize,
        field: &mut DangerField,
    ) -> (Vec<Blow>, BTreeSet<u32>, Vec<WantedPreview>, BTreeSet<u32>) {
        let (shooters, targets, candidates, mut armed) = self.strike_candidates(g, pid, field);
        if candidates.is_empty() {
            return (Vec::new(), armed, Vec::new(), BTreeSet::new());
        }
        let doomed = if self.doomed_blow_veto {
            doomed_shooters(&shooters, &targets, &candidates, field)
        } else {
            BTreeSet::new()
        };
        armed.retain(|uid| !doomed.contains(uid));
        let wanted = self.wanted_previews_of(&shooters, &targets, &candidates);
        let (sequence, score) = search_kill_sequence(&shooters, &targets, &candidates, field);
        if score <= 0.0 || sequence.is_empty() {
            return (Vec::new(), armed, wanted, doomed);
        }
        let mut dealt = vec![0.0; targets.len()];
        let mut blows = Vec::with_capacity(sequence.len());
        for index in sequence {
            let candidate = &candidates[index];
            let target = &targets[candidate.target];
            let remaining = (f64::from(target.hp) - dealt[candidate.target]).max(1.0);
            let def = effective_strength(candidate.def_base, remaining.round() as i32);
            let expected = candidate.damage_on(def);
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
        (blows, armed, wanted, doomed)
    }

    /// Every legal blow each eligible unit could make this turn, from its
    /// own tile or after a move, priced with the engine's pair on the probe.
    ///
    /// The fourth element is every unit that has a legal blow this turn —
    /// the shooters, whether or not the search spends them, and a siege
    /// unit left to the ladder for a city shot. `battle-planner-2` leaves
    /// these to the ladder rather than placing them: a declined shot is
    /// still the ladder's to price on its own clone, as it always was.
    fn strike_candidates(
        &self,
        g: &Game,
        pid: usize,
        field: &mut DangerField,
    ) -> (Vec<Shooter>, Vec<Target>, Vec<Candidate>, BTreeSet<u32>) {
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
        let mut armed: BTreeSet<u32> = BTreeSet::new();
        if targets.is_empty() {
            return (Vec::new(), Vec::new(), Vec::new(), armed);
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
                // `battle-planner-3`: the siege's taker is not the plan's.
                || (self.battle_planner_3 && self.unit_is_reserved(uid))
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
                armed.insert(uid);
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
                                    host: None,
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
                                    host: None,
                                });
                            }
                        }
                    }
                    found
                };
                let mut found = if stand == unit.pos {
                    read(g)
                } else {
                    field.at_stand(uid, stand, read).unwrap_or_default()
                };
                // `battle-planner-3`: the host's own reading of the pair
                // replaces the closed form for that pair, from every stand:
                // the reading is made of the defender's tile and health and
                // the attacker's health and promotions, none of which a move
                // changes (a river crossing is the one term it misses).
                if self.battle_planner_3 {
                    for candidate in &mut found {
                        candidate.host = g
                            .host_preview(uid, targets[candidate.target].pos, candidate.ranged)
                            .map(|(_, _, to_attacker, to_defender)| {
                                (f64::from(to_defender), f64::from(to_attacker))
                            });
                    }
                }
                own.extend(found);
            }
            if own.is_empty() {
                continue;
            }
            armed.insert(uid);
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
                        let damage =
                            candidate.damage_on(effective_strength(candidate.def_base, target.hp));
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
            return (Vec::new(), Vec::new(), Vec::new(), armed);
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
                    candidate.damage_on(effective_strength(
                        candidate.def_base,
                        targets[candidate.target].hp,
                    ))
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
        (kept_shooters, targets, kept, armed)
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
        doomed: &BTreeSet<u32>,
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
                // `battle-planner-3`: the siege's taker holds its post.
                || (self.battle_planner_3 && self.unit_is_reserved(uid))
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
            // `doomed-blow-veto`: a unit with no blow it would survive that is
            // neither wounded nor exposed where it stands holds that ground
            // and fortifies — the ladder's attack is the one thing denied it;
            // one that is also exposed rotates like any other.
            if doomed.contains(&uid) && !(wounded || exposed) {
                self.base.fortify_or_stop(g, pid, uid);
                self.battle_planner_ordered.insert(uid);
                if let Some(now) = g.units.get(&uid) {
                    think!(self.journal(), Military, Decision,
                        "Battle plan: the {} at {:?} holds rather than strike", now.kind, now.pos;
                        "{} hp, danger {here:.0} where it stands; every blow it has would leave it dead next turn",
                        now.hp);
                }
                continue;
            }
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

/// `battle-planner-2`: the kind of slot a role stands in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum SlotRole {
    Front,
    Shooter,
    Siege,
    Support,
}

/// `battle-planner-2`: one tile the plan wants held, and how deep it is —
/// its distance to the nearest contact, or to the target on an approach.
/// The front is ordered on the depth.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Slot {
    pub role: SlotRole,
    pub tile: Pos,
    pub depth: i32,
}

/// `battle-planner-2`: one unit sent to one tile. `role` is `None` for a
/// heal slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Placement {
    pub unit: u32,
    pub tile: Pos,
    pub role: Option<SlotRole>,
    pub depth: i32,
}

/// `battle-planner-2`: a force's positions plan before anything moves.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PositionPlan {
    pub group: u32,
    pub target: Pos,
    pub contacts: usize,
    pub slots: Vec<Slot>,
    /// Front to rear: heal slots first (they vacate the line), then by
    /// depth, role and unit.
    pub placements: Vec<Placement>,
    /// On the approach: the nearest distance to the target a unit may end
    /// the turn at, and the pace it came from.
    pub pace_floor: Option<(i32, i32)>,
}

/// One visible hostile that can fight, within the contact band.
#[derive(Clone, Copy, Debug)]
struct Contact {
    pos: Pos,
    hp: i32,
    uid: u32,
}

/// The minimum-cost assignment of rows to columns, rows ≤ columns: the
/// Hungarian method with potentials, O(rows² · columns). Every row gets a
/// column; a pairing at `UNASSIGNABLE` is one the caller drops. Ties fall to
/// the input order, so the plan is a function of the board alone.
fn assign_min_cost(cost: &[Vec<f64>]) -> Vec<usize> {
    let rows = cost.len();
    if rows == 0 {
        return Vec::new();
    }
    let cols = cost[0].len();
    debug_assert!(rows <= cols);
    let mut u = vec![0.0f64; rows + 1];
    let mut v = vec![0.0f64; cols + 1];
    let mut matched = vec![0usize; cols + 1];
    let mut way = vec![0usize; cols + 1];
    for row in 1..=rows {
        matched[0] = row;
        let mut j0 = 0usize;
        let mut minv = vec![f64::INFINITY; cols + 1];
        let mut used = vec![false; cols + 1];
        loop {
            used[j0] = true;
            let i0 = matched[j0];
            let mut delta = f64::INFINITY;
            let mut j1 = 0usize;
            for j in 1..=cols {
                if used[j] {
                    continue;
                }
                let current = cost[i0 - 1][j - 1] - u[i0] - v[j];
                if current < minv[j] {
                    minv[j] = current;
                    way[j] = j0;
                }
                if minv[j] < delta {
                    delta = minv[j];
                    j1 = j;
                }
            }
            for j in 0..=cols {
                if used[j] {
                    u[matched[j]] += delta;
                    v[j] -= delta;
                } else {
                    minv[j] -= delta;
                }
            }
            j0 = j1;
            if matched[j0] == 0 {
                break;
            }
        }
        loop {
            let j1 = way[j0];
            matched[j0] = matched[j1];
            j0 = j1;
            if j0 == 0 {
                break;
            }
        }
    }
    let mut out = vec![0usize; rows];
    for j in 1..=cols {
        if matched[j] != 0 {
            out[matched[j] - 1] = j - 1;
        }
    }
    out
}

impl AdvancedAi {
    /// Any version of the battle planner. The seam reads this; the
    /// positions plan itself reads `positions_plan_on`.
    pub(super) fn battle_planner_on(&self) -> bool {
        self.battle_planner || self.battle_planner_2 || self.battle_planner_3
    }

    /// Versions two and three lay out the positions plan.
    fn positions_plan_on(&self) -> bool {
        self.battle_planner_2 || self.battle_planner_3
    }

    /// The slot a role stands in. `None` is a role the plan does not place.
    fn slot_role(g: &Game, uid: u32, city_objective: bool) -> Option<SlotRole> {
        match Self::force_role(g, uid) {
            ForceRole::Vanguard | ForceRole::Mobile => Some(SlotRole::Front),
            ForceRole::Ranged => Some(SlotRole::Shooter),
            ForceRole::Siege if city_objective => Some(SlotRole::Siege),
            ForceRole::Siege => Some(SlotRole::Shooter),
            ForceRole::Support => Some(SlotRole::Support),
            ForceRole::Recon | ForceRole::AirStrike => None,
        }
    }

    /// The members of a force the positions plan may place: not already
    /// ordered or recovering, a field unit with movement left, not a garrison,
    /// not a bound guard, not a scout — and not a unit with a blow to offer.
    /// `armed` is the kill plan's set: every unit that had a legal blow on a
    /// hostile unit this turn, spent or declined; a unit with an enemy city
    /// inside its attack reach joins it here. Both are the ladder's, which
    /// prices the blow on its own clone as it always has and moves the unit
    /// as before when it declines. Measured before this rule: placing the
    /// declined shooters read −129 ± 30 a seed on four archers and two
    /// warriors, less material destroyed, not more lost — the shots the
    /// ladder would have taken. The `MAX_PLANNED_UNITS` nearest the target,
    /// ascending by id.
    fn planned_members(
        &self,
        g: &Game,
        pid: usize,
        group: &ForceGroup,
        target: Pos,
        armed: &BTreeSet<u32>,
    ) -> Vec<u32> {
        let enemy_cities: Vec<Pos> = g
            .cities
            .values()
            .filter(|city| city.owner != pid && g.is_at_war(pid, city.owner))
            .map(|city| city.pos)
            .collect();
        let city_in_reach = |uid: u32| -> bool {
            let unit = &g.units[&uid];
            let spec = &g.rules.units[unit.kind];
            if enemy_cities.is_empty()
                || unit.attacks_left <= 0
                || !(spec.is_melee_capable() || spec.has_ranged_attack())
            {
                return false;
            }
            let range = if spec.has_ranged_attack() {
                g.unit_attack_range(uid).max(1)
            } else {
                1
            };
            let mut stands = vec![unit.pos];
            stands.extend(g.reachable(uid));
            enemy_cities
                .iter()
                .any(|city| stands.iter().any(|stand| g.wdist(*stand, *city) <= range))
        };
        let mut members: Vec<u32> = group
            .units
            .iter()
            .copied()
            .filter(|uid| {
                let Some(unit) = g.units.get(uid) else {
                    return false;
                };
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && !self.battle_planner_ordered.contains(uid)
                    && !self.battle_planner_recovering.contains(uid)
                    && !armed.contains(uid)
                    && matches!(spec.class.as_str(), "military" | "support")
                    && spec.domain.as_deref() != Some("air")
                    && unit.linked_to.is_none()
                    && unit.moves_left > 0.0
                    && !g.is_embarked(unit)
                    && g.city_at(unit.pos).is_none()
                    && g.encampment_at(unit.pos).is_none()
                    && !self.guard_is_bound_to_any_settler(*uid)
                    && !matches!(
                        Self::force_role(g, *uid),
                        ForceRole::Recon | ForceRole::AirStrike
                    )
                    && !city_in_reach(*uid)
                    // `battle-planner-3`: the siege's taker is not placed.
                    && !(self.battle_planner_3 && self.unit_is_reserved(*uid))
            })
            .collect();
        members.sort_by_key(|uid| (g.wdist(g.units[uid].pos, target), *uid));
        members.truncate(MAX_PLANNED_UNITS);
        members.sort_unstable();
        members
    }

    /// Every visible hostile that can fight within `band` of a member, most
    /// finishable first.
    fn contacts(g: &Game, pid: usize, members: &[u32], band: i32) -> Vec<Contact> {
        let mut contacts: Vec<Contact> = g
            .units
            .values()
            .filter(|other| {
                let spec = &g.rules.units[other.kind];
                other.owner != pid
                    && g.is_at_war(pid, other.owner)
                    && spec.class == "military"
                    && (spec.is_melee_capable() || spec.has_ranged_attack())
                    && g.unit_visible_to(other.id, pid)
                    && members
                        .iter()
                        .any(|uid| g.wdist(g.units[uid].pos, other.pos) <= band)
            })
            .map(|other| Contact {
                pos: other.pos,
                hp: other.hp,
                uid: other.id,
            })
            .collect();
        contacts.sort_by_key(|contact| (contact.hp, contact.uid));
        contacts
    }

    /// The positions plan for one force, before anything moves: the slots,
    /// the assignment and the pace floor. `None` when the force is not one
    /// the plan places — not advancing or engaging, a force of one, or no
    /// member left to place. Pure but for the field's cache; for tests and
    /// explainers as much as for `plan_positions`.
    pub(super) fn position_plan(
        &self,
        g: &Game,
        pid: usize,
        group: &ForceGroup,
        field: &mut DangerField,
        armed: &BTreeSet<u32>,
    ) -> Option<PositionPlan> {
        if !matches!(group.posture, ForcePosture::Advance | ForcePosture::Engage) {
            return None;
        }
        let target = match group.posture {
            ForcePosture::Engage => group.focus_target.unwrap_or(group.objective),
            _ => group.objective,
        };
        // A force of one is not a formation and keeps today's step; a force
        // of several is planned even when the armed and the ordered leave a
        // single member to place.
        let members = self.planned_members(g, pid, group, target, armed);
        if group.units.len() < 2 || members.is_empty() {
            return None;
        }
        let heals = !g.is_arena() || g.tactics.heal;
        let city_objective = g.city_at(target).is_some_and(|cid| {
            let city = &g.cities[&cid];
            city.owner != pid && g.is_at_war(pid, city.owner)
        });
        let contacts = Self::contacts(g, pid, &members, CONTACT_BAND);
        let in_contact = !Self::contacts(g, pid, &members, CONTACT_RANGE).is_empty();

        // Who wants what. A wounded unit on a board that heals wants a heal
        // slot whatever its role.
        let mut wounded: Vec<u32> = Vec::new();
        let mut by_role: BTreeMap<SlotRole, Vec<u32>> = BTreeMap::new();
        for uid in &members {
            let unit = &g.units[uid];
            if heals && unit.hp < HEAL_SLOT_HP {
                wounded.push(*uid);
                continue;
            }
            if let Some(role) = Self::slot_role(g, *uid, city_objective) {
                by_role.entry(role).or_default().push(*uid);
            }
        }

        // The pool of tiles a slot may be: land within `SLOT_RADIUS` of a
        // contact (or of the target), that the force can walk and no one
        // else stands on.
        let probe = members[0];
        let centres: Vec<Pos> = if contacts.is_empty() {
            vec![target]
        } else {
            contacts.iter().map(|contact| contact.pos).collect()
        };
        let mut pool: BTreeSet<Pos> = BTreeSet::new();
        for centre in &centres {
            for tile in g.wdisk(*centre, SLOT_RADIUS) {
                let Some(t) = g.map.get(tile) else {
                    continue;
                };
                if g.rules.is_water(t)
                    || !g.unit_can_traverse(probe, tile)
                    || g.city_at(tile)
                        .is_some_and(|cid| g.cities[&cid].owner != pid)
                    || g.unit_ids_at(tile)
                        .iter()
                        .any(|other| g.units[other].owner != pid)
                {
                    continue;
                }
                pool.insert(tile);
            }
        }
        let nearest_contact = |tile: Pos| -> Option<Pos> {
            contacts
                .iter()
                .map(|contact| (g.wdist(tile, contact.pos), contact.pos))
                .min()
                .map(|(_, pos)| pos)
        };
        let depth = |tile: Pos| -> i32 {
            nearest_contact(tile)
                .map(|pos| g.wdist(tile, pos))
                .unwrap_or_else(|| g.wdist(tile, target))
        };
        // On our side of the enemy: no further from the anchor than the
        // contact the tile faces.
        let our_side = |tile: Pos| -> bool {
            nearest_contact(tile)
                .is_none_or(|pos| g.wdist(tile, group.anchor) <= g.wdist(pos, group.anchor))
        };
        let mut slots: Vec<Slot> = Vec::new();
        let mut taken: BTreeSet<Pos> = BTreeSet::new();

        // Front slots: at depth one, on our side, the best ground first.
        let n_front = by_role.get(&SlotRole::Front).map_or(0, Vec::len);
        if n_front > 0 {
            let mut candidates: Vec<(f64, i32, Pos)> = pool
                .iter()
                .copied()
                .filter(|tile| depth(*tile) == 1 && our_side(*tile))
                .map(|tile| {
                    let facing = nearest_contact(tile).unwrap_or(target);
                    let mut score = g.tile_defense_bonus(tile);
                    if g.wdist(tile, facing) == 1 && g.map.has_river_edge(tile, facing) {
                        score += RIVER_SLOT_BONUS;
                    }
                    (score, g.wdist(tile, group.anchor), tile)
                })
                .collect();
            candidates.sort_by(|a, b| {
                b.0.total_cmp(&a.0)
                    .then_with(|| a.1.cmp(&b.1))
                    .then_with(|| a.2.cmp(&b.2))
            });
            for (_, _, tile) in candidates.into_iter().take(n_front) {
                taken.insert(tile);
                slots.push(Slot {
                    role: SlotRole::Front,
                    tile,
                    depth: 1,
                });
            }
        }
        let front_tiles: Vec<Pos> = slots.iter().map(|slot| slot.tile).collect();
        let screened = |tile: Pos| -> bool {
            front_tiles
                .iter()
                .any(|front| g.wdist(tile, *front) == 1 && depth(*front) < depth(tile))
        };

        // Shooter slots: at range from the most finishable targets, in line
        // of sight, behind a front slot; where no front slot covers a tile at
        // range, one tile further back instead — a shooter the plan places
        // has no shot this turn, and standing unscreened at range only puts
        // it inside the enemy foot's reach for nothing, where a tile back it
        // steps in and fires next turn (measured: four archers and two
        // warriors read −69 ± 29 a seed with the unscreened tile at range
        // taken first). Unscreened at range is the last resort.
        let n_shooter = by_role.get(&SlotRole::Shooter).map_or(0, Vec::len);
        if n_shooter > 0 {
            let range = by_role[&SlotRole::Shooter]
                .iter()
                .map(|uid| g.unit_attack_range(*uid).max(1))
                .min()
                .unwrap_or(1);
            let aims: Vec<Pos> = if contacts.is_empty() {
                vec![target]
            } else {
                contacts
                    .iter()
                    .take(SHOOTER_TARGETS)
                    .map(|contact| contact.pos)
                    .collect()
            };
            let mut candidates: Vec<(u8, f64, i32, Pos)> = pool
                .iter()
                .copied()
                .filter(|tile| !taken.contains(tile) && depth(*tile) >= range.min(2))
                .filter_map(|tile| {
                    let at_range = aims.iter().any(|aim| {
                        g.wdist(tile, *aim) == range && g.line_of_sight_from(tile, *aim)
                    });
                    let standoff = depth(tile) > range
                        && aims.iter().any(|aim| g.wdist(tile, *aim) == range + 1);
                    let tier = if at_range && screened(tile) {
                        0
                    } else if standoff {
                        1
                    } else if at_range {
                        2
                    } else {
                        return None;
                    };
                    Some((
                        tier,
                        g.tile_defense_bonus(tile),
                        g.wdist(tile, group.anchor),
                        tile,
                    ))
                })
                .collect();
            candidates.sort_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| b.1.total_cmp(&a.1))
                    .then_with(|| a.2.cmp(&b.2))
                    .then_with(|| a.3.cmp(&b.3))
            });
            for (_, _, _, tile) in candidates.into_iter().take(n_shooter) {
                taken.insert(tile);
                slots.push(Slot {
                    role: SlotRole::Shooter,
                    tile,
                    depth: depth(tile),
                });
            }
        }

        // Siege slots: in range of the objective city, behind a front slot,
        // the furthest standoff first.
        let n_siege = by_role.get(&SlotRole::Siege).map_or(0, Vec::len);
        if n_siege > 0 {
            let range = by_role[&SlotRole::Siege]
                .iter()
                .map(|uid| g.unit_attack_range(*uid).max(1))
                .min()
                .unwrap_or(1);
            let mut candidates: Vec<(bool, i32, f64, i32, Pos)> = pool
                .iter()
                .copied()
                .filter(|tile| {
                    let reach = g.wdist(*tile, target);
                    !taken.contains(tile)
                        && reach <= range
                        && reach >= 2.min(range)
                        && g.line_of_sight_from(*tile, target)
                })
                .map(|tile| {
                    (
                        screened(tile),
                        g.wdist(tile, target),
                        g.tile_defense_bonus(tile),
                        g.wdist(tile, group.anchor),
                        tile,
                    )
                })
                .collect();
            candidates.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| b.1.cmp(&a.1))
                    .then_with(|| b.2.total_cmp(&a.2))
                    .then_with(|| a.3.cmp(&b.3))
                    .then_with(|| a.4.cmp(&b.4))
            });
            for (_, _, _, _, tile) in candidates.into_iter().take(n_siege) {
                taken.insert(tile);
                slots.push(Slot {
                    role: SlotRole::Siege,
                    tile,
                    depth: depth(tile),
                });
            }
        }

        // Support slots: beside the most front slots, behind the line.
        let n_support = by_role.get(&SlotRole::Support).map_or(0, Vec::len);
        if n_support > 0 {
            let mut candidates: Vec<(usize, f64, i32, Pos)> = pool
                .iter()
                .copied()
                .filter(|tile| !taken.contains(tile) && depth(*tile) >= 2)
                .map(|tile| {
                    let beside = front_tiles
                        .iter()
                        .filter(|front| g.wdist(tile, **front) == 1)
                        .count();
                    (
                        beside,
                        g.tile_defense_bonus(tile),
                        g.wdist(tile, group.anchor),
                        tile,
                    )
                })
                .collect();
            candidates.sort_by(|a, b| {
                b.0.cmp(&a.0)
                    .then_with(|| b.1.total_cmp(&a.1))
                    .then_with(|| a.2.cmp(&b.2))
                    .then_with(|| a.3.cmp(&b.3))
            });
            for (_, _, _, tile) in candidates.into_iter().take(n_support) {
                taken.insert(tile);
                slots.push(Slot {
                    role: SlotRole::Support,
                    tile,
                    depth: depth(tile),
                });
            }
        }

        // Heal slots first: a wounded unit steps out of the line, so its
        // tile is one the line can use.
        let mut placements: Vec<Placement> = Vec::new();
        let mut reserved: BTreeSet<Pos> = taken.clone();
        for uid in &wounded {
            let unit = &g.units[uid];
            let mut tiles = vec![unit.pos];
            tiles.extend(g.reachable(*uid));
            let best = tiles
                .into_iter()
                .filter(|tile| !reserved.contains(tile) && field.danger(*tile, *uid) <= NO_DANGER)
                .map(|tile| {
                    (
                        -heal_preference(g, pid, tile),
                        g.wdist(unit.pos, tile),
                        tile,
                    )
                })
                .min();
            if let Some((_, _, tile)) = best {
                reserved.insert(tile);
                placements.push(Placement {
                    unit: *uid,
                    tile,
                    role: None,
                    depth: -1,
                });
            }
        }

        // Units to slots, a role at a time: rows are the slots, columns the
        // units, so every slot is filled when a unit can take it.
        for (role, units) in &by_role {
            let role_slots: Vec<&Slot> = slots.iter().filter(|slot| slot.role == *role).collect();
            if role_slots.is_empty() {
                continue;
            }
            let reach: Vec<Vec<Pos>> = units.iter().map(|uid| g.reachable(*uid)).collect();
            let cost: Vec<Vec<f64>> = role_slots
                .iter()
                .map(|slot| {
                    units
                        .iter()
                        .zip(&reach)
                        .map(|(uid, reachable)| {
                            self.slot_cost(g, field, *uid, slot.tile, reachable)
                        })
                        .collect()
                })
                .collect();
            let assignment = assign_min_cost(&cost);
            for (row, column) in assignment.into_iter().enumerate() {
                if cost[row][column] >= UNASSIGNABLE {
                    continue;
                }
                let slot = role_slots[row];
                placements.push(Placement {
                    unit: units[column],
                    tile: slot.tile,
                    role: Some(slot.role),
                    depth: slot.depth,
                });
            }
        }
        placements.sort_by_key(|placement| (placement.depth, placement.role, placement.unit));

        // The pace floor, on the approach only.
        let pace_floor = (!in_contact).then(|| {
            let pace = members
                .iter()
                .map(|uid| g.unit_max_moves(*uid).floor() as i32)
                .min()
                .unwrap_or(1)
                .max(1);
            let rear = members
                .iter()
                .filter(|uid| g.unit_max_moves(**uid).floor() as i32 == pace)
                .map(|uid| g.wdist(g.units[uid].pos, target))
                .max()
                .unwrap_or(0);
            (rear - (pace + BODY_PACE_SLACK), pace)
        });
        Some(PositionPlan {
            group: group.id,
            target,
            contacts: contacts.len(),
            slots,
            placements,
            pace_floor,
        })
    }

    /// What a unit pays to take a slot: the route in turns plus the danger
    /// there past the hit points it can spare, in turns. `UNASSIGNABLE` for a
    /// slot it cannot reach or would not survive.
    fn slot_cost(
        &self,
        g: &Game,
        field: &mut DangerField,
        uid: u32,
        tile: Pos,
        reachable: &[Pos],
    ) -> f64 {
        let unit = &g.units[&uid];
        let danger = field.danger(tile, uid);
        if danger >= f64::from(unit.hp) {
            return UNASSIGNABLE;
        }
        let turns = if tile == unit.pos {
            0.0
        } else if reachable.contains(&tile) {
            1.0
        } else {
            match g.route_distance(uid, tile, 0) {
                Some(steps) => (steps as f64 / g.unit_max_moves(uid).max(1.0))
                    .ceil()
                    .max(1.0),
                None => return UNASSIGNABLE,
            }
        };
        turns + (danger - f64::from(unit.hp - DANGER_FREE_HP)).max(0.0) * DANGER_COST_PER_HP
    }

    /// Where a placed unit ends this turn: its slot if it can reach it, else
    /// the reachable tile nearest the slot that no one else has reserved,
    /// that would not kill it, and that keeps the pace floor. The flag says
    /// the floor changed the choice.
    fn position_end_tile(
        &self,
        g: &Game,
        field: &mut DangerField,
        uid: u32,
        slot: Pos,
        reserved: &BTreeSet<Pos>,
        plan: &PositionPlan,
    ) -> (Pos, bool) {
        let target = plan.target;
        let pace_floor = plan.pace_floor.map(|(floor, _)| floor);
        let unit = &g.units[&uid];
        let hp = f64::from(unit.hp);
        let mut options = vec![unit.pos];
        options.extend(g.reachable(uid));
        let free: Vec<Pos> = options
            .into_iter()
            .filter(|tile| *tile == slot || !reserved.contains(tile))
            .collect();
        let mut keyed: Vec<(i32, bool, Pos)> = free
            .iter()
            .map(|tile| (g.wdist(*tile, slot), field.danger(*tile, uid) >= hp, *tile))
            .collect();
        keyed.sort_unstable();
        let unbounded = keyed.first().map(|(_, _, tile)| *tile);
        let bounded = keyed
            .iter()
            .find(|(_, _, tile)| pace_floor.is_none_or(|floor| g.wdist(*tile, target) >= floor))
            .map(|(_, _, tile)| *tile);
        match bounded {
            Some(tile) => (tile, unbounded != Some(tile)),
            None => (unit.pos, unbounded.is_some_and(|tile| tile != unit.pos)),
        }
    }

    /// Lay out and play every advancing or engaging force's positions.
    /// Nothing is read with version two off.
    fn plan_positions(
        &mut self,
        g: &mut Game,
        pid: usize,
        field: &mut DangerField,
        armed: &BTreeSet<u32>,
    ) {
        if !self.positions_plan_on() {
            return;
        }
        let groups = self.force_groups.clone();
        for group in &groups {
            let Some(plan) = self.position_plan(g, pid, group, field, armed) else {
                continue;
            };
            let (placed, paced) = self.apply_position_plan(g, pid, field, &plan);
            self.census.battle_plan_slots += plan.slots.len() as u32;
            self.census.battle_plan_positioned += placed;
            self.census.battle_plan_paced += paced;
            think!(self.journal(), Military, Decision,
                "Battle plan: a {} force of {} takes {} slot(s) — {} unit(s) placed, {} paced",
                group.domain.as_str(), group.units.len(), plan.slots.len(), placed, paced;
                "target {:?}, {} contact(s), {} heal slot(s){}",
                plan.target, plan.contacts,
                plan.placements.iter().filter(|placement| placement.role.is_none()).count(),
                plan.pace_floor.map_or(String::new(), |(floor, pace)| {
                    format!(", pace {pace}: no unit ends nearer the target than {floor}")
                });
                plan.target);
        }
    }

    /// Play one force's plan: front to rear, each unit walking to its end
    /// tile against the reservations, two units that stand on each other's
    /// slots swapping, and a second pass for anyone the first left short.
    /// Returns how many units the plan placed and how many the pace held.
    fn apply_position_plan(
        &mut self,
        g: &mut Game,
        pid: usize,
        field: &mut DangerField,
        plan: &PositionPlan,
    ) -> (u32, u32) {
        let heals = !g.is_arena() || g.tactics.heal;
        let mut reserved: BTreeSet<Pos> = plan.placements.iter().map(|p| p.tile).collect();
        let mut placed = 0u32;
        let mut paced = 0u32;
        let floor = plan.pace_floor.map(|(floor, _)| floor);
        for placement in &plan.placements {
            let uid = placement.unit;
            let Some(unit) = g.units.get(&uid).cloned() else {
                continue;
            };
            if unit.moves_left <= 0.0 || self.battle_planner_ordered.contains(&uid) {
                continue;
            }
            let heal = placement.role.is_none();
            if unit.pos == placement.tile {
                self.base.fortify_or_stop(g, pid, uid);
                self.battle_planner_ordered.insert(uid);
                if heal && heals {
                    self.battle_planner_recovering.insert(uid);
                }
                placed += 1;
                continue;
            }
            // Two units standing on each other's slots trade places.
            let counterpart = g.unit_ids_at(placement.tile).iter().copied().find(|other| {
                plan.placements
                    .iter()
                    .any(|q| q.unit == *other && q.tile == unit.pos)
                    && !self.battle_planner_ordered.contains(other)
            });
            if let Some(other) = counterpart {
                if g.wdist(unit.pos, placement.tile) == 1
                    && g.apply(pid, &Action::Swap { unit: uid, other }).is_ok()
                {
                    self.battle_planner_ordered.insert(uid);
                    self.battle_planner_ordered.insert(other);
                    if heal && heals {
                        self.battle_planner_recovering.insert(uid);
                    }
                    placed += 2;
                    continue;
                }
            }
            let (end, held) =
                self.position_end_tile(g, field, uid, placement.tile, &reserved, plan);
            if end == unit.pos {
                // Nothing nearer the slot is open this turn: hold here.
                self.base.fortify_or_stop(g, pid, uid);
                self.battle_planner_ordered.insert(uid);
                placed += 1;
                paced += u32::from(held);
                continue;
            }
            if !self.base.path_walk_to(g, pid, uid, end) {
                // Refused: the ladder plays this unit as before.
                continue;
            }
            reserved.insert(end);
            self.battle_planner_ordered.insert(uid);
            if heal && heals {
                self.battle_planner_recovering.insert(uid);
            }
            placed += 1;
            paced += u32::from(held);
        }
        // Second pass: a unit that stopped short because its slot was still
        // occupied walks on once the occupant has left.
        for placement in &plan.placements {
            let uid = placement.unit;
            let Some(unit) = g.units.get(&uid) else {
                continue;
            };
            if !self.battle_planner_ordered.contains(&uid)
                || unit.pos == placement.tile
                || unit.moves_left <= 0.0
                || floor.is_some_and(|floor| g.wdist(placement.tile, plan.target) < floor)
                || !g.reachable(uid).contains(&placement.tile)
            {
                continue;
            }
            self.base.path_walk_to(g, pid, uid, placement.tile);
        }
        (placed, paced)
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

    /// A force of every unit seat 0 owns, anchored on its medoid the way
    /// `rebuild_force_groups` anchors it, in the given posture toward
    /// `objective`.
    fn force(g: &Game, objective: Pos, posture: ForcePosture) -> ForceGroup {
        let mut units = g.player_unit_ids(0);
        units.sort_unstable();
        let anchor = units
            .iter()
            .map(|uid| g.units[uid].pos)
            .min_by_key(|pos| {
                (
                    units
                        .iter()
                        .map(|other| g.wdist(*pos, g.units[other].pos))
                        .sum::<i32>(),
                    *pos,
                )
            })
            .expect("a force");
        ForceGroup {
            id: units[0],
            domain: super::super::ForceDomain::Land,
            units,
            anchor,
            objective,
            focus_target: None,
            posture,
            readiness: 1.0,
            local_strength_ratio: 1.0,
        }
    }

    fn version_two() -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.enable_battle_planner_2();
        ai
    }

    fn version_three() -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.enable_battle_planner_3();
        ai
    }

    /// File the host's reading of one pair on the board.
    fn host_preview(
        g: &mut Game,
        uid: u32,
        target: Pos,
        ranged: bool,
        to_attacker: i32,
        to_defender: i32,
    ) {
        let mut previews = (*g.host_previews).clone();
        previews.insert(
            (uid, target, ranged),
            crate::game::HostStrikePreview {
                attacker_strength: 0.0,
                defender_strength: 0.0,
                damage_to_attacker: to_attacker,
                damage_to_defender: to_defender,
                defender_wall_damage: 0,
            },
        );
        g.host_previews = Arc::new(previews);
    }

    #[test]
    fn version_three_ships_off_is_registered_and_turns_the_others_off() {
        let ai = AdvancedAi::new();
        assert!(!ai.battle_planner_3, "an opt-in ships off");
        assert!(ai.wanted_previews().is_empty());
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.opt_in() && gene.field == "battle_planner_3"));
        let mut on = AdvancedAi::new();
        on.enable_battle_planner();
        on.enable_battle_planner_2();
        assert_eq!((on.battle_planner, on.battle_planner_2), (false, true));
        on.enable_battle_planner_3();
        assert_eq!(
            (on.battle_planner, on.battle_planner_2, on.battle_planner_3),
            (false, false, true),
            "one version of the family plays: version three turns one and two off"
        );
        assert!(on.battle_planner_on() && on.positions_plan_on());
        on.disable_battle_planner_3();
        assert!(!on.battle_planner_3 && !on.battle_planner_on());
        super::super::test_support::opt_in_off_in_both_controllers("battle-planner-3", |ai| {
            ai.battle_planner_3
        });
    }

    /// A unit the siege has reserved as its taker is not spent by the kill
    /// plan, not rotated, not placed: version two plans it, version three
    /// leaves it to the siege train.
    #[test]
    fn a_reserved_taker_is_not_planned() {
        let mut g = open_field();
        let taker = g.spawn_unit("warrior", 0, at(9, 7));
        let victim = g.spawn_unit("warrior", 1, at(10, 7));
        wound(&mut g, victim, 25);
        let mut v2 = version_two();
        assert!(
            v2.kill_sequence(&g, 0)
                .iter()
                .any(|blow| blow.unit == taker),
            "version two spends the warrior on the wounded enemy"
        );
        v2.reserved_units.insert(taker);
        assert!(
            v2.kill_sequence(&g, 0)
                .iter()
                .any(|blow| blow.unit == taker),
            "version two does not read the reservation"
        );
        let mut v3 = version_three();
        v3.reserved_units.insert(taker);
        assert!(
            v3.kill_sequence(&g, 0).is_empty(),
            "the taker is the siege's"
        );
        let plan = conquest(&g);
        v3.force_groups = vec![force(&g, at(10, 7), ForcePosture::Engage)];
        assert!(!v3.plan_battle(&mut g, 0, &plan));
        assert!(g.units.contains_key(&victim), "nothing struck");
        assert_eq!(g.units[&taker].pos, at(9, 7), "not rotated, not placed");
        assert_eq!(v3.census.battle_plan_positioned, 0);
        // Released, the same unit is planned again.
        v3.reserved_units.clear();
        assert!(!v3.kill_sequence(&g, 0).is_empty());
    }

    /// The host's reading of a pair replaces the closed form: an archer's
    /// shot the host prices at 47 plans 47, whatever the centre roll says;
    /// and the pairs without a reading are what the plan asks the host for.
    #[test]
    fn a_host_preview_overrides_the_closed_form() {
        let mut g = open_field();
        let archer = g.spawn_unit("archer", 0, at(8, 7));
        let victim = g.spawn_unit("warrior", 1, at(10, 7));
        wound(&mut g, victim, 60);
        let v3 = version_three();
        let closed: Vec<Blow> = v3.kill_sequence(&g, 0);
        let shot = closed
            .iter()
            .find(|blow| blow.unit == archer && blow.from == at(8, 7))
            .expect("the archer shoots from its tile");
        assert!(
            (shot.expected - 47.0).abs() > 1.0,
            "the fixture's centre roll is not 47"
        );
        // Asked for: the pair, before the host has answered.
        let mut ai = version_three();
        let plan = conquest(&g);
        let mut board = g.clone();
        ai.plan_battle(&mut board, 0, &plan);
        assert!(
            ai.wanted_previews().contains(&(archer, at(10, 7), true)),
            "{:?}",
            ai.wanted_previews()
        );
        // Answered: the plan carries the host's number.
        host_preview(&mut g, archer, at(10, 7), true, 0, 47);
        let priced = v3.kill_sequence(&g, 0);
        let shot = priced
            .iter()
            .find(|blow| blow.unit == archer && blow.from == at(8, 7))
            .expect("the archer still shoots from its tile");
        assert_eq!(shot.expected, 47.0);
        let mut ai = version_three();
        ai.plan_battle(&mut g.clone(), 0, &plan);
        assert!(
            !ai.wanted_previews().contains(&(archer, at(10, 7), true)),
            "an answered pair is not asked again"
        );
        // Version two never reads the host.
        let v2 = version_two();
        let shot = v2
            .kill_sequence(&g, 0)
            .into_iter()
            .find(|blow| blow.unit == archer && blow.from == at(8, 7))
            .expect("version two shoots too");
        assert!((shot.expected - 47.0).abs() > 1.0);
    }

    /// A blow the host says kills the attacker is vetoed, however the closed
    /// form prices it and whatever the kill is worth.
    #[test]
    fn a_preview_predicted_death_is_vetoed() {
        let mut g = open_field();
        let ours = g.spawn_unit("warrior", 0, at(9, 7));
        let victim = g.spawn_unit("warrior", 1, at(10, 7));
        wound(&mut g, victim, 25);
        let v3 = version_three();
        assert!(
            v3.kill_sequence(&g, 0)
                .iter()
                .any(|blow| blow.unit == ours && !blow.ranged),
            "the closed form finishes the wounded warrior"
        );
        host_preview(&mut g, ours, at(10, 7), false, 100, 60);
        assert!(
            v3.kill_sequence(&g, 0).is_empty(),
            "the host says the attacker dies: vetoed"
        );
        // A survivable host reading plans the blow with the host's damage.
        host_preview(&mut g, ours, at(10, 7), false, 12, 60);
        let blows = v3.kill_sequence(&g, 0);
        let blow = blows
            .iter()
            .find(|blow| blow.unit == ours)
            .expect("the survivable blow is planned");
        assert_eq!(blow.expected, 60.0);
        assert!(blow.finishes);
    }

    #[test]
    fn version_two_ships_off_is_registered_and_turns_version_one_off() {
        let ai = AdvancedAi::new();
        assert!(!ai.battle_planner_2, "an opt-in ships off");
        assert!(!ai.battle_planner_on());
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.opt_in() && gene.field == "battle_planner_2"));
        let mut on = AdvancedAi::new();
        on.enable_battle_planner();
        assert!((on.battle_planner, on.battle_planner_2) == (true, false));
        on.enable_battle_planner_2();
        assert_eq!(
            (on.battle_planner, on.battle_planner_2),
            (false, true),
            "one version of the family plays: version two turns version one off"
        );
        assert!(on.battle_planner_on());
        on.disable_battle_planner_2();
        assert!(!on.battle_planner_2 && !on.battle_planner_on());
        super::super::test_support::opt_in_off_in_both_controllers("battle-planner-2", |ai| {
            ai.battle_planner_2
        });
        // And version one alone leaves the positions plan out: nothing is
        // laid out, nothing is counted.
        let mut g = open_field();
        g.spawn_unit("warrior", 0, at(9, 6));
        g.spawn_unit("archer", 0, at(8, 6));
        g.spawn_unit("warrior", 1, at(12, 6));
        let mut v1 = AdvancedAi::new();
        v1.enable_battle_planner();
        v1.force_groups = vec![force(&g, at(12, 6), ForcePosture::Advance)];
        let plan = conquest(&g);
        v1.plan_battle(&mut g, 0, &plan);
        assert_eq!(v1.census.battle_plan_slots, 0);
        assert_eq!(v1.census.battle_plan_positioned, 0);
    }

    /// A fortified enemy warrior two tiles off, a hill on the tile between:
    /// the warrior takes the front slot on the hill and the archer ends two
    /// tiles from the enemy beside the warrior and behind it. The slots are
    /// laid against the enemy, not the anchor. Planned with no unit armed
    /// and played by `apply_position_plan`: through `plan_battle` this
    /// archer has a shot to offer and is the ladder's — the next test.
    #[test]
    fn the_warrior_takes_the_hill_in_front_and_the_archer_stands_two_behind_it() {
        let mut g = open_field();
        let enemy_tile = at(12, 6);
        let hill = at(11, 6);
        g.map.tiles.get_mut(&hill).expect("a tile").hills = true;
        let warrior = g.spawn_unit("warrior", 0, at(10, 6));
        let archer = g.spawn_unit("archer", 0, at(9, 6));
        let enemy = g.spawn_unit("warrior", 1, enemy_tile);
        fortify(&mut g, enemy);
        let mut ai = version_two();
        let group = force(&g, enemy_tile, ForcePosture::Advance);
        let mut field = DangerField::new(&g, 0);
        let plan = ai
            .position_plan(&g, 0, &group, &mut field, &BTreeSet::new())
            .expect("an advancing force of two is planned");
        assert_eq!(plan.contacts, 1);
        let front: Vec<&Slot> = plan
            .slots
            .iter()
            .filter(|slot| slot.role == SlotRole::Front)
            .collect();
        assert_eq!(front.len(), 1, "{:?}", plan.slots);
        assert_eq!(front[0].tile, hill, "the front slot is the hill");
        let shooter: Vec<&Slot> = plan
            .slots
            .iter()
            .filter(|slot| slot.role == SlotRole::Shooter)
            .collect();
        assert_eq!(shooter.len(), 1, "{:?}", plan.slots);
        assert_eq!(g.wdist(shooter[0].tile, enemy_tile), 2, "at range");
        assert_eq!(g.wdist(shooter[0].tile, hill), 1, "behind the front slot");
        let placed: BTreeMap<u32, Pos> = plan
            .placements
            .iter()
            .map(|placement| (placement.unit, placement.tile))
            .collect();
        assert_eq!(placed[&warrior], hill);
        assert_eq!(placed[&archer], shooter[0].tile);
        assert_eq!(
            plan.placements[0].unit, warrior,
            "the front moves first: {:?}",
            plan.placements
        );
        assert!(plan.pace_floor.is_none(), "in contact, nothing is paced");

        let (placed, paced) = ai.apply_position_plan(&mut g, 0, &mut field, &plan);
        assert_eq!((placed, paced), (2, 0));
        let (w, a) = (g.units[&warrior].pos, g.units[&archer].pos);
        assert_eq!(w, hill, "the warrior stands on the hill");
        assert_eq!(
            g.wdist(a, enemy_tile),
            2,
            "the archer ends two from the enemy"
        );
        assert_eq!(g.wdist(a, w), 1, "beside the warrior");
        assert!(
            g.wdist(w, enemy_tile) < g.wdist(a, enemy_tile),
            "and behind it"
        );
        assert!(ai.battle_planner_claims(warrior) && ai.battle_planner_claims(archer));
        assert!(g.units.contains_key(&enemy));
    }

    /// Through `plan_battle`: an archer already standing beside the warrior
    /// on a tile two from the enemy with line of sight has a blow to offer —
    /// whether the kill plan spends it or declines it, the positions plan
    /// leaves it to the ladder, as version one would. The warrior, which
    /// cannot close and strike in one turn, is placed on the hill in front.
    #[test]
    fn a_unit_with_a_blow_to_offer_is_left_to_the_ladder() {
        let mut g = open_field();
        let enemy_tile = at(12, 6);
        let hill = at(11, 6);
        g.map.tiles.get_mut(&hill).expect("a tile").hills = true;
        let warrior = g.spawn_unit("warrior", 0, at(10, 6));
        let stand = g
            .wdisk(enemy_tile, 2)
            .into_iter()
            .filter(|tile| {
                g.wdist(*tile, enemy_tile) == 2
                    && g.wdist(*tile, at(10, 6)) == 1
                    && g.line_of_sight_from(*tile, enemy_tile)
            })
            .min()
            .expect("a firing tile beside the warrior");
        let archer = g.spawn_unit("archer", 0, stand);
        let enemy = g.spawn_unit("warrior", 1, enemy_tile);
        fortify(&mut g, enemy);
        let mut ai = version_two();
        let mut field = DangerField::new(&g, 0);
        let (blows, armed, _, _) = ai.kill_sequence_in(&g, 0, &mut field);
        assert!(armed.contains(&archer), "the archer has a shot to offer");
        assert!(
            !armed.contains(&warrior),
            "the warrior cannot close and strike"
        );
        ai.force_groups = vec![force(&g, enemy_tile, ForcePosture::Advance)];
        let strategic = conquest(&g);
        let struck = ai.plan_battle(&mut g, 0, &strategic);
        assert_eq!(struck, !blows.is_empty());
        assert_eq!(g.units[&warrior].pos, hill, "the warrior is placed");
        assert!(ai.battle_planner_claims(warrior));
        if blows.is_empty() {
            assert!(
                !ai.battle_planner_claims(archer),
                "a declined shooter is the ladder's, not the plan's"
            );
            assert_eq!(g.units[&archer].pos, stand, "and has not been moved");
        }
        assert_eq!(ai.census.battle_plan_slots, 1, "one slot: the front's");
        assert_eq!(ai.census.battle_plan_positioned, 1);
    }

    /// Three warriors and two archers against a pair of enemy warriors: every
    /// slot is a distinct tile, every placement is a distinct tile, and after
    /// the moves every unit stands on its own tile. Everyone stands three or
    /// more tiles from the enemy, so no blow is legal this turn — a melee
    /// unit that steps beside an enemy has spent its movement, and the
    /// archers cannot close to range — and the positions plan places all.
    #[test]
    fn two_units_never_receive_the_same_tile() {
        let mut g = open_field();
        for (col, row) in [(8, 5), (8, 6), (8, 7)] {
            g.spawn_unit("warrior", 0, at(col, row));
        }
        g.spawn_unit("archer", 0, at(6, 6));
        g.spawn_unit("archer", 0, at(6, 7));
        let first = g.spawn_unit("warrior", 1, at(11, 6));
        let second = g.spawn_unit("warrior", 1, at(11, 7));
        fortify(&mut g, first);
        fortify(&mut g, second);
        let mut ai = version_two();
        let group = force(&g, at(11, 6), ForcePosture::Engage);
        let mut field = DangerField::new(&g, 0);
        let plan = ai
            .position_plan(&g, 0, &group, &mut field, &BTreeSet::new())
            .expect("planned");
        let slot_tiles: BTreeSet<Pos> = plan.slots.iter().map(|slot| slot.tile).collect();
        assert_eq!(slot_tiles.len(), plan.slots.len(), "{:?}", plan.slots);
        assert_eq!(plan.slots.len(), 5, "one slot per unit: {:?}", plan.slots);
        let placed: BTreeSet<Pos> = plan.placements.iter().map(|p| p.tile).collect();
        assert_eq!(placed.len(), plan.placements.len(), "{:?}", plan.placements);
        let units: BTreeSet<u32> = plan.placements.iter().map(|p| p.unit).collect();
        assert_eq!(units.len(), plan.placements.len(), "one slot per unit");
        assert!(plan
            .slots
            .iter()
            .filter(|slot| slot.role == SlotRole::Front)
            .all(|slot| slot.depth == 1));
        ai.force_groups = vec![group];
        let strategic = conquest(&g);
        assert!(!ai.plan_battle(&mut g, 0, &strategic), "no blow is legal");
        assert_eq!(ai.census.battle_plan_kills, 0);
        let standing: BTreeSet<Pos> = g
            .player_unit_ids(0)
            .iter()
            .map(|uid| g.units[uid].pos)
            .collect();
        assert_eq!(
            standing.len(),
            5,
            "{:?} / {:?}",
            g.player_unit_ids(0)
                .iter()
                .map(|uid| (uid, g.units[uid].pos, g.units[uid].hp))
                .collect::<Vec<_>>(),
            plan.placements
        );
        assert_eq!(ai.census.battle_plan_positioned, 5, "everyone is placed");
    }

    /// A spearman and a horseman on the approach, the enemy far off: the
    /// horseman ends no more than the spearman's pace plus one closer to the
    /// objective than the spearman stood, though it could ride four.
    #[test]
    fn the_horseman_is_paced_to_the_spearman_on_the_approach() {
        let mut g = open_field();
        let spearman = g.spawn_unit("spearman", 0, at(4, 6));
        let horseman = g.spawn_unit("horseman", 0, at(5, 6));
        let enemy = at(19, 6);
        g.spawn_unit("warrior", 1, enemy);
        g.spawn_unit("warrior", 1, at(20, 6));
        let mut ai = version_two();
        let group = force(&g, enemy, ForcePosture::Advance);
        let mut field = DangerField::new(&g, 0);
        let plan = ai
            .position_plan(&g, 0, &group, &mut field, &BTreeSet::new())
            .expect("planned");
        assert_eq!(plan.contacts, 0, "no enemy within the band");
        let spear_start = g.wdist(at(4, 6), enemy);
        assert_eq!(
            plan.pace_floor,
            Some((spear_start - (2 + BODY_PACE_SLACK), 2)),
            "the floor is the slowest member's distance less its pace and the slack"
        );
        assert_eq!(plan.placements.len(), 2);
        ai.force_groups = vec![group];
        let strategic = conquest(&g);
        ai.plan_battle(&mut g, 0, &strategic);
        let horse_end = g.wdist(g.units[&horseman].pos, enemy);
        let spear_end = g.wdist(g.units[&spearman].pos, enemy);
        assert!(
            horse_end >= spear_start - 3,
            "paced: the horseman ends at {horse_end}, the spearman started at {spear_start}"
        );
        assert!(
            horse_end < g.wdist(at(5, 6), enemy),
            "and still advances ({horse_end})"
        );
        assert!(
            spear_end < spear_start,
            "the spearman marches ({spear_end})"
        );
        assert_eq!(ai.census.battle_plan_paced, 1);
        assert_eq!(ai.census.battle_plan_positioned, 2);
    }

    /// On a board that heals, a 55-hit-point warrior — above the rotation's
    /// bar, below the heal slot's — takes a zero-danger tile and fortifies
    /// while its healthy friend takes the front slot; it is then in the
    /// recovery the rotation keeps.
    #[test]
    fn a_wounded_unit_gets_a_heal_slot() {
        let mut g = open_field();
        g.tactics.heal = true;
        let enemy_tile = at(12, 6);
        let hill = at(11, 6);
        g.map.tiles.get_mut(&hill).expect("a tile").hills = true;
        let healthy = g.spawn_unit("warrior", 0, at(10, 6));
        let hurt = g.spawn_unit("warrior", 0, at(8, 6));
        let enemy = g.spawn_unit("warrior", 1, enemy_tile);
        fortify(&mut g, enemy);
        wound(&mut g, hurt, 55);
        assert!(
            danger(&g, 0, at(8, 6), hurt) <= NO_DANGER,
            "out of reach where it stands"
        );
        let mut ai = version_two();
        let group = force(&g, enemy_tile, ForcePosture::Advance);
        let mut field = DangerField::new(&g, 0);
        let plan = ai
            .position_plan(&g, 0, &group, &mut field, &BTreeSet::new())
            .expect("planned");
        let heal: Vec<&Placement> = plan
            .placements
            .iter()
            .filter(|placement| placement.role.is_none())
            .collect();
        assert_eq!(heal.len(), 1, "{:?}", plan.placements);
        assert_eq!(heal[0].unit, hurt);
        assert!(
            danger(&g, 0, heal[0].tile, hurt) <= NO_DANGER,
            "a heal slot is a tile the field reads as zero"
        );
        assert_eq!(
            plan.slots
                .iter()
                .filter(|slot| slot.role == SlotRole::Front)
                .count(),
            1,
            "the healthy warrior alone wants a front slot: {:?}",
            plan.slots
        );
        ai.force_groups = vec![group];
        let strategic = conquest(&g);
        ai.plan_battle(&mut g, 0, &strategic);
        let now = &g.units[&hurt];
        assert_eq!(now.pos, heal[0].tile);
        assert!(now.fortified, "it fortifies on its heal slot");
        assert!(
            ai.battle_planner_recovering.contains(&hurt),
            "and enters the recovery"
        );
        assert!(ai.battle_planner_claims(hurt));
        assert_eq!(
            g.units[&healthy].pos, hill,
            "the healthy warrior holds the front"
        );
        assert_eq!(ai.census.battle_plan_positioned, 2);
        assert_eq!(
            ai.census.battle_plan_rotations, 0,
            "the rotation did not fire"
        );
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

    /// `strike-reach` is an opt-in that ships off and is registered.
    #[test]
    fn strike_reach_ships_off_and_is_registered() {
        let ai = AdvancedAi::new();
        assert!(!ai.strike_reach, "an opt-in ships off");
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.opt_in() && gene.field == "strike_reach"));
        let mut on = AdvancedAi::new();
        on.enable_strike_reach();
        assert!(on.strike_reach);
        on.disable_strike_reach();
        assert!(!on.strike_reach);
        super::super::test_support::opt_in_off_in_both_controllers("strike-reach", |ai| {
            ai.strike_reach
        });
    }

    /// A melee enemy two tiles off: the movement flood writes zero movement
    /// into the tile beside us, so `attack_reach` — and the field on it —
    /// read zero where we stand; the strike reach knows the unit keeps its
    /// movement for the blow when our zone of control stops it, and reads
    /// the blow. Three tiles off, both read zero.
    #[test]
    fn a_melee_enemy_two_tiles_off_is_no_danger_to_the_flood_and_a_blow_to_the_strike_reach() {
        let mut g = open_field();
        let ours = g.spawn_unit("warrior", 0, at(10, 6));
        let enemy = g.spawn_unit("warrior", 1, at(12, 6));
        let tile = at(10, 6);
        assert_eq!(g.wdist(g.units[&enemy].pos, tile), 2);
        assert!(g.unit_max_moves(enemy) >= 2.0);
        assert!(
            !g.attack_reach(enemy).contains(&tile),
            "the flood stops at our zone of control"
        );
        assert_eq!(danger(&g, 0, tile, ours), 0.0);
        let read = strike_danger(&g, 0, tile, ours);
        let (att, def) = g.melee_exchange_strengths(enemy, ours).expect("a pair");
        assert!(
            read > 0.0,
            "the strike reach prices the closing blow: {read}"
        );
        assert!(
            (read - expected_damage(att, def)).abs() < 1e-9,
            "{read} v {}",
            expected_damage(att, def)
        );
        // Beyond one move and a blow, nothing.
        let far = at(9, 6);
        assert_eq!(g.wdist(g.units[&enemy].pos, far), 3);
        assert_eq!(strike_danger(&g, 0, far, ours), 0.0);
        assert_eq!(danger(&g, 0, far, ours), 0.0);
    }

    /// The same board, played: a 30-hit-point warrior two tiles from the
    /// enemy holds where it stands under the flood's reading — its own tile
    /// reads zero — and under `strike-reach` steps to a tile the closing blow
    /// cannot follow it to.
    #[test]
    fn with_strike_reach_the_wounded_unit_steps_out_of_the_closing_blow() {
        let mut g = open_field();
        g.tactics.heal = true;
        let hurt = g.spawn_unit("warrior", 0, at(10, 6));
        let enemy = g.spawn_unit("warrior", 1, at(12, 6));
        wound(&mut g, hurt, 30);
        let plan = conquest(&g);
        let mut flood = g.clone();
        let mut v1 = AdvancedAi::new();
        v1.enable_battle_planner();
        v1.plan_battle(&mut flood, 0, &plan);
        assert_eq!(
            flood.units[&hurt].pos,
            at(10, 6),
            "the flood reads its tile as safe, so it holds"
        );
        assert!(flood.units[&hurt].fortified);
        let mut ai = AdvancedAi::new();
        ai.enable_battle_planner();
        ai.enable_strike_reach();
        ai.plan_battle(&mut g, 0, &plan);
        let after = &g.units[&hurt];
        assert_ne!(
            after.pos,
            at(10, 6),
            "it left the tile the enemy can close on"
        );
        assert!(
            g.wdist(after.pos, g.units[&enemy].pos) >= 3,
            "beyond one move and a blow"
        );
        assert!(after.fortified);
        assert!(strike_danger(&g, 0, after.pos, hurt) <= NO_DANGER);
        assert_eq!(ai.census.strike_reach_widened, 1);
        assert_eq!(ai.census.battle_plan_rotations, 1);
    }

    /// A siege unit behind a mountain — a shooter that fires only from its
    /// own tile, so a sidestep cannot open the line: no line of sight, so
    /// the flood and the strike reach on a native board both read zero — the
    /// engine's own `do_ranged` refuses that shot there. On the mirrored
    /// board the host's rule is the player's visibility, and the strike
    /// reach reads the shot.
    #[test]
    fn on_the_mirrored_board_a_shooter_needs_no_line_of_sight_of_its_own() {
        let mut g = open_field();
        let ours = g.spawn_unit("warrior", 0, at(10, 6));
        let archer = g.spawn_unit("catapult", 1, at(8, 6));
        let tile = at(10, 6);
        g.map.tiles.get_mut(&at(9, 6)).expect("a tile").terrain = "mountain".into();
        assert!(g.rules.units[g.units[&archer].kind].siege);
        assert!(
            !g.unit_has_line_of_sight_from(archer, at(8, 6), tile),
            "the mountain blocks the line"
        );
        assert!(!g.attack_reach(archer).contains(&tile));
        assert_eq!(danger(&g, 0, tile, ours), 0.0);
        assert_eq!(
            strike_danger(&g, 0, tile, ours),
            0.0,
            "a native board keeps the engine's rule"
        );
        // The live seat, shown the board by the host.
        let observed: BTreeSet<Pos> = g.map.tiles.keys().copied().collect();
        g.host_observed = Arc::new(observed);
        let read = strike_danger(&g, 0, tile, ours);
        assert!(read > 0.0, "the mirrored board reads the shot: {read}");
        assert_eq!(danger(&g, 0, tile, ours), 0.0, "the flood still does not");
    }

    /// `doomed-blow-veto` is an opt-in that ships off and is registered.
    #[test]
    fn doomed_blow_veto_ships_off_and_is_registered() {
        let ai = AdvancedAi::new();
        assert!(!ai.doomed_blow_veto, "an opt-in ships off");
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.opt_in() && gene.field == "doomed_blow_veto"));
        let mut on = AdvancedAi::new();
        on.enable_doomed_blow_veto();
        assert!(on.doomed_blow_veto);
        on.disable_doomed_blow_veto();
        assert!(!on.doomed_blow_veto);
        super::super::test_support::opt_in_off_in_both_controllers("doomed-blow-veto", |ai| {
            ai.doomed_blow_veto
        });
    }

    /// A warrior whose one blow — on an archer two tiles off along a clear
    /// row — is struck from a tile the archer and a swordsman both reach: the
    /// return plus the danger at the stand is over its hit points, so it is
    /// doomed. The same warrior against the archer alone has a blow it
    /// survives and is not.
    #[test]
    fn a_shooter_with_no_survivable_blow_is_doomed_and_one_with_a_survivable_blow_is_not() {
        let mut g = open_field();
        let ours = g.spawn_unit("warrior", 0, at(10, 4));
        let archer = g.spawn_unit("archer", 1, at(12, 4));
        let ai = AdvancedAi::new();
        {
            let mut field = DangerField::new(&g, 0);
            let (shooters, targets, candidates, armed) = ai.strike_candidates(&g, 0, &mut field);
            assert!(
                armed.contains(&ours),
                "the warrior has a blow on the archer"
            );
            assert!(
                candidates.iter().all(|c| c.from == at(11, 4)),
                "struck from the one stand beside it"
            );
            let doomed = doomed_shooters(&shooters, &targets, &candidates, &mut field);
            assert!(doomed.is_empty(), "the archer alone is a blow it survives");
        }
        let sword = g.spawn_unit("swordsman", 1, at(11, 5));
        assert_eq!(
            g.wdist(g.units[&sword].pos, at(11, 4)),
            1,
            "the swordsman covers the stand"
        );
        assert_eq!(
            g.wdist(g.units[&sword].pos, at(10, 4)),
            2,
            "and not the warrior's own tile"
        );
        let mut field = DangerField::new(&g, 0);
        let (shooters, targets, candidates, armed) = ai.strike_candidates(&g, 0, &mut field);
        assert!(armed.contains(&ours));
        let stand = field.danger(at(11, 4), ours);
        let home = field.danger(at(10, 4), ours);
        assert!(
            stand > 80.0,
            "the stand is nearly lethal on its own: {stand}"
        );
        assert!(
            home < 80.0,
            "the warrior is not exposed where it stands: {home}"
        );
        let doomed = doomed_shooters(&shooters, &targets, &candidates, &mut field);
        assert!(doomed.contains(&ours), "no blow it would survive");
        assert!(g.units.contains_key(&archer));
    }

    /// Played: with the gene off the doomed warrior is left to the ladder —
    /// the plan orders nothing for it; with the gene on, safe where it stands,
    /// it holds its ground and fortifies and the archer is untouched; wounded
    /// as well, it is the rotation's and steps out of reach.
    #[test]
    fn with_the_veto_the_doomed_unit_is_rotated_instead_of_left_to_attack() {
        let mut g = open_field();
        g.tactics.heal = true;
        let ours = g.spawn_unit("warrior", 0, at(10, 4));
        let archer = g.spawn_unit("archer", 1, at(12, 4));
        g.spawn_unit("swordsman", 1, at(11, 5));
        let plan = conquest(&g);
        let mut off_board = g.clone();
        let mut off = version_two();
        off.plan_battle(&mut off_board, 0, &plan);
        assert!(
            !off.battle_planner_ordered.contains(&ours),
            "off, the ladder has it"
        );
        assert_eq!(off.census.battle_plan_doomed, 0);
        let mut ai = version_two();
        ai.enable_doomed_blow_veto();
        ai.plan_battle(&mut g, 0, &plan);
        assert_eq!(ai.census.battle_plan_doomed, 1);
        assert!(ai.battle_planner_ordered.contains(&ours), "on, the plan's");
        let now = &g.units[&ours];
        assert_eq!(now.pos, at(10, 4), "safe where it stands, it holds its ground");
        assert!(now.fortified, "and fortifies");
        assert_eq!(ai.census.battle_plan_rotations, 0);
        assert!(g.units.contains_key(&archer), "and struck nothing");
        assert!(!ai.battle_planner_recovering.contains(&ours), "not a recovery");
        // Wounded as well, the same unit is the rotation's and steps out of reach.
        let mut g2 = off_board.clone();
        wound(&mut g2, ours, 40);
        let mut hurt = version_two();
        hurt.enable_doomed_blow_veto();
        hurt.plan_battle(&mut g2, 0, &plan);
        assert!(hurt.battle_planner_ordered.contains(&ours));
        assert_ne!(g2.units[&ours].pos, at(10, 4), "wounded, it steps out of reach");
        assert!(hurt.battle_planner_recovering.contains(&ours));
    }
}
