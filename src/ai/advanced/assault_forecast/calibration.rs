//! The forecast's calibration and the go gene's paired read, on posed
//! sieges played out by the real AI on the engine.
//!
//! Each seed poses one siege on open ground (`world`): our capital in the
//! west, player 1's city at [`TARGET`] with 0/100/200/400 walls (sometimes
//! already damaged), its owner one era behind, level or ahead of ours (its
//! city defence follows its best unit), a garrison and field defenders or
//! none, and a force of 2-9 of our era's melee, shooters and guns between 2
//! and 10 tiles out. Our seat is the live genome as `siege_replay` seats it
//! (`live_seat`: the operator's arms of 2026-11-12 in
//! `live_force_on_20261112.txt` forced onto the deployment genome), aimed
//! at player 1 on the Domination lane; player 1 is the stock agent.
//!
//! - **Calibration**: the forecast is read before our first move, then the
//!   game plays [`HORIZON`] turns. Predicted P against the realized capture,
//!   in five bins, with the Brier score, under every [`FORECASTS`] setting.
//!   Both seats are kept solvent (`play_world`'s `solvent`); the same seeds
//!   left to their treasuries, as #5195 ran them, are printed beside.
//! - **Why high readings fail** (#5207): each reading of 0.8 or more not
//!   taken gets its first cause (`classify`), with two worked examples per
//!   cause; the same solvent sieges are played again with
//!   `siege-goes-on-the-forecast` on, paired; a tempo family
//!   (`Family::Tempo`) poses the force 6-12 tiles out with the owner raising
//!   walls on turn 8-12; and a two-city family (`Family::TwoCities`) gives
//!   the campaign a second objective. Each family's walk to the walls is
//!   tabled: target switches, members held short, the train's hold lines.
//! - **Paired**: the seed with a second city of player 1's beyond the first,
//!   played [`PAIRED_TURNS`] turns twice, the live genome with and without
//!   `siege-goes-on-the-forecast`. A war that ends in peace takes nothing;
//!   the table is printed again over the pairs no arm made peace in.
//!
//! The rows are a fixed, repeatable problem set for this codebase's AI on
//! both sides, not a forecast of live play.
//!
//! ```text
//! cargo test --profile ci --lib -- --ignored assault_forecast_calibration --nocapture
//! ```

use super::*;
use crate::ai::advanced::siege_train::SiegeStage;
use crate::ai::advanced::VictoryTarget;
use crate::ai::Ai;
use crate::game::Action;
use crate::reasoning::Journal;
use crate::rng::Rng;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const LIVE_ARMS: &str = include_str!("live_force_on_20261112.txt");
const TARGET: Pos = (18, 8);
const HOME: Pos = (2, 8);
const CALIBRATION_SEEDS: u64 = 1200;
const PAIRED_SEEDS: u64 = 400;
/// Seeds of the two-city family (`Family::TwoCities`), reported on their
/// own.
const TWO_CITY_SEEDS: u64 = 400;
/// Seeds of the tempo family (`Family::Tempo`), reported on their own.
const TEMPO_SEEDS: u64 = 400;
const PAIRED_TURNS: u32 = 30;
/// The paired arms: the live genome alone, and with the go gene.
const ARMS: [(&str, bool); 2] = [("off", false), ("on", true)];
/// Forecast settings the calibration reads side by side.
const HALF: Knobs = Knobs {
    enemy_aggression: 0.5,
    ..KNOBS
};
const FORECASTS: [(&str, Knobs); 5] = [
    (
        "melee wait for the breach, enemy always acts (shipped)",
        KNOBS,
    ),
    (
        "melee wait for the breach, enemy acts three turns in four",
        Knobs {
            enemy_aggression: 0.75,
            ..KNOBS
        },
    ),
    ("melee wait for the breach, enemy acts half its turns", HALF),
    (
        "melee chip any wall, enemy always acts",
        Knobs {
            melee_breach_share: 0.0,
            ..KNOBS
        },
    ),
    (
        "melee chip any wall, enemy acts half its turns",
        Knobs {
            melee_breach_share: 0.0,
            enemy_aggression: 0.5,
        },
    ),
];
/// Seeds whose siege journal is printed, to show the harness plays a siege.
const TRACED: u64 = 2;

struct Era {
    melee: &'static [&'static str],
    ranged: &'static [&'static str],
    siege: &'static [&'static str],
    defenders: &'static [&'static str],
    /// The owner's strongest unit built, which the city's defence follows.
    best: i64,
}

const ERAS: [Era; 4] = [
    Era {
        melee: &["warrior", "spearman", "swordsman", "horseman"],
        ranged: &["slinger", "archer"],
        siege: &["catapult"],
        defenders: &["warrior", "spearman", "archer"],
        best: 35,
    },
    Era {
        melee: &["swordsman", "man_at_arms", "pikeman", "knight"],
        ranged: &["crossbowman"],
        siege: &["trebuchet"],
        defenders: &["pikeman", "crossbowman", "man_at_arms"],
        best: 50,
    },
    Era {
        melee: &["man_at_arms", "musketman", "pike_and_shot", "knight"],
        ranged: &["crossbowman", "field_cannon"],
        siege: &["bombard"],
        defenders: &["musketman", "crossbowman", "pike_and_shot"],
        best: 55,
    },
    Era {
        melee: &["musketman", "line_infantry", "cavalry"],
        ranged: &["field_cannon"],
        siege: &["artillery"],
        defenders: &["line_infantry", "field_cannon"],
        best: 65,
    },
];

/// One posed siege.
struct World {
    g: Game,
    cid: u32,
    /// Every city of player 1's: the posed one first.
    targets: Vec<Pos>,
    force: Vec<u32>,
    label: String,
    walls: i32,
    /// `Family::Tempo`: the turn the owner raises the walls, and to what.
    wall_event: Option<(u32, i32)>,
}

/// Which posed sieges a seed draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    /// The calibration's mix.
    Mixed,
    /// The live tempo case (2026-11-12, Warsaw and Poznan): an open or
    /// lightly walled city, the force 6-12 tiles out, and the owner
    /// raising the walls to 200 eight to twelve turns in.
    Tempo,
    /// The mix with a second city of the owner's about eight tiles beyond
    /// the first, so the campaign has another objective to switch to (the
    /// live case of 2026-11-12: Warsaw and Wroclaw, five tiles apart).
    TwoCities,
}

fn free_land(g: &Game, pos: Pos) -> bool {
    g.map
        .get(pos)
        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        && g.city_at(pos).is_none()
        && g.unit_ids_at(pos).is_empty()
}

fn pick_ring(g: &Game, rng: &mut Rng, distance: i32) -> Option<Pos> {
    let mut tiles: Vec<Pos> = g
        .wring(TARGET, distance)
        .into_iter()
        .filter(|pos| free_land(g, *pos) && g.wdist(*pos, HOME) > 2)
        .collect();
    tiles.sort_unstable();
    (!tiles.is_empty()).then(|| tiles[rng.below(tiles.len())])
}

/// `world`, and for a paired seed a second city of player 1's about eight
/// tiles beyond the first, so a siege stood down has somewhere to go. Its
/// draws come from their own stream: the first city's siege is the same.
fn world(seed: u64, second: bool) -> World {
    world_of(seed, second, Family::Mixed)
}

fn world_of(seed: u64, second: bool, family: Family) -> World {
    let mut rng = Rng::new(seed ^ 0x51e6_e0f0_7ca5_7001);
    // The tempo family's own draws, so the mix's stream is untouched.
    let mut tempo = Rng::new(seed ^ 0x7e3b_0c1a_5ee4_1d07);
    let tempo_on = family == Family::Tempo;
    let era = rng.below(ERAS.len());
    let enemy_era = (era as i32 + [-1, 0, 0, 1][rng.below(4)]).clamp(0, 3) as usize;
    let walls = [0, 100, 200, 400][rng.below(4)];
    let walls = if tempo_on {
        [0, 0, 100][tempo.below(3)]
    } else {
        walls
    };
    let mut g = Game::new_full(2, 36, 22, seed.wrapping_add(17), 400, 0, false);
    let ids: Vec<_> = g.units.keys().copied().collect();
    for id in ids {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for pos in positions {
        let near = (pos.0 - TARGET.0).abs().max((pos.1 - TARGET.1).abs()) <= 7;
        let roll = rng.f64();
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        if near && pos != TARGET {
            if roll < 0.14 {
                tile.hills = true;
            } else if roll < 0.26 {
                tile.feature = Some(crate::name!("forest"));
            }
        }
    }
    g.found_city_for(0, HOME, None);
    let cid = g.found_city_for(1, TARGET, None);
    let max_era = |era: usize| {
        g.rules
            .techs
            .iter()
            .filter(|(_, tech)| tech.era <= era)
            .map(|(name, _)| *name)
            .collect::<BTreeSet<Name>>()
    };
    let (ours, theirs) = (max_era(era), max_era(enemy_era));
    g.players[0].techs.extend(ours);
    g.players[1].techs.extend(theirs);
    let all: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for pid in 0..2 {
        g.players[pid].met.insert(1 - pid);
        g.players[pid].explored.extend(all.iter().copied());
    }
    let damaged = walls > 0 && rng.chance(0.25);
    let wall_hp = if damaged {
        (f64::from(walls) * rng.uniform(0.2, 0.9)).round() as i32
    } else {
        walls
    };
    let city_hp = if rng.chance(0.3) {
        rng.randint(90, 199)
    } else {
        200
    };
    {
        let city = g.cities.get_mut(&cid).unwrap();
        if walls > 0 {
            city.buildings.push(crate::name!("walls"));
        }
        city.wall_hp = wall_hp;
        city.hp = city_hp;
    }
    Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, walls);
    let enemy = &ERAS[enemy_era];
    let mut defenders = Vec::new();
    if rng.chance(0.65) {
        let kind = enemy.defenders[rng.below(enemy.defenders.len())];
        g.spawn_test_unit(kind, 1, TARGET);
        defenders.push(kind);
    }
    for _ in 0..rng.below(3) {
        let distance = 1 + rng.below(3) as i32;
        if let Some(pos) = pick_ring(&g, &mut rng, distance) {
            let kind = enemy.defenders[rng.below(enemy.defenders.len())];
            g.spawn_test_unit(kind, 1, pos);
            defenders.push(kind);
        }
    }
    *g.players[1]
        .counters
        .entry("strongest_unit_built".to_string())
        .or_insert(0) = enemy.best;
    let ours = &ERAS[era];
    let size = 2 + rng.below(8);
    let mut force = Vec::new();
    let mut kinds = Vec::new();
    for _ in 0..size {
        let roll = rng.f64();
        let pool = if roll < 0.55 {
            ours.melee
        } else if roll < 0.8 {
            ours.ranged
        } else {
            ours.siege
        };
        let kind = pool[rng.below(pool.len())];
        let roll = rng.f64();
        let distance = if roll < 0.55 {
            2 + rng.below(3) as i32
        } else if roll < 0.85 {
            5 + rng.below(3) as i32
        } else {
            8 + rng.below(3) as i32
        };
        let distance = if tempo_on {
            6 + tempo.below(7) as i32
        } else {
            distance
        };
        if let Some(pos) = pick_ring(&g, &mut rng, distance) {
            force.push(g.spawn_test_unit(kind, 0, pos));
            kinds.push(format!("{kind}@{distance}"));
        }
    }
    let mut targets = vec![TARGET];
    let mut beyond = String::new();
    if second {
        let mut rng = Rng::new(seed ^ 0x2c17_5ec0_0d0c_17e5);
        let spot = g
            .wring(TARGET, 8)
            .into_iter()
            .filter(|pos| {
                free_land(&g, *pos)
                    && g.wdisk(*pos, 1)
                        .iter()
                        .all(|n| free_land(&g, *n) || *n == *pos)
            })
            .max_by_key(|pos| (g.wdist(*pos, HOME), std::cmp::Reverse(*pos)));
        if let Some(spot) = spot {
            let other = g.found_city_for(1, spot, None);
            let walls = [0, 0, 100, 200][rng.below(4)];
            {
                let city = g.cities.get_mut(&other).unwrap();
                if walls > 0 {
                    city.buildings.push(crate::name!("walls"));
                }
                city.wall_hp = walls;
            }
            Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(other, walls);
            let garrison = rng.chance(0.5).then(|| {
                let kind = enemy.defenders[rng.below(enemy.defenders.len())];
                g.spawn_test_unit(kind, 1, spot);
                kind
            });
            targets.push(spot);
            beyond = format!(
                "; second city walls {walls}, defence {:.0}, garrison {}",
                g.city_strength(other),
                garrison.unwrap_or("none")
            );
        }
    }
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 60 + 35 * era as u32;
    let label = format!(
        "era {era} v {enemy_era}, walls {wall_hp}/{walls}, city {city_hp}, defence {:.0}, defenders [{}], force [{}]{beyond}",
        g.city_strength(cid),
        defenders.join(" "),
        kinds.join(" ")
    );
    let wall_event = tempo_on.then(|| (8 + tempo.below(5) as u32, 200));
    let label = match wall_event {
        Some((turn, to)) => format!("{label}; walls rise to {to} on turn {turn}"),
        None => label,
    };
    World {
        g,
        cid,
        targets,
        force,
        label,
        walls,
        wall_event,
    }
}

fn live_tags() -> BTreeSet<&'static str> {
    LIVE_ARMS
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .collect()
}

/// The live seat as `siege_replay` seats it: the Domination lane aimed at
/// player 1, the live-bridge universe, then the deployment genome with the
/// live arms forced on (`apply_gene_ledger_with_forced_live`), and the go
/// gene forced on top for the `go` arm.
fn live_seat(tags: &BTreeSet<&'static str>, go: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.retarget_against(VictoryTarget::Domination, 1);
    ai.enable_live_bridge_universe();
    let mut forced: Vec<&str> = tags.iter().copied().collect();
    if go {
        forced.push("siege-goes-on-the-forecast");
    }
    ai.apply_gene_ledger_with_forced_live(&forced);
    assert_eq!(ai.siege_goes_on_the_forecast, go, "the arm seats the gene");
    ai
}

#[derive(Clone, Debug, Default)]
struct Outcome {
    /// Turn of the first capture, 1 = this turn.
    captured: Option<u32>,
    /// Player 1's cities taken.
    cities: usize,
    /// Turn the posed city itself was taken: what its forecast is read
    /// against.
    posed: Option<u32>,
    /// Force members gone by the capture or the end.
    lost: usize,
    peace: bool,
    /// Furthest siege stage reached: 0 none, 1 stage, 2 invest, 3 reduce,
    /// 4 take, 5 hold.
    stage: u8,
    seconds: f64,
    /// The gene's journal lines: go orders, held aborts, stand-downs, and
    /// board turns whose Siege row read no capture.
    goes: u32,
    holds: u32,
    stand_downs: u32,
    zero_rows: u32,
    trace: Vec<String>,
    /// What each of the posed city's turns looked like (`Round`), one per
    /// turn played.
    rounds: Vec<Round>,
    /// Per force member: the first turn it stood within two tiles of the
    /// city, and the first turn it could have.
    arrivals: Vec<Option<u32>>,
    ideal: Vec<u32>,
    /// Player 1's land soldiers that were not on the board at the start.
    new_enemy: usize,
    /// Each of them: the turn it stood, its kind, the owner's Gold before
    /// and after that turn, and its distance from the city.
    raised: Vec<(u32, String, f64, f64, i32)>,
    wall_max: (i32, i32),
    /// The first turn the stage-musters-out-of-reach muster closed.
    muster_closed: Option<u32>,
    /// The posed city.
    cid: u32,
    /// Our military units an enemy removed (`CombatLedger::units_lost`).
    combat_lost: usize,
}

/// One turn of the posed city's siege, read around our turn and the
/// owner's.
#[derive(Clone, Debug, Default)]
struct Round {
    /// The train's stage for the city after our turn (`stage_rank`).
    stage: u8,
    /// The city after our turn, and after its owner's.
    hp: i32,
    wall: i32,
    end_hp: i32,
    end_wall: i32,
    /// `ring_state` after our turn.
    sealed: usize,
    ring: usize,
    /// Force members alive, and within two tiles of the city, after our turn.
    alive: usize,
    near: usize,
    /// Our land melee beside the city after our turn.
    melee_adjacent: usize,
    /// Our shooters within range of the city with an attack at the start of
    /// our turn, and of those, the ones that spent it.
    shooters_in_range: usize,
    shooters_fired: usize,
    /// Health and walls the city lost on our turn; health the owner's units
    /// lost on our turn.
    city_damage: i32,
    unit_damage: i32,
    /// Player 1's land soldiers after its turn, and the city's wall pool.
    enemy_units: usize,
    wall_max: i32,
    /// Force members that died this turn while the walls stood above the
    /// taker's share.
    deaths_walled: usize,
    deaths: usize,
    /// Our military units an enemy removed this turn (`CombatLedger`).
    combat_deaths: usize,
    /// Force members gone by the end of our own turn: nothing of the
    /// owner's acts then, so they fell striking.
    died_striking: usize,
    /// Our melee beside the city with an attack at the start of our turn,
    /// and of those, the ones that spent it.
    melee_ready: usize,
    melee_struck: usize,
    /// Our treasury and income at the start of our turn.
    gold: f64,
    gold_per_turn: f64,
    /// Player 0's events from the start of our turn to the end of the
    /// owner's (`Game::events`).
    events: Vec<String>,
    /// The campaign's target city after our turn (`StrategicPlan::target_city`).
    target: Option<u32>,
    /// Force members farther than two tiles out, alive, that ended our turn
    /// no nearer the city.
    held: usize,
    /// The train's hold lines this turn, by the guard that wrote them.
    guards: Vec<&'static str>,
}

fn land_soldier(g: &Game, unit: &crate::game::Unit) -> bool {
    let spec = &g.rules.units[unit.kind];
    spec.class == "military"
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        && (spec.is_melee_capable() || spec.has_ranged_attack())
}

/// The siege as it stood before our turn.
struct Before {
    health: i32,
    ready_melee: Vec<u32>,
    walled: bool,
    enemy_hp: BTreeMap<u32, i32>,
    ready_shooters: Vec<u32>,
    alive: BTreeSet<u32>,
    /// Each live force member's distance to the city.
    dist: BTreeMap<u32, i32>,
}

fn before_our_turn(g: &Game, w: &World) -> Before {
    let city = g.cities.get(&w.cid).filter(|city| city.owner != 0);
    let pos = city.map_or(TARGET, |city| city.pos);
    Before {
        health: city.map_or(0, |city| city.hp.max(0) + city.wall_hp.max(0)),
        walled: city.is_some_and(|city| {
            let max = g.city_max_wall_hp(city);
            max > 0 && f64::from(city.wall_hp) > 0.2 * f64::from(max)
        }),
        enemy_hp: g
            .units
            .values()
            .filter(|unit| unit.owner == 1 && land_soldier(g, unit))
            .map(|unit| (unit.id, unit.hp))
            .collect(),
        ready_shooters: g
            .units
            .values()
            .filter(|unit| {
                unit.owner == 0
                    && land_soldier(g, unit)
                    && g.rules.units[unit.kind].has_ranged_attack()
                    && unit.attacks_left > 0
                    && g.wdist(unit.pos, pos) <= g.unit_attack_range(unit.id).max(1)
            })
            .map(|unit| unit.id)
            .collect(),
        alive: w
            .force
            .iter()
            .copied()
            .filter(|uid| g.units.get(uid).is_some_and(|unit| unit.owner == 0))
            .collect(),
        dist: w
            .force
            .iter()
            .filter_map(|uid| {
                g.units
                    .get(uid)
                    .filter(|unit| unit.owner == 0)
                    .map(|unit| (*uid, g.wdist(unit.pos, pos)))
            })
            .collect(),
        ready_melee: g
            .units
            .values()
            .filter(|unit| {
                unit.owner == 0
                    && land_soldier(g, unit)
                    && !g.rules.units[unit.kind].has_ranged_attack()
                    && unit.attacks_left > 0
                    && g.wdist(unit.pos, pos) == 1
            })
            .map(|unit| unit.id)
            .collect(),
    }
}

fn stage_rank(stage: SiegeStage) -> u8 {
    match stage {
        SiegeStage::Stage => 1,
        SiegeStage::Invest => 2,
        SiegeStage::Reduce => 3,
        SiegeStage::Take => 4,
        SiegeStage::Hold => 5,
    }
}

/// The train's hold lines (`siege_train.rs`), by the guard that writes them.
const GUARD_LINES: [(&str, &str); 8] = [
    ("holds short of a hostile seen in the fog", "fog hostile"),
    ("closed borders", "closed borders"),
    ("holds on land short of the water", "water"),
    ("waits on its dry road", "dry road"),
    ("holds at the muster line", "muster line"),
    ("the mustered train holds", "mustered train"),
    ("stops short of its post", "short of its post"),
    ("holds off the capture", "off the capture"),
];

/// Under `solvent`, each seat starts its turn holding this many turns of its
/// deficit plus `SOLVENT_MARGIN`. That is enough that neither the engine's
/// end of turn (`settle_gold_budget`, one unit disbanded per 10 Gold short)
/// nor `bankruptcy-exits-the-drain-2` (under `DRAIN_RUNWAY_TURNS` = 5 turns
/// of deficit) disbands anything. A live seat's economy carries its army.
const SOLVENT_TURNS: f64 = 7.0;
const SOLVENT_MARGIN: f64 = 20.0;
/// Under `solvent`, both seats start with at least this much Gold.
const SOLVENT_FLOOR: f64 = 100.0;

fn play_world(w: &World, mut ours: AdvancedAi, turns: u32, trace: bool, solvent: bool) -> Outcome {
    let clock = Instant::now();
    let mut g = w.g.clone();
    if solvent {
        for pid in 0..2 {
            g.players[pid].gold = g.players[pid].gold.max(SOLVENT_FLOOR);
        }
    }
    let combat_lost_at_start = g.combat_ledger(0).units_lost;
    let mut combat_seen = combat_lost_at_start as usize;
    let journal = Journal::recording();
    ours.attach_journal(journal.handle());
    let mut cursor = 0;
    let start = g.turn;
    let mut others: Vec<AdvancedAi> = (0..g.players.len()).map(|_| AdvancedAi::new()).collect();
    // As `ai::run_game` plays a headless world.
    g.set_fog_memory(
        ours.uses_player_observation() || others.iter().any(Ai::uses_player_observation),
    );
    g.set_war_ledger(false);
    let mut out = Outcome::default();
    let mut steps = 0;
    let initial_enemy: BTreeSet<u32> = g
        .units
        .values()
        .filter(|unit| unit.owner == 1 && land_soldier(&g, unit))
        .map(|unit| unit.id)
        .collect();
    let mut seen_enemy = initial_enemy.clone();
    out.cid = w.cid;
    out.wall_max = g
        .cities
        .get(&w.cid)
        .map_or((0, 0), |city| (g.city_max_wall_hp(city), 0));
    out.arrivals = vec![None; w.force.len()];
    out.ideal = w
        .force
        .iter()
        .map(|uid| {
            let unit = &g.units[uid];
            let moves = g.unit_max_moves(*uid).max(1.0);
            (f64::from((g.wdist(unit.pos, TARGET) - 2).max(0)) / moves).ceil() as u32
        })
        .collect();
    let mut before: Option<Before> = None;
    // Read before our end of turn, which may hand the moves back.
    let mut fired_now = 0;
    let mut events_seen = g.events.len();
    let mut gold_now = (0.0, 0.0);
    let mut struck_now = 0;
    let mut striking_dead = 0;
    let mut owner_gold = 0.0;
    while g.winner.is_none() && g.turn < start + turns && steps < 64 * turns {
        steps += 1;
        let pid = g.current;
        if solvent && pid < 2 {
            // Cover the deficit with a runway before the seat plays: the
            // agent ends its own turn, and the engine settles the treasury
            // then, so a top-up after the turn never ran (#5207 round 5).
            let need = SOLVENT_MARGIN + SOLVENT_TURNS * (-g.players[pid].gold_per_turn).max(0.0);
            if g.players[pid].gold < need {
                g.players[pid].gold = need;
            }
        }
        if pid == 1 {
            if let Some((turn, walls)) = w.wall_event {
                if g.turn - start + 1 == turn {
                    if let Some(city) = g.cities.get_mut(&w.cid).filter(|city| city.owner == 1) {
                        if !city.buildings.contains(&crate::name!("walls")) {
                            city.buildings.push(crate::name!("walls"));
                        }
                        city.wall_hp = city.wall_hp.max(walls);
                        Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(w.cid, walls);
                    }
                }
            }
        }
        if pid == 0 {
            let pre = before_our_turn(&g, w);
            gold_now = (g.players[0].gold, g.players[0].gold_per_turn);
            ours.take_turn(&mut g, 0);
            fired_now = pre
                .ready_shooters
                .iter()
                .filter(|uid| g.units.get(uid).is_none_or(|unit| unit.attacks_left <= 0))
                .count();
            struck_now = pre
                .ready_melee
                .iter()
                .filter(|uid| g.units.get(uid).is_none_or(|unit| unit.attacks_left <= 0))
                .count();
            striking_dead = pre
                .alive
                .iter()
                .filter(|uid| g.units.get(uid).is_none_or(|unit| unit.owner != 0))
                .count();
            before = Some(pre);
        } else {
            owner_gold = g.players[pid].gold;
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        if pid == 0 {
            let delta = journal.since(cursor);
            cursor = delta.cursor;
            let mut guards = Vec::new();
            for thought in delta.thoughts.iter().filter(|t| t.player == 0) {
                let head = &thought.headline;
                if head.starts_with("Siege") {
                    if let Some(guard) = GUARD_LINES
                        .iter()
                        .find(|(needle, _)| head.contains(needle))
                        .map(|(_, guard)| *guard)
                    {
                        guards.push(guard);
                    }
                }
                if head.contains("the mustered train closes") && out.muster_closed.is_none() {
                    out.muster_closed = Some(g.turn - start + 1);
                }
                if head.contains("the forecast goes") {
                    out.goes += 1;
                } else if head.contains("the forecast holds the train") {
                    out.holds += 1;
                } else if head.contains("the forecast reads it hopeless") {
                    out.stand_downs += 1;
                } else if head.starts_with("Siege rows rank on the forecast")
                    && thought.detail.contains(" 0.00 a turn")
                {
                    out.zero_rows += 1;
                }
                if trace
                    && out.trace.len() < 40
                    && (head.starts_with("Siege")
                        || head.starts_with("Assault forecast")
                        || head.starts_with("Grand strategy"))
                {
                    let detail: String = thought.detail.chars().take(160).collect();
                    out.trace
                        .push(format!("t{} {} | {}", thought.turn, head, detail));
                }
            }
            if let Some(siege) = ours.sieges.get(&w.cid) {
                out.stage = out.stage.max(stage_rank(siege.stage));
            }
            let round_turn = g.turn - start + 1;
            let city = g.cities.get(&w.cid).filter(|city| city.owner != 0);
            let pos = city.map_or(TARGET, |city| city.pos);
            let pre = before.as_ref().expect("read before our turn");
            let (sealed, ring) = if city.is_some() {
                super::super::siege_train::ring_state(&g, w.cid)
            } else {
                (0, 0)
            };
            let mut round = Round {
                target: ours.plan.as_ref().and_then(|plan| plan.target_city),
                held: pre
                    .dist
                    .iter()
                    .filter(|(uid, was)| {
                        **was > 2
                            && g.units
                                .get(*uid)
                                .filter(|unit| unit.owner == 0)
                                .is_some_and(|unit| g.wdist(unit.pos, pos) >= **was)
                    })
                    .count(),
                guards,
                stage: ours
                    .sieges
                    .get(&w.cid)
                    .map_or(0, |siege| stage_rank(siege.stage)),
                hp: city.map_or(0, |city| city.hp.max(0)),
                wall: city.map_or(0, |city| city.wall_hp.max(0)),
                sealed,
                ring,
                ..Round::default()
            };
            for (k, uid) in w.force.iter().enumerate() {
                let Some(unit) = g.units.get(uid).filter(|unit| unit.owner == 0) else {
                    continue;
                };
                round.alive += 1;
                if g.wdist(unit.pos, pos) <= 2 {
                    round.near += 1;
                    out.arrivals[k].get_or_insert(round_turn);
                }
            }
            round.melee_adjacent = g
                .units
                .values()
                .filter(|unit| {
                    unit.owner == 0
                        && land_soldier(&g, unit)
                        && !g.rules.units[unit.kind].has_ranged_attack()
                        && g.wdist(unit.pos, pos) == 1
                })
                .count();
            round.shooters_in_range = pre.ready_shooters.len();
            round.shooters_fired = fired_now;
            round.melee_ready = pre.ready_melee.len();
            round.melee_struck = struck_now;
            round.died_striking = striking_dead;
            round.gold = gold_now.0;
            round.gold_per_turn = gold_now.1;
            round.city_damage = pre.health - (round.hp + round.wall);
            round.unit_damage = pre
                .enemy_hp
                .iter()
                .map(|(uid, hp)| hp - g.units.get(uid).map_or(0, |unit| unit.hp.max(0)))
                .sum();
            out.rounds.push(round);
            let taken = w
                .targets
                .iter()
                .filter(|pos| g.city_at(**pos).is_none_or(|cid| g.cities[&cid].owner == 0))
                .count();
            if taken > 0 && out.captured.is_none() {
                out.captured = Some(g.turn - start + 1);
            }
            if out.posed.is_none()
                && g.city_at(TARGET)
                    .is_none_or(|cid| g.cities[&cid].owner == 0)
            {
                out.posed = Some(g.turn - start + 1);
            }
            out.cities = taken;
            if taken == w.targets.len() {
                break;
            }
        }
        if pid != 0 {
            let turn = out.rounds.len() as u32;
            if let (Some(round), Some(pre)) = (out.rounds.last_mut(), before.as_ref()) {
                let city = g.cities.get(&w.cid).filter(|city| city.owner != 0);
                round.end_hp = city.map_or(0, |city| city.hp.max(0));
                round.end_wall = city.map_or(0, |city| city.wall_hp.max(0));
                round.enemy_units = 0;
                for unit in g.units.values() {
                    if unit.owner == 1 && land_soldier(&g, unit) {
                        round.enemy_units += 1;
                        if seen_enemy.insert(unit.id) {
                            out.new_enemy += 1;
                            out.raised.push((
                                turn,
                                unit.kind.as_str().to_string(),
                                owner_gold,
                                g.players[1].gold,
                                g.wdist(unit.pos, TARGET),
                            ));
                        }
                    }
                }
                round.events.extend(
                    g.events
                        .iter()
                        .skip(events_seen)
                        .filter(|event| event.player == 0)
                        .map(|event| event.text.clone()),
                );
                events_seen = g.events.len();
                let ledger = g.combat_ledger(0).units_lost as usize;
                round.combat_deaths = ledger.saturating_sub(combat_seen);
                combat_seen = ledger;
                round.deaths = pre
                    .alive
                    .iter()
                    .filter(|uid| g.units.get(uid).is_none_or(|unit| unit.owner != 0))
                    .count();
                if pre.walled {
                    round.deaths_walled = round.deaths;
                }
                out.wall_max.1 = city.map_or(out.wall_max.0, |city| g.city_max_wall_hp(city));
                round.wall_max = out.wall_max.1;
            }
        }
        if !g.is_at_war(0, 1) {
            out.peace = true;
            break;
        }
    }
    out.lost = w
        .force
        .iter()
        .filter(|uid| g.units.get(uid).is_none_or(|unit| unit.owner != 0))
        .count();
    out.combat_lost = (g.combat_ledger(0).units_lost - combat_lost_at_start) as usize;
    out.seconds = clock.elapsed().as_secs_f64();
    out
}

struct CalibrationRow {
    seed: u64,
    family: Family,
    solvent: bool,
    label: String,
    walls: i32,
    /// The shipped forecast with its short-of reading, then the curve
    /// under each [`FORECASTS`] entry.
    forecast: AssaultForecast,
    variants: Vec<AssaultForecast>,
    /// The posed city's health and walls before our first move.
    start: (i32, i32),
    forecast_ms: f64,
    outcome: Outcome,
    /// The same world played with `siege-goes-on-the-forecast` on (mixed
    /// solvent sieges only).
    go: Option<Outcome>,
}

struct PairRow {
    seed: u64,
    label: String,
    /// The forecast before our first move, as the calibration reads it.
    forecast: AssaultForecast,
    /// One outcome per [`ARMS`] entry.
    arms: Vec<Outcome>,
}

enum Job {
    /// A calibration seed, its family, and whether the seats are kept
    /// solvent.
    Calibrate(u64, Family, bool),
    Pair(u64),
}

enum Row {
    Calibrated(CalibrationRow),
    Paired(PairRow),
}

fn run(job: &Job, tags: &BTreeSet<&'static str>) -> Row {
    match job {
        Job::Calibrate(seed, family, solvent) => {
            let mut w = world_of(*seed, *family == Family::TwoCities, *family);
            // The forecast reads the treasuries the game then plays with.
            if *solvent {
                for pid in 0..2 {
                    w.g.players[pid].gold = w.g.players[pid].gold.max(SOLVENT_FLOOR);
                }
            }
            let start =
                w.g.cities
                    .get(&w.cid)
                    .map_or((0, 0), |city| (city.hp, city.wall_hp));
            let clock = Instant::now();
            let forecast = AdvancedAi::new()
                .assault_forecast(&w.g, 0, w.cid, &w.force)
                .expect("a posed city at war reads");
            let forecast_ms = clock.elapsed().as_secs_f64() * 1000.0;
            let variants = FORECASTS
                .iter()
                .map(|(_, knobs)| {
                    super::FORECAST_KNOBS.with(|k| k.set(Some(*knobs)));
                    let read = AdvancedAi::new()
                        .assault_capture_curve(&w.g, 0, w.cid, &w.force)
                        .expect("a posed city at war reads");
                    super::FORECAST_KNOBS.with(|k| k.set(None));
                    read
                })
                .collect();
            let outcome = play_world(
                &w,
                live_seat(tags, false),
                HORIZON as u32,
                *seed < TRACED,
                *solvent,
            );
            let go = (*solvent && *family == Family::Mixed)
                .then(|| play_world(&w, live_seat(tags, true), HORIZON as u32, false, true));
            Row::Calibrated(CalibrationRow {
                seed: *seed,
                family: *family,
                solvent: *solvent,
                label: w.label.clone(),
                walls: w.walls,
                forecast,
                variants,
                start,
                forecast_ms,
                outcome,
                go,
            })
        }
        Job::Pair(seed) => {
            let w = world(*seed, true);
            let forecast = AdvancedAi::new()
                .assault_forecast(&w.g, 0, w.cid, &w.force)
                .expect("a posed city at war reads");
            let trace = *seed < 100_000 + TRACED;
            let arms = ARMS
                .iter()
                .map(|(_, go)| play_world(&w, live_seat(tags, *go), PAIRED_TURNS, trace, true))
                .collect();
            Row::Paired(PairRow {
                seed: *seed,
                label: w.label.clone(),
                forecast,
                arms,
            })
        }
    }
}

fn mean(values: impl Iterator<Item = f64>) -> (f64, usize) {
    let (sum, n) = values.fold((0.0, 0usize), |(s, n), v| (s + v, n + 1));
    (if n == 0 { 0.0 } else { sum / n as f64 }, n)
}

/// Mean and standard error of paired differences.
fn paired(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let m = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    (m, (var / n).sqrt())
}

fn taken_by(outcome: &Outcome, k: usize) -> f64 {
    f64::from(u8::from(
        outcome.posed.is_some_and(|turn| turn as usize <= k),
    ))
}

/// One forecast and what the seat it was read for did.
type Read<'a> = (&'a AssaultForecast, &'a Outcome);

fn calibration_table(out: &mut String, rows: &[Read], k: usize) {
    let edges = [0.0, 0.2, 0.4, 0.6, 0.8, 1.000_001];
    let _ = writeln!(
        out,
        "| P(capture within {k}) | sieges | mean predicted | realized | stock train invested |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|");
    for bin in 0..5 {
        let inside: Vec<&Read> = rows
            .iter()
            .filter(|row| {
                let p = f64::from(row.0.within(k));
                p >= edges[bin] && p < edges[bin + 1]
            })
            .collect();
        let (predicted, n) = mean(inside.iter().map(|row| f64::from(row.0.within(k))));
        let (realized, _) = mean(inside.iter().map(|row| taken_by(row.1, k)));
        let (invested, _) = mean(
            inside
                .iter()
                .map(|row| f64::from(u8::from(row.1.stage >= 2))),
        );
        let _ = writeln!(
            out,
            "| {:.1}-{:.1} | {n} | {predicted:.2} | {realized:.2} | {invested:.2} |",
            edges[bin],
            edges[bin + 1].min(1.0)
        );
    }
    let brier = mean(
        rows.iter()
            .map(|row| (f64::from(row.0.within(k)) - taken_by(row.1, k)).powi(2)),
    )
    .0;
    let base = mean(rows.iter().map(|row| taken_by(row.1, k))).0;
    let climatology = mean(rows.iter().map(|row| (base - taken_by(row.1, k)).powi(2))).0;
    let _ = writeln!(
        out,
        "\nBrier score within {k}: {brier:.3} (the base rate {base:.2} for every siege scores {climatology:.3})\n"
    );
}

/// Why a siege the forecast read as taken (P(12) >= 0.8) was not taken
/// within twelve turns: the first that matches, in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Cause {
    Peace,
    Stage,
    Tempo,
    Piecemeal,
    Taker,
    DiedBeforeBreach,
    Reinforced,
    Healed,
    WallsHeld,
    NearlyTaken,
    Other,
}

impl Cause {
    const ALL: [Cause; 11] = [
        Cause::Peace,
        Cause::Stage,
        Cause::Tempo,
        Cause::Piecemeal,
        Cause::Taker,
        Cause::DiedBeforeBreach,
        Cause::Reinforced,
        Cause::Healed,
        Cause::WallsHeld,
        Cause::NearlyTaken,
        Cause::Other,
    ];

    fn name(self) -> &'static str {
        match self {
            Cause::Peace => "peace: the war ended",
            Cause::Stage => "the train never left Stage, or left it on turn 4 or later",
            Cause::Tempo => {
                "the first blow came 3+ turns after the forecast's press made its first"
            }
            Cause::Piecemeal => "the force arrived piecemeal",
            Cause::Taker => "the city fell low and no taker took it",
            Cause::DiedBeforeBreach => "our units died before the walls opened",
            Cause::Reinforced => "the defender built or bought soldiers or walls",
            Cause::Healed => "the city healed as fast as we hurt it",
            Cause::WallsHeld => "the walls never opened with shooters in range",
            Cause::NearlyTaken => "time ran out with the city under 25%",
            Cause::Other => "other",
        }
    }
}

/// The city's health and walls at the start of turn `k` (1-based).
fn start_of(row: &CalibrationRow, k: usize) -> (i32, i32) {
    if k <= 1 {
        row.start
    } else {
        let r = &row.outcome.rounds[k - 2];
        (r.end_hp, r.end_wall)
    }
}

/// The first turn the forecast's press takes 10 or more off the city's
/// health and walls, and the first turn the real train does.
fn first_blows(row: &CalibrationRow) -> (Option<u32>, Option<u32>) {
    let start = row.start.0 + row.start.1;
    let dent = row
        .forecast
        .city_health
        .iter()
        .position(|h| *h < (start - 10) as f32)
        .map(|k| k as u32 + 1);
    let blow = row
        .outcome
        .rounds
        .iter()
        .position(|r| r.city_damage >= 10)
        .map(|k| k as u32 + 1);
    (dent, blow)
}

/// What the walk to the walls looked like, per siege: the turns up to the
/// first blow (all of them when none came).
struct Walk {
    /// Turns the campaign's target changed from the turn before.
    switches: usize,
    /// Turns it targeted another city or none.
    off_target: usize,
    /// Members more than two tiles out that ended our turn no nearer.
    held: usize,
    guards: Vec<&'static str>,
    spread: u32,
    blow: Option<u32>,
    muster_to_blow: Option<u32>,
}

fn walk_of(row: &CalibrationRow) -> Walk {
    let o = &row.outcome;
    let (_, blow) = first_blows(row);
    let until = blow.map_or(o.rounds.len(), |b| (b as usize).min(o.rounds.len()));
    let walk = &o.rounds[..until];
    let arrived: Vec<u32> = o.arrivals.iter().flatten().copied().collect();
    Walk {
        switches: walk
            .windows(2)
            .filter(|w| w[0].target != w[1].target)
            .count(),
        off_target: walk.iter().filter(|r| r.target != Some(o.cid)).count(),
        held: walk.iter().map(|r| r.held).sum(),
        guards: walk.iter().flat_map(|r| r.guards.iter().copied()).collect(),
        spread: arrived.iter().max().unwrap_or(&0) - arrived.iter().min().unwrap_or(&0),
        blow,
        muster_to_blow: o
            .muster_closed
            .zip(blow)
            .map(|(closed, blow)| blow.saturating_sub(closed)),
    }
}

/// Objective switches and guard holds on the walk, next to arrival spread
/// and the first blow: the read-high sieges taken, those not, and the
/// misses by first cause.
fn walk_table(
    out: &mut String,
    high: &[&&CalibrationRow],
    missed: &[(&&&CalibrationRow, Cause, String)],
) {
    let _ = writeln!(
        out,
        "The walk to the walls, up to the first blow (all 12 turns when none came): campaign target switches, turns it targeted another city or none, members more than two tiles out that ended our turn no nearer (unit-turns), the train's hold lines by guard, arrival spread, the first blow, and muster close to the first blow\n"
    );
    let _ = writeln!(
        out,
        "| sieges | n | target switches (sieges with any) | turns off target | held unit-turns | hold lines by guard | arrival spread | first blow turn (never) | muster close to first blow (n) |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|");
    let line = |out: &mut String, name: &str, rows: &[&CalibrationRow]| {
        let walks: Vec<Walk> = rows.iter().map(|row| walk_of(row)).collect();
        let n = walks.len();
        let (switches, _) = mean(walks.iter().map(|w| w.switches as f64));
        let any = walks.iter().filter(|w| w.switches > 0).count();
        let (off, _) = mean(walks.iter().map(|w| w.off_target as f64));
        let (held, _) = mean(walks.iter().map(|w| w.held as f64));
        let mut guards: BTreeMap<&str, usize> = BTreeMap::new();
        for w in &walks {
            for guard in &w.guards {
                *guards.entry(*guard).or_default() += 1;
            }
        }
        let guards = if guards.is_empty() {
            "none".to_string()
        } else {
            guards
                .iter()
                .map(|(g, k)| format!("{g} {k}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let (spread, _) = mean(walks.iter().map(|w| f64::from(w.spread)));
        let (blow, _) = mean(walks.iter().filter_map(|w| w.blow).map(f64::from));
        let never = walks.iter().filter(|w| w.blow.is_none()).count();
        let (muster, m) = mean(walks.iter().filter_map(|w| w.muster_to_blow).map(f64::from));
        let _ = writeln!(
            out,
            "| {name} | {n} | {switches:.2} ({any}) | {off:.1} | {held:.1} | {guards} | {spread:.1} | {blow:.1} ({never}) | {muster:.1} ({m}) |"
        );
    };
    let taken: Vec<&CalibrationRow> = high
        .iter()
        .filter(|row| taken_by(&row.outcome, HORIZON) > 0.0)
        .map(|row| **row)
        .collect();
    line(out, "read high, taken", &taken);
    let not: Vec<&CalibrationRow> = missed.iter().map(|(row, _, _)| ***row).collect();
    line(out, "read high, not taken", &not);
    for cause in Cause::ALL {
        let rows: Vec<&CalibrationRow> = missed
            .iter()
            .filter(|(_, c, _)| *c == cause)
            .map(|(row, _, _)| ***row)
            .collect();
        if !rows.is_empty() {
            line(out, &format!("  {}", cause.name()), &rows);
        }
    }
    let _ = writeln!(out);
}

/// The turn the forecast's press takes the city in half its rollouts, or
/// the horizon: a soldier or wall the owner raises after it cannot be why
/// the train did not take the city.
fn fell_back(row: &CalibrationRow) -> bool {
    row.outcome
        .rounds
        .windows(2)
        .any(|w| w[0].stage >= 2 && w[1].stage <= 1)
}

fn press_turn(row: &CalibrationRow) -> u32 {
    (1..=HORIZON)
        .find(|k| row.forecast.within(*k) >= 0.5)
        .unwrap_or(HORIZON) as u32
}

/// Whether the walls rose before turn `blow` (or at all, with none).
fn walls_rose_before(row: &CalibrationRow, blow: Option<u32>) -> bool {
    let until = blow.map_or(row.outcome.rounds.len(), |b| b as usize - 1);
    let mut wall = row.start.1;
    let mut max = row.outcome.wall_max.0;
    for r in row.outcome.rounds.iter().take(until) {
        if r.end_wall > wall || r.wall_max > max {
            return true;
        }
        wall = r.end_wall;
        max = r.wall_max;
    }
    false
}

fn classify(row: &CalibrationRow) -> (Cause, String) {
    let o = &row.outcome;
    let rounds = &o.rounds;
    if o.peace {
        return (Cause::Peace, "the war ended".to_string());
    }
    let first_invest = rounds.iter().position(|r| r.stage >= 2).map(|k| k + 1);
    match first_invest {
        None => return (Cause::Stage, "never left Stage".to_string()),
        Some(turn) if turn >= 4 => return (Cause::Stage, format!("left Stage on turn {turn}")),
        _ => {}
    }
    let (dent, blow) = first_blows(row);
    if let Some(dent) = dent {
        if blow.is_none_or(|blow| blow >= dent + 3) {
            let arrived: Vec<u32> = o.arrivals.iter().flatten().copied().collect();
            let spread = arrived.iter().max().unwrap_or(&0) - arrived.iter().min().unwrap_or(&0);
            return (
                Cause::Tempo,
                format!(
                    "first blow {}, the press's on turn {dent}; invested on turn {}, muster closed {}; arrival spread {spread}; walls rose before the first blow: {}",
                    blow.map_or("never".to_string(), |b| format!("on turn {b}")),
                    first_invest.map_or("-".to_string(), |t| t.to_string()),
                    o.muster_closed.map_or("-".to_string(), |t| t.to_string()),
                    walls_rose_before(row, blow)
                ),
            );
        }
    }
    let n = o.ideal.len();
    let late: Vec<i64> = o
        .ideal
        .iter()
        .zip(&o.arrivals)
        .map(|(ideal, arrived)| i64::from(arrived.unwrap_or(13)) - i64::from(*ideal))
        .collect();
    let laggards = late.iter().filter(|l| **l >= 3).count();
    let never = o.arrivals.iter().filter(|a| a.is_none()).count();
    if n > 0 && laggards * 2 > n {
        let arrived: Vec<u32> = o.arrivals.iter().flatten().copied().collect();
        let spread = arrived.iter().max().unwrap_or(&0) - arrived.iter().min().unwrap_or(&0);
        return (
            Cause::Piecemeal,
            format!("{laggards} of {n} members 3+ turns later than they could have been ({never} never within 2), arrival spread {spread}"),
        );
    }
    let max = o.wall_max.0;
    let low: Vec<usize> = (1..=rounds.len())
        .filter(|k| {
            let (hp, wall) = start_of(row, *k);
            hp <= 50 && f64::from(wall) <= 0.2 * f64::from(max.max(0))
        })
        .collect();
    if !low.is_empty() {
        let adjacent = low
            .iter()
            .filter(|k| rounds[*k - 1].melee_adjacent > 0)
            .count();
        return (
            Cause::Taker,
            format!(
                "city low at the start of {} turns (first {}), a melee unit beside it after our turn on {adjacent}",
                low.len(),
                low[0]
            ),
        );
    }
    let walled_deaths: usize = rounds.iter().map(|r| r.deaths_walled).sum();
    let deaths: usize = rounds.iter().map(|r| r.deaths).sum();
    if max > 0 && walled_deaths >= 2usize.max(n.div_ceil(3)) {
        return (
            Cause::DiedBeforeBreach,
            format!(
                "{walled_deaths} of {n} members died while the walls stood ({deaths} died in all; {} on our own turn, striking; melee struck the walled city {} times; the train fell back to Stage {})",
                rounds.iter().map(|r| r.died_striking).sum::<usize>(),
                rounds.iter().map(|r| r.melee_struck).sum::<usize>(),
                rounds
                    .windows(2)
                    .position(|w| w[0].stage >= 2 && w[1].stage <= 1)
                    .map_or("never".to_string(), |k| format!("on turn {}", k + 2))
            ),
        );
    }
    let press = press_turn(row);
    let soldier_in_time = o.raised.iter().any(|(turn, ..)| *turn <= press);
    if soldier_in_time || walls_rose_before(row, Some(press + 1)) {
        return (
            Cause::Reinforced,
            format!(
                "before the press's capture turn {press}: {} new soldiers ({}), wall pool {} -> {}",
                o.new_enemy,
                o.raised
                    .iter()
                    .map(|(turn, kind, before, after, dist)| format!(
                        "{kind} on turn {turn} at {dist}, Gold {before:.0} -> {after:.0}"
                    ))
                    .collect::<Vec<_>>()
                    .join("; "),
                o.wall_max.0,
                o.wall_max.1
            ),
        );
    }
    let start = row.start.0 + row.start.1;
    let end = rounds.last().map_or(start, |r| r.end_hp + r.end_wall);
    let dealt: i32 = rounds.iter().map(|r| r.city_damage.max(0)).sum();
    let healed: i32 = rounds
        .iter()
        .map(|r| (r.end_hp + r.end_wall - r.hp - r.wall).max(0))
        .sum();
    let sealed = rounds
        .iter()
        .filter(|r| r.ring > 0 && r.sealed == r.ring)
        .count();
    if dealt > 0 && end >= start - 20 {
        return (
            Cause::Healed,
            format!(
                "dealt {dealt}, healed {healed}, health {start} -> {end}; ring sealed on {sealed} of {} turns",
                rounds.len()
            ),
        );
    }
    let end_wall = rounds.last().map_or(row.start.1, |r| r.end_wall);
    let shooter_turns = rounds.iter().filter(|r| r.shooters_in_range > 0).count();
    if max > 0 && f64::from(end_wall) > 0.2 * f64::from(max) && shooter_turns > 0 {
        let fired: usize = rounds.iter().map(|r| r.shooters_fired).sum();
        let city_turns = rounds.iter().filter(|r| r.city_damage > 0).count();
        let unit_damage: i32 = rounds.iter().map(|r| r.unit_damage.max(0)).sum();
        return (
            Cause::WallsHeld,
            format!("walls {end_wall}/{max} at the end; shooters in range on {shooter_turns} turns, {fired} shots, the city hurt on {city_turns} turns, {unit_damage} damage to soldiers"),
        );
    }
    let end_hp = rounds.last().map_or(row.start.0, |r| r.end_hp);
    if end_hp <= 50 {
        return (Cause::NearlyTaken, format!("city at {end_hp} at the end"));
    }
    (
        Cause::Other,
        format!("health {start} -> {end}, dealt {dealt}, healed {healed}, {deaths} members died"),
    )
}

fn taxonomy(out: &mut String, rows: &[&CalibrationRow]) {
    let high: Vec<&&CalibrationRow> = rows.iter().filter(|row| row.forecast.p() >= 0.8).collect();
    let missed: Vec<(&&&CalibrationRow, Cause, String)> = high
        .iter()
        .filter(|row| taken_by(&row.outcome, HORIZON) == 0.0)
        .map(|row| {
            let (cause, detail) = classify(row);
            (row, cause, detail)
        })
        .collect();
    let _ = writeln!(
        out,
        "## Why high forecasts fail: {} sieges read P(12) >= 0.8, {} taken within 12, {} not\n",
        high.len(),
        high.len() - missed.len(),
        missed.len()
    );
    let _ = writeln!(
        out,
        "| first cause | all | walls 0 | 100 | 200 | 400 | of them, the train fell back to Stage |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|");
    for cause in Cause::ALL {
        let count = |walls: Option<i32>| {
            missed
                .iter()
                .filter(|(row, c, _)| *c == cause && walls.is_none_or(|w| row.walls == w))
                .count()
        };
        let fell = missed
            .iter()
            .filter(|(row, c, _)| *c == cause && fell_back(row))
            .count();
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {fell} |",
            cause.name(),
            count(None),
            count(Some(0)),
            count(Some(100)),
            count(Some(200)),
            count(Some(400))
        );
    }
    let by_walls = |walls: i32| {
        let n = high.iter().filter(|row| row.walls == walls).count();
        let t = high
            .iter()
            .filter(|row| row.walls == walls && taken_by(&row.outcome, HORIZON) > 0.0)
            .count();
        format!("{t}/{n}")
    };
    let _ = writeln!(
        out,
        "| (taken / read high) | {}/{} | {} | {} | {} | {} | |\n",
        high.len() - missed.len(),
        high.len(),
        by_walls(0),
        by_walls(100),
        by_walls(200),
        by_walls(400)
    );
    // Where the force died, in the sieges read high and not taken.
    let striking: usize = missed
        .iter()
        .map(|(row, _, _)| {
            row.outcome
                .rounds
                .iter()
                .map(|r| r.died_striking)
                .sum::<usize>()
        })
        .sum();
    let all_deaths: usize = missed
        .iter()
        .map(|(row, _, _)| row.outcome.rounds.iter().map(|r| r.deaths).sum::<usize>())
        .sum();
    let walled_strikes: usize = missed
        .iter()
        .map(|(row, _, _)| {
            row.outcome
                .rounds
                .iter()
                .filter(|r| r.wall > 0)
                .map(|r| r.melee_struck)
                .sum::<usize>()
        })
        .sum();
    let fell = missed.iter().filter(|(row, _, _)| fell_back(row)).count();
    let _ = writeln!(
        out,
        "\nIn the {} missed: {all_deaths} force members died, {striking} of them on our own turn (striking); our melee struck a walled city {walled_strikes} times; the train fell back from Invest or later to Stage in {fell} sieges\n",
        missed.len()
    );
    // What the owner raised, in the sieges whose first cause it was.
    let raised: Vec<&&&CalibrationRow> = missed
        .iter()
        .filter(|(_, c, _)| *c == Cause::Reinforced)
        .map(|(row, _, _)| *row)
        .collect();
    let soldiers = |n: usize| {
        raised
            .iter()
            .filter(|row| row.outcome.new_enemy == n)
            .count()
    };
    let _ = writeln!(
        out,
        "The owner raised (first cause, {} sieges): 0 new soldiers {} (walls only), 1 {}, 2 {}, 3 or more {}; walls raised in {}\n",
        raised.len(),
        soldiers(0),
        soldiers(1),
        soldiers(2),
        raised.iter().filter(|row| row.outcome.new_enemy >= 3).count(),
        raised
            .iter()
            .filter(|row| row.outcome.wall_max.1 > row.outcome.wall_max.0)
            .count()
    );
    // Each soldier the owner raised in those sieges: when, what, and whether
    // its treasury paid for it (a drop of 30 Gold or more on that turn).
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut by_turn = [0usize; HORIZON + 1];
    let (mut bought, mut inside, mut all) = (0, 0, 0);
    for row in &raised {
        for (turn, kind, before, after, dist) in &row.outcome.raised {
            all += 1;
            *kinds.entry(kind.clone()).or_default() += 1;
            by_turn[(*turn as usize).min(HORIZON)] += 1;
            bought += usize::from(before - after >= 30.0);
            inside += usize::from(*dist == 0);
        }
    }
    let _ = writeln!(
        out,
        "Those {all} soldiers: {bought} bought (the owner's Gold fell 30 or more that turn), {inside} stood in the city; by turn {}; kinds {}\n",
        by_turn
            .iter()
            .enumerate()
            .filter(|(_, n)| **n > 0)
            .map(|(t, n)| format!("t{t} {n}"))
            .collect::<Vec<_>>()
            .join(", "),
        kinds
            .iter()
            .map(|(k, n)| format!("{k} {n}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    walk_table(out, &high, &missed);
    // The forecast's press against the real train, turn by turn.
    let _ = writeln!(
        out,
        "City health + walls at the end of each turn, mean: the forecast's press (model) against the real train (real)\n"
    );
    let _ = writeln!(out, "| sieges | n | | t1 | t2 | t3 | t4 | t6 | t8 | t12 |");
    let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|---|");
    let groups: Vec<(String, Vec<&CalibrationRow>)> = std::iter::once((
        "read high, taken".to_string(),
        high.iter()
            .filter(|row| taken_by(&row.outcome, HORIZON) > 0.0)
            .map(|row| **row)
            .collect(),
    ))
    .chain(std::iter::once((
        "read high, not taken".to_string(),
        missed.iter().map(|(row, _, _)| ***row).collect(),
    )))
    .chain(Cause::ALL.iter().map(|cause| {
        (
            format!("  {}", cause.name()),
            missed
                .iter()
                .filter(|(_, c, _)| c == cause)
                .map(|(row, _, _)| ***row)
                .collect(),
        )
    }))
    .collect();
    for (name, group) in groups {
        if group.is_empty() {
            continue;
        }
        let model = |k: usize| {
            mean(
                group
                    .iter()
                    .map(|row| f64::from(row.forecast.city_health[k - 1])),
            )
            .0
        };
        let real = |k: usize| {
            mean(group.iter().map(|row| {
                row.outcome
                    .rounds
                    .get(k - 1)
                    .map_or(0.0, |r| f64::from(r.end_hp + r.end_wall))
            }))
            .0
        };
        let ks = [1, 2, 3, 4, 6, 8, 12];
        let _ = writeln!(
            out,
            "| {name} | {} | model | {} |",
            group.len(),
            ks.iter()
                .map(|k| format!("{:.0}", model(*k)))
                .collect::<Vec<_>>()
                .join(" | ")
        );
        let _ = writeln!(
            out,
            "| | | real | {} |",
            ks.iter()
                .map(|k| format!("{:.0}", real(*k)))
                .collect::<Vec<_>>()
                .join(" | ")
        );
    }
    let _ = writeln!(out, "\n### Worked examples, two per cause\n");
    for cause in Cause::ALL {
        for (row, _, detail) in missed.iter().filter(|(_, c, _)| *c == cause).take(2) {
            let _ = writeln!(
                out,
                "- **{}**, seed {}: {detail}. P12 {:.2}, short of {}. {}",
                cause.name(),
                row.seed,
                row.forecast.p(),
                row.forecast.short_of.map_or("unread", ShortOf::as_str),
                row.label
            );
            for (k, r) in row.outcome.rounds.iter().enumerate().take(8) {
                let _ = writeln!(
                    out,
                    "    t{}: stage {}, city {}/{} -> {}/{} (model {:.0}), near {}/{} alive, melee beside {} ({}/{} struck), shooters {}/{} fired, dealt city {} units {}, ring {}/{}, enemy {}, died {} ({} striking)",
                    k + 1,
                    r.stage,
                    r.hp,
                    r.wall,
                    r.end_hp,
                    r.end_wall,
                    row.forecast.city_health[k],
                    r.near,
                    r.alive,
                    r.melee_adjacent,
                    r.melee_struck,
                    r.melee_ready,
                    r.shooters_fired,
                    r.shooters_in_range,
                    r.city_damage,
                    r.unit_damage,
                    r.sealed,
                    r.ring,
                    r.enemy_units,
                    r.deaths,
                    r.died_striking
                );
                if r.deaths > 0 || k == 0 {
                    let _ = writeln!(
                        out,
                        "      gold {:.0} ({:+.0} a turn); events: {}",
                        r.gold,
                        r.gold_per_turn,
                        r.events.join(" / ").chars().take(400).collect::<String>()
                    );
                }
            }
        }
    }
    let _ = writeln!(out);
}

/// The same sieges with the seats left to their treasury (as #5195 ran
/// them) and kept solvent.
fn funding(out: &mut String, unfunded: &[&CalibrationRow], solvent: &[&CalibrationRow]) {
    let _ = writeln!(
        out,
        "## Funding: the same mixed sieges, seats left to their treasury (as #5195 ran them) and funded (7 turns of deficit kept)\n"
    );
    let _ = writeln!(
        out,
        "| seats | sieges | Brier within 12 | read >= 0.8 | of them taken | force members lost | in combat (all our units) | members lost otherwise |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|---|");
    for (name, rows) in [("unfunded", unfunded), ("solvent", solvent)] {
        let brier =
            mean(rows.iter().map(|row| {
                (f64::from(row.forecast.p()) - taken_by(&row.outcome, HORIZON)).powi(2)
            }))
            .0;
        let high: Vec<&&CalibrationRow> =
            rows.iter().filter(|row| row.forecast.p() >= 0.8).collect();
        let (taken, _) = mean(high.iter().map(|row| taken_by(&row.outcome, HORIZON)));
        let (lost, _) = mean(rows.iter().map(|row| row.outcome.lost as f64));
        let (combat, _) = mean(rows.iter().map(|row| row.outcome.combat_lost as f64));
        let (other, _) = mean(
            rows.iter()
                .map(|row| row.outcome.lost.saturating_sub(row.outcome.combat_lost) as f64),
        );
        let _ = writeln!(
            out,
            "| {name} | {} | {brier:.3} | {} | {taken:.2} | {lost:.2} | {combat:.2} | {other:.2} |",
            rows.len(),
            high.len()
        );
    }
    for (name, rows) in [("unfunded", unfunded), ("solvent", solvent)] {
        let raised = rows.iter().filter(|row| row.outcome.new_enemy > 0).count();
        let paid = rows
            .iter()
            .filter(|row| {
                row.outcome
                    .raised
                    .iter()
                    .any(|(_, _, before, after, _)| before - after >= 30.0)
            })
            .count();
        let _ = writeln!(
            out,
            "\n{name}: the owner raised a soldier in {raised} of {} sieges (paid in Gold in {paid})",
            rows.len()
        );
    }
    let reads: Vec<Read> = unfunded
        .iter()
        .map(|row| (&row.forecast, &row.outcome))
        .collect();
    let _ = writeln!(out, "\nUnfunded:\n");
    calibration_table(out, &reads, HORIZON);
}

fn paired_table(out: &mut String, title: &str, kept_pairs: &[&PairRow]) {
    let _ = writeln!(
        out,
        "## Paired, {title}: {} posed sieges x {PAIRED_TURNS} turns\n",
        kept_pairs.len()
    );
    let _ = writeln!(
        out,
        "| arm | cities taken | pairs ahead / behind off | invested | force units lost (vs off) | turns to first capture | go orders | held aborts | stand-downs | zero-rate row turns | s a siege |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|---|---|");
    for (a, (name, _)) in ARMS.iter().enumerate() {
        let taken: usize = kept_pairs.iter().map(|row| row.arms[a].cities).sum();
        let gained = kept_pairs
            .iter()
            .filter(|row| row.arms[a].cities > row.arms[0].cities)
            .count();
        let lost_caps = kept_pairs
            .iter()
            .filter(|row| row.arms[a].cities < row.arms[0].cities)
            .count();
        let invested = kept_pairs
            .iter()
            .filter(|row| row.arms[a].stage >= 2)
            .count();
        let (lost, _) = mean(kept_pairs.iter().map(|row| row.arms[a].lost as f64));
        let diffs: Vec<f64> = kept_pairs
            .iter()
            .map(|row| row.arms[a].lost as f64 - row.arms[0].lost as f64)
            .collect();
        let (lost_d, lost_se) = paired(&diffs);
        let (turn, captures) = mean(
            kept_pairs
                .iter()
                .filter_map(|row| row.arms[a].captured.map(f64::from)),
        );
        let sum =
            |f: fn(&Outcome) -> u32| -> u32 { kept_pairs.iter().map(|row| f(&row.arms[a])).sum() };
        let (secs, _) = mean(kept_pairs.iter().map(|row| row.arms[a].seconds));
        let _ = writeln!(
            out,
            "| {name} | {taken} | +{gained} / -{lost_caps} | {invested} | {lost:.2} ({lost_d:+.2} +/- {lost_se:.2}) | {turn:.1} ({captures}) | {} | {} | {} | {} | {secs:.2} |",
            sum(|o| o.goes),
            sum(|o| o.holds),
            sum(|o| o.stand_downs),
            sum(|o| o.zero_rows)
        );
    }
}

/// The go gene on the calibration's own solvent sieges, paired seed by seed:
/// does the train it drives realize the high readings the stock train does
/// not?
fn go_arm(out: &mut String, rows: &[&CalibrationRow]) {
    let rows: Vec<(&CalibrationRow, &Outcome)> = rows
        .iter()
        .filter_map(|row| row.go.as_ref().map(|go| (*row, go)))
        .collect();
    let _ = writeln!(
        out,
        "## `siege-goes-on-the-forecast` on the same {} solvent sieges, 12 turns, paired\n",
        rows.len()
    );
    let _ = writeln!(
        out,
        "| sieges | n | taken off | taken on | on - off (+/- se) | fell back to Stage off / on | force lost off / on |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|");
    let line = |out: &mut String, name: &str, set: &[&(&CalibrationRow, &Outcome)]| {
        let off: Vec<f64> = set
            .iter()
            .map(|(row, _)| taken_by(&row.outcome, HORIZON))
            .collect();
        let on: Vec<f64> = set.iter().map(|(_, go)| taken_by(go, HORIZON)).collect();
        let diffs: Vec<f64> = on.iter().zip(&off).map(|(a, b)| a - b).collect();
        let (d, se) = paired(&diffs);
        let falls = |o: &Outcome| {
            o.rounds
                .windows(2)
                .any(|w| w[0].stage >= 2 && w[1].stage <= 1)
        };
        let fell_off = set.iter().filter(|(row, _)| falls(&row.outcome)).count();
        let fell_on = set.iter().filter(|(_, go)| falls(go)).count();
        let (lost_off, _) = mean(set.iter().map(|(row, _)| row.outcome.lost as f64));
        let (lost_on, _) = mean(set.iter().map(|(_, go)| go.lost as f64));
        let _ = writeln!(
            out,
            "| {name} | {} | {:.2} | {:.2} | {d:+.3} +/- {se:.3} | {fell_off} / {fell_on} | {lost_off:.2} / {lost_on:.2} |",
            set.len(),
            mean(off.iter().copied()).0,
            mean(on.iter().copied()).0,
        );
    };
    let all: Vec<&(&CalibrationRow, &Outcome)> = rows.iter().collect();
    line(out, "all", &all);
    let high: Vec<&(&CalibrationRow, &Outcome)> = rows
        .iter()
        .filter(|(row, _)| row.forecast.p() >= 0.8)
        .collect();
    line(out, "read >= 0.8", &high);
    for walls in [0, 100, 200, 400] {
        let set: Vec<&(&CalibrationRow, &Outcome)> = high
            .iter()
            .copied()
            .filter(|(row, _)| row.walls == walls)
            .collect();
        line(out, &format!("read >= 0.8, walls {walls}"), &set);
    }
    let low: Vec<&(&CalibrationRow, &Outcome)> = rows
        .iter()
        .filter(|(row, _)| row.forecast.p() < 0.8)
        .collect();
    line(out, "read < 0.8", &low);
    let goes: u32 = rows.iter().map(|(_, go)| go.goes).sum();
    let holds: u32 = rows.iter().map(|(_, go)| go.holds).sum();
    let downs: u32 = rows.iter().map(|(_, go)| go.stand_downs).sum();
    let _ = writeln!(
        out,
        "\nWith the gene: {goes} goes, {holds} holds (an abort vetoed), {downs} stand-downs\n"
    );
}

fn report(calibrated: &[CalibrationRow], pairs: &[PairRow], seconds: f64) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "ASSAULT FORECAST REPORT ({seconds:.0} s wall)\n");
    let all = calibrated;
    let tempo_rows: Vec<&CalibrationRow> = calibrated
        .iter()
        .filter(|row| row.family == Family::Tempo && !row.outcome.peace)
        .collect();
    let unfunded: Vec<&CalibrationRow> = calibrated
        .iter()
        .filter(|row| row.family == Family::Mixed && !row.solvent && !row.outcome.peace)
        .collect();
    let calibrated: Vec<&CalibrationRow> = calibrated
        .iter()
        .filter(|row| row.family == Family::Mixed && row.solvent)
        .collect();
    funding(&mut out, &unfunded, &calibrated);
    let kept: Vec<&CalibrationRow> = calibrated
        .iter()
        .copied()
        .filter(|row| !row.outcome.peace)
        .collect();
    let (ms, _) = mean(calibrated.iter().map(|row| row.forecast_ms));
    let max_ms = calibrated
        .iter()
        .map(|row| row.forecast_ms)
        .fold(0.0, f64::max);
    let (play_s, _) = mean(calibrated.iter().map(|row| row.outcome.seconds));
    let _ = writeln!(
        out,
        "## Calibration: {} posed sieges, {} left out for peace; forecast {ms:.0} ms mean, {max_ms:.0} ms max \
         (with the short-of variants); {play_s:.2} s a siege played\n",
        calibrated.len(),
        calibrated.len() - kept.len()
    );
    let reads: Vec<Read> = kept
        .iter()
        .map(|row| (&row.forecast, &row.outcome))
        .collect();
    for k in [HORIZON, GO_HORIZON, 5] {
        calibration_table(&mut out, &reads, k);
    }
    let (lost_p, _) = mean(kept.iter().map(|row| f64::from(row.forecast.units_lost)));
    let (lost_r, _) = mean(kept.iter().map(|row| row.outcome.lost as f64));
    let _ = writeln!(
        out,
        "Units lost within {HORIZON} turns: predicted {lost_p:.2}, realized {lost_r:.2} a siege (realized counts the posed force only)\n"
    );
    let _ = writeln!(out, "| walls | sieges | mean P(12) | realized by 12 |");
    let _ = writeln!(out, "|---|---|---|---|");
    for walls in [0, 100, 200, 400] {
        let inside: Vec<&&CalibrationRow> = kept.iter().filter(|row| row.walls == walls).collect();
        let (p, n) = mean(inside.iter().map(|row| f64::from(row.forecast.p())));
        let (r, _) = mean(inside.iter().map(|row| taken_by(&row.outcome, HORIZON)));
        let _ = writeln!(out, "| {walls} | {n} | {p:.2} | {r:.2} |");
    }
    taxonomy(&mut out, &kept);
    go_arm(&mut out, &kept);
    let _ = writeln!(
        out,
        "## The tempo family: {} sieges (open or 100 walls, the force 6-12 tiles out, walls to 200 on turn 8-12)\n",
        tempo_rows.len()
    );
    let reads: Vec<Read> = tempo_rows
        .iter()
        .map(|row| (&row.forecast, &row.outcome))
        .collect();
    calibration_table(&mut out, &reads, HORIZON);
    let blows: Vec<(Option<u32>, Option<u32>, bool)> = tempo_rows
        .iter()
        .map(|row| {
            let (dent, blow) = first_blows(row);
            (dent, blow, walls_rose_before(row, blow))
        })
        .collect();
    let (dent, dn) = mean(blows.iter().filter_map(|(d, _, _)| d.map(f64::from)));
    let (blow, bn) = mean(blows.iter().filter_map(|(_, b, _)| b.map(f64::from)));
    let rose = blows.iter().filter(|(_, _, r)| *r).count();
    let _ = writeln!(
        out,
        "First dent, mean: the forecast's press turn {dent:.1} ({dn} sieges), the real train turn {blow:.1} ({bn} sieges); walls rose before the real first blow in {rose} of {}\n",
        blows.len()
    );
    taxonomy(&mut out, &tempo_rows);
    let two_rows: Vec<&CalibrationRow> = all
        .iter()
        .filter(|row| row.family == Family::TwoCities && !row.outcome.peace)
        .collect();
    let _ = writeln!(
        out,
        "## Two cities: {} of the mix's sieges with a second city of the owner's about eight tiles beyond the first ({} left out for peace)\n",
        two_rows.len(),
        all.iter()
            .filter(|row| row.family == Family::TwoCities && row.outcome.peace)
            .count()
    );
    let reads: Vec<Read> = two_rows
        .iter()
        .map(|row| (&row.forecast, &row.outcome))
        .collect();
    calibration_table(&mut out, &reads, HORIZON);
    taxonomy(&mut out, &two_rows);
    let _ = writeln!(
        out,
        "\n| forecast settings | Brier within 12 | within 10 | within 5 | mean P(12) | sieges over 0.8, taken |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|");
    for (v, (name, _)) in FORECASTS.iter().enumerate() {
        let reads: Vec<Read> = kept
            .iter()
            .map(|row| (&row.variants[v], &row.outcome))
            .collect();
        let brier = |k: usize| {
            mean(
                reads
                    .iter()
                    .map(|(f, o)| (f64::from(f.within(k)) - taken_by(o, k)).powi(2)),
            )
            .0
        };
        let (p, _) = mean(reads.iter().map(|(f, _)| f64::from(f.p())));
        let high: Vec<&Read> = reads.iter().filter(|(f, _)| f.p() >= 0.8).collect();
        let (high_taken, high_n) = mean(high.iter().map(|(_, o)| taken_by(o, HORIZON)));
        let _ = writeln!(
            out,
            "| {name} | {:.3} | {:.3} | {:.3} | {p:.2} | {high_n}, {high_taken:.2} |",
            brier(HORIZON),
            brier(GO_HORIZON),
            brier(5)
        );
    }
    let _ = writeln!(
        out,
        "\n| stock train reached | sieges | mean P(12) | realized by 12 |"
    );
    let _ = writeln!(out, "|---|---|---|---|");
    for (name, test) in [
        ("stage only", 1u8..=1),
        ("invest or further", 2u8..=5),
        ("no siege", 0u8..=0),
    ] {
        let inside: Vec<&&CalibrationRow> = kept
            .iter()
            .filter(|row| test.contains(&row.outcome.stage))
            .collect();
        let (p, n) = mean(inside.iter().map(|row| f64::from(row.forecast.p())));
        let (r, _) = mean(inside.iter().map(|row| taken_by(&row.outcome, HORIZON)));
        let _ = writeln!(out, "| {name} | {n} | {p:.2} | {r:.2} |");
    }
    let mut short: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for row in &kept {
        let entry = short
            .entry(row.forecast.short_of.map_or("unread", ShortOf::as_str))
            .or_default();
        entry.0 += 1;
        entry.1 += usize::from(row.outcome.captured.is_some());
    }
    let _ = writeln!(
        out,
        "\nShort of (sieges, of them taken within {HORIZON}): {}\n",
        short
            .iter()
            .map(|(k, (n, t))| format!("{k} {n} ({t})"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let peaceful: Vec<&PairRow> = pairs
        .iter()
        .filter(|row| row.arms.iter().all(|arm| !arm.peace))
        .collect();
    let everyone: Vec<&PairRow> = pairs.iter().collect();
    paired_table(
        &mut out,
        "every pair; a war that ends in peace takes nothing",
        &everyone,
    );
    paired_table(&mut out, "the pairs no arm made peace in", &peaceful);
    let kept_pairs = peaceful;
    for (a, title) in [
        (0, "the live genome alone"),
        (1, "the live genome with siege-goes-on-the-forecast"),
    ] {
        let reads: Vec<Read> = kept_pairs
            .iter()
            .map(|row| (&row.forecast, &row.arms[a]))
            .collect();
        let _ = writeln!(
            out,
            "\n### The paired sieges' forecasts against {title} ({} sieges)\n",
            reads.len()
        );
        for k in [HORIZON, GO_HORIZON] {
            calibration_table(&mut out, &reads, k);
        }
    }
    let _ = writeln!(out, "\n## Calibration rows\n");
    for row in &calibrated {
        let _ = writeln!(
            out,
            "cal {} | P12 {:.2} P10 {:.2} P5 {:.2} lost {:.1} short {} | taken {:?} lost {} stage {} peace {} | {}",
            row.seed,
            row.forecast.p(),
            row.forecast.within(GO_HORIZON),
            row.forecast.within(5),
            row.forecast.units_lost,
            row.forecast.short_of.map_or("unread", ShortOf::as_str),
            row.outcome.captured,
            row.outcome.lost,
            row.outcome.stage,
            row.outcome.peace,
            row.label
        );
    }
    let _ = writeln!(
        out,
        "\n## Paired rows (taken turn / units lost / stage, by arm)\n"
    );
    for row in pairs {
        let arms: Vec<String> = row
            .arms
            .iter()
            .map(|o| {
                format!(
                    "{}x{}/{}/{}{}",
                    o.captured.map_or("-".to_string(), |t| t.to_string()),
                    o.cities,
                    o.lost,
                    o.stage,
                    if o.peace { " peace" } else { "" }
                )
            })
            .collect();
        let _ = writeln!(
            out,
            "pair {} | {} | {}",
            row.seed,
            arms.join(" | "),
            row.label
        );
    }
    let _ = writeln!(out, "\n## Traces\n");
    for row in calibrated
        .iter()
        .filter(|row| !row.outcome.trace.is_empty())
    {
        let _ = writeln!(out, "cal {}: {}", row.seed, row.label);
        for line in &row.outcome.trace {
            let _ = writeln!(out, "  {line}");
        }
    }
    for row in pairs.iter().filter(|row| !row.arms[1].trace.is_empty()) {
        for (a, (name, _)) in ARMS.iter().enumerate().take(2) {
            let _ = writeln!(out, "pair {} {name}: {}", row.seed, row.label);
            for line in &row.arms[a].trace {
                let _ = writeln!(out, "  {line}");
            }
        }
    }
    out
}

/// The report `docs/ASSAULT_FORECAST.md` quotes: about twenty minutes on
/// four cores, so it runs only when asked for.
#[test]
#[ignore = "the calibration driver: run with --ignored (docs/ASSAULT_FORECAST.md)"]
fn assault_forecast_calibration() {
    let clock = Instant::now();
    let tags = live_tags();
    let mut jobs: Vec<Job> = (0..PAIRED_SEEDS)
        .map(|seed| Job::Pair(100_000 + seed))
        .collect();
    jobs.extend((0..TEMPO_SEEDS).map(|seed| Job::Calibrate(200_000 + seed, Family::Tempo, true)));
    jobs.extend((0..TWO_CITY_SEEDS).map(|seed| Job::Calibrate(seed, Family::TwoCities, true)));
    jobs.extend((0..CALIBRATION_SEEDS).map(|seed| Job::Calibrate(seed, Family::Mixed, true)));
    jobs.extend((0..CALIBRATION_SEEDS).map(|seed| Job::Calibrate(seed, Family::Mixed, false)));
    let next = AtomicUsize::new(0);
    let rows: Mutex<Vec<(usize, Row)>> = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 4);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(job) = jobs.get(index) else {
                    break;
                };
                let row = run(job, &tags);
                rows.lock().unwrap().push((index, row));
            });
        }
    });
    let mut rows = rows.into_inner().unwrap();
    rows.sort_by_key(|(index, _)| *index);
    let mut calibrated = Vec::new();
    let mut pairs = Vec::new();
    for (_, row) in rows {
        match row {
            Row::Calibrated(row) => calibrated.push(row),
            Row::Paired(row) => pairs.push(row),
        }
    }
    calibrated.sort_by_key(|row| row.seed);
    pairs.sort_by_key(|row| row.seed);
    let text = report(&calibrated, &pairs, clock.elapsed().as_secs_f64());
    println!("{text}");
}
