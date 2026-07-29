//! Measure right-censoring at the production spectator's nominal turn limit.
//!
//! Production keeps `Game::max_turns` at 250 with Score disabled, then lets
//! the server continue the same world until an enabled victory. Merely setting
//! an evaluator to 320 changes the policy because many AI gates read
//! `max_turns`. This runner preserves the nominal value and extends only the
//! observer's outer loop.

use civvis::ai::Ai;
use civvis::elo::{builtin_ai, builtin_provenance};
use civvis::game::{default_difficulty, Action, Game, GameOptions, VictoryConditions};
use civvis::rules::Rules;
use civvis::setup::{MapPoles, MapScript, MapSize, MapTopology};
use std::collections::BTreeMap;

const SCREEN_MAPS: usize = 12;
const SCREEN_SEED: u64 = 9_986_000;
const CONFIRM_MAPS: usize = 48;
const CONFIRM_SEED: u64 = 9_987_000;

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

fn realized_geometry(width: i32, height: i32, topology: MapTopology) -> (i32, i32, usize) {
    if topology.is_globe() {
        let frequency = civvis::mapgen::globe_frequency(width, height);
        (
            civvis::sphere::Sphere::width_for(frequency),
            civvis::sphere::Sphere::height_for(frequency),
            civvis::sphere::Sphere::tiles_for(frequency),
        )
    } else {
        (width, height, (width * height).max(0) as usize)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    observed_turn: u32,
    max_turns: u32,
    winner: Option<usize>,
    victory_type: Option<String>,
    scores: Vec<(usize, i64)>,
    mean_major_cities: f64,
}

impl Snapshot {
    fn winner_score_rank(&self, winner: usize) -> Option<usize> {
        let mut ranked = self.scores.clone();
        ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        ranked
            .iter()
            .position(|(player, _)| *player == winner)
            .map(|rank| rank + 1)
    }
}

fn snapshot(game: &Game, observed_turn: u32) -> Snapshot {
    let majors: Vec<usize> = game
        .players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian && !player.is_free_city)
        .map(|player| player.id)
        .collect();
    let living: Vec<usize> = majors
        .iter()
        .copied()
        .filter(|pid| game.players[*pid].alive)
        .collect();
    let cities = living
        .iter()
        .map(|pid| game.player_city_ids(*pid).len())
        .sum::<usize>();
    Snapshot {
        observed_turn,
        max_turns: game.max_turns,
        winner: game.winner,
        victory_type: game.victory_type.clone(),
        scores: majors
            .into_iter()
            .map(|pid| (pid, game.score(pid)))
            .collect(),
        mean_major_cities: cities as f64 / living.len().max(1) as f64,
    }
}

#[derive(Clone, Debug)]
struct HorizonResult {
    nominal: Snapshot,
    final_: Snapshot,
    nominal_captures: usize,
}

impl HorizonResult {
    fn nominal_complete(&self) -> bool {
        self.nominal.winner.is_some()
    }

    fn late_complete(&self) -> bool {
        self.nominal.winner.is_none() && self.final_.winner.is_some()
    }

    fn still_censored(&self) -> bool {
        self.final_.winner.is_none()
    }

    fn eventual_winner_nominal_rank(&self) -> Option<usize> {
        self.final_
            .winner
            .and_then(|winner| self.nominal.winner_score_rank(winner))
    }
}

/// Run through an external observation bound without ever changing the
/// policy-visible `Game::max_turns` value.
fn observe_game(mut game: Game, mut ais: Vec<Box<dyn Ai>>, observe_through: u32) -> HorizonResult {
    let nominal_limit = game.max_turns;
    assert!(observe_through >= nominal_limit);
    assert_eq!(ais.len(), game.players.len());
    let mut nominal = None;
    let mut nominal_captures = 0;

    loop {
        if nominal.is_none() && (game.winner.is_some() || game.turn > nominal_limit) {
            let observed_turn = if game.winner.is_some() {
                game.reported_turn().min(nominal_limit)
            } else {
                nominal_limit
            };
            nominal = Some(snapshot(&game, observed_turn));
            nominal_captures += 1;
        }

        if game.winner.is_some() || game.turn > observe_through {
            break;
        }
        let pid = game.current;
        ais[pid].take_turn(&mut game, pid);
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }

    // `observe_through >= nominal_limit`, so this is reachable only when a
    // malformed agent/game exits without advancing. Keep the invariant
    // explicit rather than silently returning no boundary observation.
    let nominal = nominal.unwrap_or_else(|| {
        nominal_captures += 1;
        snapshot(&game, nominal_limit.min(game.reported_turn()))
    });
    debug_assert_eq!(nominal_captures, 1);
    let final_turn = if game.winner.is_some() {
        game.reported_turn()
    } else {
        observe_through
    };
    let final_ = snapshot(&game, final_turn);
    HorizonResult {
        nominal,
        final_,
        nominal_captures,
    }
}

fn percentile(values: &[u32], fraction: f64) -> Option<u32> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let index = ((sorted.len() - 1) as f64 * fraction).round() as usize;
    sorted.get(index).copied()
}

fn wilson_95(hits: usize, n: usize) -> (f64, f64) {
    if n == 0 {
        return (0.0, 1.0);
    }
    let z = 1.959_963_984_540_054_f64;
    let n = n as f64;
    let p = hits as f64 / n;
    let denominator = 1.0 + z * z / n;
    let center = (p + z * z / (2.0 * n)) / denominator;
    let spread = z * ((p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt()) / denominator;
    ((center - spread).max(0.0), (center + spread).min(1.0))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 8).max(2) as usize;
    let size = MapSize::for_players(players);
    let (default_width, default_height) = size.dimensions(Default::default());
    let width = number(&args, "--width", default_width as i64).max(8) as i32;
    let height = number(&args, "--height", default_height as i64).max(8) as i32;
    let city_states =
        number(&args, "--city-states", size.default_city_states as i64).max(0) as usize;
    let maps = number(&args, "--maps", SCREEN_MAPS as i64).max(1) as usize;
    let nominal_turns = number(&args, "--turns", 250).max(1) as u32;
    let observe_through = number(&args, "--observe-through", 320).max(1) as u32;
    if observe_through < nominal_turns {
        eprintln!("--observe-through must be at least --turns");
        std::process::exit(2);
    }
    let seed = number(&args, "--seed", SCREEN_SEED as i64).max(0) as u64;
    let jobs = match number(&args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    };
    let speed = text(&args, "--speed", "online");
    let difficulty = text(&args, "--difficulty", &default_difficulty());
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
    let victory_conditions = VictoryConditions::parse(&victory_names).unwrap_or_else(|why| {
        eprintln!("--victories: {why}");
        std::process::exit(2);
    });
    let rules = Rules::embedded();
    if !rules.speeds.contains_key(&speed) {
        eprintln!("unknown game speed {speed:?}");
        std::process::exit(2);
    }
    if !rules.difficulties.contains_key(&difficulty) {
        eprintln!("unknown difficulty {difficulty:?}");
        std::process::exit(2);
    }
    let provenance = builtin_provenance("strategic_deep", "evolved");
    println!("agent: {}", provenance.line());
    if provenance.degraded() {
        eprintln!(
            "refusing to record strategic_deep: it resolves to {:?}",
            provenance.effective
        );
        std::process::exit(3);
    }
    let (realized_width, realized_height, realized_tiles) =
        realized_geometry(width, height, map_topology);
    println!("Deployment horizon censoring census");
    println!(
        "profile: {players}p requested {width}x{height}, realized \
         {realized_width}x{realized_height} ({realized_tiles} tiles), \
         {city_states} city-states, {nominal_turns} nominal turns, observe through \
         {observe_through}, {speed}, seed {seed}, {jobs} jobs, difficulty {difficulty}"
    );
    println!(
        "world: map {}, shape {}, poles {}, civilizations {}, victories {}",
        map_script.id(),
        map_topology.id(),
        map_poles.id(),
        if args.iter().any(|arg| arg == "--randomize-civs") {
            "randomized"
        } else {
            "fixed stock"
        },
        VictoryConditions::NAMES
            .into_iter()
            .filter(|name| victory_conditions.is_enabled(name))
            .collect::<Vec<_>>()
            .join(",")
    );
    println!(
        "playing {maps} independent strategic_deep worlds; Game.max_turns remains {nominal_turns}"
    );

    let randomize_civs = args.iter().any(|arg| arg == "--randomize-civs");
    let results: Vec<HorizonResult> = civvis::parallel::map_reporting(
        maps,
        jobs,
        |map| {
            let map_seed = seed + map as u64;
            let mut options =
                GameOptions::new(players, width, height, map_seed, nominal_turns, city_states);
            options.speed = speed.clone();
            options.difficulty = difficulty.clone();
            options.map_script = map_script;
            options.map_topology = map_topology;
            options.map_poles = map_poles;
            options.randomize_civs = randomize_civs;
            let mut game = Game::new_with(options);
            game.victory_conditions = victory_conditions;
            let ais: Vec<Box<dyn Ai>> = game
                .players
                .iter()
                .map(|player| {
                    let name = if player.is_minor || player.is_barbarian {
                        "basic"
                    } else {
                        "strategic_deep"
                    };
                    builtin_ai(name, map_seed.wrapping_add(player.id as u64))
                })
                .collect();
            observe_game(game, ais, observe_through)
        },
        |completed, _| eprintln!("progress: {}/{} maps complete", completed + 1, maps),
    );
    assert!(
        results.iter().all(|result| result.nominal_captures == 1),
        "every game must capture the nominal boundary exactly once"
    );

    let nominal_complete = results
        .iter()
        .filter(|result| result.nominal_complete())
        .count();
    let late_complete = results
        .iter()
        .filter(|result| result.late_complete())
        .count();
    let still_censored = results
        .iter()
        .filter(|result| result.still_censored())
        .count();
    let finish_turns: Vec<u32> = results
        .iter()
        .filter_map(|result| result.final_.winner.map(|_| result.final_.observed_turn))
        .collect();
    let score_leaders = results
        .iter()
        .filter(|result| result.eventual_winner_nominal_rank() == Some(1))
        .count();
    let resolved = finish_turns.len();
    let nominal_city_mean = results
        .iter()
        .map(|result| result.nominal.mean_major_cities)
        .sum::<f64>()
        / maps as f64;
    let final_city_mean = results
        .iter()
        .map(|result| result.final_.mean_major_cities)
        .sum::<f64>()
        / maps as f64;
    let mut victory_types: BTreeMap<String, usize> = BTreeMap::new();
    for victory in results
        .iter()
        .filter_map(|result| result.final_.victory_type.as_deref())
    {
        *victory_types.entry(victory.to_string()).or_default() += 1;
    }

    println!("\nmap  nominal  final  victory      score-rank@nominal  cities nominal->final");
    for (map, result) in results.iter().enumerate() {
        let nominal = if result.nominal_complete() {
            format!("win@{}", result.nominal.observed_turn)
        } else {
            "censored".to_string()
        };
        let final_state = if result.final_.winner.is_some() {
            format!("win@{}", result.final_.observed_turn)
        } else {
            "censored".to_string()
        };
        println!(
            "{map:>3}  {nominal:<9} {final_state:<9} {:<12} {:>6}              {:>5.2}->{:>5.2}",
            result.final_.victory_type.as_deref().unwrap_or("none"),
            result
                .eventual_winner_nominal_rank()
                .map(|rank| rank.to_string())
                .unwrap_or_else(|| "-".to_string()),
            result.nominal.mean_major_cities,
            result.final_.mean_major_cities,
        );
    }
    let (lower, upper) = wilson_95(late_complete, maps);
    println!("\nCensoring summary:");
    println!(
        "  nominal completions {nominal_complete}/{maps}; late completions {late_complete}/{maps}; still censored {still_censored}/{maps}"
    );
    println!(
        "  late-completion share {:.1}% (95% Wilson {:.1}..{:.1}%)",
        100.0 * late_complete as f64 / maps as f64,
        100.0 * lower,
        100.0 * upper,
    );
    println!(
        "  finish turn p50 {} p90 {}; eventual winner led score at nominal boundary {score_leaders}/{resolved}",
        percentile(&finish_turns, 0.50)
            .map(|turn| turn.to_string())
            .unwrap_or_else(|| "-".to_string()),
        percentile(&finish_turns, 0.90)
            .map(|turn| turn.to_string())
            .unwrap_or_else(|| "-".to_string()),
    );
    println!(
        "  mean living-major cities {nominal_city_mean:.2} at nominal boundary, {final_city_mean:.2} at final observation; victories {:?}",
        victory_types
    );

    let exact_profile = players == 8
        && width == 84
        && height == 54
        && city_states == 12
        && nominal_turns == 250
        && observe_through == 320
        && speed == "online"
        && map_script == MapScript::Continents
        && map_topology == MapTopology::Planet
        && map_poles == MapPoles::Poles
        && randomize_civs
        && victory_conditions == VictoryConditions::parse("science,culture,domination").unwrap();
    if exact_profile && maps == SCREEN_MAPS && seed == SCREEN_SEED {
        println!(
            "screen gate: {}",
            if late_complete >= 3 {
                "PASS — run only the fixed seed-9987000 confirmation"
            } else {
                "STOP — retain the turn-250 convention on this focal cell"
            }
        );
    } else if exact_profile && maps == CONFIRM_MAPS && seed == CONFIRM_SEED {
        println!(
            "confirmation gate: {}",
            if late_complete >= 10 && lower > 0.10 {
                "PASS — production-terminal studies must preserve max_turns and model continuation"
            } else {
                "STOP — retain the turn-250 convention on this focal cell"
            }
        );
    } else {
        println!("diagnostic profile: no preregistered gate applies");
    }
}

#[cfg(test)]
mod tests {
    use super::{observe_game, percentile, wilson_95, HorizonResult, Snapshot};
    use civvis::ai::{Ai, BasicAi};
    use civvis::game::{Game, VictoryConditions};

    fn no_victories() -> VictoryConditions {
        VictoryConditions {
            science: false,
            culture: false,
            religious: false,
            diplomatic: false,
            domination: false,
            score: false,
        }
    }

    #[test]
    fn external_observation_continues_without_changing_the_nominal_limit() {
        let mut game = Game::new_full(2, 24, 16, 998_599, 1, 0, false);
        game.victory_conditions = no_victories();
        let ais: Vec<Box<dyn Ai>> = BasicAi::fleet(&game)
            .into_iter()
            .map(|ai| Box::new(ai) as Box<dyn Ai>)
            .collect();
        let result = observe_game(game, ais, 3);
        assert_eq!(result.nominal_captures, 1);
        assert_eq!(result.nominal.observed_turn, 1);
        assert_eq!(result.nominal.max_turns, 1);
        assert_eq!(result.final_.observed_turn, 3);
        assert_eq!(result.final_.max_turns, 1);
        assert!(result.still_censored());
    }

    #[test]
    fn a_nominal_winner_is_not_a_late_completion() {
        let nominal = Snapshot {
            observed_turn: 250,
            max_turns: 250,
            winner: Some(1),
            victory_type: Some("science".to_string()),
            scores: vec![(0, 10), (1, 20)],
            mean_major_cities: 5.0,
        };
        let result = HorizonResult {
            final_: nominal.clone(),
            nominal,
            nominal_captures: 1,
        };
        assert!(result.nominal_complete());
        assert!(!result.late_complete());
        assert_eq!(result.eventual_winner_nominal_rank(), Some(1));
    }

    #[test]
    fn summary_statistics_are_bounded_and_deterministic() {
        assert_eq!(percentile(&[320, 250, 280, 270], 0.50), Some(280));
        let (lower, upper) = wilson_95(10, 48);
        assert!(lower > 0.10);
        assert!(lower < 10.0 / 48.0 && upper > 10.0 / 48.0);
    }
}
