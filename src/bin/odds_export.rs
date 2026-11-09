//! Export forecasts made before an outcome, followed by the real result.
//!
//! `cargo run --profile ci --features developer-tools --bin odds_export --
//! --games 48 --jobs 4 > odds.jsonl`
//! Each line is one complete game, so calibration and holdout splits can keep
//! every seat and turn of a game together. Mercy is disabled: the forecast
//! being measured must never manufacture its own target.
use civvis::ai::{AdvancedAi, Ai, BasicAi};
use civvis::game::{Action, Game, GameOptions};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::sync::Mutex;

fn option(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == flag)
        .map(|index| args.get(index + 1).expect("option needs a value").clone())
        .unwrap_or_else(|| default.to_string())
}

fn number<T: std::str::FromStr>(args: &[String], flag: &str, default: &str) -> T {
    option(args, flag, default)
        .parse()
        .unwrap_or_else(|_| panic!("invalid {flag}"))
}

fn snapshot(game: &Game) -> Value {
    let odds = civvis::odds::table(game, |_| 1500.0_f64);
    let leading_score = game
        .players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| game.team_score_rank_key(player.id).0)
        .max()
        .unwrap_or(0);
    let seats: Vec<Value> = odds
        .iter()
        .map(|(pid, odds)| {
            let player = &game.players[*pid];
            let cities = game.player_city_ids(*pid);
            let mut yields = [0.0; 4];
            for cid in &cities {
                let y = game.city_yields(*cid);
                yields[0] += y.production;
                yields[1] += y.science;
                yields[2] += y.culture;
                yields[3] += y.faith;
            }
            let races = game.victory_races(*pid, leading_score);
            json!({
                "pid": pid, "alive": player.alive, "start": odds.start,
                "now": odds.now, "prior_elo": odds.prior_elo,
                "standing_elo": odds.standing_elo, "race_pct": odds.race_pct,
                "score": game.score(*pid), "military": game.military_power(*pid),
                "cities": cities.len(), "yields": yields,
                "techs": player.techs.len(), "civics": player.civics.len(),
                "races": [races.science, races.culture, races.religious,
                           races.diplomatic, races.domination],
                "exoplanet": [races.exoplanet_distance, races.exoplanet_distance_target,
                              races.exoplanet_speed],
            })
        })
        .collect();
    json!({"turn": game.turn, "terminal": game.winner.is_some(), "seats": seats})
}

/// Read-only replay against a recorded host outcome. The counterfactual clears
/// only city-total knowledge: every other observation is identical. It is the
/// pre-fix odds path, without rerunning or changing a single game action.
fn replay(path: &str, every: u32) {
    let path = std::path::Path::new(path);
    let summary: Value = serde_json::from_str(
        &std::fs::read_to_string(path.join("summary.json")).expect("read summary"),
    )
    .expect("parse summary");
    assert_eq!(summary["configured"], true, "setup must be verified");
    assert_ne!(
        summary["seat"]["isolated_action_probes"], true,
        "exclude perturbed games"
    );
    assert_eq!(
        summary["outcome"]["kind"], "victory",
        "a real outcome is required"
    );
    assert_eq!(
        summary["seat"]["local_player"], 0,
        "replay expects local seat zero"
    );
    let players = summary["seat"]["players"].as_u64().expect("player count") as usize;
    let turns = summary["seat"]["max_turns"].as_u64().expect("turn limit") as u32;
    let end_turn = summary["outcome"]["turn"].as_u64().expect("outcome turn") as u32;
    let winner = summary["outcome"]["team"].as_u64().expect("winner") as usize;
    assert!(
        winner < players,
        "replay requires stock free-for-all team IDs"
    );
    let events = std::fs::read_to_string(path.join("events.jsonl")).expect("read events");
    let mut samples = Vec::new();
    for turn in (every..end_turn).step_by(every as usize) {
        let Ok(board) = civvis::mirror::snapshot_from_events_at(&events, Some(turn)) else {
            continue;
        };
        let Some(state) = civvis::mirror::state_from_events(&events, Some(turn)) else {
            continue;
        };
        if state.turn != turn {
            continue;
        }
        let game = civvis::mirror::rebuild_from_state(&board, &state, players, 1, turns, 0).game;
        let mut old = game.clone();
        old.unseen_city_owners.clear();
        for stats in std::sync::Arc::make_mut(&mut old.observed_public_empire_stats).values_mut() {
            stats.city_count = None;
        }
        let before = civvis::odds::table(&old, |_| 1500.0_f64);
        let mut sample = snapshot(&game);
        for seat in sample["seats"].as_array_mut().expect("seats") {
            let pid = seat["pid"].as_u64().expect("seat id") as usize;
            seat["legacy_now"] = json!(before[&pid].now);
            seat["public_cities"] = json!(game
                .observed_public_empire_stats
                .get(&pid)
                .and_then(|s| s.city_count));
        }
        samples.push(sample);
    }
    println!(
        "{}",
        json!({
            "schema": "civvis-odds-v1", "source": "host-replay", "run": path.file_name().unwrap().to_string_lossy(),
            "seed": summary["seat"]["game_seed"], "players": players,
            "max_turns": turns, "speed": summary["speed"], "difficulty": summary["difficulty"],
            "winners": [winner], "end_turn": end_turn, "victory": summary["outcome"]["victory_type"],
            "samples": samples,
        })
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flags = [
        "--games",
        "--jobs",
        "--start-seed",
        "--players",
        "--turns",
        "--width",
        "--height",
        "--city-states",
        "--every",
        "--controller",
        "--difficulty",
        "--speed",
        "--map",
        "--human-seat",
        "--replay",
    ];
    let pairs = args[1..].chunks_exact(2);
    assert!(pairs.remainder().is_empty(), "every option needs a value");
    let mut seen = std::collections::BTreeSet::new();
    for pair in pairs {
        assert!(
            flags.contains(&pair[0].as_str()),
            "unknown option {}",
            pair[0]
        );
        assert!(seen.insert(&pair[0]), "duplicate option {}", pair[0]);
    }
    let games: usize = number(&args, "--games", "48");
    let jobs: usize = number(&args, "--jobs", "4");
    let first_seed: u64 = number(&args, "--start-seed", "940000");
    let players: usize = number(&args, "--players", "4");
    let turns: u32 = number(&args, "--turns", "220");
    let width: i32 = number(&args, "--width", "32");
    let height: i32 = number(&args, "--height", "22");
    let city_states: usize = number(&args, "--city-states", "2");
    let every: u32 = number(&args, "--every", "10");
    if args.iter().any(|arg| arg == "--replay") {
        assert!(every > 0);
        replay(&option(&args, "--replay", ""), every);
        return;
    }
    let controller = option(&args, "--controller", "basic");
    let difficulty = option(&args, "--difficulty", "prince");
    let speed = option(&args, "--speed", "online");
    let map = option(&args, "--map", "tennis_ball");
    let human_seat = option(&args, "--human-seat", "none");
    let human_seat =
        (human_seat != "none").then(|| human_seat.parse::<usize>().expect("invalid human seat"));
    assert!(games > 0 && jobs > 0 && every > 0 && turns > 0);
    assert!(matches!(controller.as_str(), "basic" | "advanced"));
    assert!(human_seat.is_none_or(|pid| pid < players));
    assert!(
        civvis::setup::GameSpeed::from_id(&speed).is_some(),
        "unknown speed"
    );
    let map_script = civvis::setup::MapScript::from_id(&map).expect("unknown map");
    let output = Mutex::new(io::stdout());
    civvis::parallel::map(games, jobs, |index| {
        let seed = first_seed.checked_add(index as u64).expect("seed overflow");
        let mut options = GameOptions::new(players, width, height, seed, turns, city_states);
        options.difficulty = difficulty.clone();
        options.speed = speed.clone();
        options.map_script = map_script;
        options.human_seats.extend(human_seat);
        options.randomize_civs = true;
        let mut game = Game::new_with(options);
        assert!(
            game.rules.difficulties.contains_key(difficulty.as_str()),
            "unknown difficulty"
        );
        let mut ais: Vec<Box<dyn Ai>> = if controller == "basic" {
            BasicAi::fleet(&game)
                .into_iter()
                .map(|ai| Box::new(ai) as Box<dyn Ai>)
                .collect()
        } else {
            AdvancedAi::fleet(&game)
                .into_iter()
                .map(|ai| Box::new(ai) as Box<dyn Ai>)
                .collect()
        };
        game.set_fog_memory(false);
        game.set_war_ledger(false);
        let mut samples = Vec::new();
        let mut sampled_turn = None;
        while game.winner.is_none() && game.turn <= game.max_turns {
            if sampled_turn != Some(game.turn)
                && (game.turn == 1 || game.turn.is_multiple_of(every))
            {
                samples.push(snapshot(&game));
                sampled_turn = Some(game.turn);
            }
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                game.apply(pid, &Action::EndTurn).expect("end turn");
            }
        }
        let row = json!({
            "schema": "civvis-odds-v1", "seed": seed, "players": players,
            "width": width, "height": height, "city_states": city_states,
            "max_turns": turns, "speed": game.speed, "difficulty": game.difficulty,
            "map": game.map_script.id(), "human_seat": human_seat,
            "controller": controller, "winners": game.winning_players(),
            "end_turn": game.turn, "victory": game.victory_type, "samples": samples,
        });
        let mut writer = output.lock().expect("stdout lock");
        serde_json::to_writer(&mut *writer, &row).expect("write game");
        writeln!(writer).expect("write newline");
        writer.flush().expect("flush game");
        eprintln!(
            "completed seed {seed} at turn {} ({:?})",
            game.turn, game.victory_type
        );
    });
}
