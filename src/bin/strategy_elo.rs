//! Measure the Elo of every Civ AI Agent and of Civilization VI's difficulty
//! rungs, and publish the result where the game reads it.
//!
//! Two kinds of table are played, all of them full games:
//!
//! - **pool** tables seat the registered agents (`strategies::ROSTER`) against
//!   each other at Prince, where the rules hand nobody anything. Each round
//!   deals the whole roster out, so every agent sits equally often;
//! - **rung** tables seat the reference agent (`strategies::CIV6_REFERENCE`)
//!   alone, with the rung's shipped AI handicap, at a table of roster agents
//!   who each play on the terms a person gets at that rung. It is rated as the
//!   entrant `civ6:<rung>`. That is the table Prince itself is measured at: the
//!   reference agent with nothing added, one seat among agents. The log calls
//!   these `lone`; its older `rung` lines are the retired shape, one
//!   challenger against the rung in every other seat, which the fit no longer
//!   reads (`docs/ELO.md` has the comparison that retired it).
//!
//! Every finished table is appended to the game log, so a run that is killed
//! resumes where it stopped and a later run only adds games. The ratings are
//! then refitted from the whole log (`strategies::fit`), pinned so that Prince
//! is `strategies::PRINCE_ELO`, and written to `data/strategy_elo.json`.
//!
//! ```text
//! cargo run --features developer-tools --profile ci --bin strategy_elo -- \
//!   --games 240 --rung-games 40 --jobs 4
//! ```
//!
//! `--games` and `--rung-games` are totals the log should reach, not amounts
//! to add: running the same command twice plays nothing the second time.
//! `--fit-only` refits and republishes without playing.
//!
//! A scheduled re-rate instead adds a labelled run and keeps a window:
//!
//! ```text
//! strategy_elo --run 2026-11-23-15125c4 --add-games 480 --add-rung-games 120 \
//!   --keep-runs 4 --jobs 3
//! ```
//!
//! The run's tables continue after the newest logged index of each series
//! (`strategies::added_indices`), carry the label, and resume under it if the
//! run is killed. `--keep-runs K` then drops every table outside the newest K
//! runs (`strategies::kept_runs`), so the window tracks the genome while the
//! margin of error shrinks as runs accumulate.
use civvis::ai::{Ai, BasicAi};
use civvis::game::{Action, Game, GameOptions};
use civvis::parallel;
use civvis::setup::MapSize;
use civvis::strategies::{
    self, Table, CIV6_REFERENCE, CIV6_RUNGS, MARGIN_Z, PRINCE, PRINCE_ELO, ROSTER,
};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;

const DEFAULT_LOG: &str = "data/strategy_elo_games.jsonl";
const DEFAULT_OUT: &str = "data/strategy_elo.json";
/// The gene screen's speed and its turn limit, so a rating describes the same
/// length of game the genes were priced in.
const SPEED: &str = "online";
const DEFAULT_TURNS: u32 = 250;
const BOOTSTRAP_ROUNDS: usize = 200;

struct Options {
    players: usize,
    turns: u32,
    seed: u64,
    jobs: usize,
    games: usize,
    rung_games: usize,
    /// Tables a labelled run adds to the pool and to each rung.
    add_games: usize,
    add_rung_games: usize,
    /// The label every table this run plays carries.
    run: Option<String>,
    /// Runs the window keeps; 0 keeps the whole log.
    keep_runs: usize,
    log: String,
    out: String,
    fit_only: bool,
    /// The agent seated as every rung's rivals. Anything but the published
    /// reference is a calibration run and wants its own `--out`.
    reference: String,
}

fn text(args: &[String], key: &str) -> Option<String> {
    let at = args.iter().position(|arg| arg == key)?;
    args.get(at + 1).cloned()
}

fn number(args: &[String], key: &str, default: u64) -> u64 {
    match text(args, key) {
        None => default,
        Some(value) => value.parse().unwrap_or_else(|_| {
            eprintln!("{key} wants a whole number, not {value:?}");
            std::process::exit(2);
        }),
    }
}

/// One table to play.
#[derive(Clone)]
struct Plan {
    kind: &'static str,
    rung: String,
    /// Position in its own series, which is what makes a resumed run pick the
    /// same tables an uninterrupted one would have.
    index: usize,
    seed: u64,
    /// The ledger key in each major seat.
    seats: Vec<String>,
    /// The seats that play on the human's terms: every seat but the rung's
    /// own on a rung table, none at the pool.
    challengers: Vec<usize>,
}

/// What a rung table is logged as: the rung's rival alone among agents.
const RUNG_KIND: &str = "lone";
/// The retired shape's lines, still in logs written before 2026-11-22: one
/// challenger against the rung in every other seat. Not fitted, and not
/// counted when a series looks for where its indices end.
const RETIRED_RUNG_KIND: &str = "rung";
/// What keeps a rung's deals and world seeds clear of the retired series at
/// the same rung. Far past any index a series reaches.
const LONE_SERIES: u64 = 500_000;

/// The `index`th pool table: a hand of the roster (`strategies::dealt`), so
/// every agent sits equally often and no table holds one agent twice.
fn pool_plan(options: &Options, index: usize) -> Plan {
    let seats = strategies::dealt(options.seed, index, options.players)
        .into_iter()
        .map(str::to_string)
        .collect();
    Plan {
        kind: "pool",
        rung: PRINCE.to_string(),
        index,
        seed: options.seed.wrapping_add(index as u64),
        seats,
        challengers: Vec::new(),
    }
}

/// The `index`th table at `rung`: the rung's rival sits alone, its seat
/// moving round the table, and a hand of the roster takes every other seat on
/// the human's terms.
fn rung_plan(options: &Options, rung_order: usize, rung: &str, index: usize) -> Plan {
    let alone = index % options.players;
    let series = options
        .seed
        .wrapping_add(1_000_000 * (rung_order as u64 + 1))
        .wrapping_add(LONE_SERIES);
    let mut hand = strategies::dealt(series, index, options.players - 1).into_iter();
    let seats = (0..options.players)
        .map(|seat| {
            if seat == alone {
                strategies::rung_key(rung)
            } else {
                hand.next().expect("a hand fills the table").to_string()
            }
        })
        .collect();
    Plan {
        kind: RUNG_KIND,
        rung: rung.to_string(),
        index,
        seed: series.wrapping_add(index as u64),
        seats,
        challengers: (0..options.players).filter(|seat| *seat != alone).collect(),
    }
}

fn agent_for(options: &Options, key: &str) -> Box<dyn Ai + Send> {
    let id = if key.starts_with("civ6:") {
        options.reference.as_str()
    } else {
        key
    };
    strategies::find(id)
        .unwrap_or_else(|| panic!("no agent named {id}"))
        .build()
}

fn play(options: &Options, plan: &Plan) -> Value {
    let size = MapSize::for_players(options.players);
    let mut setup = GameOptions::new(
        options.players,
        size.width,
        size.height,
        plan.seed,
        options.turns,
        size.default_city_states,
    );
    setup.speed = SPEED.to_string();
    setup.difficulty = plan.rung.clone();
    setup.human_seats.extend(plan.challengers.iter().copied());
    let mut game = Game::new_with(setup);
    let majors: Vec<usize> = game
        .players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    assert_eq!(
        majors,
        (0..options.players).collect::<Vec<_>>(),
        "the major seats are the first player ids"
    );
    let mut fleet: Vec<Box<dyn Ai + Send>> = game
        .players
        .iter()
        .map(|player| -> Box<dyn Ai + Send> {
            if player.is_minor || player.is_barbarian {
                Box::new(BasicAi::new())
            } else {
                agent_for(options, &plan.seats[player.id])
            }
        })
        .collect();

    for _ in 0..options.turns + 5 {
        if game.winner.is_some() {
            break;
        }
        for (pid, agent) in fleet.iter_mut().enumerate() {
            if game.winner.is_some() {
                break;
            }
            agent.take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }
    }

    let scores: Vec<i64> = majors.iter().map(|pid| game.score(*pid)).collect();
    // The engine awards the turn limit on score itself. A table it left
    // undecided is settled the same way rather than thrown away, and says so.
    let (winner, victory) = match game.winner {
        Some(pid) if pid < options.players => (
            pid,
            game.victory_type
                .clone()
                .unwrap_or_else(|| "unknown".to_string()),
        ),
        _ => {
            let best = (0..options.players)
                .filter(|pid| game.players[*pid].alive)
                .max_by_key(|pid| scores[*pid])
                .unwrap_or(0);
            (best, "score-at-cap".to_string())
        }
    };
    json!({
        "kind": plan.kind,
        "rung": plan.rung,
        "index": plan.index,
        "seed": plan.seed,
        "players": options.players,
        "turns": options.turns,
        "reference": options.reference,
        "seats": plan.seats,
        "challengers": plan.challengers,
        "winner": winner,
        "victory": victory,
        "end_turn": game.turn,
        "scores": scores,
    })
}

/// Log lines this run's settings can be rated with. A table played at another
/// size or length is a different instrument, and so is a rung table whose
/// rival was another reference agent, or one of the retired shape. A pool
/// table never seats the reference as such, so it survives a change of
/// reference.
fn read_log(options: &Options) -> Vec<Value> {
    let Ok(body) = std::fs::read_to_string(&options.log) else {
        return Vec::new();
    };
    body.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|line| {
            line["players"].as_u64() == Some(options.players as u64)
                && line["turns"].as_u64() == Some(options.turns as u64)
                && line["kind"] != RETIRED_RUNG_KIND
                && (line["kind"] == "pool"
                    || line["reference"].as_str() == Some(options.reference.as_str()))
        })
        .collect()
}

fn known(key: &str) -> bool {
    ROSTER.iter().any(|row| row.id == key)
        || CIV6_RUNGS
            .iter()
            .any(|(rung, _)| strategies::rung_key(rung) == key)
}

fn publish(options: &Options) {
    let lines = read_log(options);
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut tables = Vec::new();
    // The runs the window holds, oldest first, with their rated tables.
    let mut runs: Vec<(String, usize)> = Vec::new();
    let mut victories: BTreeMap<String, u32> = BTreeMap::new();
    for line in &lines {
        let seats: Vec<String> = line["seats"]
            .as_array()
            .map(|seats| {
                seats
                    .iter()
                    .filter_map(|seat| seat.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let Some(winner) = line["winner"].as_u64().map(|seat| seat as usize) else {
            continue;
        };
        // An agent that has since left the roster takes its tables with it.
        if seats.len() != options.players
            || winner >= seats.len()
            || !seats.iter().all(|s| known(s))
        {
            continue;
        }
        let seats = seats
            .iter()
            .map(|key| {
                let next = index.len();
                *index.entry(key.clone()).or_insert(next)
            })
            .collect();
        *victories
            .entry(line["victory"].as_str().unwrap_or("unknown").to_string())
            .or_default() += 1;
        tables.push(Table { seats, winner });
        let run = line["run"].as_str().unwrap_or("");
        match runs.iter_mut().find(|(label, _)| label == run) {
            Some((_, count)) => *count += 1,
            None => runs.push((run.to_string(), 1)),
        }
    }
    let reference = options.reference.as_str();
    let Some(&anchor) = index.get(reference) else {
        eprintln!("no rated table seats the reference agent {reference} yet; nothing published");
        return;
    };
    let elo = strategies::fit(index.len(), &tables);
    let sd = strategies::bootstrap_sd(index.len(), &tables, anchor, BOOTSTRAP_ROUNDS, options.seed);
    let mut sat = vec![0_u32; index.len()];
    let mut won = vec![0_u32; index.len()];
    for table in &tables {
        let mut seen = std::collections::BTreeSet::new();
        for seat in &table.seats {
            if seen.insert(*seat) {
                sat[*seat] += 1;
            }
        }
        won[table.seats[table.winner]] += 1;
    }
    let round = |value: f64| (value * 10.0).round() / 10.0;
    let mut ratings = BTreeMap::new();
    for (key, at) in &index {
        ratings.insert(
            key.clone(),
            json!({
                "elo": round(elo[*at] - elo[anchor] + PRINCE_ELO),
                "sd": round(sd[*at]),
                // The 95% margin of error the rating is listed with.
                "margin95": (MARGIN_Z * sd[*at]).round(),
                "games": sat[*at],
                "wins": won[*at],
                // Never lost, or never won: the fit bounds it from one side.
                "bound": if sat[*at] > 0 && won[*at] == sat[*at] {
                    "lower"
                } else if won[*at] == 0 {
                    "upper"
                } else {
                    ""
                },
            }),
        );
    }
    // Prince's rivals are the reference agent with nothing added: one entrant
    // under two names.
    ratings.insert(strategies::rung_key(PRINCE), ratings[reference].clone());
    // The ladder a person is shown, once every rung has been played.
    let rungs: Vec<&Value> = CIV6_RUNGS
        .iter()
        .filter_map(|(rung, _)| ratings.get(&strategies::rung_key(rung)))
        .collect();
    let mut ladder = BTreeMap::new();
    if rungs.len() == CIV6_RUNGS.len() {
        let measured: Vec<f64> = rungs
            .iter()
            .map(|r| r["elo"].as_f64().unwrap_or(0.0))
            .collect();
        let weight: Vec<f64> = rungs
            .iter()
            .map(|r| r["games"].as_f64().unwrap_or(0.0))
            .collect();
        let pin = CIV6_RUNGS
            .iter()
            .position(|(rung, _)| *rung == PRINCE)
            .unwrap_or(0);
        for ((rung, _), elo) in CIV6_RUNGS
            .iter()
            .zip(strategies::monotone_ladder(&measured, &weight, pin))
        {
            ladder.insert(rung.to_string(), round(elo));
        }
    }
    let file = json!({
        "protocol": 1,
        "prince_elo": PRINCE_ELO,
        "reference": reference,
        "players": options.players,
        "turns": options.turns,
        "speed": SPEED,
        "tables": tables.len(),
        "runs": runs
            .iter()
            .map(|(run, count)| json!({"run": run, "tables": count}))
            .collect::<Vec<_>>(),
        "victories": victories,
        "ladder": ladder,
        "ratings": ratings,
    });
    let mut body = serde_json::to_string_pretty(&file).expect("ratings serialise");
    body.push('\n');
    std::fs::write(&options.out, body).expect("the ratings file is writable");

    let mut rows: Vec<(&String, &Value)> = ratings.iter().collect();
    rows.sort_by(|a, b| {
        b.1["elo"]
            .as_f64()
            .unwrap_or(0.0)
            .total_cmp(&a.1["elo"].as_f64().unwrap_or(0.0))
    });
    println!("{} rated tables -> {}", tables.len(), options.out);
    println!("ladder: {}", json!(ladder));
    println!(
        "{:<18} {:>6} {:>5} {:>5} {:>6} {:>5}",
        "entrant", "elo", "sd", "±95%", "tables", "wins"
    );
    for (key, row) in rows {
        println!(
            "{:<18} {:>6.0} {:>5.0} {:>5.0} {:>6} {:>5}",
            key,
            row["elo"].as_f64().unwrap_or(0.0),
            row["sd"].as_f64().unwrap_or(0.0),
            row["margin95"].as_f64().unwrap_or(0.0),
            row["games"],
            row["wins"]
        );
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let options = Options {
        players: number(&args, "--players", 4).clamp(2, ROSTER.len() as u64) as usize,
        turns: number(&args, "--turns", DEFAULT_TURNS as u64).max(1) as u32,
        seed: number(&args, "--seed", 20_261_111),
        jobs: number(&args, "--jobs", parallel::default_jobs() as u64).max(1) as usize,
        games: number(&args, "--games", 0) as usize,
        rung_games: number(&args, "--rung-games", 0) as usize,
        add_games: number(&args, "--add-games", 0) as usize,
        add_rung_games: number(&args, "--add-rung-games", 0) as usize,
        run: text(&args, "--run").filter(|label| !label.is_empty()),
        keep_runs: number(&args, "--keep-runs", 0) as usize,
        log: text(&args, "--log").unwrap_or_else(|| DEFAULT_LOG.to_string()),
        out: text(&args, "--out").unwrap_or_else(|| DEFAULT_OUT.to_string()),
        fit_only: args.iter().any(|arg| arg == "--fit-only"),
        reference: text(&args, "--reference").unwrap_or_else(|| CIV6_REFERENCE.to_string()),
    };
    if strategies::find(&options.reference).map(|row| row.id) != Some(options.reference.as_str()) {
        eprintln!("--reference wants a roster id, not {:?}", options.reference);
        std::process::exit(2);
    }

    let adding = options.add_games > 0 || options.add_rung_games > 0;
    if adding && options.run.is_none() {
        eprintln!("--add-games and --add-rung-games want a --run label to resume under");
        std::process::exit(2);
    }
    if adding && (options.games > 0 || options.rung_games > 0) {
        eprintln!("--add-* adds a labelled run; --games/--rung-games fill totals. Pick one.");
        std::process::exit(2);
    }

    if !options.fit_only {
        let logged = read_log(&options);
        let have = |kind: &str, rung: &str| -> std::collections::BTreeSet<u64> {
            logged
                .iter()
                .filter(|line| line["kind"] == kind && line["rung"] == rung)
                .filter_map(|line| line["index"].as_u64())
                .collect()
        };
        // What each series (the pool, each rung) has to play.
        let mut series: Vec<Vec<Plan>> = Vec::new();
        // A labelled run: each series adds its count after its newest index.
        let added = |kind: &str, rung: &str, count: usize| {
            let run = options.run.as_deref();
            strategies::added_indices(
                logged
                    .iter()
                    .filter(|line| line["kind"] == kind && line["rung"] == rung)
                    .filter_map(|line| {
                        let index = line["index"].as_u64()? as usize;
                        Some((index, line["run"].as_str() == run))
                    }),
                count,
            )
        };
        if adding {
            series.push(
                added("pool", PRINCE, options.add_games)
                    .map(|index| pool_plan(&options, index))
                    .collect(),
            );
            for (order, (rung, _)) in CIV6_RUNGS.iter().enumerate() {
                if *rung == PRINCE {
                    continue;
                }
                series.push(
                    added(RUNG_KIND, rung, options.add_rung_games)
                        .map(|index| rung_plan(&options, order, rung, index))
                        .collect(),
                );
            }
        }
        let played = have("pool", PRINCE);
        series.push(
            (0..options.games)
                .filter(|index| !played.contains(&(*index as u64)))
                .map(|index| pool_plan(&options, index))
                .collect(),
        );
        for (order, (rung, _)) in CIV6_RUNGS.iter().enumerate() {
            if *rung == PRINCE {
                continue;
            }
            let played = have(RUNG_KIND, rung);
            series.push(
                (0..options.rung_games)
                    .filter(|index| !played.contains(&(*index as u64)))
                    .map(|index| rung_plan(&options, order, rung, index))
                    .collect(),
            );
        }
        // Every series advances through its own share together, so a run that
        // is cut short has measured a little of everything instead of all of
        // one thing. A table's index cannot order them: each series counts on
        // from wherever its own log ends.
        let counts: Vec<usize> = series.iter().map(Vec::len).collect();
        let plans: Vec<Plan> = strategies::interleaved(&counts)
            .into_iter()
            .map(|(series_at, place)| series[series_at][place].clone())
            .collect();
        println!(
            "{} tables to play ({} already logged), {} jobs",
            plans.len(),
            logged.len(),
            options.jobs
        );
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&options.log)
            .expect("the game log is writable");
        let started = std::time::Instant::now();
        let mut done = 0;
        for batch in plans.chunks(options.jobs * 2) {
            let results = parallel::map(batch.len(), options.jobs, |at| play(&options, &batch[at]));
            for mut line in results {
                if let Some(run) = &options.run {
                    line["run"] = json!(run);
                }
                writeln!(log, "{line}").expect("the game log is writable");
            }
            log.flush().expect("the game log is writable");
            done += batch.len();
            println!(
                "  {done}/{} tables, {:.0}s",
                plans.len(),
                started.elapsed().as_secs_f64()
            );
        }
    }
    if options.keep_runs > 0 {
        keep_window(&options);
    }
    publish(&options);
}

/// Rewrite the log with only the newest `--keep-runs` runs' tables.
fn keep_window(options: &Options) {
    let Ok(body) = std::fs::read_to_string(&options.log) else {
        return;
    };
    let lines: Vec<(&str, String)> = body
        .lines()
        .filter_map(|line| {
            let value: Value = serde_json::from_str(line).ok()?;
            Some((line, value["run"].as_str().unwrap_or("").to_string()))
        })
        .collect();
    let kept = strategies::kept_runs(lines.iter().map(|(_, run)| run.as_str()), options.keep_runs);
    let mut out = String::new();
    let mut dropped = 0;
    for (line, run) in &lines {
        if kept.contains(run) {
            out.push_str(line);
            out.push('\n');
        } else {
            dropped += 1;
        }
    }
    if dropped > 0 {
        std::fs::write(&options.log, out).expect("the game log is writable");
        println!(
            "window: kept the newest {} runs, dropped {dropped} older tables",
            options.keep_runs
        );
    }
}
