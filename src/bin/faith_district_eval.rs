//! Matched evaluation of buying a district the stock controller already chose.
//!
//! A focal turn is first run on a clone and then replayed.  The treatment may
//! replace at most one stock `Produce(District)` action with the exact legal
//! Faith `BuyDistrict` action for the same city, district, and tile.  It does
//! not rank districts or change any earlier Faith-spending decision.
use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, ActionFamilies, Game, GameOptions, Item, VictoryConditions};
use civvis::name::Name;
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use std::collections::BTreeMap;

const NULL_MAPS: usize = 6;
const NULL_SEED: u64 = 9_995_000;
const SCREEN_MAPS: usize = 30;
const SCREEN_SEED: u64 = 9_996_000;
const HOLDOUT_MAPS: usize = 120;
const HOLDOUT_SEED: u64 = 9_997_000;
const Z_95: f64 = 1.959_963_984_540_054;

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

fn district_family(game: &Game, district: Name) -> Name {
    let mut current = district;
    for _ in 0..game.rules.districts.len() {
        let Some(parent) = game
            .rules
            .districts
            .get(&current)
            .and_then(|spec| spec.replaces)
        else {
            break;
        };
        current = parent;
    }
    current
}

fn faith_reserve(strategy: Option<&str>) -> f64 {
    if strategy == Some("religion") {
        250.0
    } else {
        100.0
    }
}

#[derive(Clone, Debug)]
struct DistrictCandidate {
    purchase: Action,
    family: String,
    faith_cost: f64,
    production_exposure: f64,
    reserve_eligible: bool,
}

/// Match a stock district-production decision to the exact ordinary Faith
/// purchase, without adding a treatment-side ranking or changing legality.
fn district_candidate(
    game: &Game,
    pid: usize,
    stock: &Action,
    strategy: Option<&str>,
) -> Option<DistrictCandidate> {
    if strategy == Some("culture") {
        return None;
    }
    let Action::Produce {
        city,
        item: Item::District { district, pos },
    } = stock
    else {
        return None;
    };
    let purchase = game
        .legal_actions_within(pid, ActionFamilies::PURCHASES | ActionFamilies::EMPIRE)
        .into_iter()
        .find(|candidate| {
            matches!(
                candidate,
                Action::BuyDistrict {
                    city: buy_city,
                    district: buy_district,
                    pos: buy_pos,
                    currency,
                } if buy_city == city
                    && buy_district == district
                    && buy_pos == pos
                    && currency == "faith"
            )
        })?;
    let before = game.players[pid].faith;
    let mut after = game.clone();
    after
        .apply(pid, &purchase)
        .expect("an action returned by legal_actions_within must apply to its clone");
    let faith_cost = (before - after.players[pid].faith).max(0.0);
    Some(DistrictCandidate {
        purchase,
        family: district_family(game, *district).to_string(),
        faith_cost,
        production_exposure: game.city_yields(*city).production.max(0.0),
        reserve_eligible: after.players[pid].faith + f64::EPSILON >= faith_reserve(strategy),
    })
}

#[derive(Clone, Debug, Default, PartialEq)]
struct SpendCensus {
    focal_turns: u32,
    stock_district_choices: u32,
    legal_matches: u32,
    reserve_opportunities: u32,
    purchases: u32,
    faith_spent: f64,
    production_exposure: f64,
    families: BTreeMap<String, u32>,
}

/// Run stock on a clone, retain its controller state, and replay its actions.
/// The treatment substitutes only the first reserve-eligible exact district
/// purchase in the stock action order.  A replay failure invalidates the
/// harness instead of silently changing the treatment.
fn replay_focal_turn(
    game: &mut Game,
    ai: &mut AdvancedAi,
    pid: usize,
    treatment: bool,
    census: &mut SpendCensus,
) -> Result<(), String> {
    let mut observed = game.clone();
    let before = observed.log.len();
    let mut actor = ai.clone();
    actor.take_turn(&mut observed, pid);
    let strategy = actor.strategy_label().map(str::to_string);
    let mut actions: Vec<(usize, Action)> = observed.log.since(before).cloned().collect();
    let ended = actions
        .last()
        .is_some_and(|(owner, action)| *owner == pid && matches!(action, Action::EndTurn));
    if ended {
        actions.pop();
    }

    census.focal_turns += 1;
    let mut substituted = false;
    for (owner, action) in actions {
        if owner != pid {
            return Err(format!(
                "stock seat {pid} logged an action for seat {owner}: {action:?}"
            ));
        }
        if matches!(
            action,
            Action::Produce {
                item: Item::District { .. },
                ..
            }
        ) {
            census.stock_district_choices += 1;
            if !substituted {
                if let Some(candidate) = district_candidate(game, pid, &action, strategy.as_deref())
                {
                    census.legal_matches += 1;
                    if candidate.reserve_eligible {
                        census.reserve_opportunities += 1;
                        if treatment {
                            game.apply(pid, &candidate.purchase).map_err(|why| {
                                format!(
                                    "selected Faith district purchase failed for seat {pid}: \
                                     {why}; {:?}",
                                    candidate.purchase
                                )
                            })?;
                            census.purchases += 1;
                            census.faith_spent += candidate.faith_cost;
                            census.production_exposure += candidate.production_exposure;
                            *census.families.entry(candidate.family).or_default() += 1;
                            substituted = true;
                            continue;
                        }
                    }
                }
            }
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

#[derive(Clone, Debug, PartialEq)]
struct GameResult {
    won: bool,
    victory: Option<String>,
    reported_turn: u32,
    score: i64,
    faith: f64,
    cities: usize,
    districts: usize,
    census: SpendCensus,
}

fn play(options: GameOptions, focal: usize, treatment: bool) -> GameResult {
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
    let mut census = SpendCensus::default();

    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        if pid == focal {
            replay_focal_turn(&mut game, &mut ais[pid], pid, treatment, &mut census)
                .unwrap_or_else(|why| panic!("turn {} seat {pid}: {why}", game.turn));
        } else {
            ais[pid].take_turn(&mut game, pid);
        }
        if game.winner.is_none() && game.current == pid {
            game.apply(pid, &Action::EndTurn).unwrap_or_else(|why| {
                panic!("turn {} seat {pid}: EndTurn failed: {why}", game.turn)
            });
        }
    }

    GameResult {
        won: game.winner == Some(focal),
        victory: (game.winner == Some(focal))
            .then(|| game.victory_type.clone())
            .flatten(),
        reported_turn: game.reported_turn(),
        score: game.score(focal),
        faith: game.players[focal].faith,
        cities: game.player_city_ids(focal).len(),
        districts: game
            .cities
            .values()
            .filter(|city| city.owner == focal)
            .map(|city| city.districts.len())
            .sum(),
        census,
    }
}

#[derive(Clone, Debug)]
struct MapResult {
    control: [GameResult; 2],
    treatment: [GameResult; 2],
}

fn map_score(control_wins: usize, treatment_wins: usize) -> f64 {
    0.5 + (treatment_wins as f64 - control_wins as f64) / 4.0
}

fn terminal_share(control: &GameResult, treatment: &GameResult) -> f64 {
    let old = control.score.max(0) as f64;
    let new = treatment.score.max(0) as f64;
    if old + new > 0.0 {
        new / (old + new)
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

fn wilson(mean: f64, n: usize) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let n = n as f64;
    let z2 = Z_95 * Z_95;
    let denominator = 1.0 + z2 / n;
    let center = (mean + z2 / (2.0 * n)) / denominator;
    let radius = Z_95 * ((mean * (1.0 - mean) / n + z2 / (4.0 * n * n)).sqrt()) / denominator;
    ((center - radius).max(0.0), (center + radius).min(1.0))
}

#[derive(Default)]
struct ArmSummary {
    games: usize,
    wins: usize,
    turns: u64,
    score: i64,
    faith: f64,
    cities: usize,
    districts: usize,
    focal_turns: u64,
    stock_district_choices: u64,
    legal_matches: u64,
    reserve_opportunities: u64,
    purchases: u64,
    faith_spent: f64,
    production_exposure: f64,
    fired_games: usize,
    victories: BTreeMap<String, usize>,
    families: BTreeMap<String, u64>,
}

impl ArmSummary {
    fn record(&mut self, result: &GameResult) {
        self.games += 1;
        self.wins += result.won as usize;
        self.turns += result.reported_turn as u64;
        self.score += result.score;
        self.faith += result.faith;
        self.cities += result.cities;
        self.districts += result.districts;
        self.focal_turns += result.census.focal_turns as u64;
        self.stock_district_choices += result.census.stock_district_choices as u64;
        self.legal_matches += result.census.legal_matches as u64;
        self.reserve_opportunities += result.census.reserve_opportunities as u64;
        self.purchases += result.census.purchases as u64;
        self.faith_spent += result.census.faith_spent;
        self.production_exposure += result.census.production_exposure;
        self.fired_games += (result.census.purchases > 0) as usize;
        for (family, count) in &result.census.families {
            *self.families.entry(family.clone()).or_default() += *count as u64;
        }
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
}

#[derive(Clone, Copy)]
struct GateInputs {
    coverage: f64,
    purchases: u64,
    paired_score: f64,
    favorable: usize,
    adverse: usize,
    sign_p: f64,
    terminal_score: f64,
}

fn screen_passes(gate: GateInputs) -> bool {
    gate.coverage >= 0.10
        && gate.purchases >= 10
        && gate.paired_score >= 0.525
        && gate.favorable > gate.adverse
        && gate.terminal_score >= 0.50
}

fn holdout_passes(gate: GateInputs) -> bool {
    gate.coverage >= 0.10
        && gate.purchases >= 40
        && gate.favorable > gate.adverse
        && gate.sign_p < 0.05
        && gate.paired_score > 0.50
        && gate.terminal_score >= 0.50
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
    let turns = number(&args, "--turns", 250).max(1) as u32;
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
    let default_speed = civvis::game::default_speed();
    let speed = text(&args, "--speed", &default_speed);
    let map_name = text(&args, "--map", MapScript::default().id());
    let map_script = MapScript::from_id(&map_name).unwrap_or_else(|| {
        eprintln!("unknown map script {map_name:?}");
        std::process::exit(2);
    });
    let shape_name = text(&args, "--shape", MapTopology::default().id());
    let map_topology = MapTopology::from_id(&shape_name).unwrap_or_else(|| {
        eprintln!("unknown map shape {shape_name:?}");
        std::process::exit(2);
    });
    let poles_name = text(&args, "--poles", MapPoles::default().id());
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
    let frozen_victories = VictoryConditions {
        science: true,
        culture: true,
        religious: false,
        diplomatic: false,
        domination: true,
        score: false,
    };
    if victories != frozen_victories {
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
    println!("Faith district conversion evaluator");
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
        "batch: {maps} independent maps x seats 0/{} x control/treatment = {} games; seed {seed}; {jobs} jobs",
        players - 1,
        maps * 4
    );
    println!(
        "treatment: {}",
        if null_replay {
            "NULL replay (substitution disabled in both arms)"
        } else {
            "replace at most one stock district-production choice with its exact legal Faith purchase"
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
                play(options.clone(), seats[0], false),
                play(options.clone(), seats[1], false),
            ];
            let treatment = [
                play(options.clone(), seats[0], !null_replay),
                play(options, seats[1], !null_replay),
            ];
            MapResult { control, treatment }
        },
        |completed, _| eprintln!("progress: {}/{} maps complete", completed + 1, maps),
    );

    let mut control = ArmSummary::default();
    let mut treatment = ArmSummary::default();
    let mut pair_scores = Vec::with_capacity(maps);
    let mut pair_terminal = Vec::with_capacity(maps);
    let mut favorable = 0usize;
    let mut adverse = 0usize;
    let mut terminal_favorable = 0usize;
    let mut terminal_adverse = 0usize;
    let mut helped_cells = 0usize;
    let mut hurt_cells = 0usize;
    let mut exact_mismatches = 0usize;

    for result in &results {
        let control_wins = result.control.iter().filter(|game| game.won).count();
        let treatment_wins = result.treatment.iter().filter(|game| game.won).count();
        pair_scores.push(map_score(control_wins, treatment_wins));
        match treatment_wins.cmp(&control_wins) {
            std::cmp::Ordering::Greater => favorable += 1,
            std::cmp::Ordering::Less => adverse += 1,
            std::cmp::Ordering::Equal => {}
        }
        let terminal = result
            .control
            .iter()
            .zip(&result.treatment)
            .map(|(old, new)| terminal_share(old, new))
            .sum::<f64>()
            / 2.0;
        pair_terminal.push(terminal);
        if terminal > 0.5 + f64::EPSILON {
            terminal_favorable += 1;
        } else if terminal < 0.5 - f64::EPSILON {
            terminal_adverse += 1;
        }
        for (old, new) in result.control.iter().zip(&result.treatment) {
            control.record(old);
            treatment.record(new);
            match (old.won, new.won) {
                (false, true) => helped_cells += 1,
                (true, false) => hurt_cells += 1,
                _ => {}
            }
            exact_mismatches += (old != new) as usize;
        }
    }

    let paired_score = pair_scores.iter().sum::<f64>() / maps as f64;
    let (paired_low, paired_high) = wilson(paired_score, maps);
    let terminal_score = pair_terminal.iter().sum::<f64>() / maps as f64;
    let sign_p = exact_two_sided(favorable, favorable + adverse);
    let terminal_p = exact_two_sided(terminal_favorable, terminal_favorable + terminal_adverse);
    let coverage = treatment.fired_games as f64 / treatment.games.max(1) as f64;
    let gate = GateInputs {
        coverage,
        purchases: treatment.purchases,
        paired_score,
        favorable,
        adverse,
        sign_p,
        terminal_score,
    };

    println!();
    println!("arm        wins/games  turns  score   faith  cities  districts  choices  legal  eligible  purchases");
    for (name, arm) in [("control", &control), ("treatment", &treatment)] {
        let n = arm.games.max(1) as f64;
        println!(
            "{name:<10} {:>3}/{:<3} {:>6.1} {:>6.1} {:>7.1} {:>7.2} {:>10.2} {:>8} {:>6} {:>9} {:>10}",
            arm.wins,
            arm.games,
            arm.turns as f64 / n,
            arm.score as f64 / n,
            arm.faith / n,
            arm.cities as f64 / n,
            arm.districts as f64 / n,
            arm.stock_district_choices,
            arm.legal_matches,
            arm.reserve_opportunities,
            arm.purchases,
        );
    }
    println!(
        "victory types: control {:?}; treatment {:?}",
        control.victories, treatment.victories
    );
    println!(
        "treatment mechanism: {} focal turns; {}/{} seat-games fired ({:.1}%); {} purchases; families {:?}",
        treatment.focal_turns,
        treatment.fired_games,
        treatment.games,
        100.0 * coverage,
        treatment.purchases,
        treatment.families,
    );
    println!(
        "treatment cost: {:.1} Faith total ({:.1}/purchase); {:.1} base Production exposed total ({:.1}/purchase)",
        treatment.faith_spent,
        treatment.faith_spent / treatment.purchases.max(1) as f64,
        treatment.production_exposure,
        treatment.production_exposure / treatment.purchases.max(1) as f64,
    );
    println!(
        "matched seat cells: treatment helped {helped_cells}, hurt {hurt_cells}, unchanged {} (descriptive; map is the inference unit)",
        control.games - helped_cells - hurt_cells
    );
    println!(
        "paired map win score: {:.1}% (95% conservative Wilson {:.1}%..{:.1}%)",
        100.0 * paired_score,
        100.0 * paired_low,
        100.0 * paired_high,
    );
    println!(
        "win direction: favorable {favorable}, neutral {}, adverse {adverse}; exact two-sided sign p={sign_p:.4}",
        maps - favorable - adverse
    );
    println!(
        "paired terminal-score share: {:.1}%; favorable {terminal_favorable}, neutral {}, adverse {terminal_adverse}; exact p={terminal_p:.4}",
        100.0 * terminal_score,
        maps - terminal_favorable - terminal_adverse,
    );

    if null_replay {
        let exact_profile = players == 8
            && width == 84
            && height == 54
            && city_states == 12
            && turns == 250
            && speed == "online"
            && map_script == MapScript::Continents
            && map_topology == MapTopology::Planet
            && map_poles == MapPoles::Poles
            && randomize_civs
            && maps == NULL_MAPS
            && seed == NULL_SEED;
        if exact_mismatches == 0 && exact_profile {
            println!(
                "null sanity: PASS — all {} matched seat replays reproduced exactly",
                control.games
            );
        } else {
            println!(
                "null sanity: BROKEN — {exact_mismatches}/{} matched seat replays differed{}",
                control.games,
                if exact_profile {
                    ""
                } else {
                    "; profile is not the frozen null"
                }
            );
            std::process::exit(3);
        }
        return;
    }

    let exact_profile = players == 8
        && width == 84
        && height == 54
        && city_states == 12
        && turns == 250
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
                "STOP — at least one preregistered term failed; do not tune or retry"
            }
        );
    } else if exact_profile && maps == HOLDOUT_MAPS && seed == HOLDOUT_SEED {
        println!(
            "holdout gate: {}",
            if holdout_passes(gate) {
                "PASS — a separate gameplay integration PR is permitted"
            } else {
                "RETAIN AdvancedAi — no gameplay integration"
            }
        );
    } else {
        println!(
            "decision: DIAGNOSTIC ONLY — this is neither the preregistered screen nor holdout profile"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use civvis::game::GovernorState;
    use std::collections::BTreeSet;

    fn district_fixture() -> (Game, usize, Action) {
        for seed in 81_100..81_180 {
            let mut game = Game::new_full(1, 24, 16, seed, 120, 0, false);
            let settler = game
                .player_unit_ids(0)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            if game.apply(0, &Action::FoundCity { unit: settler }).is_err() {
                continue;
            }
            let city = game.player_city_ids(0)[0];
            game.turn = 10;
            game.players[0].faith = 10_000.0;
            game.players[0].techs.insert(Name::new("writing"));
            game.players[0].governor_roster.insert(
                "moksha".to_string(),
                GovernorState {
                    city: Some(city),
                    assigned_turn: 0,
                    disabled_until: 0,
                    promotions: BTreeSet::from(["divine_architect".to_string()]),
                },
            );
            if let Some(purchase) = game
                .legal_actions_within(0, ActionFamilies::PURCHASES | ActionFamilies::EMPIRE)
                .into_iter()
                .find(|action| {
                    matches!(
                        action,
                        Action::BuyDistrict { district, currency, .. }
                            if district == "campus" && currency == "faith"
                    )
                })
            {
                let Action::BuyDistrict {
                    city,
                    district,
                    pos,
                    ..
                } = purchase
                else {
                    unreachable!()
                };
                return (
                    game,
                    0,
                    Action::Produce {
                        city,
                        item: Item::District { district, pos },
                    },
                );
            }
        }
        panic!("no deterministic fixture exposed a Faith Campus purchase");
    }

    #[test]
    fn candidate_is_the_exact_stock_district_and_preserves_the_reserve() {
        let (mut game, pid, stock) = district_fixture();
        let candidate = district_candidate(&game, pid, &stock, Some("science")).unwrap();
        assert_eq!(candidate.family, "campus");
        assert!(candidate.reserve_eligible);
        let faith_before = game.players[pid].faith;
        game.apply(pid, &candidate.purchase).unwrap();
        assert!((faith_before - game.players[pid].faith - candidate.faith_cost).abs() < 1e-9);
        let Action::Produce {
            city,
            item: Item::District { district, .. },
        } = stock
        else {
            unreachable!()
        };
        assert!(game.cities[&city].districts.contains_key(&district));

        let (mut scarce, pid, stock) = district_fixture();
        let cost = district_candidate(&scarce, pid, &stock, Some("science"))
            .unwrap()
            .faith_cost;
        scarce.players[pid].faith = cost + 99.0;
        assert!(
            !district_candidate(&scarce, pid, &stock, Some("science"))
                .unwrap()
                .reserve_eligible
        );
        assert!(district_candidate(&scarce, pid, &stock, Some("culture")).is_none());
    }

    #[test]
    fn stock_trace_replay_defers_only_end_turn() {
        let mut game = Game::new(2, 20, 14, 81_991, 20, 0);
        game.set_fog_memory(false);
        game.victory_conditions = VictoryConditions {
            science: true,
            culture: true,
            religious: false,
            diplomatic: false,
            domination: true,
            score: false,
        };
        let mut direct = game.clone();
        let mut direct_ai = AdvancedAi::new();
        direct_ai.take_turn(&mut direct, 0);
        let expected_log: Vec<_> = direct.log.iter().cloned().collect();

        let mut replay_ai = AdvancedAi::new();
        let mut census = SpendCensus::default();
        replay_focal_turn(&mut game, &mut replay_ai, 0, false, &mut census).unwrap();
        assert_eq!(game.current, 0);
        game.apply(0, &Action::EndTurn).unwrap();
        let replayed_log: Vec<_> = game.log.iter().cloned().collect();
        assert_eq!(replayed_log, expected_log);
        assert_eq!(game.turn, direct.turn);
        assert_eq!(game.current, direct.current);
        assert_eq!(game.players[0].gold, direct.players[0].gold);
        assert_eq!(game.players[0].faith, direct.players[0].faith);
        assert_eq!(replay_ai.strategy_label(), direct_ai.strategy_label());
    }

    #[test]
    fn map_statistics_and_gates_match_the_frozen_contract() {
        assert_eq!(map_score(0, 2), 1.0);
        assert_eq!(map_score(0, 1), 0.75);
        assert_eq!(map_score(1, 1), 0.5);
        assert_eq!(map_score(2, 0), 0.0);
        assert!((exact_two_sided(5, 5) - 0.0625).abs() < 1e-12);
        assert!((exact_two_sided(8, 8) - 0.0078125).abs() < 1e-12);

        let passing = GateInputs {
            coverage: 0.10,
            purchases: 40,
            paired_score: 0.525,
            favorable: 8,
            adverse: 2,
            sign_p: 0.049,
            terminal_score: 0.50,
        };
        assert!(screen_passes(passing));
        assert!(holdout_passes(passing));
        assert!(!screen_passes(GateInputs {
            coverage: 0.099,
            ..passing
        }));
        assert!(!holdout_passes(GateInputs {
            purchases: 39,
            ..passing
        }));
        assert!(!holdout_passes(GateInputs {
            paired_score: 0.50,
            ..passing
        }));
    }
}
