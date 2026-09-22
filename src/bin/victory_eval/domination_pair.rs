//! Same-seed policy probe for one Gran Colombia Domination seat against
//! three adaptive CIVVIS rivals. This is a simulator probe, not native Civ VI
//! verification or a deployment-ledger screen.
//!
//! victory_eval --domination-pair siege-is-progress-3 --games 16 \
//!   --start-seed 37140000 --out /tmp/domination-pairs.jsonl
//!
//! Both legs use King, 4 players, 60x38 Pangaea, 6 city-states, Online,
//! barbarians, all victory conditions and the natural 250-turn clock.
//! Only seat zero's named policy is disabled/enabled. The other seats keep
//! their adaptive deployed controllers. The focal seat also carries the
//! repository's compiled live-force-on bundle, recorded in every pair.
use civvis::ai::{gene, run_game_observed, AdvancedAi, Gene, VictoryTarget};
use civvis::game::{Game, GameOptions, LeaderPool};
use civvis::setup::MapScript;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

const FORCED: &str = include_str!("../../../deploy/live-force-on.txt");

struct Config {
    policy: &'static Gene,
    games: u64,
    start_seed: u64,
    out: PathBuf,
}

fn parse(args: &[String]) -> Result<Config, String> {
    let mut policy = None;
    let mut games = 16;
    let mut start_seed = 37_140_000;
    let mut out = None;
    let mut seen = BTreeSet::new();
    for pair in args.chunks(2) {
        let flag = &pair[0];
        if !seen.insert(flag) {
            return Err(format!("duplicate argument {flag}"));
        }
        let value = pair.get(1).ok_or_else(|| format!("{flag} needs a value"))?;
        match flag.as_str() {
            "--domination-pair" => {
                policy = Some(gene(value).ok_or_else(|| format!("unknown policy {value}"))?);
            }
            "--games" => games = value.parse::<u64>().map_err(|e| e.to_string())?,
            "--start-seed" => start_seed = value.parse::<u64>().map_err(|e| e.to_string())?,
            "--out" => out = Some(PathBuf::from(value)),
            _ => {
                return Err(format!(
                    "unsupported argument {flag} for fixed-profile Domination probe"
                ))
            }
        }
    }
    if games == 0 || start_seed.checked_add(games - 1).is_none() {
        return Err("games must be positive and the seed range must fit u64".to_string());
    }
    Ok(Config {
        policy: policy.ok_or("--domination-pair is required")?,
        games,
        start_seed,
        out: out.ok_or("--out is required; existing files are never overwritten")?,
    })
}

fn options(seed: u64) -> GameOptions {
    GameOptions {
        map_script: MapScript::Pangaea,
        difficulty: "king".to_string(),
        speed: "online".to_string(),
        civs: vec!["Gran Colombia".to_string()],
        leader_pool: LeaderPool::Civ6,
        randomize_civs: true,
        handicap_exempt: BTreeSet::from([0]),
        barbarians: true,
        // Shipped Civ VI Maps.xml:106: MAPSIZE_TINY, four players, 60x38.
        ..GameOptions::new(4, 60, 38, seed, 250, 6)
    }
}

fn profile(seed: u64) -> serde_json::Value {
    let setup = options(seed);
    serde_json::json!({
        "players": setup.players, "width": setup.width, "height": setup.height,
        "map": format!("{:?}", setup.map_script), "difficulty": setup.difficulty,
        "speed": setup.speed, "max_turns": setup.max_turns,
        "city_states": setup.city_states, "barbarians": setup.barbarians,
        "barbarian_difficulty": setup.barbarian_difficulty,
        "handicap_exempt": setup.handicap_exempt, "human_seats": setup.human_seats,
        "leader_pool": setup.leader_pool.id(), "civs": setup.civs,
        "randomize_civs": setup.randomize_civs, "victories": setup.victory_conditions,
        "mercy_rule": setup.mercy_rule, "required_victory_types": setup.required_victory_types,
        "game_modes": setup.game_modes, "teams": setup.teams,
        "disaster_intensity": setup.disaster_intensity,
        "base_ruleset": format!("{:?}", setup.base_ruleset),
        "future_era": format!("{:?}", setup.future_era), "start_era": setup.start_era,
        "turn_structure": format!("{:?}", setup.turn_structure),
        "map_topology": format!("{:?}", setup.map_topology),
        "map_poles": format!("{:?}", setup.map_poles),
    })
}

fn fleet(game: &Game, policy: &Gene, enabled: bool) -> Vec<AdvancedAi> {
    let mut ais = AdvancedAi::fleet(game);
    for ai in &mut ais {
        ai.enable_live_bridge();
    }
    ais[0] = AdvancedAi::targeting(VictoryTarget::Domination);
    ais[0].enable_live_bridge_universe();
    let forced: Vec<&str> = FORCED.trim().split(',').collect();
    ais[0].apply_gene_ledger_with_forced_live(&forced);
    if enabled {
        (policy.enable)(&mut ais[0]);
    } else {
        (policy.disable)(&mut ais[0]);
    }
    ais
}

#[derive(Debug, Serialize)]
struct Outcome {
    winner: Option<usize>,
    victory: Option<String>,
    turn: u32,
    focal_won: bool,
    focal_domination_won: bool,
    focal_score: i64,
    foreign_cities_observed_held: usize,
    foreign_cities_held_at_end: usize,
    applied_actions: usize,
}

struct Trial {
    outcome: Outcome,
    // Compare the complete canonical applied-action records, not a rounded
    // score or an outcome that could match despite different decisions.
    actions: Vec<u8>,
    civs: Vec<String>,
}

fn trial(seed: u64, policy: &Gene, enabled: bool) -> Trial {
    let mut game = Game::new_with(options(seed));
    let civs = game.players.iter().take(4).map(|p| p.civ.clone()).collect();
    let mut ais = fleet(&game, policy, enabled);
    let mut held = BTreeSet::new();
    let mut observe = |g: &Game| {
        for city in g.cities.values() {
            if city.owner == 0 && city.original_owner != 0 {
                held.insert(city.id);
            }
        }
    };
    run_game_observed(&mut game, &mut ais, &mut observe);
    observe(&game);
    let outcome = Outcome {
        winner: game.winner,
        victory: game.victory_type.clone(),
        turn: game.reported_turn(),
        focal_won: game.winner == Some(0),
        focal_domination_won: game.winner == Some(0)
            && game.victory_type.as_deref() == Some("domination"),
        focal_score: game.score(0),
        foreign_cities_observed_held: held.len(),
        foreign_cities_held_at_end: game
            .cities
            .values()
            .filter(|c| c.owner == 0 && c.original_owner != 0)
            .count(),
        applied_actions: game.log.len(),
    };
    Trial {
        outcome,
        actions: serde_json::to_vec(&game.log).expect("applied actions serialize"),
        civs,
    }
}

pub(super) fn run(args: &[String]) -> Result<(), String> {
    let config = parse(args)?;
    for tag in FORCED.trim().split(',') {
        gene(tag).ok_or_else(|| format!("unregistered live bundle policy {tag}"))?;
    }
    let output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&config.out)
        .map_err(|e| format!("{}: {e}", config.out.display()))?;
    let mut output = BufWriter::new(output);
    let mut changed = 0;
    let mut wins = [0, 0];
    let mut domination = [0, 0];
    for index in 0..config.games {
        let seed = config.start_seed + index;
        // Alternate execution order to avoid confounding every treatment
        // with the second leg. Both worlds and controllers start fresh.
        let (off, on) = if index % 2 == 0 {
            (
                trial(seed, config.policy, false),
                trial(seed, config.policy, true),
            )
        } else {
            let on = trial(seed, config.policy, true);
            (trial(seed, config.policy, false), on)
        };
        if off.civs != on.civs {
            return Err(format!("paired civilizations differ at seed {seed}"));
        }
        let actions_identical = off.actions == on.actions;
        changed += usize::from(!actions_identical);
        for (i, arm) in [&off, &on].into_iter().enumerate() {
            wins[i] += usize::from(arm.outcome.focal_won);
            domination[i] += usize::from(arm.outcome.focal_domination_won);
        }
        let row = serde_json::json!({
            "schema": 1, "kind": "simulator_domination_policy_pair",
            "policy": config.policy.tag, "seed": seed,
            "execution_order": if index % 2 == 0 { "off,on" } else { "on,off" },
            "profile": profile(seed), "civilizations": off.civs,
            "focal_seat": 0, "focal_target": "domination",
            "rivals": "adaptive CIVVIS live bridge; not Firaxis AI",
            "forced_focal_policies": FORCED.trim().split(',').collect::<Vec<_>>(),
            "actions_identical": actions_identical,
            "off": off.outcome, "on": on.outcome,
        });
        serde_json::to_writer(&mut output, &row).map_err(|e| e.to_string())?;
        writeln!(output).map_err(|e| e.to_string())?;
        output.flush().map_err(|e| e.to_string())?;
        eprintln!("pair {}/{}, seed {seed}: wins off/on {wins:?}, domination {domination:?}, changed {changed}", index + 1, config.games);
    }
    if changed == 0 {
        return Err(
            "all paired action records are identical; this batch provides no behavioral contrast"
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "domination_pair_tests.rs"]
mod tests;
