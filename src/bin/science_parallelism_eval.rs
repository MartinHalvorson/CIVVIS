//! Matched evaluation of adaptive Science Spaceport parallelism.
//!
//! The treatment changes neither Production nor strategy. On a focal adaptive
//! Science turn after the Moon or Mars milestone, it orders at most one legal
//! Spaceport until the explicit-Science policy's two/three-site target is met.
//! Two focal seats are paired within every map; the map is the inference unit.
use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, Game, GameOptions, Item, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use civvis::Pos;
use std::collections::BTreeMap;

const NULL_MAPS: usize = 4;
const NULL_SEED: u64 = 9_982_000;
const SCREEN_MAPS: usize = 30;
const SCREEN_SEED: u64 = 9_983_000;
const HOLDOUT_MAPS: usize = 120;
const HOLDOUT_SEED: u64 = 9_984_000;

fn number(args: &[String], key: &str, default: i64) -> i64 {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], key: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Milestone {
    Moon,
    Mars,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SiteCandidate {
    production: f64,
    city: u32,
    pos: Pos,
}

fn choose_site(candidates: impl IntoIterator<Item = SiteCandidate>) -> Option<SiteCandidate> {
    candidates.into_iter().fold(None, |best, candidate| {
        if best.is_none_or(|old| {
            candidate.production > old.production
                || (candidate.production == old.production
                    && (candidate.city, candidate.pos) < (old.city, old.pos))
        }) {
            Some(candidate)
        } else {
            best
        }
    })
}

fn desired_spaceports(
    strategy: Option<&str>,
    completed: &std::collections::BTreeSet<String>,
) -> Option<(usize, Milestone)> {
    if strategy != Some("science") {
        return None;
    }
    if completed.contains("launch_mars_colony") {
        Some((3, Milestone::Mars))
    } else if completed.contains("launch_moon_landing") {
        Some((2, Milestone::Moon))
    } else {
        None
    }
}

#[derive(Clone, Debug, PartialEq)]
struct SpaceportOrder {
    milestone: Milestone,
    action: Action,
}

/// Select one actually producible site with the same city/site ordering as the
/// shipped explicit-Science branch. Existing and first-queued sites count once
/// per city, so a queue cannot manufacture fake parallelism.
fn spaceport_order(g: &Game, pid: usize, strategy: Option<&str>) -> Option<SpaceportOrder> {
    if !g.victory_conditions.science {
        return None;
    }
    let (desired, milestone) = desired_spaceports(strategy, &g.players[pid].science_projects)?;
    let city_ids = g.player_city_ids(pid);
    let built = city_ids
        .iter()
        .filter(|city| {
            g.cities[city]
                .districts
                .contains_key(civvis::name!("spaceport"))
        })
        .count();
    let queued = city_ids
        .iter()
        .filter(|city| {
            matches!(
                g.cities[city].queue.first(),
                Some(Item::District { district, .. }) if district == "spaceport"
            )
        })
        .count();
    if built + queued >= desired.min(city_ids.len()) {
        return None;
    }

    let candidates = city_ids.into_iter().flat_map(|city| {
        let ineligible = g.cities[&city]
            .districts
            .contains_key(civvis::name!("spaceport"))
            || matches!(
                g.cities[&city].queue.first(),
                Some(Item::District { district, .. }) if district == "spaceport"
            );
        let production = g.city_yields(city).production;
        g.producible_items(pid, city)
            .into_iter()
            .filter_map(move |item| {
                let Item::District { district, pos } = item else {
                    return None;
                };
                (!ineligible && district == "spaceport").then_some(SiteCandidate {
                    production,
                    city,
                    pos,
                })
            })
    });
    let site = choose_site(candidates)?;
    Some(SpaceportOrder {
        milestone,
        action: Action::Produce {
            city: site.city,
            item: Item::District {
                district: civvis::name!("spaceport"),
                pos: site.pos,
            },
        },
    })
}

/// Run the shipped controller, retain its internal state, and replay every
/// successful action except its final `EndTurn`. This opens a genuine
/// post-policy, pre-turn-boundary treatment point without changing the AI.
fn replay_stock_actions_without_end(
    game: &mut Game,
    ai: &mut AdvancedAi,
    pid: usize,
) -> Result<(), String> {
    let mut observed = game.clone();
    let before = observed.log.len();
    let mut actor = ai.clone();
    actor.take_turn(&mut observed, pid);
    let mut actions: Vec<(usize, Action)> = observed.log.since(before).cloned().collect();
    let ended = actions
        .last()
        .is_some_and(|(owner, action)| *owner == pid && matches!(action, Action::EndTurn));
    if ended {
        actions.pop();
    }
    for (owner, action) in actions {
        if owner != pid {
            return Err(format!(
                "stock seat {pid} logged an action for seat {owner}: {action:?}"
            ));
        }
        game.apply(owner, &action).map_err(|why| {
            format!("stock action replay failed for seat {pid}: {why}; {action:?}")
        })?;
    }
    *ai = actor;
    if ended && game.winner.is_none() && game.current != pid {
        return Err(format!(
            "stock replay advanced from seat {pid} before the deferred EndTurn"
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TreatmentCensus {
    science_plan_turns: u32,
    milestone_turns: u32,
    opportunities: u32,
    orders: u32,
    moon_orders: u32,
    mars_orders: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Control,
    ReplayNull,
    Treatment,
}

#[derive(Clone, Debug, PartialEq)]
struct GameResult {
    won: bool,
    victory: Option<String>,
    reported_turn: u32,
    score: i64,
    science_progress: f64,
    science_projects: usize,
    exoplanet_distance: f64,
    exoplanet_speed: f64,
    lasers: i64,
    built_spaceports: usize,
    queued_spaceports: usize,
    census: TreatmentCensus,
}

fn play(options: GameOptions, focal: usize, mode: Mode) -> GameResult {
    let mut game = Game::new_with(options);
    game.set_fog_memory(false);
    game.victory_conditions = VictoryConditions {
        science: true,
        culture: true,
        religious: false,
        diplomatic: false,
        domination: true,
        score: false,
    };
    let mut ais = AdvancedAi::fleet(&game);
    let mut census = TreatmentCensus::default();

    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        if pid == focal && mode != Mode::Control {
            replay_stock_actions_without_end(&mut game, &mut ais[pid], pid)
                .unwrap_or_else(|why| panic!("turn {} seat {pid}: {why}", game.turn));
        } else {
            ais[pid].take_turn(&mut game, pid);
        }

        if mode == Mode::Treatment && pid == focal && game.winner.is_none() && game.current == pid {
            let strategy = ais[pid].strategy_label();
            census.science_plan_turns += (strategy == Some("science")) as u32;
            if desired_spaceports(strategy, &game.players[pid].science_projects).is_some() {
                census.milestone_turns += 1;
            }
            if let Some(order) = spaceport_order(&game, pid, strategy) {
                census.opportunities += 1;
                game.apply(pid, &order.action).unwrap_or_else(|why| {
                    panic!(
                        "turn {} seat {pid}: selected Spaceport order became illegal: {why}; {:?}",
                        game.turn, order.action
                    )
                });
                census.orders += 1;
                match order.milestone {
                    Milestone::Moon => census.moon_orders += 1,
                    Milestone::Mars => census.mars_orders += 1,
                }
            }
        }
        if mode != Mode::Control && game.winner.is_none() && game.current == pid {
            game.apply(pid, &Action::EndTurn).unwrap_or_else(|why| {
                panic!("turn {} seat {pid}: EndTurn failed: {why}", game.turn)
            });
        }
    }

    let city_ids = game.player_city_ids(focal);
    let built_spaceports = city_ids
        .iter()
        .filter(|city| {
            game.cities[city]
                .districts
                .contains_key(civvis::name!("spaceport"))
        })
        .count();
    let queued_spaceports = city_ids
        .iter()
        .filter(|city| {
            matches!(
                game.cities[city].queue.first(),
                Some(Item::District { district, .. }) if district == "spaceport"
            )
        })
        .count();
    let player = &game.players[focal];
    let lasers = player
        .counters
        .get("project:lagrange_laser_station")
        .copied()
        .unwrap_or(0)
        + player
            .counters
            .get("project:terrestrial_laser_station")
            .copied()
            .unwrap_or(0);
    let race = game.victory_races(focal, 0);

    GameResult {
        won: game.winner == Some(focal),
        victory: (game.winner == Some(focal))
            .then(|| game.victory_type.clone())
            .flatten(),
        reported_turn: game.reported_turn(),
        score: game.score(focal),
        science_progress: race.science,
        science_projects: race.science_projects,
        exoplanet_distance: player.exoplanet_distance,
        exoplanet_speed: game.exoplanet_speed(focal),
        lasers,
        built_spaceports,
        queued_spaceports,
        census,
    }
}

#[derive(Clone, Debug)]
struct MapResult {
    control: [GameResult; 2],
    comparison: [GameResult; 2],
}

fn map_win_score(control_wins: usize, treatment_wins: usize) -> f64 {
    0.5 + (treatment_wins as f64 - control_wins as f64) / 4.0
}

fn terminal_share(control: i64, treatment: i64) -> f64 {
    let control = control.max(0) as f64;
    let treatment = treatment.max(0) as f64;
    let total = control + treatment;
    if total > 0.0 {
        treatment / total
    } else {
        0.5
    }
}

fn exact_two_sided(hits: usize, n: usize) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let extreme = hits.min(n - hits);
    let mut coefficient = 1.0_f64;
    let mut tail = 0.0_f64;
    for k in 0..=n {
        if k > 0 {
            coefficient *= (n - k + 1) as f64 / k as f64;
        }
        if k <= extreme || k >= n - extreme {
            tail += coefficient;
        }
    }
    (tail / 2f64.powi(n as i32)).min(1.0)
}

#[derive(Default)]
struct ArmSummary {
    games: usize,
    wins: usize,
    science_wins: usize,
    turns: u64,
    score: i64,
    science_progress: f64,
    science_projects: usize,
    exoplanet_distance: f64,
    exoplanet_speed: f64,
    lasers: i64,
    built_spaceports: usize,
    queued_spaceports: usize,
    science_plan_turns: u64,
    milestone_turns: u64,
    opportunities: u64,
    orders: u64,
    moon_orders: u64,
    mars_orders: u64,
    fired_games: usize,
    victories: BTreeMap<String, usize>,
}

impl ArmSummary {
    fn record(&mut self, result: &GameResult) {
        self.games += 1;
        self.wins += result.won as usize;
        self.science_wins += (result.victory.as_deref() == Some("science")) as usize;
        self.turns += result.reported_turn as u64;
        self.score += result.score;
        self.science_progress += result.science_progress;
        self.science_projects += result.science_projects;
        self.exoplanet_distance += result.exoplanet_distance;
        self.exoplanet_speed += result.exoplanet_speed;
        self.lasers += result.lasers;
        self.built_spaceports += result.built_spaceports;
        self.queued_spaceports += result.queued_spaceports;
        self.science_plan_turns += result.census.science_plan_turns as u64;
        self.milestone_turns += result.census.milestone_turns as u64;
        self.opportunities += result.census.opportunities as u64;
        self.orders += result.census.orders as u64;
        self.moon_orders += result.census.moon_orders as u64;
        self.mars_orders += result.census.mars_orders as u64;
        self.fired_games += (result.census.orders > 0) as usize;
        if result.won {
            *self
                .victories
                .entry(
                    result
                        .victory
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                )
                .or_default() += 1;
        }
    }

    fn spaceports(&self) -> usize {
        self.built_spaceports + self.queued_spaceports
    }
}

#[derive(Clone, Copy)]
struct GateInputs {
    coverage: f64,
    orders: u64,
    treatment_spaceports: usize,
    control_spaceports: usize,
    treatment_lasers: i64,
    control_lasers: i64,
    science_delta: f64,
    science_favorable: usize,
    science_adverse: usize,
    science_p: f64,
    treatment_science_wins: usize,
    control_science_wins: usize,
    treatment_wins: usize,
    control_wins: usize,
    paired_win_score: f64,
    terminal_score_share: f64,
}

fn mechanism_passes(gate: GateInputs) -> bool {
    gate.coverage >= 0.10
        && gate.orders >= 10
        && gate.treatment_spaceports > gate.control_spaceports
        && gate.treatment_lasers > gate.control_lasers
}

fn outcome_guards_pass(gate: GateInputs, score_floor: f64) -> bool {
    gate.treatment_science_wins >= gate.control_science_wins
        && gate.treatment_wins >= gate.control_wins
        && gate.paired_win_score >= 0.50
        && gate.terminal_score_share >= score_floor
}

fn screen_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate)
        && gate.science_delta >= 0.5
        && gate.science_favorable > gate.science_adverse
        && gate.science_p <= 0.20
        && outcome_guards_pass(gate, 0.495)
}

fn holdout_passes(gate: GateInputs) -> bool {
    mechanism_passes(gate)
        && gate.science_delta > 0.0
        && gate.science_favorable > gate.science_adverse
        && gate.science_p < 0.05
        && outcome_guards_pass(gate, 0.50)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let null_replay = args.iter().any(|arg| arg == "--null");
    let maps = number(
        &args,
        "--maps",
        if null_replay {
            NULL_MAPS as i64
        } else {
            SCREEN_MAPS as i64
        },
    )
    .max(1) as usize;
    let players = number(&args, "--players", 8).max(2) as usize;
    let width = number(&args, "--width", 84).max(8) as i32;
    let height = number(&args, "--height", 54).max(8) as i32;
    let city_states = number(&args, "--city-states", 12).max(0) as usize;
    let turns = number(&args, "--turns", 320).max(1) as u32;
    let seed = number(
        &args,
        "--seed",
        if null_replay {
            NULL_SEED as i64
        } else {
            SCREEN_SEED as i64
        },
    )
    .max(0) as u64;
    let jobs = match number(&args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    };
    let speed = text(&args, "--speed", "online");
    let map_name = text(&args, "--map", "continents");
    let map_script = MapScript::from_id(&map_name).unwrap_or_else(|| {
        eprintln!("unknown map script {map_name:?}");
        std::process::exit(2);
    });
    let shape_name = text(&args, "--shape", "planet");
    let map_topology = MapTopology::from_id(&shape_name).unwrap_or_else(|| {
        eprintln!("unknown map shape {shape_name:?}");
        std::process::exit(2);
    });
    let poles_name = text(&args, "--poles", "poles");
    let map_poles = MapPoles::from_id(&poles_name).unwrap_or_else(|| {
        eprintln!("unknown thermal distribution {poles_name:?}");
        std::process::exit(2);
    });
    let victory_names = text(&args, "--victories", "science,culture,domination");
    let victories = VictoryConditions::parse(&victory_names).unwrap_or_else(|why| {
        eprintln!(
            "--victories: {why}; choose from {:?}",
            VictoryConditions::NAMES
        );
        std::process::exit(2);
    });
    let expected_victories = VictoryConditions {
        science: true,
        culture: true,
        religious: false,
        diplomatic: false,
        domination: true,
        score: false,
    };
    if victories != expected_victories {
        eprintln!(
            "this treatment is defined only for science,culture,domination; got {victory_names:?}"
        );
        std::process::exit(2);
    }
    let randomize_civs = args.iter().any(|arg| arg == "--randomize-civs");
    let rules = Rules::embedded();
    if !rules.speeds.contains_key(&speed) {
        eprintln!("unknown game speed {speed:?}");
        std::process::exit(2);
    }

    let stored_dimensions = MapSize::from_dimensions(width, height)
        .map(|size| size.dimensions(map_topology))
        .unwrap_or((width, height));
    println!("Adaptive Science Spaceport parallelism evaluator");
    println!(
        "profile: {players}p requested {width}x{height}, stored {}x{}, {city_states} city-states, \
         {turns} {speed} turns, map {}, shape {}, poles {}, civilizations {}, victories {}",
        stored_dimensions.0,
        stored_dimensions.1,
        map_script.id(),
        map_topology.id(),
        map_poles.id(),
        if randomize_civs {
            "randomized"
        } else {
            "fixed"
        },
        VictoryConditions::NAMES
            .into_iter()
            .filter(|name| victories.is_enabled(name))
            .collect::<Vec<_>>()
            .join(","),
    );
    println!(
        "batch: {maps} independent maps x seats 0/{} x control/comparison = {} games; seed {seed}; {jobs} jobs",
        players - 1,
        maps * 4
    );
    println!(
        "comparison: {}",
        if null_replay {
            "NULL action-log replay with no added order"
        } else {
            "adaptive Science plan uses the explicit target's 2/3-Spaceport schedule"
        }
    );

    let results: Vec<MapResult> = civvis::parallel::map_reporting(
        maps,
        jobs,
        |map| {
            let options = GameOptions {
                speed: speed.clone(),
                map_script,
                map_topology,
                map_poles,
                randomize_civs,
                ..GameOptions::new(
                    players,
                    width,
                    height,
                    seed + map as u64,
                    turns,
                    city_states,
                )
            };
            let seats = [0, players - 1];
            let control = [
                play(options.clone(), seats[0], Mode::Control),
                play(options.clone(), seats[1], Mode::Control),
            ];
            let comparison_mode = if null_replay {
                Mode::ReplayNull
            } else {
                Mode::Treatment
            };
            let comparison = [
                play(options.clone(), seats[0], comparison_mode),
                play(options, seats[1], comparison_mode),
            ];
            MapResult {
                control,
                comparison,
            }
        },
        |completed, _| eprintln!("progress: {}/{} maps complete", completed + 1, maps),
    );

    let mut control = ArmSummary::default();
    let mut comparison = ArmSummary::default();
    let mut paired_win_scores = Vec::with_capacity(maps);
    let mut paired_terminal_scores = Vec::with_capacity(maps);
    let mut science_deltas = Vec::with_capacity(maps);
    let mut science_favorable = 0usize;
    let mut science_adverse = 0usize;
    let mut win_favorable = 0usize;
    let mut win_adverse = 0usize;
    let mut helped_cells = 0usize;
    let mut hurt_cells = 0usize;
    let mut exact_mismatches = 0usize;

    for result in &results {
        let control_wins = result.control.iter().filter(|game| game.won).count();
        let comparison_wins = result.comparison.iter().filter(|game| game.won).count();
        paired_win_scores.push(map_win_score(control_wins, comparison_wins));
        match comparison_wins.cmp(&control_wins) {
            std::cmp::Ordering::Greater => win_favorable += 1,
            std::cmp::Ordering::Less => win_adverse += 1,
            std::cmp::Ordering::Equal => {}
        }

        paired_terminal_scores.push(
            result
                .control
                .iter()
                .zip(&result.comparison)
                .map(|(old, new)| terminal_share(old.score, new.score))
                .sum::<f64>()
                / 2.0,
        );
        let science_delta = result
            .control
            .iter()
            .zip(&result.comparison)
            .map(|(old, new)| new.science_progress - old.science_progress)
            .sum::<f64>()
            / 2.0;
        science_deltas.push(science_delta);
        if science_delta > 1e-9 {
            science_favorable += 1;
        } else if science_delta < -1e-9 {
            science_adverse += 1;
        }

        for (old, new) in result.control.iter().zip(&result.comparison) {
            control.record(old);
            comparison.record(new);
            match (old.won, new.won) {
                (false, true) => helped_cells += 1,
                (true, false) => hurt_cells += 1,
                _ => {}
            }
            exact_mismatches += (old != new) as usize;
        }
    }

    let paired_win_score = paired_win_scores.iter().sum::<f64>() / maps as f64;
    let terminal_score_share = paired_terminal_scores.iter().sum::<f64>() / maps as f64;
    let science_delta = science_deltas.iter().sum::<f64>() / maps as f64;
    let science_p = exact_two_sided(science_favorable, science_favorable + science_adverse);
    let win_p = exact_two_sided(win_favorable, win_favorable + win_adverse);
    let coverage = comparison.fired_games as f64 / comparison.games.max(1) as f64;
    let gate = GateInputs {
        coverage,
        orders: comparison.orders,
        treatment_spaceports: comparison.spaceports(),
        control_spaceports: control.spaceports(),
        treatment_lasers: comparison.lasers,
        control_lasers: control.lasers,
        science_delta,
        science_favorable,
        science_adverse,
        science_p,
        treatment_science_wins: comparison.science_wins,
        control_science_wins: control.science_wins,
        treatment_wins: comparison.wins,
        control_wins: control.wins,
        paired_win_score,
        terminal_score_share,
    };

    println!();
    println!(
        "arm         wins  sci-wins  turns  score  science  projects  distance  speed  lasers  spaceports"
    );
    for (name, arm) in [("control", &control), ("comparison", &comparison)] {
        let n = arm.games.max(1) as f64;
        println!(
            "{name:<11} {:>3}/{:<3} {:>3}/{:<3} {:>6.1} {:>6.1} {:>8.2} {:>9.2} {:>9.2} {:>6.2} {:>7} {:>5}+{:<5}",
            arm.wins,
            arm.games,
            arm.science_wins,
            arm.games,
            arm.turns as f64 / n,
            arm.score as f64 / n,
            arm.science_progress / n,
            arm.science_projects as f64 / n,
            arm.exoplanet_distance / n,
            arm.exoplanet_speed / n,
            arm.lasers,
            arm.built_spaceports,
            arm.queued_spaceports,
        );
    }
    println!(
        "victory types: control {:?}; comparison {:?}",
        control.victories, comparison.victories
    );
    println!(
        "treatment mechanism: {}/{} focal games fired ({:.1}%); {} opportunities, {} successful orders ({} after Moon, {} after Mars); {} Science-plan turns, {} milestone turns",
        comparison.fired_games,
        comparison.games,
        100.0 * coverage,
        comparison.opportunities,
        comparison.orders,
        comparison.moon_orders,
        comparison.mars_orders,
        comparison.science_plan_turns,
        comparison.milestone_turns,
    );
    println!(
        "matched seat cells: helped {helped_cells}, hurt {hurt_cells}, unchanged {} (descriptive; map is the inference unit)",
        control.games - helped_cells - hurt_cells
    );
    println!(
        "paired map win score: {:.1}%; favorable {win_favorable}, neutral {}, adverse {win_adverse}; exact p={win_p:.4}",
        100.0 * paired_win_score,
        maps - win_favorable - win_adverse,
    );
    println!(
        "primary Science delta: {science_delta:+.3} points/map; favorable {science_favorable}, neutral {}, adverse {science_adverse}; exact p={science_p:.4}",
        maps - science_favorable - science_adverse,
    );
    println!(
        "paired terminal-score share: {:.2}%",
        100.0 * terminal_score_share
    );

    if null_replay {
        if exact_mismatches == 0 {
            println!(
                "null sanity: PASS — all {} direct/replay matched focal cells reproduced exactly",
                control.games
            );
        } else {
            println!(
                "null sanity: BROKEN — {exact_mismatches}/{} direct/replay cells differed",
                control.games
            );
            std::process::exit(3);
        }
        return;
    }

    let exact_profile = players == 8
        && width == 84
        && height == 54
        && city_states == 12
        && turns == 320
        && speed == "online"
        && map_script == MapScript::Continents
        && map_topology == MapTopology::Planet
        && map_poles == MapPoles::Poles
        && randomize_civs;
    if exact_profile && maps == SCREEN_MAPS && seed == SCREEN_SEED {
        println!(
            "development gate: {}",
            if screen_passes(gate) {
                "PASS — run only the fixed disjoint holdout"
            } else {
                "STOP — retain AdvancedAi; do not tune, retry, or inspect the holdout"
            }
        );
    } else if exact_profile && maps == HOLDOUT_MAPS && seed == HOLDOUT_SEED {
        println!(
            "holdout gate: {}",
            if holdout_passes(gate) {
                "PASS — a separate gameplay-integration PR is permitted"
            } else {
                "RETAIN AdvancedAi — no gameplay integration"
            }
        );
    } else {
        println!("decision: DIAGNOSTIC ONLY — not a preregistered treatment batch");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn science_fixture() -> Game {
        let mut game = Game::new_full(1, 34, 20, 79_200, 320, 0, false);
        game.victory_conditions = VictoryConditions {
            science: true,
            culture: true,
            religious: false,
            diplomatic: false,
            domination: true,
            score: false,
        };
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let cities = game.player_city_ids(0);
        game.players[0].techs = game.rules.techs.keys().cloned().collect();
        game.players[0].civics = game.rules.civics.keys().cloned().collect();
        game.players[0].science_projects.extend([
            "launch_earth_satellite".to_string(),
            "launch_moon_landing".to_string(),
        ]);
        for city in &cities {
            game.cities.get_mut(city).unwrap().pop = 12;
            for position in game.cities[city].owned_tiles.clone() {
                if position == game.cities[city].pos {
                    continue;
                }
                let tile = game.map.tiles.get_mut(&position).unwrap();
                tile.terrain = civvis::name!("plains");
                tile.feature = None;
                tile.hills = false;
                tile.resource = None;
                tile.improvement = None;
                tile.district = None;
                tile.district_foundation = None;
                tile.wonder = None;
            }
        }
        game
    }

    #[test]
    fn site_choice_prefers_production_then_stable_low_coordinates() {
        let chosen = choose_site([
            SiteCandidate {
                production: 12.0,
                city: 8,
                pos: (3, 3),
            },
            SiteCandidate {
                production: 15.0,
                city: 9,
                pos: (4, 4),
            },
            SiteCandidate {
                production: 15.0,
                city: 2,
                pos: (5, 5),
            },
            SiteCandidate {
                production: 15.0,
                city: 2,
                pos: (1, 1),
            },
        ])
        .unwrap();
        assert_eq!(chosen.city, 2);
        assert_eq!(chosen.pos, (1, 1));
    }

    #[test]
    fn adaptive_schedule_is_science_only_and_orders_one_legal_site_at_a_time() {
        let mut projects = std::collections::BTreeSet::new();
        assert_eq!(desired_spaceports(Some("science"), &projects), None);
        projects.insert("launch_moon_landing".to_string());
        assert_eq!(
            desired_spaceports(Some("culture"), &projects),
            None,
            "a non-Science plan must never receive the treatment"
        );
        assert_eq!(
            desired_spaceports(Some("science"), &projects),
            Some((2, Milestone::Moon))
        );
        projects.insert("launch_mars_colony".to_string());
        assert_eq!(
            desired_spaceports(Some("science"), &projects),
            Some((3, Milestone::Mars))
        );

        let mut game = science_fixture();
        assert!(spaceport_order(&game, 0, Some("culture")).is_none());
        let moon = spaceport_order(&game, 0, Some("science")).unwrap();
        assert_eq!(moon.milestone, Milestone::Moon);
        game.apply(0, &moon.action).unwrap();
        assert!(spaceport_order(&game, 0, Some("science")).is_none());
    }

    #[test]
    fn replay_defers_the_stock_end_turn() {
        let mut game = Game::new(2, 20, 14, 79_201, 20, 0);
        game.set_fog_memory(false);
        let mut ais = AdvancedAi::fleet(&game);
        replay_stock_actions_without_end(&mut game, &mut ais[0], 0).unwrap();
        assert_eq!(game.current, 0);
        game.apply(0, &Action::EndTurn).unwrap();
        assert_eq!(game.current, 1);
    }

    #[test]
    fn frozen_gates_require_mechanism_progress_and_harm_guards() {
        let passing = GateInputs {
            coverage: 0.20,
            orders: 12,
            treatment_spaceports: 20,
            control_spaceports: 10,
            treatment_lasers: 5,
            control_lasers: 2,
            science_delta: 1.0,
            science_favorable: 8,
            science_adverse: 2,
            science_p: 0.109,
            treatment_science_wins: 3,
            control_science_wins: 2,
            treatment_wins: 4,
            control_wins: 3,
            paired_win_score: 0.52,
            terminal_score_share: 0.501,
        };
        assert!(screen_passes(passing));
        assert!(holdout_passes(GateInputs {
            science_p: 0.01,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_lasers: 2,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_wins: 2,
            ..passing
        }));
    }
}
