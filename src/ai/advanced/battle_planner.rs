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
//! **`safest-stand`** (opt-in, separate from the version family): the
//! rotation's answer when no tile in reach reads as zero. Until this the
//! rotation skipped such a unit in silence and the ladder played it — over
//! the seven live games of 2026-09-02 the largest row of the loss taxonomy
//! (38 of 145) was a unit killed under 50 hit points that the rotation had
//! logged nothing for, and twelve of those were a ladder attack by the
//! wounded unit. With the gene on, `least_danger_stand` ranks the unit's own
//! tile and every reachable tile by the field's reading, then the heal
//! preference, then the distance, and the unit takes the least — moving if it
//! is elsewhere, holding if it is here — and fortifies, marked ordered and
//! recovering like any rotation. Two refusals: a stand the field says would
//! still remove the unit is no stand (the ladder keeps it for a last blow),
//! and an exposed-but-healthy unit falls back only where the step cuts the
//! danger by `ROTATE_DANGER_MARGIN`. Census `battle_plan_fallbacks`; the
//! rotation's Decision line says "falls back to the least danger" or "holds
//! under fire and fortifies".
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
use super::last_orders::OrderStep;
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
/// `falling-city-outranks-the-heal`: a hostile city with no walls standing
/// at or under this health is falling.
pub(super) const FALLING_CITY_HP: i32 = 50;
/// `falling-city-outranks-the-heal`: how near a falling city calls a
/// recovering melee unit back: the muster line, two marching turns. The
/// G428 takers held to heal seven tiles from Yokohama.
pub(super) const FALLING_CITY_REACH: i32 = 8;
/// A unit under this is rotated out to heal where the board heals.
pub(super) const ROTATE_HP: i32 = 50;
/// A rotated unit rejoins the kill plan at this.
pub(super) const RETURN_HP: i32 = 80;
/// Danger above `hp - this` rotates a unit whatever its hit points.
pub(super) const ROTATE_DANGER_MARGIN: i32 = 20;
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
pub(super) fn wounded_penalty(hp: i32) -> f64 {
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
/// movement flood records it. The hostile is taken at the start of its own
/// turn — full movement, no zone-of-control memory from the turn it has just
/// ended — `approach_reach` gives the tiles it can end on with the movement
/// it keeps there (a zone-of-control stop keeps it; `flow_past` zeroes it),
/// and from its own tile and every stand with movement left it strikes each
/// neighbour (melee) or each tile in range (ranged; a siege unit only from
/// its own tile unless it may attack after moving). Ranged blows keep the
/// engine's line-of-sight test on a native board and drop it on the mirrored
/// one, where the host's rule is the player's visibility. Ascending and
/// distinct, like `attack_reach`.
///
/// ★★★ IT READS THE BOARD IT IS GIVEN. Until 2026-11-11 this took `&mut
/// Game`, wrote the hostile's turn state fresh, flooded, and wrote it back,
/// so every caller first made a copy of the game to hand it. Now the flood
/// is asked for the fresh turn directly
/// (`Game::approach_arrivals_on_a_fresh_turn`, where the argument that the
/// two are the same flood is written down). `strike_reach_tests` keeps the
/// old route as its reference.
pub(crate) fn strike_reach_of(g: &Game, pid: usize, uid: u32) -> Vec<Pos> {
    let Some(unit) = g.units.get(&uid) else {
        return Vec::new();
    };
    let spec = &g.rules.units[unit.kind];
    if spec.class != "military" || !(spec.is_melee_capable() || spec.has_ranged_attack()) {
        return Vec::new();
    }
    if spec.domain.as_deref() == Some("air") {
        return g.attack_reach(uid);
    }
    let max_moves = g.unit_max_moves(uid);
    if max_moves <= 0.0 {
        return Vec::new();
    }
    let mut stands: Vec<(Pos, f64)> = vec![(unit.pos, max_moves)];
    stands.extend(g.approach_arrivals_on_a_fresh_turn(uid));
    strikes_from_stands(g, pid, uid, stands)
}

/// The reference for [`strike_reach_of`]: the route it took before it read
/// the board, standing the hostile up on a board it may write to.
#[cfg(test)]
pub(crate) fn strike_reach_by_standing_the_hostile_up(
    probe: &mut Game,
    pid: usize,
    uid: u32,
) -> Vec<Pos> {
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
    if let Some(live) = probe.units.get_mut(&uid) {
        live.moves_left = max_moves;
        live.moved = false;
        live.acted = false;
        live.zoc_stopped = false;
        live.started_turn_in_zoc = false;
    }
    let mut stands: Vec<(Pos, f64)> = vec![(saved.pos, max_moves)];
    stands.extend(probe.approach_arrivals(uid));
    if let Some(live) = probe.units.get_mut(&uid) {
        *live = saved;
    }
    strikes_from_stands(probe, pid, uid, stands)
}

/// Every tile `uid` strikes from its own tile and from each stand it reaches
/// with movement left. `stands` leads with the unit's own tile.
fn strikes_from_stands(g: &Game, pid: usize, uid: u32, stands: Vec<(Pos, f64)>) -> Vec<Pos> {
    let unit = &g.units[&uid];
    let spec = &g.rules.units[unit.kind];
    let melee = spec.is_melee_capable();
    let ranged = spec.has_ranged_attack();
    let siege = spec.siege;
    let sea = spec.domain.as_deref() == Some("sea");
    let range = if ranged {
        g.unit_attack_range(uid).max(1)
    } else {
        0
    };
    let after_move = g.promotion_effect(unit, "attack_after_move") > 0.0;
    let host_sight = mirrored_board(g, pid);
    let mut targets: Vec<Pos> = Vec::new();
    for (from, kept) in stands {
        if kept <= 0.0 {
            continue;
        }
        // A land unit standing on water is embarked there and strikes nothing.
        let embarked = !sea && g.map.get(from).is_some_and(|tile| g.rules.is_water(tile));
        if embarked {
            continue;
        }
        if melee {
            for target in g.nbrs(from) {
                if g.map.tiles.contains_key(&target) && g.unit_can_melee_target_domain(uid, target)
                {
                    targets.push(target);
                }
            }
        }
        if ranged && (!siege || from == unit.pos || after_move) {
            for target in g.wdisk(from, range) {
                if target != from
                    && g.map.tiles.contains_key(&target)
                    && (host_sight || g.unit_has_line_of_sight_from(uid, from, target))
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
/// movement hazard; a unit source is named so a plan that kills it can leave
/// its blow out; a City Center or Encampment strike is named by its city id
/// tagged with [`STRUCTURE_SOURCE`], so `shared-danger` can split it too.
type Blows = Arc<Vec<(Option<u32>, f64)>>;

/// One source's blow before the roll (`DangerField::blow_pairs_at_hp`): a
/// movement hazard's planning damage, or a strike's two strengths as the
/// engine resolves it, each already through its own side's wound penalty.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum BlowPair {
    Hazard(f64),
    Strike { att: f64, def: f64 },
}

impl BlowPair {
    /// The blow at the centre of its roll: what the field reads.
    pub(super) fn expected(self) -> f64 {
        match self {
            BlowPair::Hazard(damage) => damage,
            BlowPair::Strike { att, def } => expected_damage(att, def),
        }
    }
}

/// The tag on a structure's blow source: a city id with this bit set is its
/// City Center's strike, and with [`ENCAMPMENT_SOURCE`] set as well, its
/// Encampment's.
pub(super) const STRUCTURE_SOURCE: u32 = 1 << 31;
const ENCAMPMENT_SOURCE: u32 = 1 << 30;

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
    /// (tile, our unit, remaining HP) → each source's expected blow there.
    cache: BTreeMap<(Pos, u32, i32), Blows>,
    /// `strike-reach`: hostiles whose strike reach held a tile the movement
    /// flood did not. Zero with the gene off.
    pub(super) widened: u32,
    /// `shared-danger`: each hostile's share, one over the number of our
    /// land units inside its reach. Empty with the gene off, and then every
    /// reading below is the full field.
    shares: BTreeMap<u32, f64>,
    /// `field-reads-the-tile-it-leaves`: each hostile's movement flood, kept
    /// beside its strike reach. Empty without `strike-reach`.
    floods: BTreeMap<u32, Vec<Pos>>,
    /// `field-reads-the-tile-it-leaves`: see `steps_into_the_tile_left`.
    pub(super) reads_the_tile_it_leaves: bool,
}

impl DangerField {
    /// Possible replies after a visible enemy has had one turn to reposition.
    /// Each source remains one source even when several routes cover a tile.
    pub(super) fn second_turn(g: &Game, pid: usize) -> Self {
        let mut field = Self::with_reach(g, pid, true);
        let mut probe = g.speculative_clone();
        for (uid, reach) in &mut field.reaches {
            let saved = probe.units[uid].clone();
            let moves = probe.unit_max_moves(*uid);
            if let Some(unit) = probe.units.get_mut(uid) {
                unit.moves_left = moves;
                unit.moved = false;
                unit.acted = false;
                unit.zoc_stopped = false;
                unit.started_turn_in_zoc = false;
            }
            let stands = probe.reachable(*uid);
            for stand in stands {
                probe.relocate(*uid, stand);
                reach.extend(strike_reach_of(&probe, pid, *uid));
            }
            probe.relocate(*uid, saved.pos);
            *probe.units.get_mut(uid).expect("forecast retains enemy") = saved;
            reach.sort_unstable();
            reach.dedup();
        }
        field
    }

    /// The field on the movement flood's reach (`Game::attack_reach`).
    pub(super) fn new(g: &Game, pid: usize) -> Self {
        Self::with_reach(g, pid, false)
    }

    /// The field on the flood's reach, or with `strike_reach` on the reach
    /// the engine resolves a blow over (`strike_reach_of`).
    pub(super) fn with_reach(g: &Game, pid: usize, strike_reach: bool) -> Self {
        let probe = g.speculative_clone();
        let mut reaches = Vec::new();
        let mut floods = BTreeMap::new();
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
                let strike = strike_reach_of(g, pid, unit.id);
                if strike.iter().any(|tile| flood.binary_search(tile).is_err()) {
                    widened += 1;
                }
                floods.insert(unit.id, flood);
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
            shares: BTreeMap::new(),
            floods,
            reads_the_tile_it_leaves: false,
        }
    }

    /// `field-reads-the-tile-it-leaves`: whether `enemy`, which cannot strike
    /// `tile` as the board stands, could once the unit asked about has left
    /// `left`. The strike reach stops at tiles our units hold, so it never
    /// stands a hostile on the tile a unit is about to vacate, and a unit that
    /// steps back one tile reads no danger from the attacker beside it. Only
    /// that unit's own tile opens: the hostile must already strike `left`, its
    /// flood (which reads through units) must hold `tile`, and `tile` must lie
    /// within its range of `left`.
    fn steps_into_the_tile_left(&self, enemy: u32, reach: &[Pos], left: Pos, tile: Pos) -> bool {
        self.reads_the_tile_it_leaves
            && left != tile
            && reach.binary_search(&left).is_ok()
            && self
                .floods
                .get(&enemy)
                .is_some_and(|flood| flood.binary_search(&tile).is_ok())
            && self.probe.wdist(left, tile) <= self.probe.unit_attack_range(enemy).max(1)
    }

    /// `shared-danger`: a hostile strikes once a turn, so its blow lands on
    /// one of the units inside its reach, not on each of them. The full field
    /// charges every blow to every unit at once; around unwalled Stockholm
    /// (live King `civvis-20260930T221624Z`) a crossbowman, a catapult and a
    /// defender made every firing tile read 95-127, and a nine-unit siege
    /// rotated and held for 27 turns against three strikes a turn. Each
    /// hostile's share is one over the land units of ours standing in its
    /// reach now; `shared_reading` never falls under the strongest single
    /// blow, because any one hostile can still spend all of its strike on
    /// the unit asked about.
    pub(super) fn share(&mut self, g: &Game) {
        let ours: Vec<Pos> = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == self.pid
                    && spec.class == "military"
                    && spec.domain.as_deref() != Some("air")
                    && !g.is_embarked(unit)
            })
            .map(|unit| unit.pos)
            .collect();
        self.shares = self
            .reaches
            .iter()
            .map(|(id, reach)| {
                let exposed = ours
                    .iter()
                    .filter(|pos| reach.binary_search(pos).is_ok())
                    .count()
                    .max(1);
                (*id, 1.0 / exposed as f64)
            })
            .collect();
        // A City Center or an Encampment strikes once a turn too, at one
        // unit within two tiles of it.
        let exposed_within_two = |at: Pos| {
            ours.iter()
                .filter(|pos| g.wdist(**pos, at) <= 2)
                .count()
                .max(1) as f64
        };
        for city in g.cities.values() {
            if city.owner == self.pid || !g.is_at_war(self.pid, city.owner) {
                continue;
            }
            debug_assert!(
                city.id < ENCAMPMENT_SOURCE,
                "city ids stay below the tag bits"
            );
            self.shares.insert(
                STRUCTURE_SOURCE | city.id,
                1.0 / exposed_within_two(city.pos),
            );
            if let Some(at) = g
                .wdisk(city.pos, 3)
                .into_iter()
                .find(|pos| g.encampment_at(*pos) == Some(city.id))
            {
                self.shares.insert(
                    STRUCTURE_SOURCE | ENCAMPMENT_SOURCE | city.id,
                    1.0 / exposed_within_two(at),
                );
            }
        }
    }

    fn shared_reading(&self, blows: impl Iterator<Item = (Option<u32>, f64)>) -> f64 {
        let (mut shared, mut strongest) = (0.0_f64, 0.0_f64);
        for (source, blow) in blows {
            strongest = strongest.max(blow);
            let share = source
                .and_then(|id| self.shares.get(&id).copied())
                .unwrap_or(1.0);
            shared += blow * share;
        }
        shared.max(strongest)
    }

    /// The rotation's reading at one tile: the full field, or the shared
    /// one with `shared-danger` on.
    pub(super) fn rotation_danger(&mut self, tile: Pos, uid: u32) -> f64 {
        if self.shares.is_empty() {
            return self.danger(tile, uid);
        }
        let blows = self.contributions(tile, uid);
        self.shared_reading(blows.iter().copied())
    }

    /// `guns-enter-together`: the reading for one of `entering` siege guns
    /// stepping into a walled city's strike ring together. A City Center or
    /// an Encampment strikes once a turn, so its blow is charged once across
    /// the group (or across the units already in its reach, when that share
    /// is smaller); every unit's blow and every movement hazard is charged as
    /// the rotation charges it, and the reading never falls under the
    /// strongest single unit's blow. Live King civvis-20261005T051413Z
    /// (game 102): four fit Bombards stood three tiles from walled Toronto
    /// for nine turns because each post read the city's whole blow, ~108.
    pub(super) fn group_entry_danger(&mut self, tile: Pos, uid: u32, entering: usize) -> f64 {
        let group = 1.0 / entering.max(1) as f64;
        let blows = self.contributions(tile, uid);
        let (mut total, mut strongest_unit) = (0.0_f64, 0.0_f64);
        for (source, blow) in blows.iter() {
            let share = source.and_then(|id| self.shares.get(&id).copied());
            match source {
                Some(id) if id & STRUCTURE_SOURCE != 0 => {
                    total += blow * share.map_or(group, |share| share.min(group));
                }
                _ => {
                    total += blow * share.unwrap_or(1.0);
                    if source.is_some() {
                        strongest_unit = strongest_unit.max(*blow);
                    }
                }
            }
        }
        total.max(strongest_unit)
    }

    /// Every blow that would land on `uid` standing unfortified on `tile`
    /// next turn, by source.
    pub(super) fn contributions(&mut self, tile: Pos, uid: u32) -> Blows {
        let hp = self.probe.units.get(&uid).map_or(0, |unit| unit.hp);
        self.contributions_at_hp(tile, uid, hp)
    }

    /// A melee strike wounds the attacker before the enemy replies. Its
    /// reduced defensive strength belongs in the reply price and cache key.
    pub(super) fn contributions_at_hp(&mut self, tile: Pos, uid: u32, hp: i32) -> Blows {
        if let Some(hit) = self.cache.get(&(tile, uid, hp)) {
            return Arc::clone(hit);
        }
        if !self.probe.units.contains_key(&uid) {
            return Arc::new(Vec::new());
        }
        let out: Vec<(Option<u32>, f64)> = self
            .blow_pairs_at_hp(tile, uid, hp)
            .into_iter()
            .map(|(source, pair)| (source, pair.expected()))
            .collect();
        let out = Arc::new(out);
        self.cache.insert((tile, uid, hp), Arc::clone(&out));
        out
    }

    /// Every blow [`Self::contributions_at_hp`] reads, before the roll: a
    /// movement hazard's planning damage, or each strike's two strengths as
    /// the engine resolves them (the striker's through its own wound, ours
    /// through the wound at `hp`). Same sources, same order. Uncached; the
    /// `risk-is-priced` reading (`priced_risk.rs`) takes the whole damage
    /// distribution from these pairs where the field takes the centre.
    pub(super) fn blow_pairs_at_hp(
        &mut self,
        tile: Pos,
        uid: u32,
        hp: i32,
    ) -> Vec<(Option<u32>, BlowPair)> {
        let Some(saved) = self.probe.units.get(&uid).cloned() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        let hazard = crate::ai::BasicAi::movement_hazard_damage(&self.probe, self.pid, tile);
        if hazard > 0.0 {
            out.push((None, BlowPair::Hazard(hazard)));
        }
        // A garrison is not a combat target: blows on a City Center or an
        // Encampment damage the district, not the unit inside it.
        let garrisoned =
            self.probe.city_at(tile).is_some() || self.probe.defending_district_at(tile).is_some();
        if !garrisoned {
            self.probe.relocate(uid, tile);
            if let Some(unit) = self.probe.units.get_mut(&uid) {
                unit.fortified = false;
                unit.fortify_turns = 0;
                unit.hp = hp;
            }
            for (enemy, reach) in &self.reaches {
                if reach.binary_search(&tile).is_err()
                    && !self.steps_into_the_tile_left(*enemy, reach, saved.pos, tile)
                {
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
                    out.push((Some(*enemy), BlowPair::Strike { att, def }));
                }
            }
            // A walled city or a standing defending district strikes within
            // two tiles next turn whether or not it fired this one. The defence
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
                            Some(STRUCTURE_SOURCE | cid),
                            BlowPair::Strike {
                                att: self.probe.city_ranged_strength(cid),
                                def: defence,
                            },
                        ));
                    }
                }
                if let Some(cid) = self.probe.defending_district_at(pos) {
                    let city = &self.probe.cities[&cid];
                    if city.owner != self.pid
                        && self.probe.is_at_war(self.pid, city.owner)
                        && self
                            .probe
                            .defending_district_state(cid, pos)
                            .is_some_and(|district| {
                                // The enemy's turn resets its shot budget.
                                district.hp > 0 && district.wall_hp > 0 && !district.pillaged
                            })
                        && encampments.insert((cid, pos))
                    {
                        out.push((
                            Some(STRUCTURE_SOURCE | ENCAMPMENT_SOURCE | cid),
                            BlowPair::Strike {
                                att: self.probe.city_ranged_strength(cid),
                                def: defence,
                            },
                        ));
                    }
                }
            }
            self.probe.relocate(uid, saved.pos);
            if let Some(unit) = self.probe.units.get_mut(&uid) {
                *unit = saved;
            }
        }
        out
    }

    /// Whether any visible hostile's strike reach holds `tile` — the only
    /// tiles where a unit blow can land at all.
    pub(super) fn reaches_tile(&self, tile: Pos) -> bool {
        self.reaches
            .iter()
            .any(|(_, reach)| reach.binary_search(&tile).is_ok())
    }

    /// The whole field at one tile.
    pub(super) fn danger(&mut self, tile: Pos, uid: u32) -> f64 {
        self.contributions(tile, uid)
            .iter()
            .map(|(_, blow)| blow)
            .sum()
    }

    /// The field with the named enemies already dead.
    fn danger_without(&mut self, tile: Pos, uid: u32, hp: i32, dead: &BTreeSet<u32>) -> f64 {
        self.contributions_at_hp(tile, uid, hp)
            .iter()
            .filter(|(source, _)| source.is_none_or(|id| !dead.contains(&id)))
            .map(|(_, blow)| blow)
            .sum()
    }

    /// `slots-survive-the-roll`: the whole field at one tile with every
    /// blow at the top of its roll. A movement hazard is no roll and counts
    /// as it reads. Unshared: the question is whether the unit can be
    /// removed there, and any hostile may spend its strike on it.
    pub(super) fn upper_danger(&mut self, tile: Pos, uid: u32) -> f64 {
        self.contributions(tile, uid)
            .iter()
            .map(|(source, blow)| {
                if source.is_some() {
                    upper_roll_damage(*blow)
                } else {
                    *blow
                }
            })
            .sum()
    }

    fn upper_danger_without(&mut self, tile: Pos, uid: u32, hp: i32, dead: &BTreeSet<u32>) -> f64 {
        let blows = self.contributions_at_hp(tile, uid, hp);
        let alive = blows
            .iter()
            .filter(|(source, _)| source.is_none_or(|id| !dead.contains(&id)))
            .map(|(source, blow)| (*source, upper_roll_damage(*blow)));
        if self.shares.is_empty() {
            return alive.map(|(_, blow)| blow).sum();
        }
        self.shared_reading(alive)
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
/// alone, for a caller asking about one tile.
pub(super) fn danger(g: &Game, pid: usize, tile: Pos, uid: u32) -> f64 {
    DangerField::new(g, pid).danger(tile, uid)
}

/// The same reading on the strike reach (`strike-reach` on).
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
    /// `defenders-hold-the-walls`: a healthy defender of a threatened city
    /// of ours, whose blow is not refused for the danger at its stand. See
    /// `advanced/defenders_hold_the_walls.rs`.
    holds_walls: bool,
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
    /// `siege-at-the-walls-dies-first`: how hard this unit presses our walls
    /// (`wall_threat_scale`; zero when it does not). Its kill value already
    /// carries the premium at that scale; a wound earns the damage premium at
    /// it. Zero with the gene off.
    wall_threat: f64,
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
    /// `ranged-kills-are-free`: a ranged blow from the shooter's own tile by
    /// a recovering or doomed shooter. It joins a sequence only where its
    /// target dies in that sequence, and the danger at its stand is not
    /// charged to it: the unit stands there whether or not it shoots.
    kill_only: bool,
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
    /// Version two reprices every striker when a later blow removes a reply.
    /// Each stand records the shooter, its end tile and HP after melee return.
    reply_stands: Vec<(usize, Pos, f64)>,
    reply_cost: f64,
    survives: bool,
    /// `ranged-kills-are-free`: targets a kill-only blow has landed on; the
    /// sequence is worthless unless each of them dies in it.
    kill_only_on: u32,
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
            reply_stands: Vec::new(),
            reply_cost: 0.0,
            survives: true,
            kill_only_on: 0,
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
        survival_after_sequence: bool,
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
            ) + super::siege_at_the_walls_dies_first::WALL_THREAT_DAMAGE_PREMIUM
                * target.wall_threat
                * damage.min(remaining)
        };
        let mut cost = 0.0;
        let mut returned = 0.0;
        let mut dies = false;
        if !candidate.ranged {
            returned = candidate.return_on(def);
            if survival_after_sequence {
                returned = upper_roll_damage(returned);
            }
            if returned >= f64::from(shooter.hp) {
                // The host's word that the attacker dies is a veto, whatever
                // the kill is worth (`battle-planner-3`).
                if candidate.host.is_some() || survival_after_sequence {
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
        if !dies && !survival_after_sequence && !candidate.kill_only {
            let dead = Self::dead_of(killed, targets);
            danger = field.danger_without(
                end,
                shooter.uid,
                (f64::from(shooter.hp) - returned).floor().max(1.0) as i32,
                &dead,
            );
            if danger >= f64::from(shooter.hp) - returned {
                cost += death_value(shooter.cost);
            } else {
                cost += DANGER_WEIGHT * loss_value(danger, shooter.cost);
            }
        }
        // A wounded unit strikes only to finish, only from safety, and
        // never as a trade of itself.
        if shooter.hp < WOUNDED_STRIKER_HP
            && !candidate.kill_only
            && (dies || !(kill && (survival_after_sequence || danger <= NO_DANGER)))
        {
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
        if candidate.kill_only {
            next.kill_only_on |= target_bit;
        }
        if !candidate.ranged {
            next.melee_on |= target_bit;
        }
        next.score = self.score + gain - cost;
        if survival_after_sequence {
            next.reply_stands
                .push((candidate.shooter, end, f64::from(shooter.hp) - returned));
            next.reply_cost = 0.0;
            next.survives = true;
            let dead = Self::dead_of(killed, targets);
            for (index, tile, hp) in &next.reply_stands {
                let striker = &shooters[*index];
                let incoming = field.upper_danger_without(
                    *tile,
                    striker.uid,
                    hp.floor().max(1.0) as i32,
                    &dead,
                );
                let lethal = incoming >= *hp;
                // `defenders-hold-the-walls`: a city's defender may be
                // answered where it strikes.
                next.survives &= striker.holds_walls
                    || (!lethal && (striker.hp >= WOUNDED_STRIKER_HP || incoming <= NO_DANGER));
                next.reply_cost += if lethal {
                    death_value(striker.cost)
                } else {
                    DANGER_WEIGHT * loss_value(incoming, striker.cost)
                };
            }
            // A later kill can remove an earlier striker's only reply. Keep
            // that unfinished combination in the beam and refund its old
            // exposure price; only a surviving terminal plan can be selected.
            next.score += self.reply_cost - next.reply_cost;
        }
        Some(next)
    }

    /// The sequence's worth once it stops: its running score less the
    /// penalty for every target it left low with a finisher still to spare.
    fn terminal(&self, shooters: &[Shooter], targets: &[Target], candidates: &[Candidate]) -> f64 {
        if !self.survives {
            return f64::NEG_INFINITY;
        }
        // `ranged-kills-are-free`: a kill-only blow on a target that lives
        // is no part of a plan. Its shooters take no return and no new
        // exposure, so the centre of the roll is the bar, not KILL_MARGIN:
        // a near miss still leaves the attacker all but dead.
        for (index, target) in targets.iter().enumerate() {
            let bit = 1u32 << index;
            if self.kill_only_on & bit != 0
                && self.killed & bit == 0
                && self.dealt[index] < f64::from(target.hp)
            {
                return f64::NEG_INFINITY;
            }
        }
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
    survival_after_sequence: bool,
) -> (Vec<usize>, f64) {
    let mut beam = vec![BeamState::empty(targets.len())];
    let mut best: (Vec<usize>, f64) = (Vec::new(), 0.0);
    for _depth in 0..MAX_SHOOTERS.min(shooters.len()) {
        let mut next: Vec<BeamState> = Vec::new();
        for state in &beam {
            for (index, candidate) in candidates.iter().enumerate() {
                if let Some(extended) = state.extend(
                    index,
                    candidate,
                    shooters,
                    targets,
                    field,
                    survival_after_sequence,
                ) {
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
/// post-strike stand, priced at its remaining HP, at or over the unit's hit
/// points. A killed target contributes no reply; a melee finisher stands on
/// the tile it emptied. Such a unit has no blow worth the ladder's freedom:
/// the rotation takes it as exposed instead.
fn doomed_shooters(
    shooters: &[Shooter],
    targets: &[Target],
    candidates: &[Candidate],
    field: &mut DangerField,
) -> BTreeSet<u32> {
    let mut doomed = BTreeSet::new();
    for (index, shooter) in shooters.iter().enumerate() {
        // `defenders-hold-the-walls`: a threatened city's defender is never
        // doomed by the danger at its stand.
        if shooter.holds_walls {
            continue;
        }
        let mut any = false;
        let mut survivable = false;
        for candidate in candidates.iter().filter(|c| c.shooter == index) {
            any = true;
            let target = &targets[candidate.target];
            let def = effective_strength(candidate.def_base, target.hp);
            let back = if candidate.ranged {
                0.0
            } else {
                upper_roll_damage(candidate.return_on(def))
            };
            let hp = f64::from(shooter.hp) - back;
            if hp <= 0.0 {
                continue;
            }
            let kill = candidate.damage_on(def) >= f64::from(target.hp) * KILL_MARGIN;
            let end = if kill && !candidate.ranged {
                target.pos
            } else {
                candidate.from
            };
            let dead = if kill {
                BTreeSet::from([target.uid])
            } else {
                BTreeSet::new()
            };
            let after =
                field.upper_danger_without(end, shooter.uid, hp.floor().max(1.0) as i32, &dead);
            if hp - after > 0.0 {
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

fn upper_roll_damage(mean: f64) -> f64 {
    (mean * crate::ai::COMBAT_ROLL_MAX).ceil().min(100.0)
}

pub(super) fn melee_health_floor(g: &Game, pid: usize, action: &Action) -> Option<(u32, i32)> {
    let Action::Attack { unit, target } = action else {
        return None;
    };
    let attacker = g.units.get(unit)?;
    g.unit_ids_at(*target)
        .iter()
        .filter_map(|enemy| {
            let defender = &g.units[enemy];
            (defender.owner != pid
                && g.is_at_war(pid, defender.owner)
                && g.rules.units[defender.kind].class == "military")
                .then(|| g.melee_exchange_strengths(*unit, *enemy))
                .flatten()
                .map(|(att, def)| upper_roll_damage(expected_damage(def, att)) as i32)
        })
        .max()
        .map(|damage| (*unit, attacker.hp - damage))
}

impl AdvancedAi {
    /// `falling-city-outranks-the-heal`: whether a melee land unit fit to
    /// fight ([`ROTATE_HP`] or more) but still recovering toward
    /// [`RETURN_HP`] stays in the fight because a hostile city with no
    /// walls standing and at most [`FALLING_CITY_HP`] health lies within
    /// [`FALLING_CITY_REACH`] tiles. Such a city cannot strike, and a blow
    /// takes it; the heal slot and the recovery rotation otherwise held the
    /// bodies that would.
    ///
    /// Live Emperor civvis-20261008T182825Z (game 428): Yokohama stood with
    /// no walls at 1 to 40 health from turn 142 to 153 while the takers in
    /// reach logged "holds position to heal" at 63 and 78 hp seven tiles out
    /// and the capture read "6 no walk"; Japan won on Culture at 161. Over the 10-06/07/08
    /// runs, 11 of 26 such falling cities took more than two turns or never
    /// fell.
    pub(super) fn falling_city_calls(&self, g: &Game, pid: usize, uid: u32) -> bool {
        if !self.falling_city_outranks_the_heal {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let spec = &g.rules.units[unit.kind];
        if unit.hp < ROTATE_HP
            || !spec.is_melee_capable()
            || spec
                .domain
                .as_deref()
                .is_some_and(|domain| domain != "land")
            || g.is_embarked(unit)
        {
            return false;
        }
        g.cities.values().any(|city| {
            city.owner != pid
                && g.is_at_war(pid, city.owner)
                && city.wall_hp <= 0
                && city.hp <= FALLING_CITY_HP
                && g.wdist(city.pos, unit.pos) <= FALLING_CITY_REACH
        })
    }

    /// The live controller must also honor this policy when its fresh combat
    /// preview disagrees with the native damage model.
    pub fn live_strike_survival_enabled(&self) -> bool {
        self.unit_preservation || self.doomed_blow_veto || self.doomed_blow_veto_2
    }

    /// Gene `dialogue-never-declares-war`. The brain never sees a leader
    /// statement; the live controller's dialogue closer answers it, so the
    /// bridge reply carries the policy there.
    pub fn dialogue_never_declares_war_enabled(&self) -> bool {
        self.dialogue_never_declares_war
    }

    /// Gene `ranged-before-melee`: the live bridge orders a frame's shots at
    /// a target ahead of the melee blows on it.
    pub fn ranged_before_melee_enabled(&self) -> bool {
        self.ranged_before_melee
    }

    /// Apply the selected survival policy to a live bridge finishing volley.
    /// The bridge commits these actions before `take_turn`, so the ordinary
    /// battle planner cannot protect their strikers afterwards. Recheck the
    /// whole resulting board: a later friendly kill can remove an earlier
    /// striker's reply threat. Off, neither the actions nor the board are read.
    pub fn live_finishing_actions_survive<'a>(
        &self,
        before: &Game,
        pid: usize,
        actions: impl IntoIterator<Item = &'a Action>,
    ) -> bool {
        if !self.unit_preservation && !self.doomed_blow_veto && !self.doomed_blow_veto_2 {
            return true;
        }
        if self.unit_preservation {
            let actions: Vec<Action> = actions.into_iter().cloned().collect();
            return self.preservation_finishing_safe(before, pid, &actions);
        }
        let mut after = before.speculative_clone();
        let mut strikers = BTreeSet::new();
        for action in actions {
            // A sampled melee exchange is not the health the host promises.
            // Reserve the upper roll before pricing the reply at the resulting
            // wounded strength. Do not spend uncertain post-kill healing here.
            let health_floor = melee_health_floor(&after, pid, action);
            if after.apply(pid, action).is_err() {
                return false;
            }
            if let Some((uid, hp)) = health_floor {
                if hp <= 0 {
                    return false;
                }
                if let Some(unit) = after.units.get_mut(&uid) {
                    unit.hp = unit.hp.min(hp);
                }
            }
            if let Action::Attack { unit, .. } | Action::Ranged { unit, .. } = action {
                strikers.insert(*unit);
            }
        }
        let mut field = DangerField::with_reach(&after, pid, self.strike_reach);
        strikers.into_iter().all(|uid| {
            after.units.get(&uid).is_some_and(|unit| {
                // Each source rounds independently; rounding only their sum
                // can lose a lethal point when several enemies can reply.
                let incoming = field.upper_danger_without(unit.pos, uid, unit.hp, &BTreeSet::new());
                let started_healthy = before
                    .units
                    .get(&uid)
                    .is_some_and(|unit| unit.hp >= WOUNDED_STRIKER_HP);
                incoming < f64::from(unit.hp) && (started_healthy || incoming <= NO_DANGER)
            })
        })
    }

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
        self.battle_planner_ordered = self.withdraw_before_kill_prepass(g, pid, plan);
        self.credit_orders(g, OrderStep::Withdraw);
        let withdrew = !self.battle_planner_ordered.is_empty();
        // `battle-solver`: the fights it can work out to the end are settled
        // here, and the planner below plays what it left. See
        // `advanced/battle_solver.rs`.
        let solved = self.run_battle_solver(g, pid, plan);
        self.credit_orders(g, OrderStep::BattleSolver);
        self.battle_planner_recovering.retain(|uid| {
            g.units.get(uid).is_some_and(|unit| {
                unit.owner == pid
                    && unit.hp
                        < if self.unit_preservation {
                            100
                        } else {
                            RETURN_HP
                        }
            })
        });
        let mut field = DangerField::with_reach(g, pid, self.strike_reach);
        field.reads_the_tile_it_leaves = self.field_reads_the_tile_it_leaves;
        if self.shared_danger {
            field.share(g);
        }
        // `walls-release-the-one-blow`: before the kill search and the
        // rotation read `holds_the_walls`.
        self.walls_one_blow = self.walls_one_blow_units(g, pid, &mut field);
        self.census.strike_reach_widened += field.widened;
        if field.widened > 0 {
            think!(self.journal(), Military, Detail,
                "Strike reach: {} hostile(s) can hit tiles the movement flood read as safe", field.widened;
                "a unit stopped by our zone of control keeps its movement for the blow; \
                 the danger field, the rotation and the slots read that reach");
        }
        let (blows, armed, wanted, mut doomed) = self.kill_sequence_in(g, pid, &mut field);
        self.battle_planner_wanted_previews = wanted;
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
            self.credit_orders(g, OrderStep::BattlePlanner);
        }
        if struck {
            field = DangerField::with_reach(g, pid, self.strike_reach);
            field.reads_the_tile_it_leaves = self.field_reads_the_tile_it_leaves;
            if self.shared_danger {
                field.share(g);
            }
        }
        if self.doomed_blow_veto_2 {
            doomed.retain(|uid| !strikers.contains(uid));
        }
        // An active Domination siege owns its members' blows: its train has
        // priced the assault as a whole. See `active_siege_member`.
        doomed.retain(|uid| !self.active_siege_member(g, pid, *uid));
        // `breaker-walks-under-a-shield`: so does a declared strike its
        // bodies' blows at the city. Nothing with the gene off.
        doomed.retain(|uid| !self.shield_presses(g, pid, *uid));
        self.census.battle_plan_doomed += doomed.len() as u32;
        if !doomed.is_empty() {
            think!(self.journal(), Military, Decision,
                "Battle plan: {} unit(s) have no blow they would survive", doomed.len();
                "return damage plus the danger at every stand reaches their hit points; \
                 the rotation takes them as exposed and the ladder leaves them alone");
        }
        let rotations = self.rotate_wounded(g, pid, &mut field, &strikers, &doomed);
        self.credit_orders(g, OrderStep::Rotation);
        self.census.battle_plan_rotations += rotations;
        // `battle-planner-2` (and three): the positions plan, on a force
        // picture that no longer holds the defenders the kill plan removed.
        // The caller's own rebuild after a strike is the version-one
        // contract and stands.
        if self.positions_plan_on() {
            if struck || withdrew || solved {
                self.rebuild_force_groups(g, pid, plan);
                self.force_groups_dirty = false;
            }
            self.plan_positions(g, pid, plan, &mut field, &armed);
        }
        struck || withdrew || solved
    }

    /// The kill plan alone — the ordered blows the search chose, before any
    /// clone has seen them. Pure: for tests and explainers.
    #[cfg(test)]
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
        let (shooters, targets, mut candidates, mut armed) = self.strike_candidates(g, pid, field);
        if candidates.is_empty() {
            return (Vec::new(), armed, Vec::new(), BTreeSet::new());
        }
        let doomed = if self.doomed_blow_veto || self.doomed_blow_veto_2 {
            doomed_shooters(&shooters, &targets, &candidates, field)
        } else {
            BTreeSet::new()
        };
        armed.retain(|uid| !doomed.contains(uid));
        let wanted = self.wanted_previews_of(&shooters, &targets, &candidates);
        // Rotation runs after the selected blows have been applied. A veto
        // must therefore reach the search itself, before a profitable trade
        // can spend a unit whose every strike was judged unsurvivable.
        if self.doomed_blow_veto && !doomed.is_empty() {
            // `ranged-kills-are-free`: a doomed shooter's ranged blow from
            // its own tile survives the veto as kill-only — it is in that
            // danger whether or not it shoots, and a kill only lowers it.
            if self.ranged_kills_are_free {
                for candidate in candidates.iter_mut() {
                    let shooter = &shooters[candidate.shooter];
                    if doomed.contains(&shooter.uid)
                        && candidate.ranged
                        && candidate.from == shooter.pos
                    {
                        candidate.kill_only = true;
                    }
                }
            }
            candidates.retain(|candidate| {
                candidate.kill_only || !doomed.contains(&shooters[candidate.shooter].uid)
            });
        }
        let (sequence, score) = search_kill_sequence(
            &shooters,
            &targets,
            &candidates,
            field,
            self.doomed_blow_veto_2,
        );
        if self.ranged_kills_are_free && (score <= 0.0 || sequence.is_empty()) {
            let kill_only = candidates
                .iter()
                .filter(|candidate| candidate.kill_only)
                .count();
            think!(self.journal(), Military, Detail,
                "Kill search: no sequence from {} blow(s) of {} shooter(s) on {} target(s)",
                candidates.len(), shooters.len(), targets.len();
                "{} doomed, {} blow(s) kill-only (recovering, a civilian's guard or doomed: \
                 only where the target falls)", doomed.len(), kill_only);
        }
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
                || g.defending_district_at(unit.pos).is_some()
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
                // `siege-at-the-walls-dies-first`: see
                // `advanced/siege_at_the_walls_dies_first.rs`.
                let wall_threat = if self.siege_at_the_walls_dies_first {
                    self.wall_threat_scale(g, pid, uid)
                } else {
                    0.0
                };
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
                    ) + super::siege_at_the_walls_dies_first::WALL_THREAT_KILL_PREMIUM
                        * wall_threat,
                    siege: spec.siege,
                    captures: spec.is_melee_capable(),
                    wall_threat,
                }
            })
            .collect();
        let mut armed: BTreeSet<u32> = BTreeSet::new();
        if targets.is_empty() {
            return (Vec::new(), Vec::new(), Vec::new(), armed);
        }
        let wall_threats = targets
            .iter()
            .filter(|target| target.wall_threat > 0.0)
            .count();
        if wall_threats > 0 {
            think!(self.journal(), Military, Detail,
                "Wall threats: {} gun(s) or taker(s) at our walls are the kill plan's first targets", wall_threats;
                "siege-at-the-walls-dies-first: a kill adds {:.0}, a point of damage {:.0}",
                super::siege_at_the_walls_dies_first::WALL_THREAT_KILL_PREMIUM,
                super::siege_at_the_walls_dies_first::WALL_THREAT_DAMAGE_PREMIUM);
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
                || (self.battle_planner_recovering.contains(&uid)
                    && !(self.ranged_kills_are_free && spec.has_ranged_attack()))
                || self.battle_planner_ordered.contains(&uid)
                || (self.guard_is_reserved_for_civilian(uid)
                    && !(self.ranged_kills_are_free && spec.has_ranged_attack())
                    // `every-shooter-at-the-walls`: an escort may shoot a
                    // wall threat from where it stands.
                    && !(self.every_shooter_at_the_walls && spec.has_ranged_attack()))
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
            // So is a shooter of a siege reducing an unwalled city: its shot
            // is the city's health, not the next reliever. See
            // `siege_train::unwalled_siege_member`.
            if spec.has_ranged_attack() && self.unwalled_siege_member(g, pid, uid) {
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
            let firing_post = self.defensive_firing_radius(g, pid, uid);
            let mut stands: Vec<Pos> = vec![unit.pos];
            stands.extend(g.reachable(uid));
            for stand in stands {
                let ranged_stand_allowed =
                    firing_post.is_none_or(|(city, radius)| g.wdist(stand, city) <= radius);
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
                            && ranged_stand_allowed
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
                                    kill_only: false,
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
                                    kill_only: false,
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
            // `centre-keeps-its-garrison`: a threatened city's garrison
            // fires from its City Center or not at all; a melee blow that
            // kills would carry it out. `false` with the gene off.
            if self.keeps_the_centre(g, pid, uid) {
                own.retain(|candidate| candidate.ranged && candidate.from == unit.pos);
            }
            // `ranged-kills-are-free`: a recovering shooter, or a guard a
            // civilian holds, keeps only its ranged blows from its own tile,
            // kill-only: it stays the rotation's or the escort's otherwise,
            // and is not offered as armed. (Only the gene lets either here.)
            let recovering = self.battle_planner_recovering.contains(&uid)
                || self.guard_is_reserved_for_civilian(uid);
            if recovering {
                own.retain(|candidate| candidate.ranged && candidate.from == unit.pos);
                // `every-shooter-at-the-walls`: an escort's shot at a wall
                // threat is a full blow, not kill-only. Nothing else changes.
                let escort_fires = self.every_shooter_at_the_walls
                    && !self.battle_planner_recovering.contains(&uid);
                for candidate in &mut own {
                    candidate.kill_only =
                        !(escort_fires && targets[candidate.target].wall_threat > 0.0);
                }
                if escort_fires {
                    own.retain(|candidate| !candidate.kill_only || self.ranged_kills_are_free);
                }
            }
            if own.is_empty() {
                continue;
            }
            if !recovering {
                armed.insert(uid);
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
                holds_walls: self.holds_the_walls(g, pid, uid),
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
            // `siege-at-the-walls-dies-first`: wall threats are never the
            // targets trimmed away. Constant false with the gene off.
            targets[*b]
                .wall_threat
                .total_cmp(&targets[*a].wall_threat)
                .then_with(|| targets[*a].hp.cmp(&targets[*b].hp))
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
        if self.doomed_blow_veto_2 {
            loop {
                let (rejected, surviving_kills) =
                    self.rejected_strikers_in_sequence(g, pid, plan, &verified);
                if rejected.is_empty() {
                    kills = surviving_kills;
                    break;
                }
                let before = verified.len();
                verified.retain(|blow| !rejected.contains(&blow.unit));
                dropped += (before - verified.len()) as u32;
            }
        }
        (verified, kills, dropped)
    }

    /// Replay only retained blows. Removing a rejected rescuer may expose
    /// an earlier striker, so the caller repeats until the sequence is
    /// stable. Independent safe shots remain available throughout.
    fn rejected_strikers_in_sequence(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        blows: &[Blow],
    ) -> (BTreeSet<u32>, u32) {
        let mut rejected = BTreeSet::new();
        let mut kills = 0;
        if blows.is_empty() {
            return (rejected, kills);
        }
        let mut after = g.speculative_clone();
        for blow in blows {
            let Some(unit) = after.units.get(&blow.unit) else {
                rejected.insert(blow.unit);
                continue;
            };
            if unit.pos != blow.from
                && (after
                    .apply(
                        pid,
                        &Action::MoveTo {
                            unit: blow.unit,
                            to: blow.from,
                        },
                    )
                    .is_err()
                    || after
                        .units
                        .get(&blow.unit)
                        .is_none_or(|unit| unit.pos != blow.from))
            {
                rejected.insert(blow.unit);
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
            let health_floor = melee_health_floor(&after, pid, &action);
            let (result, applied) =
                Self::tactical_attack_result_in(&mut after, pid, blow.unit, &action, plan);
            if let Some((uid, hp)) = health_floor {
                if hp <= 0 {
                    rejected.insert(uid);
                } else if let Some(unit) = after.units.get_mut(&uid) {
                    unit.hp = unit.hp.min(hp);
                }
            }
            if matches!(applied, AppliedAttack::Applied)
                && (result.eliminates_enemy_unit || result.value >= 0.0)
            {
                kills += u32::from(result.eliminates_enemy_unit);
            } else {
                rejected.insert(blow.unit);
            }
        }
        let mut field = DangerField::with_reach(&after, pid, self.strike_reach);
        for blow in blows {
            let survives = after.units.get(&blow.unit).is_some_and(|unit| {
                let incoming =
                    field.upper_danger_without(unit.pos, unit.id, unit.hp, &BTreeSet::new());
                incoming < f64::from(unit.hp)
                    && (g.units[&blow.unit].hp >= WOUNDED_STRIKER_HP || incoming <= NO_DANGER)
            });
            // `defenders-hold-the-walls`: see `holds_the_walls`.
            if !survives && !self.holds_the_walls(g, pid, blow.unit) {
                rejected.insert(blow.unit);
            }
        }
        (rejected, kills)
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

    /// Denying a suicidal attack need not deny a healthy siege reinforcement
    /// its approach. Take only one legal, strictly closer step with no reply
    /// damage predicted by the same field that vetoed the attack. The caller
    /// ends the unit's turn afterwards, so this cannot reopen that attack.
    fn advance_vetoed_siege_unit(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        field: &mut DangerField,
    ) -> bool {
        if !self.siege_train {
            return false;
        }
        let unit = &g.units[&uid];
        if unit.hp < RETURN_HP || g.is_embarked(unit) {
            return false;
        }
        let Some(target) = self.force_groups.iter().find_map(|group| {
            (group.domain == super::ForceDomain::Land
                && group.units.contains(&uid)
                && matches!(
                    group.posture,
                    ForcePosture::Muster | ForcePosture::Advance | ForcePosture::Engage
                ))
            .then_some(group.objective)
            .filter(|pos| {
                g.city_at(*pos)
                    .is_some_and(|cid| g.is_at_war(pid, g.cities[&cid].owner))
            })
        }) else {
            return false;
        };
        let distance = g.wdist(unit.pos, target);
        if distance <= super::siege_train::STAGING_FAR {
            return false;
        }
        let next = g
            .nbrs(unit.pos)
            .into_iter()
            .filter(|pos| g.wdist(*pos, target) < distance && g.can_move(uid, *pos))
            .filter(|pos| {
                !g.unit_ids_at(*pos)
                    .iter()
                    .any(|other| g.is_at_war(pid, g.units[other].owner))
            })
            .filter(|pos| field.danger(*pos, uid) <= NO_DANGER)
            .min_by_key(|pos| (g.wdist(*pos, target), *pos));
        next.is_some_and(|pos| self.base.tactical_apply_move(g, pid, uid, pos))
    }

    /// A free scout must not spend every turn beside a contact merely because
    /// attacking it is fatal. Leave for reachable dry ground with no predicted
    /// reply damage; the caller still ends the turn and owns the attack veto.
    fn escape_vetoed_recon(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        field: &mut DangerField,
    ) -> bool {
        if !self.distance_scout_available(g, pid, uid) {
            return false;
        }
        let from = g.units[&uid].pos;
        if field.danger(from, uid) <= NO_DANGER || g.is_embarked(&g.units[&uid]) {
            return false;
        }
        let destination = g
            .reachable(uid)
            .into_iter()
            .filter(|pos| {
                *pos != from && g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))
            })
            .filter(|pos| {
                !g.unit_ids_at(*pos)
                    .iter()
                    .any(|other| g.is_at_war(pid, g.units[other].owner))
            })
            .filter(|pos| field.danger(*pos, uid) <= NO_DANGER)
            .min_by_key(|pos| (g.wdist(from, *pos), *pos));
        destination.is_some_and(|to| self.base.path_walk_to(g, pid, uid, to))
    }

    /// Recovery orders belong to the support formation, not just its carrier.
    /// The follower's later unit step must not undo the carrier's safe stand.
    fn claim_recovery_formation(&mut self, g: &Game, uid: u32) {
        self.battle_planner_ordered.insert(uid);
        if super::siege_train::linked_support_carrier(g, uid) {
            if let Some(peer) = g.units[&uid].linked_to {
                self.battle_planner_ordered.insert(peer);
            }
        }
    }

    /// Pull the wounded and the exposed out of reach and fortify them.
    /// Returns how many actually moved or swapped.
    pub(super) fn rotate_wounded(
        &mut self,
        g: &mut Game,
        pid: usize,
        field: &mut DangerField,
        strikers: &BTreeSet<u32>,
        doomed: &BTreeSet<u32>,
    ) -> u32 {
        let heals = !g.is_arena() || g.tactics.heal;
        // `siege-keeps-a-shooter`: see `siege_wall_keepers`.
        self.siege_wall_keepers = self.siege_wall_keepers(g, pid, field);
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
                // Support carriers already advance with the siege train.
                // Their formation must not exempt them from recovery, while
                // civilian/religious escorts retain their separate controller.
                || (unit.linked_to.is_some()
                    && !super::siege_train::linked_support_carrier(g, uid))
                || unit.moves_left <= 0.0
                || !(spec.is_melee_capable() || spec.has_ranged_attack())
                || self.guard_is_reserved_for_civilian(uid)
                // `battle-planner-3`: the siege's taker holds its post.
                || (self.battle_planner_3 && self.unit_is_reserved(uid))
            {
                continue;
            }
            // `breaker-walks-under-a-shield`: a body pressing the declared
            // strike is that module's to move; the rotation pulled 98-hp
            // Warriors off a breached Antananarivo for the danger of the
            // tile. `false` with the gene off.
            if self.shield_presses(g, pid, uid) {
                self.battle_planner_recovering.remove(&uid);
                continue;
            }
            if self.siege_wall_keepers.contains(&uid) {
                self.battle_planner_recovering.remove(&uid);
                think!(self.journal(), Military, Decision,
                    "Battle plan: the {} at {:?} keeps firing on the walls", unit.kind, unit.pos;
                    "{} hp; every other shooter in range is out to heal, and the walls would be restored in full after a few turns without damage",
                    unit.hp);
                continue;
            }
            if g.city_at(unit.pos).is_some() || g.defending_district_at(unit.pos).is_some() {
                // A recovering garrison already reached safety. Keep its
                // reservation across live frames until RETURN_HP, otherwise
                // the per-unit ladder can undo the rotation with a sortie.
                // Other garrisons still need the proposed-strike veto.
                if doomed.contains(&uid) || (heals && self.battle_planner_recovering.contains(&uid))
                {
                    self.base.fortify_or_stop(g, pid, uid);
                    self.claim_recovery_formation(g, uid);
                }
                continue;
            }
            let here = field.rotation_danger(unit.pos, uid);
            // `falling-city-outranks-the-heal`: see `falling_city_calls`.
            let wounded = heals
                && (unit.hp < ROTATE_HP
                    || (self.battle_planner_recovering.contains(&uid)
                        && !self.falling_city_calls(g, pid, uid)));
            // A healthy member of an active Domination siege stays on its
            // post: the field charges every enemy blow to every unit at once,
            // and on King `civvis-20260930T221624Z` it rotated 89-hp archers
            // off an unwalled Stockholm for 27 turns while the train read
            // "damage ready". Only the wounded come out.
            // A healthy member of a siege still in Stage is the train's to
            // move as well: its Stage step holds a gun back on the gun's own
            // risk limit. Live King civvis-20261004T033533Z (game 46) rotated
            // four 100-hp catapults out at Babylon every turn from 135 to 153.
            if !wounded
                && (self.active_siege_member(g, pid, uid) || self.staging_siege_member(g, pid, uid))
            {
                continue;
            }
            // `defenders-hold-the-walls`: so does a threatened city's healthy
            // defender. See `advanced/defenders_hold_the_walls.rs`.
            if !wounded && self.holds_the_walls(g, pid, uid) {
                continue;
            }
            // A healthy explorer is not a garrison either. The same summed
            // field read 88-138 danger around full-health Skirmishers on King
            // `civvis-20261001T050754Z` and "rotated them out to heal" at 100
            // hp every turn from t90 to t130, so the two explorers paced the
            // home cities while Germany, the culture winner, went uncharted
            // until t147. Its own route (`explorer_turn`) steers around
            // known threats; only the wounded come out. A vetoed attacker
            // keeps the doomed path below, which has its own recon escape.
            if !wounded && !doomed.contains(&uid) && self.distance_scout_available(g, pid, uid) {
                continue;
            }
            // Where nothing heals, a unit is pulled out only when it would
            // otherwise be removed: without a recovery to remember, a margin
            // would walk it out of reach one turn and back into it the next.
            let margin = if heals { ROTATE_DANGER_MARGIN } else { 0 };
            let exposed = here > f64::from(unit.hp - margin);
            // `doomed-blow-veto`: a unit with no blow it would survive that is
            // neither wounded nor exposed may take a safe siege approach
            // step before fortifying — the ladder's attack stays denied;
            // one that is also exposed rotates like any other.
            if doomed.contains(&uid) && !(wounded || exposed) {
                let advanced = self.advance_vetoed_siege_unit(g, pid, uid, field);
                let escaped = !advanced && self.escape_vetoed_recon(g, pid, uid, field);
                self.base.fortify_or_stop(g, pid, uid);
                self.claim_recovery_formation(g, uid);
                if let Some(now) = g.units.get(&uid) {
                    if advanced {
                        think!(self.journal(), Military, Decision,
                            "Battle plan: the {} advances from {:?} to {:?} toward its siege", now.kind, unit.pos, now.pos;
                            "every attack was vetoed; this approach step has no predicted reply damage, and the unit's turn ends here");
                    } else if escaped {
                        think!(self.journal(), Military, Decision,
                            "Battle plan: the {} escapes from {:?} to {:?}", now.kind, unit.pos, now.pos;
                            "every attack was vetoed; this recon move has no predicted reply damage, and the unit's turn ends here");
                    } else {
                        think!(self.journal(), Military, Decision,
                        "Battle plan: the {} at {:?} holds rather than strike", now.kind, now.pos;
                        "{} hp, danger {here:.0} where it stands; every blow it has would leave it dead next turn",
                        now.hp);
                    }
                }
                continue;
            }
            if !(wounded || exposed) {
                continue;
            }
            // `wounded-ship-sails-on`: neutral water heals a ship nothing, so
            // one with no friendly water in reach would wait here forever.
            // See `advanced/wounded_ship_sails_on.rs`.
            if wounded && here <= NO_DANGER && self.wounded_ship_sails_on(g, pid, uid) {
                self.battle_planner_recovering.remove(&uid);
                *g.players[pid]
                    .counters
                    .entry("wounded_ships_sailed_on".to_string())
                    .or_insert(0) += 1;
                think!(self.journal(), Military, Decision,
                    "Battle plan: the {} at {:?} sails on", unit.kind, unit.pos;
                    "wounded-ship-sails-on: {} hp, no danger here, and no water in reach heals a \
                     ship, so it charts rather than wait for health that cannot come",
                    unit.hp);
                continue;
            }
            let healing_pillage = wounded
                && here <= NO_DANGER
                && g.pillageable_at(pid, unit.pos)
                && g.map
                    .get(unit.pos)
                    .and_then(|tile| tile.improvement.as_ref())
                    .and_then(|improvement| g.rules.improvements.get(improvement))
                    .is_some_and(|improvement| {
                        improvement.plunder_type.as_deref() == Some("heal")
                            && improvement.plunder_amount > 0.0
                    });
            if healing_pillage && g.apply(pid, &Action::Pillage { unit: uid }).is_ok() {
                self.base.fortify_or_stop(g, pid, uid);
                self.claim_recovery_formation(g, uid);
                self.battle_planner_recovering.insert(uid);
                if let Some(now) = g.units.get(&uid) {
                    think!(self.journal(), Military, Decision,
                        "Battle plan: the {} at {:?} pillages to recover", now.kind, now.pos;
                        "{} hp becomes {} hp on a legal healing improvement with no predicted incoming damage",
                        unit.hp, now.hp);
                }
                continue;
            }
            let mut tiles = vec![unit.pos];
            tiles.extend(g.reachable(uid));
            let mut safe: Vec<(i32, i32, Pos)> = tiles
                .into_iter()
                .filter(|tile| field.danger(*tile, uid) <= NO_DANGER)
                .map(|tile| {
                    (
                        -g.unit_heal_rate_at(uid, tile),
                        g.wdist(unit.pos, tile),
                        tile,
                    )
                })
                .collect();
            safe.sort_unstable();
            let relief = self.rotation_relief(g, pid, uid, field, here);
            let mut moved = false;
            let mut fell_back = false;
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
                let stand = match safe.first() {
                    Some(&(_, _, best)) => Some(best),
                    // `safest-stand`: no tile out of reach — the least
                    // dangerous one, or nothing.
                    None if self.safest_stand => {
                        self.least_danger_stand(g, uid, field, here, wounded)
                    }
                    None => None,
                };
                let Some(best) = stand else {
                    continue;
                };
                fell_back = safe.is_empty();
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
            if fell_back {
                self.census.battle_plan_fallbacks += 1;
            }
            self.base.fortify_or_stop(g, pid, uid);
            self.claim_recovery_formation(g, uid);
            if heals {
                self.battle_planner_recovering.insert(uid);
            }
            if let Some(now) = g.units.get(&uid) {
                think!(self.journal(), Military, Decision,
                    "Battle plan: the {} at {:?} {} to heal", now.kind, now.pos,
                    match (fell_back, moved) {
                        (false, true) => "rotates out",
                        (false, false) => "holds position",
                        (true, true) => "falls back to the least danger",
                        (true, false) => "holds under fire",
                    };
                    "{} hp, danger {here:.0} where it stood, {} where it stands{}",
                    now.hp, field.danger(now.pos, uid),
                    if fell_back { "; no tile in reach is out of reach" } else { "" });
            }
        }
        rotations
    }

    /// `safest-stand`: the least dangerous tile a unit with no zero-danger
    /// tile can stand on — the field's reading first, then the heal
    /// preference, then the distance — or `None` when even that tile would
    /// remove the unit (the ladder keeps it for a last blow) or, for a unit
    /// that is exposed but not wounded, when the step does not cut the danger
    /// by `ROTATE_DANGER_MARGIN` (a healthy unit's line is worth more than a
    /// few points off the field).
    fn least_danger_stand(
        &self,
        g: &Game,
        uid: u32,
        field: &mut DangerField,
        here: f64,
        wounded: bool,
    ) -> Option<Pos> {
        let unit = g.units.get(&uid)?;
        let mut tiles = vec![unit.pos];
        tiles.extend(g.reachable(uid));
        let mut ranked: Vec<(i64, i32, i32, Pos)> = tiles
            .into_iter()
            .map(|tile| {
                (
                    (field.danger(tile, uid) * 100.0).round() as i64,
                    -g.unit_heal_rate_at(uid, tile),
                    g.wdist(unit.pos, tile),
                    tile,
                )
            })
            .collect();
        ranked.sort_unstable();
        let &(least, _, _, best) = ranked.first()?;
        let least = least as f64 / 100.0;
        if least >= f64::from(unit.hp) {
            return None;
        }
        if !wounded && here - least < f64::from(ROTATE_DANGER_MARGIN) {
            return None;
        }
        Some(best)
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
                    && g.defending_district_at(unit.pos).is_none()
                    && !self.guard_is_reserved_for_civilian(*uid)
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
        let defense_anchor = self.defensive_formation_anchor(g, pid, group);
        let target = defense_anchor.unwrap_or(match group.posture {
            ForcePosture::Engage => group.focus_target.unwrap_or(group.objective),
            _ => group.objective,
        });
        let on_post = |tile| {
            defense_anchor.is_none_or(|anchor| {
                g.wdist(tile, anchor) <= super::defenders_hold_the_walls::WALL_DEFENDER_REACH
            })
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
        let mut contacts = Self::contacts(g, pid, &members, CONTACT_BAND);
        // `siege-at-the-walls-dies-first`: the shooters are laid at range of
        // the guns at our walls before the most finishable contacts, so the
        // next turn's shot is from where they stand. A stable sort; nothing
        // moves with the gene off.
        if self.siege_at_the_walls_dies_first {
            contacts.sort_by_key(|contact| !self.wall_threat(g, pid, contact.uid));
        }
        if let Some(anchor) = defense_anchor {
            contacts.retain(|contact| {
                g.wdist(contact.pos, anchor) <= super::defenders_hold_the_walls::WALL_THREAT_REACH
            });
        }
        let in_contact = !Self::contacts(g, pid, &members, CONTACT_RANGE).is_empty();

        // Who wants what. A wounded unit on a board that heals wants a heal
        // slot whatever its role.
        let mut wounded: Vec<u32> = Vec::new();
        let mut by_role: BTreeMap<SlotRole, Vec<u32>> = BTreeMap::new();
        for uid in &members {
            let unit = &g.units[uid];
            if heals && unit.hp < HEAL_SLOT_HP && !self.falling_city_calls(g, pid, *uid) {
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
                .filter(|tile| on_post(*tile))
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
                .filter(|tile| on_post(*tile))
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
                .filter(|tile| on_post(*tile))
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
                .filter(|tile| on_post(*tile))
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
                        -g.unit_heal_rate_at(*uid, tile),
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
        // `slots-survive-the-roll`: the centre roll is not the bar. A slot
        // whose blows at the top of their roll meet the unit's hit points is
        // one it would not survive, and is never assigned.
        if self.slots_survive_the_roll && field.upper_danger(tile, uid) >= f64::from(unit.hp) {
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
        // `slots-survive-the-roll`: a tile the top of the roll removes the
        // unit on ranks after every tile it does not, whatever the distance.
        // Without the gene the first key is always `false` and distance
        // leads, as it always has.
        let roll = self.slots_survive_the_roll;
        let mut keyed: Vec<(bool, i32, bool, Pos)> = free
            .iter()
            .map(|tile| {
                let removed = roll && field.upper_danger(*tile, uid) >= hp;
                (
                    removed,
                    g.wdist(*tile, slot),
                    field.danger(*tile, uid) >= hp,
                    *tile,
                )
            })
            .collect();
        keyed.sort_unstable();
        let unbounded = keyed.first().map(|(_, _, _, tile)| *tile);
        let bounded = keyed
            .iter()
            .find(|(_, _, _, tile)| pace_floor.is_none_or(|floor| g.wdist(*tile, target) >= floor))
            .map(|(_, _, _, tile)| *tile);
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
        strategy: &StrategicPlan,
        field: &mut DangerField,
        armed: &BTreeSet<u32>,
    ) {
        if !self.positions_plan_on() {
            return;
        }
        let groups = self.force_groups.clone();
        for group in &groups {
            // The siege doctrine owns this formation's approach and firing
            // posts. A generic slot can hold a gun outside range and claim
            // its turn before the siege ladder gets to move it. Kill shots
            // and wounded-unit rotations have already run above.
            if self.siege_train
                && group.domain == super::ForceDomain::Land
                && self.siege_city_of(g, pid, strategy, group).is_some()
            {
                continue;
            }
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

#[cfg(test)]
mod healing_tests;

#[cfg(test)]
mod linked_recovery_tests;

#[cfg(test)]
mod roll_tests;

#[cfg(test)]
mod recon_veto_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod siege_position_tests;

#[cfg(test)]
mod siege_veto_advance_tests;

#[cfg(test)]
mod future_district_fire_tests;

#[cfg(test)]
mod strike_reach_tests;
