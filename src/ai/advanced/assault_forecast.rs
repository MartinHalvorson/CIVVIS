//! The capture forecast: the chance a siege force takes its city within each
//! of the next [`HORIZON`] turns, the units it should expect to lose doing
//! it, and the one extra unit that would raise that chance most.
//!
//! Two opt-in genes read it, both off and byte-identical when off:
//!
//! - `assault-forecast-is-read` computes it for every siege the train
//!   assesses and writes one `Military/Decision` journal line. It changes no
//!   decision; it is there to calibrate the forecast on live games.
//! - `siege-goes-on-the-forecast` (implies the reading) lets the train act on
//!   it in `assess_siege`: Stage moves to Invest when P(capture within
//!   [`GO_HORIZON`]) reaches [`GO_P`] whatever the bill, the breaker and the
//!   damage budget say; a forecast at or above [`HOLD_P`] vetoes the invested
//!   train's abort; [`HOPELESS_STREAK`] gathered assessments in a row under
//!   [`HOPELESS_P`] that no one more unit would lift stand the siege down for
//!   [`HOPELESS_STAND_DOWN_TURNS`] (the train does not muster for it and the
//!   Objective Board leaves its row out); and the board ranks Siege rows by
//!   value × P / expected turns.
//!
//! Why: over 62 live Deity games on 2026-11-12 the siege planner made 873
//! decisions, 795 of them `stage` (631 reading "nothing to open the walls"),
//! and over 45 games the seat was at war with an unwalled city in reach on
//! 408 war-turns and took one. Both siege bills (`siege_bill` and the
//! campaign bill behind `siege_requirement`) are linear strength sums, and the
//! Stage -> Invest gate is a conjunction of them patched by a dozen genes;
//! none of it is a probability of taking the city.
//!
//! **The model** is the battle solver's Game-free board
//! (`battle_solver::model`): exact dice with the wound penalty, walls routing
//! blows by `city_take_damage`'s rule, the city's strike, its heal unless
//! besieged, garrison, zones of control and capture, with every strength read
//! once from the engine (`Game::melee_pair_bases` and its kin). The board is
//! every tile within [`GROUND_RADIUS`] of the city. Both sides play the Press
//! posture for [`HORIZON`] turns, [`ROLLOUTS`] times on common dice. A member
//! of the force, or a hostile in sight within [`HOSTILE_REACH`], that stands
//! off the board walks in: it enters on the edge tile its cheapest route
//! reaches, at the end of the turn that route takes (movement costs from
//! `Game::battle_step_cost`, a reverse search from the edge), and is never
//! dropped while it can arrive inside the horizon. The forecast drives
//! `Sim::side_turn` itself and places arrivals on its state. Two switches
//! on the board are its own and off on every solver board, so the solver's
//! play is unchanged: `march_on_objective` (our side walks to the city when
//! no enemy soldier is left; without it nobody moved on an empty city) and
//! `melee_breach_share` (our melee wait for the walls to fall to the train's
//! `MELEE_WALL_FRACTION`, as the taker does). The enemy acts every turn, not
//! on the solver's half ([`KNOBS`]).
//!
//! **What it is short of** is found by running the same rollouts again with
//! one hypothetical unit added, spawned on a clone of the board so the engine
//! reads its strengths: our best melee taker, our best siege gun, or one more
//! of the force's most common unit, entering when the force's median member
//! does. The kind whose addition raises P(capture within [`HORIZON`]) most,
//! by at least [`SHORT_OF_GAIN`], is named.
//!
//! **What it does not see:** units the city's owner will build or bring from
//! out of reach, units out of sight, ships and aircraft, a Battering Ram or
//! Siege Tower being killed on its way (it works from the turn its walk
//! would bring it beside the city), and promotions earned during the fight.
//! Its enemy is the solver's greedy one. On 1,200 posed sieges
//! (`docs/ASSAULT_FORECAST.md`) it is right where it reads under 20% (4%
//! taken) and overconfident above 80% (44% taken as #5195 ran them, 81% with
//! the seats kept solvent): read it as "this force could take this city", not
//! "this train will". The reading gene is there to calibrate it on live
//! games.
//!
//! **For callers** (`docs/ASSAULT_FORECAST.md`, all callable with both genes
//! off): `assault_forecast` (at war), `assault_forecast_at_peace` (as if the
//! war began this turn), `assault_forecast_recruiting` (plus hypothetical
//! units arriving at the board's edge), `assault_corps_for` (the smallest
//! subset of a pool that reaches a chance), and the provisional
//! `AssaultForecast::calibrated_within`. A Battering Ram or Siege Tower in
//! the force works the engine's way (`Game::siege_support_effects`).
//!
//! A forecast is cached for the turn by the city's tile and the signature of
//! the force and the hostiles (kind, tile, health), so a turn pays once; the
//! hopeless streak is kept by the city's tile, as live city ids change
//! between frames.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use super::battle_solver::model::{
    Board, CitySpec, Objective, Posture, Sim, State, Unit, Weights, F_CAPTURES, F_IGNORES_ZOC,
    F_MELEE, F_MOVE_AND_FIRE, F_RANGED, F_SIEGE, F_WALL_SUPPORTED, F_ZOC, IMPASSABLE, MP, NO_CITY,
    NO_TILE, NO_UNIT, OURS, SUPPORT_RAM, SUPPORT_TOWER, THEIRS,
};
use super::AdvancedAi;
use crate::game::{Game, Item};
use crate::name::Name;
use crate::rules::UnitSpec;
use crate::think;
use crate::Pos;

/// Turns the forecast looks ahead: P(capture within k turns), k = 1..=12.
pub(crate) const HORIZON: usize = 12;
/// Rollouts per forecast, and per hypothetical variant on the same dice.
pub(crate) const ROLLOUTS: u32 = 64;
/// The board: every tile within this many of the city.
pub(crate) const GROUND_RADIUS: i32 = 4;
/// Hostiles in sight within this many tiles of the city join the fight.
pub(crate) const HOSTILE_REACH: i32 = 8;
/// The farthest a member may stand and still be routed in.
pub(crate) const ROUTE_REACH: i32 = 24;
/// Members read, nearest the city first; the solver's own cap.
pub(crate) const MAX_MEMBERS: usize = 20;
const MAX_HOSTILES: usize = 24;
/// The least gain in P(capture within the horizon) that names a unit the
/// force is short of.
pub(crate) const SHORT_OF_GAIN: f32 = 0.05;
/// A member on the board within this many turns counts as gathered.
const GATHERED_TURNS: u8 = 3;
/// A held-out unit's entry round.
const NEVER: u8 = u8::MAX;
/// A city's worth to the solver's score, which the forecast never reads; set
/// as `battle_solver::extract` sets it so the board is the solver's board.
const CITY_BASE_VALUE: f32 = 220.0;
const CITY_POP_VALUE: f32 = 45.0;

/// `siege-goes-on-the-forecast`: Stage moves to Invest at or above this
/// P(capture within [`GO_HORIZON`]).
pub(crate) const GO_P: f32 = 0.55;
/// The horizon the go and hold readings use.
pub(crate) const GO_HORIZON: usize = 10;
/// At or above this P(capture within [`GO_HORIZON`]) an invested train does
/// not abort.
pub(crate) const HOLD_P: f32 = 0.4;
/// Under this P(capture within [`HORIZON`]) a gathered assessment is
/// hopeless.
pub(crate) const HOPELESS_P: f32 = 0.1;
/// Hopeless gathered assessments in a row that stand the siege down.
pub(crate) const HOPELESS_STREAK: u32 = 3;
/// How long a hopeless siege stays down.
pub(crate) const HOPELESS_STAND_DOWN_TURNS: u32 = 10;
/// A falling city (walls down, at most half its health) is never stood down.
const FALLING_CITY_HP: i32 = 100;

/// The extra unit a force is short of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShortOf {
    Nothing,
    Taker,
    Breaker,
    Bodies,
}

impl ShortOf {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            ShortOf::Nothing => "nothing",
            ShortOf::Taker => "taker",
            ShortOf::Breaker => "breaker",
            ShortOf::Bodies => "bodies",
        }
    }
}

/// One forecast of one siege.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AssaultForecast {
    /// P(captured by the end of our turn `k`) at index `k - 1`.
    pub(crate) by_turn: [f32; HORIZON],
    /// Own units expected lost before the capture or the horizon.
    pub(crate) units_lost: f32,
    /// The mean turn of the captures; `None` with none.
    pub(crate) mean_turn: Option<f32>,
    /// What one more unit would help most; `None` when not asked.
    pub(crate) short_of: Option<ShortOf>,
    /// Members on the board now, and members walking in.
    pub(crate) present: usize,
    pub(crate) arriving: usize,
    /// Members on the board within [`GATHERED_TURNS`] turns.
    pub(crate) gathered: usize,
    pub(crate) hostiles: usize,
    pub(crate) rollouts: u32,
    /// The city's health and walls at the end of each turn (after its
    /// owner's), averaged over the rollouts; a taken city counts zero. What
    /// a press on this board does to the city, turn by turn.
    pub(crate) city_health: [f32; HORIZON],
}

impl AssaultForecast {
    /// P(capture within `turns` turns), `turns` clamped to `1..=HORIZON`.
    pub(crate) fn within(&self, turns: usize) -> f32 {
        self.by_turn[turns.clamp(1, HORIZON) - 1]
    }

    /// P(capture within [`HORIZON`]).
    pub(crate) fn p(&self) -> f32 {
        self.within(HORIZON)
    }

    pub(crate) fn members(&self) -> usize {
        self.present + self.arriving
    }

    /// Three in four of the members stand on the board within
    /// [`GATHERED_TURNS`]: a low reading is about the city, not the march.
    pub(crate) fn gathered(&self) -> bool {
        self.members() > 0 && self.gathered * 4 >= self.members() * 3
    }

    /// Captures per turn: P over the expected turn of a capture. The Siege
    /// row's rank weight under `siege-goes-on-the-forecast`.
    pub(crate) fn rate(&self) -> f64 {
        match self.mean_turn {
            Some(turn) if self.p() > 0.0 => f64::from(self.p()) / f64::from(turn.max(1.0)),
            _ => 0.0,
        }
    }

    /// PROVISIONAL. A conservative P(capture within `turns`) for a caller
    /// that turns the chance into an expected value, against a city whose
    /// wall pool is `walls`. A raw reading under [`CALIBRATED_FROM`] passes
    /// through: low readings were right. Above it the reading is pulled
    /// down, linearly, to the rate the high bin was realized at for that
    /// wall tier ([`HIGH_BIN_TAKEN`], `docs/ASSAULT_FORECAST.md`), and never
    /// raised. To be replaced once #5207 fixes the cause of the gap.
    #[allow(
        dead_code,
        reason = "an entry point for the Score lane's target board; no caller in this tree yet"
    )]
    pub(crate) fn calibrated_within(&self, turns: usize, walls: i32) -> f32 {
        let raw = self.within(turns);
        if raw < CALIBRATED_FROM {
            return raw;
        }
        let tier = if walls < 50 {
            0
        } else if walls < 150 {
            1
        } else if walls < 250 {
            2
        } else {
            3
        };
        let (_, read_high, taken) = HIGH_BIN_TAKEN[tier];
        let top = taken as f32 / read_high as f32;
        let calibrated = CALIBRATED_FROM
            + (raw - CALIBRATED_FROM) / (1.0 - CALIBRATED_FROM) * (top - CALIBRATED_FROM);
        calibrated.min(raw)
    }

    /// The first turn by which nine tenths of the horizon's chance is in.
    pub(crate) fn turn_by(&self) -> usize {
        let p = self.p();
        if p <= 0.0 {
            return HORIZON;
        }
        (1..=HORIZON)
            .find(|k| self.within(*k) >= 0.9 * p)
            .unwrap_or(HORIZON)
    }
}

/// `AssaultForecast::calibrated_within`: readings under this pass through.
pub(crate) const CALIBRATED_FROM: f32 = 0.2;
/// PROVISIONAL, `docs/ASSAULT_FORECAST.md`: of the 1,200 posed sieges with
/// solvent seats (#5207, CI run 43303003805 on c908fea4c6), per wall pool,
/// the sieges read at P(capture within 12) >= 0.8 and how many were taken
/// within 12 turns. A test holds these to the doc's table.
pub(crate) const HIGH_BIN_TAKEN: [(i32, u32, u32); 4] =
    [(0, 165, 141), (100, 113, 94), (200, 109, 86), (400, 86, 60)];

/// The turn's forecasts, by city tile and the force and hostile signature.
#[derive(Clone, Default)]
pub(crate) struct ForecastFrame {
    turn: u32,
    entries: BTreeMap<(Pos, u64), AssaultForecast>,
    corps: BTreeMap<(Pos, u64), CorpsSearch>,
    /// Forecasts computed (not served from the cache) this turn.
    pub(crate) computed: u32,
}

/// `siege-goes-on-the-forecast`: one city's run of hopeless readings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HopelessRun {
    pub(crate) streak: u32,
    pub(crate) last: u32,
    /// Stood down while the turn is under this.
    pub(crate) down_until: u32,
}

/// The forecast's own settings on the solver's board.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Knobs {
    /// `Board::melee_breach_share`.
    pub(crate) melee_breach_share: f32,
    /// `Weights::enemy_aggression`.
    pub(crate) enemy_aggression: f32,
}

/// What the forecast plays: our melee wait for the breach as the train's
/// taker does (`siege_train::MELEE_WALL_FRACTION`), and the enemy acts on
/// every turn rather than the solver's half: on 1,200 posed sieges that
/// scored the better Brier at 12, 10 and 5 turns (`docs/ASSAULT_FORECAST.md`).
pub(crate) const KNOBS: Knobs = Knobs {
    melee_breach_share: super::siege_train::MELEE_WALL_FRACTION as f32,
    enemy_aggression: 1.0,
};

// CI scaffolding: the forecast settings this thread reads, `KNOBS` unless
// set.
#[cfg(test)]
thread_local! {
    pub(crate) static FORECAST_KNOBS: std::cell::Cell<Option<Knobs>> =
        const { std::cell::Cell::new(None) };
}

#[cfg(test)]
fn knobs() -> Knobs {
    FORECAST_KNOBS.with(|k| k.get()).unwrap_or(KNOBS)
}

#[cfg(not(test))]
fn knobs() -> Knobs {
    KNOBS
}

/// CI scaffolding: variants of `siege-goes-on-the-forecast` for the
/// whole-world experiment (`world_tests`), set per thread. Outside tests,
/// and by default, the gene as written.
#[cfg(test)]
pub(crate) mod variant {
    use std::cell::Cell;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(crate) struct Variant {
        /// Hopeless readings in a row that stand a siege down; `None` is
        /// `HOPELESS_STREAK`.
        pub(crate) streak: Option<u32>,
        /// A reading under `HOPELESS_P` counts whatever one more unit would
        /// do.
        pub(crate) any_short_of: bool,
        /// The stand-down alone: no go, no held abort.
        pub(crate) stand_down_only: bool,
    }

    impl Variant {
        pub(crate) const MERGED: Variant = Variant {
            streak: None,
            any_short_of: false,
            stand_down_only: false,
        };
    }

    thread_local! {
        static VARIANT: Cell<Variant> = const { Cell::new(Variant::MERGED) };
    }

    pub(crate) fn set(variant: Variant) {
        VARIANT.with(|cell| cell.set(variant));
    }

    pub(crate) fn get() -> Variant {
        VARIANT.with(Cell::get)
    }
}

/// Gathered hopeless readings in a row that stand a siege down.
fn hopeless_streak() -> u32 {
    #[cfg(test)]
    {
        variant::get().streak.unwrap_or(HOPELESS_STREAK)
    }
    #[cfg(not(test))]
    {
        HOPELESS_STREAK
    }
}

/// Whether a hopeless reading needs no one more unit to lift it.
fn hopeless_needs_nothing_short() -> bool {
    #[cfg(test)]
    {
        !variant::get().any_short_of
    }
    #[cfg(not(test))]
    {
        true
    }
}

/// Whether the forecast's go and held abort act (the stand-down always does).
pub(super) fn forecast_goes_and_holds() -> bool {
    #[cfg(test)]
    {
        !variant::get().stand_down_only
    }
    #[cfg(not(test))]
    {
        true
    }
}

/// A land soldier the model plays: no ships, aircraft or embarked units, as
/// `battle_solver::extract` reads one.
fn plays_on_land(g: &Game, uid: u32) -> bool {
    let unit = &g.units[&uid];
    let spec = &g.rules.units[unit.kind];
    spec.class == "military"
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        && !g.is_embarked(unit)
        && (spec.is_melee_capable() || spec.has_ranged_attack())
}

/// A land melee unit that can take a city: not a gun and not a shooter.
fn melee_taker(spec: &UnitSpec) -> bool {
    spec.class == "military"
        && !spec.siege
        && spec.is_melee_capable()
        && !spec.has_ranged_attack()
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
}

/// A land siege gun.
fn siege_gun(spec: &UnitSpec) -> bool {
    spec.class == "military" && spec.siege && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
}

#[inline]
fn mix(a: u64) -> u64 {
    let mut z = a.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// FNV-1a over the words given: deterministic on every machine.
fn fnv(words: impl Iterator<Item = u64>) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for w in words {
        for byte in w.to_le_bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// The members the forecast reads: land soldiers of ours in `force`, nearest
/// the city first, within [`ROUTE_REACH`].
fn members_of(g: &Game, pid: usize, center: Pos, force: &[u32]) -> Vec<u32> {
    let mut members: Vec<u32> = force
        .iter()
        .copied()
        .filter(|uid| {
            g.units
                .get(uid)
                .is_some_and(|unit| unit.owner == pid && g.wdist(unit.pos, center) <= ROUTE_REACH)
                && plays_on_land(g, *uid)
        })
        .collect();
    members.sort_unstable();
    members.dedup();
    members.sort_by_key(|uid| (g.wdist(g.units[uid].pos, center), *uid));
    members.truncate(MAX_MEMBERS);
    members
}

/// The hostiles in sight within [`HOSTILE_REACH`] of the city, nearest first.
fn hostiles_of(g: &Game, pid: usize, center: Pos) -> Vec<u32> {
    let mut hostiles: Vec<u32> = g
        .units
        .values()
        .filter(|unit| {
            unit.owner != pid
                && g.wdist(unit.pos, center) <= HOSTILE_REACH
                && g.is_at_war(pid, unit.owner)
                && plays_on_land(g, unit.id)
                && g.unit_visible_to(unit.id, pid)
        })
        .map(|unit| unit.id)
        .collect();
    hostiles.sort_by_key(|uid| (g.wdist(g.units[uid].pos, center), *uid));
    hostiles.truncate(MAX_HOSTILES);
    hostiles
}

fn signature(g: &Game, pid: usize, members: &[u32], hostiles: &[u32]) -> u64 {
    let unit_words = |uid: &u32| {
        let unit = &g.units[uid];
        [
            u64::from(unit.kind.id()),
            ((unit.pos.0 as u32 as u64) << 32) | (unit.pos.1 as u32 as u64),
            unit.hp as u32 as u64,
        ]
    };
    let mut ours: Vec<[u64; 3]> = members.iter().map(unit_words).collect();
    ours.sort_unstable();
    let mut theirs: Vec<[u64; 3]> = hostiles.iter().map(unit_words).collect();
    theirs.sort_unstable();
    fnv(std::iter::once(pid as u64)
        .chain(ours.into_iter().flatten())
        .chain(std::iter::once(u64::MAX))
        .chain(theirs.into_iter().flatten()))
}

/// Movement twelfths from each tile off the board (within `reach` of the
/// city) to the board's edge, walking as `rep` walks, and the edge tile the
/// cheapest walk enters by. A reverse search from the edge; it stops once
/// every tile in `wanted` is settled.
fn routes_in(
    g: &Game,
    rep: u32,
    center: Pos,
    wanted: &[Pos],
    reach: i32,
) -> BTreeMap<Pos, (u32, Pos)> {
    let mut best: BTreeMap<Pos, (u32, Pos)> = BTreeMap::new();
    let mut heap: BinaryHeap<Reverse<(u32, Pos)>> = BinaryHeap::new();
    for edge in g.wring(center, GROUND_RADIUS) {
        if g.map.get(edge).is_some() {
            best.insert(edge, (0, edge));
            heap.push(Reverse((0, edge)));
        }
    }
    let mut left: BTreeSet<Pos> = wanted.iter().copied().collect();
    while let Some(Reverse((cost, at))) = heap.pop() {
        let Some(&(known, origin)) = best.get(&at) else {
            continue;
        };
        if known < cost {
            continue;
        }
        left.remove(&at);
        if left.is_empty() {
            break;
        }
        for from in g.nbrs(at) {
            let distance = g.wdist(from, center);
            if distance <= GROUND_RADIUS || distance > reach {
                continue;
            }
            let Some(step) = g.battle_step_cost(rep, from, at) else {
                continue;
            };
            let next = cost + (step * f64::from(MP)).round().max(1.0) as u32;
            if best.get(&from).is_none_or(|(old, _)| next < *old) {
                best.insert(from, (next, origin));
                heap.push(Reverse((next, from)));
            }
        }
    }
    best
}

/// A siege laid out on the solver's board.
struct Posed {
    board: Board,
    /// Start of every rollout: units not yet on the board are dead and off
    /// the tile table until they enter.
    state: State,
    city: usize,
    /// Per unit: the round at whose end it stands on the board; 0 = there
    /// now; [`NEVER`] = held out (a hypothetical outside its own variant).
    enter: Vec<u8>,
    entry_hp: Vec<i16>,
    entry_tile: Vec<u16>,
    /// Per class and tile: a unit of the class may stand there.
    standable: Vec<Vec<bool>>,
    /// Members are units `0..members`.
    members: usize,
    /// The hypothetical units, by index.
    extras: Vec<(ShortOf, usize)>,
    /// The round at whose end our Battering Ram, and our Siege Tower, stand
    /// beside the city; [`NEVER`] without one (`support_rounds`).
    support: [u8; 2],
    present: usize,
    arriving: usize,
    gathered: usize,
    hostiles: usize,
}

/// Lay the siege of `cid` out: `members` and `hostiles` (read off this
/// board), then `extras`, held out of every run but their own.
fn pose(
    g: &Game,
    pid: usize,
    cid: u32,
    members: &[u32],
    hostiles: &[u32],
    extras: &[(ShortOf, u32)],
    arrive: &BTreeMap<u32, u8>,
) -> Option<Posed> {
    let target = g.cities.get(&cid)?;
    let center = target.pos;
    let mut ground: Vec<Pos> = g
        .wdisk(center, GROUND_RADIUS)
        .into_iter()
        .filter(|pos| g.map.get(*pos).is_some())
        .collect();
    ground.sort_unstable();
    ground.dedup();
    let index: BTreeMap<Pos, u16> = ground
        .iter()
        .enumerate()
        .map(|(k, pos)| (*pos, k as u16))
        .collect();
    let tiles = ground.len();

    // Who stands off the board, and when their route brings them on.
    let candidates: Vec<(u32, u8)> = members
        .iter()
        .map(|uid| (*uid, OURS))
        .chain(hostiles.iter().map(|uid| (*uid, THEIRS)))
        .collect();
    let mut arrival: Vec<Option<(u8, u16)>> = vec![None; candidates.len()];
    let mut routing: BTreeMap<(Name, u8), Vec<usize>> = BTreeMap::new();
    for (k, (uid, side)) in candidates.iter().enumerate() {
        let unit = &g.units[uid];
        if let (Some(round), Some(tile)) = (arrive.get(uid), index.get(&unit.pos)) {
            // A recruit: it stands on its entry tile and walks in on its
            // round.
            if *round <= HORIZON as u8 {
                arrival[k] = Some((*round, *tile));
            }
        } else if let Some(tile) = index.get(&unit.pos) {
            arrival[k] = Some((0, *tile));
        } else {
            routing.entry((unit.kind, *side)).or_default().push(k);
        }
    }
    for ks in routing.values() {
        let rep = candidates[ks[0]].0;
        let wanted: Vec<Pos> = ks.iter().map(|k| g.units[&candidates[*k].0].pos).collect();
        let reach = wanted
            .iter()
            .map(|pos| g.wdist(*pos, center) + 1)
            .max()
            .unwrap_or(0)
            .min(ROUTE_REACH);
        let routes = routes_in(g, rep, center, &wanted, reach);
        for k in ks {
            let uid = candidates[*k].0;
            let Some((cost, edge)) = routes.get(&g.units[&uid].pos) else {
                continue;
            };
            let moves = (g.unit_max_moves(uid) * f64::from(MP)).round().max(1.0);
            let turns = (f64::from(*cost) / moves).ceil().max(1.0);
            if turns <= HORIZON as f64 {
                arrival[*k] = Some((turns as u8, index[edge]));
            }
        }
    }

    // The units that play: those that arrive, then the hypotheticals last,
    // so a run without them rolls the same dice as a board without them.
    let mut ids: Vec<u32> = Vec::new();
    let mut sides: Vec<u8> = Vec::new();
    let mut enter: Vec<u8> = Vec::new();
    let mut entry_tile: Vec<u16> = Vec::new();
    for (k, (uid, side)) in candidates.iter().enumerate() {
        if let Some((round, tile)) = arrival[k] {
            ids.push(*uid);
            sides.push(*side);
            enter.push(round);
            entry_tile.push(tile);
        }
    }
    let member_count = sides.iter().filter(|side| **side == OURS).count();
    let hostile_count = sides.len() - member_count;
    let mut extra_index = Vec::new();
    for (what, uid) in extras {
        let Some(tile) = g.units.get(uid).and_then(|unit| index.get(&unit.pos)) else {
            continue;
        };
        extra_index.push((*what, ids.len()));
        ids.push(*uid);
        sides.push(OURS);
        enter.push(NEVER);
        entry_tile.push(*tile);
    }
    let n = ids.len();

    // Classes: one per kind and side, read off its first unit.
    let mut classes: Vec<(Name, u8, u32)> = Vec::new();
    let class: Vec<u8> = ids
        .iter()
        .zip(&sides)
        .map(|(uid, side)| {
            let kind = g.units[uid].kind;
            match classes
                .iter()
                .position(|(k, s, _)| *k == kind && *s == *side)
            {
                Some(c) => c as u8,
                None => {
                    classes.push((kind, *side, *uid));
                    (classes.len() - 1) as u8
                }
            }
        })
        .collect();
    let classes_n = classes.len().max(1);

    let mut units = Vec::with_capacity(n);
    let mut entry_hp = Vec::with_capacity(n);
    for (i, uid) in ids.iter().enumerate() {
        let u = &g.units[uid];
        let spec = &g.rules.units[u.kind];
        let mut flags = 0u8;
        if g.exerts_zoc(u) {
            flags |= F_ZOC;
        }
        if g.unit_ignores_zoc(*uid) {
            flags |= F_IGNORES_ZOC;
        }
        if spec.siege {
            flags |= F_SIEGE;
        } else if spec.has_ranged_attack() {
            flags |= F_RANGED;
        } else if spec.is_melee_capable() {
            flags |= F_MELEE | F_CAPTURES;
        }
        if g.siege_may_attack_after_moving(u) {
            flags |= F_MOVE_AND_FIRE;
        }
        if sides[i] == OURS && matches!(spec.promotion_class.as_str(), "melee" | "anti_cavalry") {
            flags |= F_WALL_SUPPORTED;
        }
        let here = enter[i] == 0;
        let max_mp = (g.unit_max_moves(*uid) * f64::from(MP)).round() as u16;
        let max_attacks = g.battle_max_attacks(*uid).clamp(1, 5) as u8;
        units.push(Unit {
            tile: entry_tile[i],
            hp: if here { u.hp as i16 } else { 0 },
            mp: max_mp,
            max_mp,
            value: spec.cost as f32,
            side: sides[i],
            attacks: max_attacks,
            max_attacks,
            fort: if here {
                u.fortify_turns.clamp(0, 2) as u8
            } else {
                0
            },
            flags,
            range: if spec.has_ranged_attack() {
                g.unit_attack_range(*uid).clamp(1, 8) as u8
            } else {
                1
            },
            class: class[i],
            acted: if here { u.acted } else { true },
        });
        entry_hp.push(u.hp.clamp(1, 100) as i16);
    }

    // Ground: neighbours, rivers, footing; a neutral soldier or city is a
    // wall.
    let mut nbrs = vec![[NO_TILE; 6]; tiles];
    let mut river = vec![0u8; tiles];
    let mut defense = vec![0.0f32; tiles];
    let mut land = vec![false; tiles];
    let mut blocked = vec![false; tiles];
    for (t, pos) in ground.iter().enumerate() {
        for (d, nb) in g.nbrs(*pos).into_iter().enumerate().take(6) {
            if let Some(k) = index.get(&nb) {
                nbrs[t][d] = *k;
                if g.battle_crosses_river(*pos, nb) {
                    river[t] |= 1 << d;
                }
            }
        }
        defense[t] = g.tile_defense_bonus(*pos) as f32;
        land[t] = g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile));
        blocked[t] = g.unit_ids_at(*pos).iter().any(|id| {
            let other = &g.units[id];
            other.owner != pid
                && !g.is_at_war(pid, other.owner)
                && g.rules.units[other.kind].class == "military"
        }) || g.city_at(*pos).is_some_and(|other| {
            let owner = g.cities[&other].owner;
            owner != pid && !g.is_at_war(pid, owner)
        });
    }
    let mut edge = vec![IMPASSABLE; classes_n * tiles * 6];
    let mut standable = vec![vec![false; tiles]; classes_n];
    for (c, (_, _, rep)) in classes.iter().enumerate() {
        for (t, pos) in ground.iter().enumerate() {
            for (d, &k) in nbrs[t].iter().enumerate() {
                if k == NO_TILE || blocked[k as usize] {
                    continue;
                }
                if let Some(cost) = g.battle_step_cost(*rep, *pos, ground[k as usize]) {
                    edge[(c * tiles + t) * 6 + d] = (cost * f64::from(MP)).round().max(1.0) as u16;
                    standable[c][k as usize] = true;
                }
            }
        }
    }
    let mut dist = vec![0u8; tiles * tiles];
    for (a, from) in ground.iter().enumerate() {
        for (b, to) in ground.iter().enumerate() {
            dist[a * tiles + b] = g.wdist(*from, *to).min(250) as u8;
        }
    }

    // Cities on the ground: ours and those we are at war with.
    let mut city_at = vec![NO_CITY; tiles];
    let mut cities = Vec::new();
    let mut city_ids = Vec::new();
    let mut city_state = Vec::new();
    for (id, city) in g.cities.iter() {
        let Some(tile) = index.get(&city.pos) else {
            continue;
        };
        let side = if city.owner == pid {
            OURS
        } else if g.is_at_war(pid, city.owner) {
            THEIRS
        } else {
            continue;
        };
        city_at[*tile as usize] = cities.len() as u8;
        let wound = (10.0 - f64::from(city.hp.clamp(0, 200)) / 20.0).round();
        cities.push(CitySpec {
            tile: *tile,
            strength: (g.city_strength(*id) + wound) as f32,
            wall_max: g.city_max_wall_hp(city) as i16,
            strike: if city.wall_hp > 0 {
                g.city_ranged_strength(*id) as f32
            } else {
                0.0
            },
            value: CITY_BASE_VALUE + CITY_POP_VALUE * city.pop as f32,
            zoc: true,
        });
        city_ids.push(*id);
        city_state.push((city.hp as i16, city.wall_hp as i16, side));
    }
    let city = city_ids.iter().position(|id| *id == cid)?;

    // Heal rates by ground and side, from a representative soldier of each.
    let rep_ours = ids.iter().zip(&sides).find(|(_, s)| **s == OURS);
    let rep_theirs = ids.iter().zip(&sides).find(|(_, s)| **s == THEIRS);
    let mut heal = vec![[0u8; 2]; tiles];
    for (t, pos) in ground.iter().enumerate() {
        if let Some((uid, _)) = rep_ours {
            heal[t][OURS as usize] = g.unit_heal_rate_at(*uid, *pos).clamp(0, 100) as u8;
        }
        if let Some((uid, _)) = rep_theirs {
            heal[t][THEIRS as usize] = g.unit_heal_rate_at(*uid, *pos).clamp(0, 100) as u8;
        }
    }

    // Strength tables, as the engine reads each pair.
    let city_n = cities.len();
    let mut melee_att = vec![0.0f32; n * n];
    let mut melee_def = vec![0.0f32; n * n];
    let mut ranged_att = vec![0.0f32; n * n];
    let mut ranged_def = vec![0.0f32; n * n];
    let mut city_melee = vec![0.0f32; n * city_n];
    let mut city_ranged = vec![0.0f32; n * city_n];
    for (i, a) in ids.iter().enumerate() {
        for (j, d) in ids.iter().enumerate() {
            if i == j {
                // The diagonal holds a unit's plain defence, for city strikes.
                let u = &g.units[d];
                melee_def[i * n + j] =
                    (g.unit_strength(u, true) - 3.0 * u.fortify_turns.clamp(0, 2) as f64) as f32;
                continue;
            }
            if sides[i] == sides[j] {
                continue;
            }
            if let Some((att, def)) = g.melee_pair_bases(*a, *d) {
                melee_att[i * n + j] = att as f32;
                melee_def[i * n + j] = def as f32;
            }
            if units[i].flags & (F_RANGED | F_SIEGE) != 0 {
                if let Some((att, def)) = g.ranged_pair_bases(*a, *d) {
                    ranged_att[i * n + j] = att as f32;
                    ranged_def[i * n + j] = def as f32;
                }
            }
        }
        for (c, id) in city_ids.iter().enumerate() {
            if let Some((melee, ranged)) = g.city_pair_bases(*a, *id) {
                city_melee[i * city_n + c] = melee as f32;
                city_ranged[i * city_n + c] = ranged as f32;
            }
        }
    }
    let flanks = |owner: usize| {
        if g.battle_flanks_unlocked(owner) {
            2.0
        } else {
            0.0
        }
    };
    let board = Board {
        tiles,
        nbrs,
        river,
        defense,
        land,
        dist,
        edge,
        city_at,
        cities,
        heal,
        flank: [flanks(pid), flanks(target.owner)],
        support: [flanks(pid), flanks(target.owner)],
        n,
        melee_att,
        melee_def,
        ranged_att,
        ranged_def,
        city_melee,
        city_ranged,
        reach0: vec![Vec::new(); n],
        los0: Vec::new(),
        has_los: false,
        // A city with no soldier left beside it is still to be walked to.
        march_on_objective: true,
        melee_breach_share: knobs().melee_breach_share,
        // No host traits: the forecast's enemy is the solver's greedy one.
        host: Vec::new(),
    };
    let state = State::new(&board, units, city_state);
    let member_rounds = &enter[..member_count];
    let present = member_rounds.iter().filter(|r| **r == 0).count();
    let gathered = member_rounds
        .iter()
        .filter(|r| **r <= GATHERED_TURNS)
        .count();
    Some(Posed {
        board,
        state,
        city,
        enter,
        entry_hp,
        entry_tile,
        standable,
        members: member_count,
        extras: extra_index,
        support: [NEVER; 2],
        present,
        arriving: member_count - present,
        gathered,
        hostiles: hostile_count,
    })
}

/// What a set of rollouts saw.
#[derive(Default)]
struct Tally {
    captured: [u32; HORIZON],
    lost: u32,
    turn_sum: u32,
    taken: u32,
    runs: u32,
    health: [f32; HORIZON],
}

impl Tally {
    fn p(&self) -> f32 {
        if self.runs == 0 {
            0.0
        } else {
            self.captured[HORIZON - 1] as f32 / self.runs as f32
        }
    }

    fn forecast(&self, posed: &Posed) -> AssaultForecast {
        let runs = self.runs.max(1) as f32;
        let mut by_turn = [0.0f32; HORIZON];
        for (p, count) in by_turn.iter_mut().zip(&self.captured) {
            *p = *count as f32 / runs;
        }
        AssaultForecast {
            by_turn,
            units_lost: self.lost as f32 / runs,
            mean_turn: (self.taken > 0).then(|| self.turn_sum as f32 / self.taken as f32),
            short_of: None,
            present: posed.present,
            arriving: posed.arriving,
            gathered: posed.gathered,
            hostiles: posed.hostiles,
            rollouts: self.runs,
            city_health: self.health.map(|sum| sum / runs),
        }
    }
}

/// Put unit `i` on the board at its entry tile, or the nearest free tile a
/// unit of its class may stand on, two or more from the city. False when the
/// board has no room this turn.
fn place(sim: &mut Sim, posed: &Posed, i: usize) -> bool {
    let b = &posed.board;
    let class = sim.s.units[i].class as usize;
    let city_tile = b.cities[posed.city].tile;
    let free = |t: u16| {
        sim.s.occ[t as usize] == NO_UNIT
            && b.city_at[t as usize] == NO_CITY
            && posed.standable[class][t as usize]
    };
    let want = posed.entry_tile[i];
    let tile = if free(want) {
        Some(want)
    } else {
        (0..b.tiles as u16)
            .filter(|t| free(*t) && b.d(*t, city_tile) >= 2)
            .min_by_key(|t| (b.d(want, *t), *t))
    };
    let Some(tile) = tile else {
        return false;
    };
    let unit = &mut sim.s.units[i];
    unit.hp = posed.entry_hp[i];
    unit.tile = tile;
    unit.mp = 0;
    unit.attacks = 0;
    unit.fort = 0;
    unit.acted = true;
    sim.s.occ[tile as usize] = i as u16;
    true
}

/// [`ROLLOUTS`] sieges played forward from `posed`, with `extra` (a
/// hypothetical's index and its entry round) on the board too.
fn play_out(posed: &Posed, extra: Option<(usize, u8)>, seed: u64, rollouts: u32) -> Tally {
    let objective = Objective::Capture(posed.city as u8);
    let weights = Weights {
        enemy_aggression: knobs().enemy_aggression,
        ..Weights::default()
    };
    let mut sim = Sim::new(&posed.board, &posed.state, weights, objective);
    let n = posed.state.units.len();
    let mut enter = posed.enter.clone();
    if let Some((i, round)) = extra {
        enter[i] = round;
    }
    let mut on_board = vec![false; n];
    let mut tally = Tally::default();
    for run in 0..rollouts {
        sim.reset(
            &posed.state,
            mix(seed ^ u64::from(run + 1).wrapping_mul(0xA24B_AED4_963E_E407)),
        );
        for (flag, (round, unit)) in on_board
            .iter_mut()
            .zip(enter.iter().zip(&posed.state.units))
        {
            *flag = *round == 0 && unit.alive();
        }
        if let Some((i, 0)) = extra {
            on_board[i] = place(&mut sim, posed, i);
        }
        let mut taken = None;
        for round in 1..=HORIZON as u8 {
            // Our Battering Ram and Siege Tower work from the turn after
            // they stand beside the city.
            for (k, bit) in [SUPPORT_RAM, SUPPORT_TOWER].into_iter().enumerate() {
                if posed.support[k] < round {
                    sim.s.city_support[posed.city] |= bit;
                }
            }
            if round == 1 {
                // This turn's moves are still ours; the heal came at its start.
                for unit in sim.s.units.iter_mut() {
                    if unit.alive() && unit.side == OURS {
                        unit.mp = unit.max_mp;
                        unit.attacks = unit.max_attacks;
                    }
                }
            } else {
                sim.begin_side(OURS);
            }
            sim.side_turn(OURS, Posture::Press);
            if sim.s.city_side[posed.city] == OURS {
                taken = Some(round);
                break;
            }
            for side in [OURS, THEIRS] {
                if side == THEIRS {
                    sim.begin_side(THEIRS);
                    sim.side_turn(THEIRS, Posture::Press);
                }
                for (i, at) in enter.iter().enumerate() {
                    // A unit the board had no room for tries again next turn.
                    if *at <= round && sim.s.units[i].side == side && !on_board[i] {
                        on_board[i] = place(&mut sim, posed, i);
                    }
                }
            }
            tally.health[round as usize - 1] +=
                f32::from(sim.s.city_hp[posed.city].max(0) + sim.s.city_wall[posed.city].max(0));
        }
        let ours = (0..posed.members).chain(extra.map(|(i, _)| i));
        tally.lost += ours
            .filter(|i| on_board[*i] && !sim.s.units[*i].alive())
            .count() as u32;
        if let Some(round) = taken {
            for count in tally.captured[(round as usize - 1)..].iter_mut() {
                *count += 1;
            }
            tally.turn_sum += u32::from(round);
            tally.taken += 1;
        }
        tally.runs += 1;
    }
    tally
}

/// The dice for a city's forecast this turn.
fn seed_for(center: Pos, turn: u32) -> u64 {
    mix(((center.0 as u32 as u64) << 32) ^ (center.1 as u32 as u64) ^ (u64::from(turn) << 52))
}

/// Whether `pid` can lay `kind` down in any city now.
fn buildable_now(g: &Game, pid: usize, kind: Name, spec: &UnitSpec) -> bool {
    spec.buildable
        && spec
            .tech
            .as_ref()
            .is_none_or(|tech| g.players[pid].techs.contains(tech))
        && g.player_city_ids(pid)
            .into_iter()
            .any(|cid| g.can_produce(pid, cid, &Item::Unit { unit: kind }))
}

/// The strongest kind `fits` takes, by `key`, among the kinds `pid` fields
/// or can build now.
fn best_kind(
    g: &Game,
    pid: usize,
    fits: fn(&UnitSpec) -> bool,
    key: fn(&UnitSpec) -> f64,
) -> Option<Name> {
    let fielded: BTreeSet<Name> = g
        .units
        .values()
        .filter(|unit| unit.owner == pid && fits(&g.rules.units[unit.kind]))
        .map(|unit| unit.kind)
        .collect();
    let mut kinds: Vec<(Name, f64)> = g
        .rules
        .units
        .iter()
        .filter(|(_, spec)| fits(spec))
        .map(|(kind, spec)| (*kind, key(spec)))
        .collect();
    kinds.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    kinds
        .into_iter()
        .find(|(kind, _)| {
            fielded.contains(kind) || buildable_now(g, pid, *kind, &g.rules.units[*kind])
        })
        .map(|(kind, _)| kind)
}

/// The force's most common kind, the stronger on a tie.
fn body_kind(g: &Game, members: &[u32]) -> Option<Name> {
    let mut counts: BTreeMap<Name, usize> = BTreeMap::new();
    for uid in members {
        *counts.entry(g.units[uid].kind).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| {
            a.1.cmp(&b.1)
                .then_with(|| {
                    g.rules.units[a.0]
                        .strength
                        .total_cmp(&g.rules.units[b.0].strength)
                })
                .then_with(|| b.0.cmp(&a.0))
        })
        .map(|(kind, _)| kind)
}

/// Rollouts per candidate in the corps search's ranking; the corps it
/// settles on is read with the full [`ROLLOUTS`].
const SEARCH_ROLLOUTS: u32 = 24;
/// The corps search reads a candidate set in full once its quick reading is
/// within this of the chance asked for.
const SEARCH_MARGIN: f32 = 0.15;

/// The support units a reading counts: a Battering Ram works on walls under
/// Medieval Walls, a Siege Tower under Renaissance Walls
/// (`Game::city_allows_siege_support`).
const SUPPORT_KINDS: [&str; 2] = ["battering_ram", "siege_tower"];

/// Hypothetical units a reading adds to the force: `count` of `kind`,
/// arriving `turns` from now at the edge of the board nearest our cities.
/// A Battering Ram or Siege Tower among them stands beside the city on the
/// turn its walk from that edge brings it there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Recruit {
    pub(crate) kind: Name,
    pub(crate) count: u32,
    pub(crate) turns: u32,
}

/// What one corps search found: the corps it ended with, and whether its
/// reading reached the chance asked for.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CorpsSearch {
    reached: bool,
    corps: Option<AssaultCorps>,
}

/// The smallest corps a pool offers against a city, and its reading.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AssaultCorps {
    /// Land soldiers and support units of the pool, in the order the search
    /// took them.
    pub(crate) members: Vec<u32>,
    pub(crate) forecast: AssaultForecast,
}

/// The board a reading plays: `g` itself at war, or a copy with the war
/// declared.
fn war_board(g: &Game, pid: usize, owner: usize) -> Option<Game> {
    if g.is_at_war(pid, owner) {
        return None;
    }
    let mut board = g.clone();
    board.at_war.insert((pid.min(owner), pid.max(owner)));
    Some(board)
}

/// Turns for a support unit standing at `pos` to stand beside the city:
/// its route to the board's edge, then the walk in.
fn support_turns(g: &Game, uid: u32, center: Pos) -> Option<u32> {
    let unit = g.units.get(&uid)?;
    let moves = g.unit_max_moves(uid).max(1.0);
    let walk_in = |tiles: i32| (f64::from(tiles.max(0)) / moves).ceil() as u32;
    let d = g.wdist(unit.pos, center);
    if d <= GROUND_RADIUS {
        return Some(walk_in(d - 1));
    }
    let routes = routes_in(g, uid, center, &[unit.pos], (d + 1).min(ROUTE_REACH));
    let (cost, _) = routes.get(&unit.pos)?;
    let to_edge = (f64::from(*cost) / (moves * f64::from(MP))).ceil() as u32;
    Some(to_edge + walk_in(GROUND_RADIUS - 1))
}

/// The round at whose end our Battering Ram, and our Siege Tower, stand
/// beside city `cid`, from the support units in `units` and `recruits`;
/// [`NEVER`] without one, or where the city's walls are beyond it.
fn support_rounds(
    g: &Game,
    pid: usize,
    cid: u32,
    center: Pos,
    units: &[u32],
    recruits: &[Recruit],
) -> [u8; 2] {
    let mut rounds = [NEVER; 2];
    for (k, kind) in SUPPORT_KINDS.iter().enumerate() {
        if !g.city_allows_siege_support(cid, kind) {
            continue;
        }
        let name = Name::new(kind);
        let fielded = units
            .iter()
            .filter(|uid| {
                g.units
                    .get(uid)
                    .is_some_and(|unit| unit.owner == pid && unit.kind == name)
            })
            .filter_map(|uid| support_turns(g, *uid, center));
        let moves = g
            .rules
            .units
            .get(kind)
            .map_or(2.0, |spec| spec.moves.max(1.0));
        let walk_in = (f64::from(GROUND_RADIUS - 1) / moves).ceil() as u32;
        let recruited = recruits
            .iter()
            .filter(|recruit| recruit.kind == name && recruit.count > 0)
            .map(|recruit| recruit.turns + walk_in);
        if let Some(turns) = fielded.chain(recruited).min() {
            if turns <= HORIZON as u32 {
                rounds[k] = turns as u8;
            }
        }
    }
    rounds
}

/// Spawn the fighting recruits on `board` at free tiles of the board's edge
/// nearest our cities; their entry rounds go in `arrive`.
fn spawn_recruits(
    board: &mut Game,
    pid: usize,
    center: Pos,
    recruits: &[Recruit],
    arrive: &mut BTreeMap<u32, u8>,
) -> Vec<u32> {
    let anchor = board
        .player_city_ids(pid)
        .into_iter()
        .map(|id| board.cities[&id].pos)
        .min_by_key(|pos| (board.wdist(*pos, center), *pos))
        .unwrap_or(center);
    let free = |g: &Game, pos: Pos| {
        g.map
            .get(pos)
            .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            && g.city_at(pos).is_none()
            && g.unit_ids_at(pos).is_empty()
    };
    let mut spots: Vec<Pos> = [GROUND_RADIUS, GROUND_RADIUS - 1]
        .into_iter()
        .flat_map(|r| board.wring(center, r))
        .filter(|pos| free(board, *pos))
        .collect();
    spots.sort_by_key(|pos| (board.wdist(*pos, anchor), *pos));
    spots.dedup();
    let mut spots = spots.into_iter();
    let mut spawned = Vec::new();
    for recruit in recruits {
        let Some(spec) = board.rules.units.get(recruit.kind.as_str()) else {
            continue;
        };
        let fights = spec.class == "military"
            && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            && (spec.is_melee_capable() || spec.has_ranged_attack());
        if !fights || recruit.turns > HORIZON as u32 {
            continue;
        }
        for _ in 0..recruit.count {
            let Some(spot) = spots.next() else {
                return spawned;
            };
            let uid = board.spawn_unit(recruit.kind.as_str(), pid, spot);
            arrive.insert(uid, recruit.turns as u8);
            spawned.push(uid);
        }
    }
    spawned
}

/// The curve alone.
#[allow(clippy::too_many_arguments)]
fn forecast_curve(
    g: &Game,
    pid: usize,
    cid: u32,
    members: &[u32],
    hostiles: &[u32],
    arrive: &BTreeMap<u32, u8>,
    support: [u8; 2],
    rollouts: u32,
) -> Option<AssaultForecast> {
    let mut posed = pose(g, pid, cid, members, hostiles, &[], arrive)?;
    posed.support = support;
    let seed = seed_for(g.cities[&cid].pos, g.turn);
    Some(play_out(&posed, None, seed, rollouts).forecast(&posed))
}

/// The curve and what the force is short of: the same rollouts again with
/// one hypothetical unit of each kind, spawned on a clone of the board.
fn forecast_short_of(
    g: &Game,
    pid: usize,
    cid: u32,
    members: &[u32],
    hostiles: &[u32],
    arrive: &BTreeMap<u32, u8>,
    support: [u8; 2],
) -> Option<AssaultForecast> {
    let center = g.cities.get(&cid)?.pos;
    let wanted = [
        (
            ShortOf::Taker,
            best_kind(g, pid, melee_taker, |spec| spec.strength),
        ),
        (
            ShortOf::Breaker,
            best_kind(g, pid, siege_gun, |spec| spec.bombard_strength),
        ),
        (
            ShortOf::Bodies,
            body_kind(g, members).or_else(|| best_kind(g, pid, melee_taker, |spec| spec.strength)),
        ),
    ];
    // The extras stand on free ground inside the board's edge, on the side
    // the force comes from.
    let anchor = if members.is_empty() {
        g.player_city_ids(pid)
            .into_iter()
            .map(|id| g.cities[&id].pos)
            .min_by_key(|pos| (g.wdist(*pos, center), *pos))
            .unwrap_or(center)
    } else {
        let (sx, sy) = members.iter().fold((0i64, 0i64), |(x, y), uid| {
            let pos = g.units[uid].pos;
            (x + i64::from(pos.0), y + i64::from(pos.1))
        });
        let count = members.len() as i64;
        let mean = ((sx / count) as i32, (sy / count) as i32);
        g.wring(center, GROUND_RADIUS)
            .into_iter()
            .min_by_key(|pos| (g.wdist(*pos, mean), *pos))
            .unwrap_or(center)
    };
    let mut spots: Vec<Pos> = g
        .wring(center, GROUND_RADIUS - 1)
        .into_iter()
        .filter(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                && g.city_at(*pos).is_none()
                && g.unit_ids_at(*pos).is_empty()
        })
        .collect();
    spots.sort_by_key(|pos| (g.wdist(*pos, anchor), *pos));
    let mut board = g.clone();
    let mut extras = Vec::new();
    let mut spots = spots.into_iter();
    for (what, kind) in wanted {
        let Some(kind) = kind else {
            continue;
        };
        let Some(spot) = spots.next() else {
            break;
        };
        let uid = board.spawn_unit(kind.as_str(), pid, spot);
        extras.push((what, uid));
    }
    let mut posed = pose(&board, pid, cid, members, hostiles, &extras, arrive)?;
    posed.support = support;
    let seed = seed_for(center, g.turn);
    let base = play_out(&posed, None, seed, ROLLOUTS);
    let mut forecast = base.forecast(&posed);
    // A hypothetical arrives with the force: on the median member's round.
    let mut rounds: Vec<u8> = posed.enter[..posed.members].to_vec();
    rounds.sort_unstable();
    let with_the_force = rounds.get(rounds.len() / 2).copied().unwrap_or(0);
    let mut short_of = (ShortOf::Nothing, 0.0f32);
    if base.p() < 1.0 {
        for (what, i) in &posed.extras {
            let gain = play_out(&posed, Some((*i, with_the_force)), seed, ROLLOUTS).p() - base.p();
            if gain >= SHORT_OF_GAIN && gain > short_of.1 {
                short_of = (*what, gain);
            }
        }
    }
    forecast.short_of = Some(short_of.0);
    Some(forecast)
}

impl AdvancedAi {
    /// Either gene reads the forecast.
    pub(super) fn assault_forecast_read(&self) -> bool {
        self.assault_forecast_is_read || self.siege_goes_on_the_forecast
    }

    /// P(capture of `city` within each of the next [`HORIZON`] turns by
    /// `force`, expected losses, and what one more unit would help most.
    /// `None` for a city of ours, one we are not at war with, or one gone.
    pub(crate) fn assault_forecast(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
        force: &[u32],
    ) -> Option<AssaultForecast> {
        self.forecast_with(g, pid, city, force, true)
    }

    /// [`Self::assault_forecast`] without the limiting factor: a quarter of
    /// the rollouts.
    pub(crate) fn assault_capture_curve(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
        force: &[u32],
    ) -> Option<AssaultForecast> {
        self.forecast_with(g, pid, city, force, false)
    }

    fn forecast_with(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
        force: &[u32],
        short_of: bool,
    ) -> Option<AssaultForecast> {
        self.read(g, pid, city, force, &[], short_of, false)
    }

    /// The reading `assault_forecast` gives, read as if the war with the
    /// city's owner (a major or a city-state) began this turn when it has
    /// not: the owner's units in sight are hostiles, and its borders open.
    /// At war, exactly `assault_forecast`. `None` for a city of ours or one
    /// gone.
    #[allow(
        dead_code,
        reason = "an entry point for the Score lane's target board; no caller in this tree yet"
    )]
    pub(crate) fn assault_forecast_at_peace(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
        force: &[u32],
    ) -> Option<AssaultForecast> {
        self.read(g, pid, city, force, &[], true, true)
    }

    /// [`Self::assault_forecast_at_peace`] with `recruits` added to `force`:
    /// "the force plus 4 Legions and a Battering Ram arriving in 8 turns".
    #[allow(
        dead_code,
        reason = "an entry point for the Score lane's target board; no caller in this tree yet"
    )]
    pub(crate) fn assault_forecast_recruiting(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
        force: &[u32],
        recruits: &[Recruit],
    ) -> Option<AssaultForecast> {
        self.read(g, pid, city, force, recruits, true, true)
    }

    /// Every reading: `assume_war` reads a city we are at peace with as if
    /// the war began this turn.
    #[allow(clippy::too_many_arguments)]
    fn read(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
        force: &[u32],
        recruits: &[Recruit],
        short_of: bool,
        assume_war: bool,
    ) -> Option<AssaultForecast> {
        let target = g.cities.get(&city)?;
        if target.owner == pid || (!assume_war && !g.is_at_war(pid, target.owner)) {
            return None;
        }
        let center = target.pos;
        let mut owned = war_board(g, pid, target.owner);
        let declared = owned.is_some();
        let hostiles = hostiles_of(owned.as_ref().unwrap_or(g), pid, center);
        let mut arrive = BTreeMap::new();
        let mut force: Vec<u32> = force.to_vec();
        if !recruits.is_empty() {
            let board = owned.get_or_insert_with(|| g.clone());
            force.extend(spawn_recruits(board, pid, center, recruits, &mut arrive));
        }
        let board = owned.as_ref().unwrap_or(g);
        let members = members_of(board, pid, center, &force);
        let support = support_rounds(board, pid, city, center, &force, recruits);
        // The at-war reading of a force with no support unit keeps the key
        // it always had.
        let variant = if !declared && recruits.is_empty() && support == [NEVER; 2] {
            0
        } else {
            fnv([
                u64::from(declared),
                u64::from(support[0]),
                u64::from(support[1]),
            ]
            .into_iter()
            .chain(recruits.iter().flat_map(|r| {
                [
                    u64::from(r.kind.id()),
                    u64::from(r.count),
                    u64::from(r.turns),
                ]
            }))
            .chain(
                arrive
                    .iter()
                    .map(|(uid, round)| (u64::from(*uid) << 8) | u64::from(*round)),
            ))
        };
        let key = (center, signature(board, pid, &members, &hostiles) ^ variant);
        {
            let mut frame = self.assault_forecast_frame.borrow_mut();
            if frame.turn != g.turn {
                frame.turn = g.turn;
                frame.entries.clear();
                frame.corps.clear();
                frame.computed = 0;
            }
            if let Some(hit) = frame.entries.get(&key) {
                if !short_of || hit.short_of.is_some() {
                    return Some(hit.clone());
                }
            }
        }
        let forecast = if short_of {
            forecast_short_of(board, pid, city, &members, &hostiles, &arrive, support)?
        } else {
            forecast_curve(
                board, pid, city, &members, &hostiles, &arrive, support, ROLLOUTS,
            )?
        };
        let mut frame = self.assault_forecast_frame.borrow_mut();
        frame.computed += 1;
        frame.entries.insert(key, forecast.clone());
        Some(forecast)
    }

    /// The smallest set of `pool` whose reading reaches `want_p` within
    /// `horizon` turns (1..=[`HORIZON`]), at war or as if the war began this
    /// turn; `None` when the whole pool falls short, or no city reads (one
    /// of ours, or gone). [`Self::assault_corps_best`] then gives the best
    /// the pool reached, from the same search. Greedy: each step takes the
    /// land soldier or support unit of the pool that raises the chance most
    /// (a quick reading of [`SEARCH_ROLLOUTS`]), the dearer then the nearer
    /// on a tie, up to [`MAX_MEMBERS`] soldiers; the first set whose full
    /// reading reaches `want_p` is the answer. One search a turn per city,
    /// pool, chance and horizon, cached like the readings.
    #[allow(
        dead_code,
        reason = "an entry point for the Score lane's target board; no caller in this tree yet"
    )]
    pub(crate) fn assault_corps_for(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
        pool: &[u32],
        want_p: f32,
        horizon: usize,
    ) -> Option<AssaultCorps> {
        let search = self.corps_search(g, pid, city, pool, want_p, horizon);
        search.corps.filter(|_| search.reached)
    }

    /// The corps [`Self::assault_corps_for`]'s search ended with: the corps
    /// that reaches `want_p`, or else the best reading the pool reached, for
    /// the caller to log. `None` only when no city reads.
    #[allow(
        dead_code,
        reason = "an entry point for the Score lane's target board; no caller in this tree yet"
    )]
    pub(crate) fn assault_corps_best(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
        pool: &[u32],
        want_p: f32,
        horizon: usize,
    ) -> Option<AssaultCorps> {
        self.corps_search(g, pid, city, pool, want_p, horizon).corps
    }

    fn corps_search(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
        pool: &[u32],
        want_p: f32,
        horizon: usize,
    ) -> CorpsSearch {
        let horizon = horizon.clamp(1, HORIZON);
        let Some(target) = g.cities.get(&city).filter(|city| city.owner != pid) else {
            return CorpsSearch {
                reached: false,
                corps: None,
            };
        };
        let center = target.pos;
        let owned = war_board(g, pid, target.owner);
        let board = owned.as_ref().unwrap_or(g);
        let hostiles = hostiles_of(board, pid, center);
        let fighters = members_of(board, pid, center, pool);
        let supports: Vec<u32> = SUPPORT_KINDS
            .iter()
            .filter_map(|kind| {
                let name = Name::new(kind);
                pool.iter()
                    .copied()
                    .filter(|uid| {
                        board.units.get(uid).is_some_and(|unit| {
                            unit.owner == pid
                                && unit.kind == name
                                && board.wdist(unit.pos, center) <= ROUTE_REACH
                        })
                    })
                    .min_by_key(|uid| (board.wdist(board.units[uid].pos, center), *uid))
            })
            .collect();
        let key = (
            center,
            signature(board, pid, &fighters, &hostiles)
                ^ fnv(supports
                    .iter()
                    .map(|uid| u64::from(*uid))
                    .chain([u64::from(want_p.to_bits()), horizon as u64])),
        );
        {
            let mut frame = self.assault_forecast_frame.borrow_mut();
            if frame.turn != g.turn {
                frame.turn = g.turn;
                frame.entries.clear();
                frame.corps.clear();
                frame.computed = 0;
            }
            if let Some(hit) = frame.corps.get(&key) {
                return hit.clone();
            }
        }
        let empty = BTreeMap::new();
        let seed = seed_for(center, g.turn);
        let reading = |set: &[u32], rollouts: u32| -> Option<AssaultForecast> {
            let soldiers: Vec<u32> = set
                .iter()
                .copied()
                .filter(|uid| fighters.contains(uid))
                .collect();
            let support = support_rounds(board, pid, city, center, set, &[]);
            let mut posed = pose(board, pid, city, &soldiers, &hostiles, &[], &empty)?;
            posed.support = support;
            Some(play_out(&posed, None, seed, rollouts).forecast(&posed))
        };
        let mut chosen: Vec<u32> = Vec::new();
        let mut left: Vec<u32> = fighters.iter().chain(&supports).copied().collect();
        let mut best: Option<AssaultCorps> = None;
        let mut found = None;
        while !left.is_empty()
            && chosen.iter().filter(|uid| fighters.contains(uid)).count() < MAX_MEMBERS
        {
            let mut pick: Option<(f32, f64, i32, u32, usize)> = None;
            for (k, uid) in left.iter().enumerate() {
                let mut set = chosen.clone();
                set.push(*uid);
                let p = reading(&set, SEARCH_ROLLOUTS).map_or(0.0, |f| f.within(horizon));
                let unit = &board.units[uid];
                let cost = board.rules.units[unit.kind].cost;
                let near = -board.wdist(unit.pos, center);
                let better = pick.is_none_or(|(bp, bc, bn, bu, _)| {
                    (p, cost, near, std::cmp::Reverse(*uid))
                        .partial_cmp(&(bp, bc, bn, std::cmp::Reverse(bu)))
                        .is_some_and(|o| o == std::cmp::Ordering::Greater)
                });
                if better {
                    pick = Some((p, cost, near, *uid, k));
                }
            }
            let Some((quick, _, _, _, k)) = pick else {
                break;
            };
            chosen.push(left.remove(k));
            let last = left.is_empty()
                || chosen.iter().filter(|uid| fighters.contains(uid)).count() >= MAX_MEMBERS;
            if quick + SEARCH_MARGIN < want_p && !last {
                continue;
            }
            let Some(full) = reading(&chosen, ROLLOUTS) else {
                break;
            };
            let corps = AssaultCorps {
                members: chosen.clone(),
                forecast: full,
            };
            if corps.forecast.within(horizon) >= want_p {
                found = Some(corps);
                break;
            }
            if best
                .as_ref()
                .is_none_or(|b| corps.forecast.within(horizon) > b.forecast.within(horizon))
            {
                best = Some(corps);
            }
        }
        let answer = CorpsSearch {
            reached: found.is_some(),
            corps: found.or(best),
        };
        let mut frame = self.assault_forecast_frame.borrow_mut();
        frame.computed += 1;
        frame.corps.insert(key, answer.clone());
        answer
    }

    /// Forecasts computed this turn, not served from the cache.
    #[cfg(test)]
    pub(crate) fn assault_forecasts_computed(&self, g: &Game) -> u32 {
        let frame = self.assault_forecast_frame.borrow();
        if frame.turn == g.turn {
            frame.computed
        } else {
            0
        }
    }

    /// The reading's journal line for one assessed siege.
    pub(super) fn journal_assault_forecast(&self, g: &Game, cid: u32, forecast: &AssaultForecast) {
        let Some(city) = g.cities.get(&cid) else {
            return;
        };
        let curve: Vec<String> = forecast
            .by_turn
            .iter()
            .map(|p| format!("{:.0}", p * 100.0))
            .collect();
        let computed = self.assault_forecast_frame.borrow().computed;
        think!(self.journal(), Military, Decision,
            "Assault forecast for {}: {:.0}% by turn +{}, {:.1} units lost, short of: {}",
            city.name, forecast.p() * 100.0, forecast.turn_by(), forecast.units_lost,
            forecast.short_of.map_or("unread", ShortOf::as_str);
            "P% by turn +1..+{HORIZON}: {}; {} members on the board, {} walking in ({} within {GATHERED_TURNS} turns), \
             {} hostiles; walls {}/{}, city {}/200; {} rollouts, {computed} forecast(s) computed this turn{}",
            curve.join("/"), forecast.present, forecast.arriving, forecast.gathered,
            forecast.hostiles, city.wall_hp, g.city_max_wall_hp(city), city.hp, forecast.rollouts,
            forecast.mean_turn.map_or_else(String::new, |turn| format!(", captures at turn {turn:.1} on average"));
            city.pos);
    }

    /// `siege-goes-on-the-forecast`: whether the city at `pos` stands down
    /// on a hopeless forecast. False with the gene off.
    pub(super) fn forecast_stood_down_at(&self, g: &Game, pos: Pos) -> bool {
        self.siege_goes_on_the_forecast
            && self
                .siege_forecast_hopeless
                .get(&pos)
                .is_some_and(|run| g.turn < run.down_until)
    }

    /// `siege-goes-on-the-forecast`: one assessment's reading of the
    /// hopeless streak. True on the assessment that stands the siege down.
    /// A reading is hopeless when the force has gathered, P(capture within
    /// [`HORIZON`]) is under [`HOPELESS_P`], and no one more taker, breaker
    /// or body would lift it by [`SHORT_OF_GAIN`]: a siege short of one
    /// unit is waiting for it, not hopeless. Anything else, or a falling
    /// city, resets the streak.
    ///
    /// The last clause is the evidence's: as first specified (any gathered
    /// reading under [`HOPELESS_P`]), the stand-down cost cities on both
    /// posed sets (`docs/ASSAULT_FORECAST.md`: 132 to 119 with one enemy
    /// city, every lost one taken after the twelfth turn; 132 to 123 with
    /// two), and with the clause it read 134 against 132.
    pub(super) fn note_forecast_hopeless(
        &mut self,
        g: &Game,
        cid: u32,
        forecast: &AssaultForecast,
    ) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        let (pos, name) = (city.pos, city.name.clone());
        let falling = city.wall_hp <= 0 && city.hp <= FALLING_CITY_HP;
        let turn = g.turn;
        self.siege_forecast_hopeless
            .retain(|_, run| run.last + HOPELESS_STAND_DOWN_TURNS >= turn || turn < run.down_until);
        let hopeless = !falling
            && forecast.gathered()
            && forecast.p() < HOPELESS_P
            && (!hopeless_needs_nothing_short() || forecast.short_of == Some(ShortOf::Nothing));
        let run = self.siege_forecast_hopeless.entry(pos).or_default();
        if turn < run.down_until {
            return false;
        }
        if !hopeless {
            run.streak = 0;
            run.last = turn;
            return false;
        }
        if run.last != turn {
            run.streak += 1;
            run.last = turn;
        }
        if run.streak < hopeless_streak() {
            return false;
        }
        run.down_until = turn + HOPELESS_STAND_DOWN_TURNS;
        run.streak = 0;
        think!(self.journal(), Military, Decision,
            "Siege of {} stands down: the forecast reads it hopeless", name;
            "siege-goes-on-the-forecast: {} gathered assessments in a row under {:.0}% within {HORIZON} turns \
             that no one more taker, breaker or body lifts (now {:.0}%, {} members); down for {HOPELESS_STAND_DOWN_TURNS} turns",
            hopeless_streak(), HOPELESS_P * 100.0, forecast.p() * 100.0, forecast.members();
            pos);
        true
    }

    /// `siege-goes-on-the-forecast`: a Siege row's rank weight, captures per
    /// turn, read with the board's pool: its land soldiers nearest the city.
    /// `None` where no forecast reads (a city we are not at war with).
    pub(super) fn forecast_siege_rate(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        pool: &[u32],
    ) -> Option<f64> {
        self.assault_capture_curve(g, pid, cid, pool)
            .map(|forecast| forecast.rate())
    }
}

#[cfg(test)]
mod calibration;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod world_tests;
