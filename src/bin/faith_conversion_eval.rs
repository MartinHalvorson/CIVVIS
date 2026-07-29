//! Matched evaluation of converting surplus Faith into Culture assets.
//!
//! The treatment changes no income and invents no action.  On a focal
//! `AdvancedAi` turn whose reported plan is not Culture, it executes at most
//! one legal Naturalist/Rock Band Faith purchase after the stock policy has
//! finished.  Every map is replayed from seats 0 and N-1 with and without the
//! treatment; inference is aggregated by map rather than pretending the two
//! starts are independent.
use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, ActionFamilies, Game, GameOptions, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use std::collections::BTreeMap;

const SCREEN_MAPS: usize = 30;
const SCREEN_SEED: u64 = 9_980_000;
const HOLDOUT_MAPS: usize = 120;
const HOLDOUT_SEED: u64 = 9_981_000;
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CultureAsset {
    Naturalist,
    RockBand,
}

/// Select the same priority order as `AdvancedAi::culture_spending`, from
/// already-legal actions.  Keeping this pure makes the policy's one choice
/// independently testable without manufacturing a late-game fixture.
fn select_culture_purchase(
    actions: impl IntoIterator<Item = Action>,
    has_park_site: bool,
    active_naturalists: usize,
    active_bands: usize,
) -> Option<(CultureAsset, Action)> {
    let mut naturalist = None;
    let mut band = None;
    for action in actions {
        let Action::Buy { unit, currency, .. } = &action else {
            continue;
        };
        if currency != "faith" {
            continue;
        }
        if unit == "naturalist" && naturalist.is_none() {
            naturalist = Some((CultureAsset::Naturalist, action));
        } else if unit == "rock_band" && band.is_none() {
            band = Some((CultureAsset::RockBand, action));
        }
    }
    if has_park_site && active_naturalists == 0 {
        if let Some(candidate) = naturalist {
            return Some(candidate);
        }
    }
    (active_bands < 2).then_some(band).flatten()
}

fn culture_purchase(g: &Game, pid: usize) -> Option<(CultureAsset, Action)> {
    if g.victory_conditions.religious || !g.victory_conditions.culture {
        return None;
    }
    let active_naturalists = g
        .units
        .values()
        .filter(|unit| unit.owner == pid && unit.kind == "naturalist")
        .count();
    let active_bands = g
        .units
        .values()
        .filter(|unit| unit.owner == pid && unit.kind == "rock_band")
        .count();
    select_culture_purchase(
        g.legal_actions_within(pid, ActionFamilies::PURCHASES | ActionFamilies::EMPIRE),
        !g.national_park_sites(pid).is_empty(),
        active_naturalists,
        active_bands,
    )
}

/// Run the shipped controller, retain its internal state, and replay every
/// successful action except its final `EndTurn`. `AdvancedAi::take_turn`
/// normally advances the game itself, so this action-log splice is what makes
/// a genuine post-policy, pre-turn-boundary treatment possible without
/// changing the controller under test. The engine guarantees that replaying
/// its action log reconstructs the same game state.
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
struct SpendCensus {
    nonculture_turns: u32,
    opportunities: u32,
    purchases: u32,
    naturalists: u32,
    rock_bands: u32,
}

#[derive(Clone, Debug, PartialEq)]
struct GameResult {
    won: bool,
    victory: Option<String>,
    reported_turn: u32,
    score: i64,
    faith: f64,
    tourists: i64,
    cities: usize,
    census: SpendCensus,
}

/// Play one focal seat. Every controller starts from the same stock state in
/// each replay. The purchase runs after the normal AI turn so Culture-plan
/// turns stay byte-for-byte stock and a treatment unit cannot act early.
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
        if treatment && pid == focal {
            replay_stock_actions_without_end(&mut game, &mut ais[pid], pid)
                .unwrap_or_else(|why| panic!("turn {} seat {pid}: {why}", game.turn));
        } else {
            ais[pid].take_turn(&mut game, pid);
        }
        if pid == focal
            && game.winner.is_none()
            && game.current == pid
            && ais[pid]
                .strategy_label()
                .is_some_and(|strategy| strategy != "culture")
        {
            census.nonculture_turns += 1;
            if let Some((asset, action)) = culture_purchase(&game, pid) {
                census.opportunities += 1;
                if treatment && game.apply(pid, &action).is_ok() {
                    census.purchases += 1;
                    match asset {
                        CultureAsset::Naturalist => census.naturalists += 1,
                        CultureAsset::RockBand => census.rock_bands += 1,
                    }
                }
            }
        }
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
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
        tourists: game.foreign_tourists(focal),
        cities: game.player_city_ids(focal).len(),
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
    let control_score = control.score.max(0) as f64;
    let treatment_score = treatment.score.max(0) as f64;
    let total = control_score + treatment_score;
    if total > 0.0 {
        treatment_score / total
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
    tourists: i64,
    cities: usize,
    nonculture_turns: u64,
    opportunities: u64,
    purchases: u64,
    naturalists: u64,
    rock_bands: u64,
    fired_games: usize,
    victories: BTreeMap<String, usize>,
}

impl ArmSummary {
    fn record(&mut self, result: &GameResult) {
        self.games += 1;
        self.wins += result.won as usize;
        self.turns += result.reported_turn as u64;
        self.score += result.score;
        self.faith += result.faith;
        self.tourists += result.tourists;
        self.cities += result.cities;
        self.nonculture_turns += result.census.nonculture_turns as u64;
        self.opportunities += result.census.opportunities as u64;
        self.purchases += result.census.purchases as u64;
        self.naturalists += result.census.naturalists as u64;
        self.rock_bands += result.census.rock_bands as u64;
        self.fired_games += (result.census.purchases > 0) as usize;
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
    treatment_culture_wins: usize,
    control_culture_wins: usize,
}

fn screen_passes(gate: GateInputs) -> bool {
    gate.coverage >= 0.10
        && gate.purchases >= 10
        && gate.paired_score >= 0.525
        && gate.favorable > gate.adverse
        && gate.terminal_score >= 0.50
        && gate.treatment_culture_wins >= gate.control_culture_wins
}

fn holdout_passes(gate: GateInputs) -> bool {
    gate.coverage >= 0.10
        && gate.purchases >= 10
        && gate.favorable > gate.adverse
        && gate.sign_p < 0.05
        && gate.terminal_score >= 0.50
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let maps = number(&args, "--maps", SCREEN_MAPS as i64).max(1) as usize;
    let players = number(&args, "--players", 8).max(2) as usize;
    let width = number(&args, "--width", 84).max(8) as i32;
    let height = number(&args, "--height", 54).max(8) as i32;
    let city_states = number(&args, "--city-states", 12).max(0) as usize;
    let turns = number(&args, "--turns", 250).max(1) as u32;
    let seed = number(&args, "--seed", SCREEN_SEED as i64).max(0) as u64;
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
    if victories
        != (VictoryConditions {
            science: true,
            culture: true,
            religious: false,
            diplomatic: false,
            domination: true,
            score: false,
        })
    {
        eprintln!(
            "this treatment is defined only for science,culture,domination; got {victory_names:?}"
        );
        std::process::exit(2);
    }
    let randomize_civs = args.iter().any(|arg| arg == "--randomize-civs");
    let null_replay = args.iter().any(|arg| arg == "--null");
    let rules = Rules::embedded();
    if !rules.speeds.contains_key(&speed) {
        eprintln!("unknown game speed {speed:?}");
        std::process::exit(2);
    }

    let stored_dimensions = MapSize::from_dimensions(width, height)
        .map(|size| size.dimensions(map_topology))
        .unwrap_or((width, height));
    println!("Surplus Faith conversion evaluator");
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
            "NULL replay (stock AdvancedAi in both arms)"
        } else {
            "after a non-Culture AdvancedAi turn, buy at most one legal Naturalist/Rock Band"
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
    let control_culture = control.victories.get("culture").copied().unwrap_or(0);
    let treatment_culture = treatment.victories.get("culture").copied().unwrap_or(0);
    let gate = GateInputs {
        coverage,
        purchases: treatment.purchases,
        paired_score,
        favorable,
        adverse,
        sign_p,
        terminal_score,
        treatment_culture_wins: treatment_culture,
        control_culture_wins: control_culture,
    };

    println!();
    println!(
        "arm        wins/games  turns  score  faith  tourists  cities  opportunities  purchases"
    );
    for (name, arm) in [("control", &control), ("treatment", &treatment)] {
        let n = arm.games.max(1) as f64;
        println!(
            "{name:<10} {:>3}/{:<3} {:>6.1} {:>6.1} {:>6.1} {:>9.1} {:>7.2} {:>14} {:>10}",
            arm.wins,
            arm.games,
            arm.turns as f64 / n,
            arm.score as f64 / n,
            arm.faith / n,
            arm.tourists as f64 / n,
            arm.cities as f64 / n,
            arm.opportunities,
            arm.purchases,
        );
    }
    println!(
        "victory types: control {:?}; treatment {:?}",
        control.victories, treatment.victories
    );
    println!(
        "treatment mechanism: {}/{} seat-games fired ({:.1}%); {} purchases = {} naturalists + {} rock bands; {} non-Culture focal turns",
        treatment.fired_games,
        treatment.games,
        100.0 * coverage,
        treatment.purchases,
        treatment.naturalists,
        treatment.rock_bands,
        treatment.nonculture_turns,
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
        if exact_mismatches == 0 {
            println!(
                "null sanity: PASS — all {} matched seat replays reproduced exactly",
                control.games
            );
        } else {
            println!(
                "null sanity: BROKEN — {exact_mismatches}/{} matched seat replays differed",
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
    use civvis::name::Name;

    fn buy(unit: &str, currency: &str, city: u32) -> Action {
        Action::Buy {
            city,
            unit: Name::new(unit),
            formation: 0,
            currency: currency.to_string(),
        }
    }

    #[test]
    fn culture_purchase_matches_the_shipped_priority_and_caps() {
        let actions = vec![
            buy("rock_band", "faith", 2),
            buy("naturalist", "faith", 1),
            buy("naturalist", "gold", 3),
        ];
        let (asset, _) = select_culture_purchase(actions.clone(), true, 0, 0).unwrap();
        assert_eq!(asset, CultureAsset::Naturalist);
        let (asset, _) = select_culture_purchase(actions.clone(), true, 1, 0).unwrap();
        assert_eq!(asset, CultureAsset::RockBand);
        let (asset, _) = select_culture_purchase(actions.clone(), false, 0, 0).unwrap();
        assert_eq!(asset, CultureAsset::RockBand);
        assert!(select_culture_purchase(actions, true, 1, 2).is_none());
    }

    #[test]
    fn stock_action_replay_opens_a_real_post_policy_purchase_window() {
        let mut game = Game::new(2, 20, 14, 79_001, 20, 0);
        game.set_fog_memory(false);
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
        game.players[0].civics.insert(Name::new("cold_war"));
        game.players[0].faith = 10_000.0;
        let mut ai = AdvancedAi::new();
        let turn = game.turn;

        replay_stock_actions_without_end(&mut game, &mut ai, 0).unwrap();

        assert_eq!(game.current, 0, "the stock EndTurn must remain deferred");
        assert_eq!(game.turn, turn);
        let faith_before = game.players[0].faith;
        let (asset, purchase) = culture_purchase(&game, 0).unwrap();
        assert_eq!(asset, CultureAsset::RockBand);
        game.apply(0, &purchase).unwrap();
        assert!(game.players[0].faith < faith_before);
        assert!(game
            .units
            .values()
            .any(|unit| unit.owner == 0 && unit.kind == "rock_band"));
        game.apply(0, &Action::EndTurn).unwrap();
        assert_ne!(game.current, 0);
    }

    #[test]
    fn map_score_keeps_two_seats_inside_one_independent_observation() {
        assert_eq!(map_score(0, 2), 1.0);
        assert_eq!(map_score(0, 1), 0.75);
        assert_eq!(map_score(1, 1), 0.5);
        assert_eq!(map_score(2, 0), 0.0);
    }

    #[test]
    fn exact_sign_test_matches_known_edges() {
        assert!((exact_two_sided(5, 5) - 0.0625).abs() < 1e-12);
        assert!((exact_two_sided(8, 8) - 0.0078125).abs() < 1e-12);
        assert_eq!(exact_two_sided(0, 0), 1.0);
    }

    #[test]
    fn screen_and_holdout_gates_enforce_the_preregistered_terms() {
        let passing = GateInputs {
            coverage: 0.20,
            purchases: 10,
            paired_score: 0.525,
            favorable: 8,
            adverse: 2,
            sign_p: 0.05,
            terminal_score: 0.50,
            treatment_culture_wins: 2,
            control_culture_wins: 2,
        };
        assert!(screen_passes(passing));
        assert!(!holdout_passes(passing));
        assert!(holdout_passes(GateInputs {
            sign_p: 0.049,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            coverage: 0.099,
            ..passing
        }));
        assert!(!screen_passes(GateInputs {
            treatment_culture_wins: 1,
            ..passing
        }));
    }
}
