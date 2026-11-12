//! Rival chairs that fight like the live host: a harness switch for a
//! simulated opponent's tactics, and the census that measures how any
//! opponent fights.
//!
//! Every tactical gene is judged in paired simulations whose rival chairs are
//! this codebase's own AI, and that AI does not fight like the host. Over 116
//! live Deity games (`docs/HOST_ENEMY_MODEL.md`) the host struck one of ours
//! on a quarter of the turns it had one in reach (43% when it need not move,
//! 11% when it must), took the unit in front of it, and did not retreat when
//! wounded. A gene priced against chairs that strike far more often is priced
//! against an opponent nobody meets live.
//!
//! **The census** ([`TacticsCensus`]) reads a chair's turns with the same
//! definitions as `tools/fit_host_enemy.py`: a row is one of the chair's land
//! fighters at war with a measured seat and on a tile it sees at the start of
//! the chair's turn; reach is open ground; strikes are read from the game's action
//! log; "closer" compares the end of the chair's turn with where the measured
//! seat's units stood. It observes only.
//!
//! **The switch** ([`HostTactics::gate`]) governs a chair's land fighters
//! within [`MAX_GAP`] tiles of a measured seat's unit, at the start of the
//! chair's turn, with a draw per unit per turn keyed by the game seed:
//!
//! - **Strike.** With a measured seat's unit in reach (exact reach: the
//!   engine's own movement flood and line of sight), the unit strikes with
//!   `host_enemy::strike_probability`. The harness plays that strike before
//!   the chair's AI moves: the target is drawn on the fitted weights
//!   (`host_enemy::target_utility`) among the units it can reach, from its
//!   own tile when it can. A unit that draws no strike may not strike the
//!   measured seat's units this turn.
//! - **Close or hold.** A unit that struck nothing holds its ground (moves
//!   refused) with `1 - host_enemy::advance_probability`; otherwise it moves
//!   as the chair's own logic says, except that a unit at
//!   [`WOUNDED_HP`] or less may not end a move farther from the measured
//!   seat, beyond the host's own stepping-away rate ([`LIVE_AWAY`]).
//! - **Untouched.** Strikes on cities and districts, city strikes, units
//!   beyond [`MAX_GAP`], every other player, and the measured seat itself.
//!
//! The refusals are made by the engine (`Game::host_gate`, read in
//! `Game::apply`) only while a governed chair plays its turn; with no seat
//! wrapped the field is never set and every game is byte for byte what it
//! was. A unit refused [`VALVE`] times in one turn has its moves and attacks
//! spent, so an AI loop that retries a refused order still ends.

use std::collections::{BTreeMap, BTreeSet};

use crate::ai::host_enemy::{
    advance_probability, strike_probability, target_utility, HostClass, Owner, Situation,
    StrikeFeatures, TargetFeatures,
};
use crate::ai::{Ai, ExpansionCensus, PlanReport, SiteShortcuts};
use crate::game::{Action, Game, Unit};
use crate::reasoning::Journal;
use crate::Pos;

/// A chair's unit this far or farther from every unit of the measured seat is
/// outside the fight: the fit's gap columns stop here.
pub const MAX_GAP: i32 = 8;
/// A unit at this many hit points or fewer is wounded for the no-retreat rule.
pub const WOUNDED_HP: i32 = 50;
/// The live host's rate of ending a turn farther from us after striking
/// nothing: at 25 hp or less, and above. A wounded governed unit is left free
/// to step back on this share of its turns.
pub const LIVE_AWAY: [f64; 2] = [0.213, 0.121];
/// Refusals of one unit in one turn before its moves and attacks are spent.
pub const VALVE: u32 = 12;
/// A siege gun's bombard strength against a land unit (`Game::ranged_pair_bases`).
const SIEGE_VS_UNITS: f64 = 17.0;

/// The live host's behaviour table, `tools/fit_host_enemy.py rates` on the
/// 116 games of `docs/HOST_ENEMY_MODEL.md`: `(row, hits, rows)`.
pub const LIVE: &[(&str, f64, u32)] = &[
    ("strike | in reach", 4415.0, 17669),
    ("strike | in reach without moving", 3324.0, 7738),
    ("strike | must move first", 1091.0, 9931),
    ("target: nearest", 2209.0, 2512),
    ("target: nearest (chance)", 1220.55, 2512),
    ("target: weakest", 1349.0, 2512),
    ("target: weakest (chance)", 1265.19, 2512),
    ("target: most damaged", 1263.0, 2512),
    ("target: most damaged (chance)", 1216.92, 2512),
    ("closes | struck nothing", 5769.0, 17563),
    ("closes | declined", 3253.0, 9349),
    ("closes | nothing in reach", 2516.0, 8214),
    ("steps away | hp<=25, struck nothing", 279.0, 1309),
    ("holds | hp<=25, struck nothing", 530.0, 1309),
    ("steps away | hp>25, struck nothing", 1962.0, 16254),
];

/// The switch for one chair: whom it fights and whether it is governed.
#[derive(Clone, Debug)]
pub struct HostTactics {
    /// The game's seed: every draw is a hash of it, the turn and the unit.
    pub seed: u64,
    /// The measured seats.
    pub victims: Vec<usize>,
    /// Govern the chair's tactics. False reads the census only.
    pub gate: bool,
}

/// One count: `hits` of `rows`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tally {
    pub hits: f64,
    pub rows: u32,
}

impl Tally {
    pub fn rate(&self) -> f64 {
        self.hits / self.rows.max(1) as f64
    }
}

/// What the switch did.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GateCounts {
    /// Governed unit-turns.
    pub units: u32,
    pub strikes_drawn: u32,
    pub strikes_landed: u32,
    /// Drawn strikes the engine refused (a stand blocked, a shot unseen).
    pub strikes_failed: u32,
    pub held: u32,
    pub no_retreat: u32,
    /// Orders the gate refused, and units whose moves it spent at [`VALVE`].
    pub refusals: u32,
    pub valves: u32,
}

/// A chair's turns read as `tools/fit_host_enemy.py` reads the host's.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TacticsCensus {
    pub rows: BTreeMap<&'static str, Tally>,
    pub gate: GateCounts,
}

impl TacticsCensus {
    fn add(&mut self, key: &'static str, hit: bool) {
        self.add_weight(key, if hit { 1.0 } else { 0.0 });
    }

    fn add_weight(&mut self, key: &'static str, hits: f64) {
        let tally = self.rows.entry(key).or_default();
        tally.hits += hits;
        tally.rows += 1;
    }

    /// Fold another census in.
    pub fn absorb(&mut self, other: &TacticsCensus) {
        for (key, tally) in &other.rows {
            let mine = self.rows.entry(key).or_default();
            mine.hits += tally.hits;
            mine.rows += tally.rows;
        }
        let (a, b) = (&mut self.gate, &other.gate);
        a.units += b.units;
        a.strikes_drawn += b.strikes_drawn;
        a.strikes_landed += b.strikes_landed;
        a.strikes_failed += b.strikes_failed;
        a.held += b.held;
        a.no_retreat += b.no_retreat;
        a.refusals += b.refusals;
        a.valves += b.valves;
    }

    /// The rows as `{row: [hits, rows]}`, for a report.
    pub fn map(&self) -> BTreeMap<String, [f64; 2]> {
        self.rows
            .iter()
            .map(|(key, tally)| (key.to_string(), [tally.hits, tally.rows as f64]))
            .collect()
    }
}

/// The table beside the live host's: one line per [`LIVE`] row, then a column
/// per census.
pub fn compare(columns: &[(&str, &TacticsCensus)]) -> String {
    let mut out = format!("{:<38} {:>16}", "row", "live host");
    for (name, _) in columns {
        out.push_str(&format!(" {name:>22}"));
    }
    out.push('\n');
    for (key, hits, rows) in LIVE {
        out.push_str(&format!(
            "{key:<38} {:>6.3} ({:>6})",
            hits / *rows as f64,
            rows
        ));
        for (_, census) in columns {
            let tally = census.rows.get(key).copied().unwrap_or_default();
            out.push_str(&format!("       {:>6.3} ({:>6})", tally.rate(), tally.rows));
        }
        out.push('\n');
    }
    out
}

/// The engine's half of the switch: what a governed chair may not do this
/// turn. Set on `Game::host_gate` by [`HostTacticsSeat`] for the chair's turn
/// only, and read by `Game::apply` through [`gate_refuses`].
#[derive(Clone, Debug, Default)]
pub struct Gate {
    player: usize,
    victims: Vec<usize>,
    /// Units that may not strike a measured seat's unit this turn.
    no_unit_strike: BTreeSet<u32>,
    /// Units that hold their ground this turn.
    hold: BTreeSet<u32>,
    /// Wounded units that may not end a move farther from the measured seat
    /// than the gap they started the turn at.
    no_retreat: BTreeMap<u32, i32>,
    /// Where the measured seat's units stood at the start of the turn.
    victim_plots: Vec<Pos>,
    refused: BTreeMap<u32, u32>,
    refusals: u32,
    valves: u32,
}

/// Whether the gate refuses `action`; called by `Game::apply` only while a
/// gate is set. Counts the refusal and spends a unit's moves at [`VALVE`].
pub(crate) fn gate_refuses(g: &mut Game, pid: usize, action: &Action) -> Option<&'static str> {
    let refused = {
        let gate = g.host_gate.as_deref()?;
        if pid != gate.player {
            return None;
        }
        match action {
            Action::Attack { unit, target } | Action::Ranged { unit, target } => {
                let on_a_unit = gate.no_unit_strike.contains(unit)
                    && g.city_at(*target).is_none()
                    && g.unit_ids_at(*target).iter().any(|id| {
                        g.units
                            .get(id)
                            .is_some_and(|u| gate.victims.contains(&u.owner))
                    });
                on_a_unit.then_some((*unit, "host tactics: this unit holds its blow this turn"))
            }
            Action::Move { unit, to } | Action::MoveTo { unit, to } => {
                if gate.hold.contains(unit) {
                    Some((*unit, "host tactics: this unit holds its ground this turn"))
                } else if let Some(&start) = gate.no_retreat.get(unit) {
                    (gap(g, *to, &gate.victim_plots) > start)
                        .then_some((*unit, "host tactics: a wounded unit does not fall back"))
                } else {
                    None
                }
            }
            Action::Swap { unit, .. } => gate
                .hold
                .contains(unit)
                .then_some((*unit, "host tactics: this unit holds its ground this turn")),
            _ => None,
        }
    };
    let (uid, reason) = refused?;
    let spend = {
        let gate = g.host_gate.as_deref_mut()?;
        gate.refusals += 1;
        let count = gate.refused.entry(uid).or_insert(0);
        *count += 1;
        if *count == VALVE {
            gate.valves += 1;
        }
        *count == VALVE
    };
    if spend {
        if let Some(unit) = g.units.get_mut(&uid) {
            unit.moves_left = 0.0;
            unit.attacks_left = 0;
        }
    }
    Some(reason)
}

fn gap(g: &Game, from: Pos, plots: &[Pos]) -> i32 {
    plots
        .iter()
        .map(|p| g.wdist(from, *p))
        .min()
        .unwrap_or(i32::MAX)
}

/// A draw on `[0, 1)` for one game, turn, unit and purpose. A hash, so it
/// never touches the game's own random stream.
fn draw(seed: u64, turn: u32, uid: u32, salt: u64) -> f64 {
    let mut z = seed
        ^ (turn as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (uid as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ salt.wrapping_mul(0x1656_67B1_9E37_79F9);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

/// `dice::effective_strength`: one point of strength per ten hit points lost.
fn effective(base: f64, hp: i32) -> f64 {
    (base - (10.0 - hp.clamp(0, 100) as f64 / 10.0).round()).max(0.0)
}

/// `dice::mean_damage`.
fn mean_damage(att: f64, def: f64) -> f64 {
    (30.0 * ((att - def) / 25.0).exp()).clamp(1.0, 100.0)
}

/// A land unit that can strike, on land.
fn land_fighter(g: &Game, unit: &Unit) -> bool {
    let spec = &g.rules.units[unit.kind];
    spec.class == "military"
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        && !g.is_embarked(unit)
        && (spec.is_melee_capable() || spec.has_ranged_attack())
}

fn host_class(g: &Game, unit: &Unit) -> HostClass {
    let spec = &g.rules.units[unit.kind];
    if spec.siege {
        HostClass::Siege
    } else if spec.has_ranged_attack() {
        HostClass::Ranged
    } else if matches!(
        spec.promotion_class.as_str(),
        "light_cavalry" | "heavy_cavalry"
    ) {
        HostClass::Cavalry
    } else {
        HostClass::Melee
    }
}

/// The base strength a unit strikes a land unit with, before wounds, as the
/// fit reads it: melee strength, ranged strength, or a siege gun's bombard
/// less 17.
fn attack_base(g: &Game, unit: &Unit) -> f64 {
    let spec = &g.rules.units[unit.kind];
    if spec.siege {
        (spec.bombard_strength.max(spec.ranged_strength) - SIEGE_VS_UNITS).max(0.0)
    } else if spec.has_ranged_attack() {
        spec.ranged_strength
    } else {
        spec.strength
    }
}

/// The blow's edge and expected damage, base strengths and wounds only.
fn blow(g: &Game, attacker: &Unit, defender: &Unit) -> (f64, f64) {
    let att = effective(attack_base(g, attacker), attacker.hp);
    let def = effective(g.rules.units[defender.kind].strength, defender.hp);
    (att - def, mean_damage(att, def))
}

/// A measured seat's unit a blow can land on: a land fighter not standing in
/// its own city (the blow would land on the city).
fn struck_unit_of(g: &Game, unit: &Unit) -> bool {
    land_fighter(g, unit)
        && !g
            .city_at(unit.pos)
            .is_some_and(|cid| g.cities.get(&cid).is_some_and(|c| c.owner == unit.owner))
}

fn shooter(g: &Game, unit: &Unit) -> bool {
    let spec = &g.rules.units[unit.kind];
    spec.siege || spec.has_ranged_attack()
}

/// One unit of the measured seat a governed unit can strike this turn.
#[derive(Clone, Debug)]
struct Target {
    pos: Pos,
    stand: Pos,
    edge: f64,
    features: TargetFeatures,
}

/// Exact reach: from its own tile, or a tile of the engine's movement flood
/// with movement left to strike, free of other units, in line of sight.
fn exact_targets(g: &Game, uid: u32, victims: &[u32]) -> Vec<Target> {
    let unit = &g.units[&uid];
    let shoots = shooter(g, unit);
    let range = if shoots { g.unit_attack_range(uid) } else { 1 };
    let fires_after_moving =
        !g.rules.units[unit.kind].siege || g.siege_may_attack_after_moving(unit);
    let arrivals: Vec<(Pos, f64)> = if unit.moves_left > 0.0 {
        g.approach_arrivals(uid)
            .into_iter()
            .filter(|(pos, kept)| *kept > 0.0 && g.unit_ids_at(*pos).is_empty())
            .collect()
    } else {
        Vec::new()
    };
    let mut out = Vec::new();
    for vid in victims {
        let Some(other) = g.units.get(vid) else {
            continue;
        };
        let distance = g.wdist(unit.pos, other.pos);
        if distance > MAX_GAP {
            continue;
        }
        let stand = if shoots {
            if distance <= range && g.unit_has_line_of_sight(uid, other.pos) {
                Some(unit.pos)
            } else if fires_after_moving {
                arrivals
                    .iter()
                    .filter(|(pos, _)| {
                        g.wdist(*pos, other.pos) <= range && g.battle_line_of_sight(*pos, other.pos)
                    })
                    .min_by_key(|(pos, _)| (g.wdist(*pos, unit.pos), *pos))
                    .map(|(pos, _)| *pos)
            } else {
                None
            }
        } else if distance == 1 {
            Some(unit.pos)
        } else {
            arrivals
                .iter()
                .filter(|(pos, _)| g.wdist(*pos, other.pos) == 1 && g.city_at(*pos).is_none())
                .max_by(|a, b| {
                    g.tile_defense_bonus(a.0)
                        .total_cmp(&g.tile_defense_bonus(b.0))
                        .then(b.0.cmp(&a.0))
                })
                .map(|(pos, _)| *pos)
        };
        let Some(stand) = stand else {
            continue;
        };
        let (edge, dealt) = blow(g, unit, other);
        out.push(Target {
            pos: other.pos,
            stand,
            edge,
            features: TargetFeatures {
                hp: other.hp as i16,
                dealt: dealt as f32,
                shooter: shooter(g, other),
                distance: distance.clamp(0, 255) as u8,
                still: stand == unit.pos,
            },
        });
    }
    out
}

/// Play one drawn strike: walk to the stand if it must, then strike.
fn strike(g: &mut Game, pid: usize, uid: u32, target: &Target, shoots: bool) -> bool {
    if g.units.get(&uid).map(|u| u.pos) != Some(target.stand) {
        let moved = g
            .apply(
                pid,
                &Action::MoveTo {
                    unit: uid,
                    to: target.stand,
                },
            )
            .is_ok();
        if !moved || g.units.get(&uid).map(|u| u.pos) != Some(target.stand) {
            return false;
        }
    }
    let action = if shoots {
        Action::Ranged {
            unit: uid,
            target: target.pos,
        }
    } else {
        Action::Attack {
            unit: uid,
            target: target.pos,
        }
    };
    g.apply(pid, &action).is_ok()
}

/// The measured seats `pid` is at war with.
fn foes(g: &Game, pid: usize, victims: &[usize]) -> Vec<usize> {
    victims
        .iter()
        .copied()
        .filter(|&v| v != pid && g.is_at_war(pid, v))
        .collect()
}

/// Set the gate for `pid`'s turn and play the strikes it draws.
fn govern(g: &mut Game, pid: usize, host: &HostTactics, counts: &mut GateCounts) {
    let foes = foes(g, pid, &host.victims);
    if foes.is_empty() {
        return;
    }
    let victim_plots: Vec<Pos> = g
        .units
        .values()
        .filter(|u| foes.contains(&u.owner))
        .map(|u| u.pos)
        .collect();
    let mut gate = Gate {
        player: pid,
        victims: foes.clone(),
        victim_plots: victim_plots.clone(),
        ..Gate::default()
    };
    let turn = g.turn;
    let mut ours: Vec<u32> = g
        .player_unit_ids(pid)
        .into_iter()
        .filter(|uid| land_fighter(g, &g.units[uid]))
        .collect();
    ours.sort_by_key(|uid| (!shooter(g, &g.units[uid]), *uid));
    for uid in ours {
        let Some(unit) = g.units.get(&uid).cloned() else {
            continue;
        };
        let start = gap(g, unit.pos, &victim_plots);
        if start > MAX_GAP {
            continue;
        }
        counts.units += 1;
        let victims: Vec<u32> = g
            .units
            .values()
            .filter(|u| foes.contains(&u.owner) && struck_unit_of(g, u))
            .map(|u| u.id)
            .collect();
        let targets = exact_targets(g, uid, &victims);
        let situation = if targets.is_empty() {
            Situation::Gap(start.clamp(0, 255) as u8)
        } else {
            let home = g
                .cities
                .values()
                .any(|c| c.owner == pid && g.wdist(c.pos, unit.pos) <= 1);
            let features = StrikeFeatures {
                class: host_class(g, &unit),
                owner: Owner::Major,
                hp: unit.hp as i16,
                home,
                edge: targets.iter().map(|t| t.edge).fold(f64::MIN, f64::max) as f32,
                targets: targets.len(),
                still: targets.iter().any(|t| t.features.still),
            };
            gate.no_unit_strike.insert(uid);
            if draw(host.seed, turn, uid, 1) < strike_probability(&features) as f64 {
                counts.strikes_drawn += 1;
                let utility: Vec<f64> = targets
                    .iter()
                    .map(|t| target_utility(&t.features) as f64)
                    .collect();
                let top = utility.iter().copied().fold(f64::MIN, f64::max);
                let total: f64 = utility.iter().map(|u| (u - top).exp()).sum();
                let mut left = draw(host.seed, turn, uid, 2) * total;
                let mut pick = targets.len() - 1;
                for (k, u) in utility.iter().enumerate() {
                    left -= (u - top).exp();
                    if left < 0.0 {
                        pick = k;
                        break;
                    }
                }
                if strike(g, pid, uid, &targets[pick], shooter(g, &unit)) {
                    counts.strikes_landed += 1;
                } else {
                    counts.strikes_failed += 1;
                }
                continue;
            }
            Situation::Declined
        };
        if draw(host.seed, turn, uid, 3) >= advance_probability(unit.hp as i16, situation) as f64 {
            gate.hold.insert(uid);
            counts.held += 1;
        } else if unit.hp <= WOUNDED_HP {
            let free = LIVE_AWAY[usize::from(unit.hp > 25)];
            if draw(host.seed, turn, uid, 4) >= free {
                gate.no_retreat.insert(uid, start);
                counts.no_retreat += 1;
            }
        }
    }
    g.host_gate = Some(Box::new(gate));
}

/// One of the chair's units as the census reads it at the start of its turn.
struct Watched {
    uid: u32,
    start: Pos,
    hp: i32,
    gap: Option<i32>,
    /// `(victim uid, distance, hp, expected damage, still)` per unit in
    /// open-ground reach.
    targets: Vec<(u32, i32, i32, f64, bool)>,
}

/// The census's view of a chair's turn: taken at the start, closed at the end.
struct Watch {
    rows: Vec<Watched>,
    /// The measured seats' units at the start: tile and id.
    victims_at: BTreeMap<Pos, u32>,
    victim_cities: BTreeSet<Pos>,
    victim_plots: Vec<Pos>,
    log_start: usize,
}

/// Open-ground reach, as `tools/fit_host_enemy.py::reaches` has it.
fn open_ground(g: &Game, unit: &Unit, distance: i32) -> (bool, bool) {
    let spec = &g.rules.units[unit.kind];
    let moves = g.unit_max_moves(unit.id).floor().max(1.0) as i32;
    if !shooter(g, unit) {
        return (distance <= moves, distance == 1);
    }
    let range = g.unit_attack_range(unit.id).max(1);
    let still = distance <= range;
    if spec.siege && !g.siege_may_attack_after_moving(unit) {
        return (still, still);
    }
    (distance <= range + (moves - 1).max(0), still)
}

impl Watch {
    fn begin(g: &Game, pid: usize, host: &HostTactics) -> Watch {
        let foes = foes(g, pid, &host.victims);
        // The unit a blow on a tile lands on: its fighter, else whoever is there.
        let mut victims_at: BTreeMap<Pos, u32> = BTreeMap::new();
        for unit in g.units.values().filter(|u| foes.contains(&u.owner)) {
            let fighter = struck_unit_of(g, unit);
            let held = victims_at.get(&unit.pos).is_some_and(|id| {
                g.units
                    .get(id)
                    .is_some_and(|other| struck_unit_of(g, other))
            });
            if fighter || !held {
                victims_at.insert(unit.pos, unit.id);
            }
        }
        let victim_cities: BTreeSet<Pos> = g
            .cities
            .values()
            .filter(|c| foes.contains(&c.owner))
            .map(|c| c.pos)
            .collect();
        let victim_plots: Vec<Pos> = victims_at.keys().copied().collect();
        let struck: Vec<&Unit> = g
            .units
            .values()
            .filter(|u| foes.contains(&u.owner) && struck_unit_of(g, u))
            .collect();
        // The fit saw only what the seat could see.
        let seen: BTreeSet<Pos> = foes.iter().flat_map(|&v| g.player_visibility(v)).collect();
        let mut rows = Vec::new();
        for uid in g.player_unit_ids(pid) {
            let unit = &g.units[&uid];
            if !land_fighter(g, unit)
                || !seen.contains(&unit.pos)
                || !foes.iter().any(|&v| g.unit_visible_to(uid, v))
            {
                continue;
            }
            let mut targets = Vec::new();
            for other in &struck {
                let distance = g.wdist(unit.pos, other.pos);
                let (reach, still) = open_ground(g, unit, distance);
                if reach {
                    let (_, dealt) = blow(g, unit, other);
                    targets.push((other.id, distance, other.hp, dealt, still));
                }
            }
            let gap = (!victim_plots.is_empty()).then(|| gap(g, unit.pos, &victim_plots));
            rows.push(Watched {
                uid,
                start: unit.pos,
                hp: unit.hp,
                gap,
                targets,
            });
        }
        Watch {
            rows,
            victims_at,
            victim_cities,
            victim_plots,
            log_start: g.log.len(),
        }
    }

    /// What each watched unit struck this turn, from the action log: a
    /// measured seat's unit (its id), a city of theirs, or someone else.
    fn finish(self, g: &Game, pid: usize, census: &mut TacticsCensus) {
        let mut struck: BTreeMap<u32, Struck> = BTreeMap::new();
        for (seat, action) in g.log.since(self.log_start) {
            if *seat != pid {
                continue;
            }
            let (Action::Attack { unit, target } | Action::Ranged { unit, target }) = action else {
                continue;
            };
            let what = if self.victim_cities.contains(target) {
                Struck::City
            } else if let Some(vid) = self.victims_at.get(target) {
                Struck::Unit(*vid)
            } else {
                Struck::Other
            };
            let entry = struck.entry(*unit).or_insert(what);
            if !matches!(entry, Struck::Unit(_)) && matches!(what, Struck::Unit(_)) {
                *entry = what;
            }
        }
        for row in self.rows {
            let hit = struck.get(&row.uid).copied();
            let on_ours = match hit {
                Some(Struck::Unit(vid)) => Some(vid),
                _ => None,
            };
            if !row.targets.is_empty() {
                let still = row.targets.iter().any(|t| t.4);
                census.add("strike | in reach", on_ours.is_some());
                census.add(
                    if still {
                        "strike | in reach without moving"
                    } else {
                        "strike | must move first"
                    },
                    on_ours.is_some(),
                );
                if let Some(vid) = on_ours {
                    if row.targets.len() >= 2 {
                        if let Some(chosen) = row.targets.iter().find(|t| t.0 == vid) {
                            let n = row.targets.len() as f64;
                            let near = row.targets.iter().map(|t| t.1).min().unwrap_or(0);
                            let weak = row.targets.iter().map(|t| t.2).min().unwrap_or(0);
                            let hurt = row.targets.iter().map(|t| t.3).fold(f64::MIN, f64::max);
                            let tied = |pred: &dyn Fn(&(u32, i32, i32, f64, bool)) -> bool| -> f64 {
                                row.targets.iter().filter(|t| pred(t)).count() as f64 / n
                            };
                            census.add("target: nearest", chosen.1 == near);
                            census.add_weight("target: nearest (chance)", tied(&|t| t.1 == near));
                            census.add("target: weakest", chosen.2 == weak);
                            census.add_weight("target: weakest (chance)", tied(&|t| t.2 == weak));
                            census.add("target: most damaged", chosen.3 == hurt);
                            census.add_weight(
                                "target: most damaged (chance)",
                                tied(&|t| t.3 == hurt),
                            );
                        }
                    }
                }
            }
            let Some(gap_then) = row.gap else {
                continue;
            };
            if hit.is_some() || (row.targets.is_empty() && gap_then > MAX_GAP) {
                continue;
            }
            let Some(now) = g.units.get(&row.uid).map(|u| u.pos) else {
                continue;
            };
            let gap_now = gap(g, now, &self.victim_plots);
            let closer = now != row.start && gap_now < gap_then;
            let away = now != row.start && gap_now > gap_then;
            census.add("closes | struck nothing", closer);
            census.add(
                if row.targets.is_empty() {
                    "closes | nothing in reach"
                } else {
                    "closes | declined"
                },
                closer,
            );
            if row.hp <= 25 {
                census.add("steps away | hp<=25, struck nothing", away);
                census.add("holds | hp<=25, struck nothing", now == row.start);
            } else {
                census.add("steps away | hp>25, struck nothing", away);
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Struck {
    Unit(u32),
    City,
    Other,
}

/// A seat's AI with the switch around it. With `host` unset it is the bare
/// AI; a game whose seats are all unset plays exactly as it did.
pub struct HostTacticsSeat<A> {
    pub inner: A,
    pub host: Option<HostTactics>,
    pub census: TacticsCensus,
}

impl<A> HostTacticsSeat<A> {
    pub fn new(inner: A, host: Option<HostTactics>) -> HostTacticsSeat<A> {
        HostTacticsSeat {
            inner,
            host,
            census: TacticsCensus::default(),
        }
    }
}

impl<A: Ai> Ai for HostTacticsSeat<A> {
    fn uses_player_observation(&self) -> bool {
        self.inner.uses_player_observation()
    }

    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        let Some(host) = self.host.clone() else {
            self.inner.take_turn(g, pid);
            return;
        };
        let watch = Watch::begin(g, pid, &host);
        if host.gate {
            govern(g, pid, &host, &mut self.census.gate);
        }
        self.inner.take_turn(g, pid);
        if let Some(gate) = g.host_gate.take() {
            self.census.gate.refusals += gate.refusals;
            self.census.gate.valves += gate.valves;
        }
        watch.finish(g, pid, &mut self.census);
    }

    fn set_site_shortcuts(&mut self, shortcuts: SiteShortcuts) {
        self.inner.set_site_shortcuts(shortcuts);
    }

    fn site_shortcuts(&self) -> SiteShortcuts {
        self.inner.site_shortcuts()
    }

    fn strategy_label(&self) -> Option<&'static str> {
        self.inner.strategy_label()
    }

    fn plan_report(&self) -> Option<PlanReport> {
        self.inner.plan_report()
    }

    fn review_census(&self) -> Option<crate::strategic::ReviewCensus> {
        self.inner.review_census()
    }

    fn expansion_census(&self) -> Option<ExpansionCensus> {
        self.inner.expansion_census()
    }

    fn attach_journal(&mut self, journal: Journal) {
        self.inner.attach_journal(journal);
    }
}

#[cfg(test)]
mod tests;
