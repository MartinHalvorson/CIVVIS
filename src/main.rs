//! CLI: simulate / soak / benchmark (mirrors the Python CLI outputs).
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use civvis::ai::{run_game, AdvancedAi, Ai};
use civvis::game::{
    default_difficulty, default_speed, Game, GameOptions, LeaderPool, VictoryConditions,
    WarRecord, DEFAULT_DISASTER_INTENSITY, GAME_MODES,
};
use civvis::leader_roster;
use civvis::rules::Rules;
use civvis::setup::{self, BaseRuleset, GameSpeed, MapPoles, MapScript, MapSize, MapTopology};

/// `advanced_v1` — `AdvancedAi::legacy()`, every gene off — freezes the
/// planning configuration, but deliberately shares the production
/// `BasicAi`/`AdvancedAi` implementation. What stops a code edit from silently
/// changing that frozen anchor is [`ANCHOR_BEHAVIOUR_FNV`] below, which pins
/// the anchor's decision stream rather than its source bytes. The anchor used
/// to be the longitudinal Elo tournament's fixed point; the tournament is
/// retired (#2357) and the pin stays, because what it now guards is the gene
/// discipline itself: a change that moves `legacy()` is a change that reached
/// production without a gene in front of it.
///
/// The per-change argument for every edit that has reached those shared files —
/// **1,371 lines of it**, one paragraph per pull request — moved to
/// `docs/ELO_REPINS.md` on 2026-08-17. It was a changelog living in a doc
/// comment, appended at a fixed point by 173 of the 359 commits that touched
/// this file in thirty days, and it is version-control history either way.
/// Nothing was deleted; the entries are the record of what was argued at the
/// time, and new ones still belong there when a change reaches the shared AI
/// files. They no longer have to accompany a constant edit: if `advanced_v1`
/// still plays the same game, the test below stays green on its own.
///
/// What `advanced_v1` DOES, hashed — not what its source looks like.
///
/// ⚠⚠⚠ THIS REPLACES A BYTE HASH OF `src/ai.rs` AND `src/ai/advanced.rs`, AND
/// THE REPLACEMENT IS THE POINT. That hash covered every byte of two files
/// totalling ~70,000 lines — comments and tests included — under an anchor that
/// deliberately SHARES its implementation with the production controller. So a
/// typo fix in a doc comment moved it, and the only way past was to re-pin.
///
/// Measured over the thirty days to 2026-08-17: **248 of ~1,669 merged pull
/// requests had to rewrite that one constant**, and the share was climbing —
/// 14% on 08-04, 40% on 08-15, 48% on 08-16, 6 of 6 on 08-17. Every one of them
/// also appended to an 808-line doc comment above it, and 173 of the 359 commits
/// that touched `main.rs` at all touched nothing else. Both edits land at a fixed
/// point in the file, so concurrent pull requests conflicted structurally rather
/// than occasionally.
///
/// A gate re-pinned reflexively 248 times a month is not protecting anything: it
/// is a ritual. The claim each of those re-pins actually made — "the frozen
/// anchor still plays the same game" — is now the thing that is tested, by
/// playing it. `advanced_v1` runs five profiles from a 2-player 20x14 duel to
/// the 6-player 54x34 deployment shape, and every action it applies is hashed.
///
/// What that buys, measured on this branch:
///
/// * flipping `battlefront_observation` on inside `legacy()` moves ALL FIVE
///   fingerprints and the decision count;
/// * changing `FIRST_MOVE_SCORE_BONUS` from 4.0 to 5.0 moves all five;
/// * adding a default-off field cannot move any of them, because the anchor
///   never reads it — which is exactly what those 248 comments each asserted in
///   prose and nothing checked.
///
/// ⚠ AND WHAT IT DOES NOT BUY, STATED PLAINLY. It catches a change that fires
/// on these profiles. Changing `WATER_MARCH_PENALTY` from 18.0 to 17.0 moved
/// nothing on the four land profiles — which is why `MapScript::Islands` is now
/// one of them, and why a change that fires nowhere in ~17k decisions is a
/// change this test will call free. The byte hash caught those too, but only by
/// catching everything, which is how it stopped being read. The targeted
/// `*_cannot_reach_the_frozen_anchor` tests below remain the second line.
#[cfg(test)]
const ANCHOR_BEHAVIOUR_FNV: u64 = 0x0a41_d781_b45b_b3ae;

/// How many actions the anchor applies across `ANCHOR_PROFILES`. Pinned beside
/// the hash because a fingerprint that moved tells you nothing about how far,
/// and "9,256 decisions rather than 8,959" is a much better first sentence of a
/// diagnosis than a changed 64-bit number.
#[cfg(test)]
const ANCHOR_DECISIONS: usize = 18_400;

fn arg(args: &[String], key: &str, default: i64) -> i64 {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn arg_text(args: &[String], key: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

/// The turn budget the named game speed ships with, which `--turns` overrides.
///
/// Every path that judges which AI is stronger has to play a whole game. A
/// short cap does not just shorten the game, it changes who won: over 9336
/// six-seat league games capped at 250 turns, 81.8% ended on the cap, no game
/// ever ended on a natural score victory, and domination and science never
/// happened at all. Replaying 24 seeds at both budgets, the cap names a
/// different winner in 13 of them.
fn stock_turns(args: &[String]) -> i64 {
    let rules = Rules::embedded();
    let speed = arg_text(args, "--speed", &default_speed());
    rules
        .speeds
        .get(&speed)
        .map(|spec| i64::from(spec.turns))
        .unwrap_or(500)
}

fn victory_conditions(args: &[String]) -> VictoryConditions {
    let Some(enabled) = args
        .iter()
        .position(|value| value == "--victories")
        .and_then(|index| args.get(index + 1))
    else {
        return VictoryConditions::default();
    };
    VictoryConditions::parse(enabled).unwrap_or_else(|why| {
        eprintln!(
            "--victories: {why}; choose from {:?}",
            VictoryConditions::NAMES
        );
        std::process::exit(2);
    })
}

/// A lobby checkbox, spelled the several ways a preset or a script writes one.
fn arg_toggle(args: &[String], key: &str, default: bool) -> bool {
    let Some(value) = args
        .iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
    else {
        return default;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" => true,
        "off" | "false" | "no" | "0" => false,
        other => {
            eprintln!("{key} takes on or off, got {other:?}");
            std::process::exit(2);
        }
    }
}

fn auto_cs(args: &[String], players: i64) -> usize {
    let cs = arg(args, "--city-states", -1);
    if cs < 0 {
        return MapSize::for_players(players.max(1) as usize).default_city_states;
    }
    // The engine seats only as many city-states as the ruleset has distinct
    // identities for, because each one owns a unique Suzerain bonus and two
    // seats sharing a name would share it. Asking for more used to be clamped
    // in silence, which turns a pinned lobby setting into a different game
    // than the one that was set up.
    let named = civvis::game::CITY_STATE_NAMES.len();
    if cs as usize > named {
        eprintln!(
            "--city-states {cs} exceeds the {named} city-states the ruleset carries; \
             each owns a unique Suzerain bonus, so the extra seats cannot be filled"
        );
        std::process::exit(2);
    }
    cs as usize
}

fn auto_dimension(args: &[String], key: &str, players: i64, width: bool) -> i32 {
    // A Tactics arena is not sized like a world. Left to the world ladder,
    // `--map battlefield` produced an eighty-hex "arena" that two eight-unit
    // armies could spend a whole battle failing to find each other on; the
    // mode's own smallest field is the honest default, and `--width` and
    // `--height` still name any other.
    let (default_width, default_height) = if map_script(args).is_battlefield() {
        let script = map_script(args);
        let sizes = setup::battlefield_sizes();
        let arena = sizes
            .iter()
            .find(|size| size.script == script)
            .copied()
            .unwrap_or(sizes[0]);
        // A scenario is drawn at the size of its chart and no other: resizing
        // it would read the chart through a window and lose the coastline the
        // battle was fought against. So this one is not a default, and
        // `--width`/`--height` are declined rather than obeyed.
        if script.is_scenario() {
            return if width { arena.width } else { arena.height };
        }
        (arena.width, arena.height)
    } else {
        // A globe stores itself in a rectangle of its own shape, so the size's
        // default dimensions depend on which world shape was asked for.
        MapSize::for_players(players.max(1) as usize).dimensions(map_topology(args))
    };
    arg(
        args,
        key,
        if width { default_width } else { default_height } as i64,
    ) as i32
}

/// The world type asked for, as every command reads it.
fn map_script(args: &[String]) -> MapScript {
    MapScript::from_id(&arg_text(args, "--map", "tennis_ball")).unwrap_or(MapScript::TeninsBall)
}

/// The world's shape, which is asked for separately from what fills it.
/// Fixed geography changes where the land comes from, not which shape it is
/// sampled onto: even True Start Earth can be a flat atlas or a globe.
fn map_topology(args: &[String]) -> MapTopology {
    // New games open on a globe; Flat remains an explicit opt-in shape.
    MapTopology::from_id(&arg_text(args, "--shape", MapTopology::Planet.id()))
        .unwrap_or(MapTopology::Planet)
}

/// Whether the world has cold ends.
fn map_poles(args: &[String]) -> MapPoles {
    MapPoles::from_id(&arg_text(args, "--poles", "poles")).unwrap_or_default()
}

/// Which published game's rules the world is played by.
fn base_ruleset(args: &[String]) -> BaseRuleset {
    let id = arg_text(args, "--ruleset", BaseRuleset::default().id());
    BaseRuleset::from_id(&id).unwrap_or_else(|| {
        eprintln!("unknown ruleset {id:?}; this build models civ6");
        std::process::exit(2);
    })
}

/// How far into history the game opens.
///
/// A rung of the ladder that is declared but not built yet is refused here
/// rather than quietly played as the Ancient era — the whole point of listing
/// it is that it is not the same game.
/// The era every civilization opens in.
///
/// `--start-era random` is the training lane's answer to overfitting. A
/// Tactics sweep fought only from the Ancient era teaches an AI Ancient-era
/// tactics — slingers and warriors on open ground — and nothing about
/// crossbows behind walls or armour in the open. Varying the opening across
/// the ladder spreads a sweep over the whole unit roster instead.
///
/// The roll comes from the game's own seed rather than a fresh source, so a
/// soak stays exactly reproducible: the same `--start-seed` replays the same
/// eras in the same order. The mix is there because consecutive seeds are
/// consecutive integers, and taking those modulo the ladder length directly
/// would march through the eras in lockstep with the seed instead of
/// scattering them.
/// The arena economy a run plays under, read from the `--tactics-*` flags.
///
/// Every launch path needs its own call because they build their world
/// differently — `soak` and `simulate` through `game_options`, `tournament`
/// through its own per-game `GameOptions`, and `play` through
/// `server::Params` — and each one that forgets accepts the flags and
/// silently ignores them. Both of the others did, in turn; this is the single
/// reader they now share.
fn tactics_rules(args: &[String]) -> setup::TacticsRules {
    let stock = setup::TacticsRules::default();
    setup::TacticsRules {
        cities: arg(args, "--tactics-cities", i64::from(stock.cities)).max(0) as u8,
        production: arg(args, "--tactics-production", i64::from(stock.production)).max(0) as u32,
        gold: arg(args, "--tactics-gold", i64::from(stock.gold)).max(0) as u32,
        turns_per_tech: arg(args, "--tactics-turns-per-tech", i64::from(stock.turns_per_tech))
            .max(0) as u32,
        turn_limit: arg(args, "--tactics-turn-limit", i64::from(stock.turn_limit)).max(0) as u32,
        best_of: arg(args, "--tactics-best-of", i64::from(stock.best_of)).max(1) as u32,
        unique_units: flag_or(args, "--tactics-unique-units", stock.unique_units),
        fog: flag_or(args, "--tactics-fog", stock.fog),
        flag: flag_or(args, "--tactics-flag", stock.flag),
        // The command line's era is `--start-era`, which these flags leave
        // alone: a run's era is part of the experiment, not the economy.
        era: stock.era,
    }
    .sanitized()
}

/// The civilizations named on the command line, in seat order.
fn named_civs(args: &[String]) -> Vec<String> {
    arg_text(args, "--civs", &arg_text(args, "--civ", ""))
        .split(',')
        .map(|civ| civ.trim().to_string())
        .filter(|civ| !civ.is_empty())
        .collect()
}

/// Which roster the seats are drawn from.
fn leader_pool(args: &[String]) -> LeaderPool {
    let id = arg_text(args, "--leader-pool", LeaderPool::default().id());
    let pool = LeaderPool::from_id(&id).unwrap_or_else(|| {
        eprintln!("unknown leader pool {id:?}; choose civ6, historical, or today");
        std::process::exit(2);
    });
    if !pool.is_available() {
        eprintln!("leader pool {id:?} has no supplied roster data yet");
        std::process::exit(2);
    }
    pool
}

/// The civilizations a Tactics match is between, resolved the same way the
/// engine seats them.
///
/// A match has to know its two contenders *before* the first battle, because
/// it swaps the sides over between battles and keeps the score by
/// civilization. Naming them explicitly for every battle also makes the
/// pairing a property of the match rather than of the seating order the stock
/// fill happened to produce.
fn match_contenders(args: &[String], players: i64, chosen: &[String]) -> Vec<String> {
    let rules = Rules::embedded();
    let mut known: std::collections::BTreeSet<civvis::name::Name> =
        rules.civs.keys().cloned().collect();
    known.extend(
        leader_roster::all()
            .iter()
            .filter(|record| record.available)
            .map(|record| civvis::name::Name::new(&record.civ)),
    );
    civvis::game::seat_civs(players.max(1) as usize, chosen, &known, leader_pool(args))
}

/// An on/off flag that keeps its default when absent, and reads the usual
/// spellings of both answers when present: `--flag`, `--flag on|off`,
/// `true|false`, `yes|no`, `1|0`.
fn flag_or(args: &[String], key: &str, default: bool) -> bool {
    let Some(index) = args.iter().position(|arg| arg == key) else {
        return default;
    };
    match args.get(index + 1).map(String::as_str) {
        Some("on" | "true" | "yes" | "1") | None => true,
        Some("off" | "false" | "no" | "0") => false,
        // A value that is not an answer is the next flag: `--tactics-unique-
        // units --games 8` asks for unique units and eight games.
        Some(_) => true,
    }
}

fn start_era(args: &[String], seed: u64) -> usize {
    let id = arg_text(args, "--start-era", setup::stock_start_era_id());
    if id == "random" {
        return setup::random_start_era(seed);
    }
    setup::start_era_from_id(&id).unwrap_or_else(|| {
        let playable: Vec<&str> = setup::playable_start_eras().map(|spec| spec.id).collect();
        let known = setup::START_ERAS.iter().any(|spec| spec.id == id);
        if known {
            eprintln!("cannot open in the {id:?} era yet; choose one of: {}", playable.join(", "));
        } else {
            eprintln!("unknown start era {id:?}; choose one of: {}", playable.join(", "));
        }
        std::process::exit(2);
    })
}

/// Which rules the far end of the game is played by.
///
/// Same contract as `--start-era`: an era that is declared but not built is
/// refused rather than quietly played as the classic one. The Modified Future
/// Era can also be had as what it is made of — `--mods
/// mods/modified-future-era` loads the same overlay off disk.
fn future_era(args: &[String]) -> setup::FutureEra {
    let id = arg_text(args, "--future-era", setup::FutureEra::default().id());
    setup::future_era_from_id(&id).unwrap_or_else(|| {
        let playable: Vec<&str> = setup::FUTURE_ERAS
            .iter()
            .filter(|spec| spec.is_playable())
            .map(|spec| spec.id)
            .collect();
        eprintln!("unknown Future Era {id:?}; choose one of: {}", playable.join(", "));
        std::process::exit(2);
    })
}

/// Whether the seats of a game turn act one after another or plan against a
/// shared snapshot and commit together. Same contract as the eras: an unknown
/// id is refused rather than quietly played as some stock regime.
///
/// The default is the caller's to name, but today every caller names
/// `Sequential`: the product is hard-committed to sequential turns, and the
/// simultaneous driver is a retained research regime reached only through
/// this explicit flag. `TurnStructure::default()` itself stays `Sequential`
/// as the save-compatibility and setup-contract anchor.
fn turn_structure(args: &[String], default: setup::TurnStructure) -> setup::TurnStructure {
    let id = arg_text(args, "--turn-structure", default.id());
    setup::turn_structure_from_id(&id).unwrap_or_else(|| {
        let known: Vec<&str> = setup::TURN_STRUCTURES.iter().map(|spec| spec.id).collect();
        eprintln!("unknown turn structure {id:?}; choose one of: {}", known.join(", "));
        std::process::exit(2);
    })
}

/// Difficulty and speed are chosen the same way everywhere: by name, against
/// the shipped ruleset, with the stock levels as defaults. The turn-structure
/// default is the command's own (see [`turn_structure`]).
fn game_options(
    args: &[String],
    players: i64,
    seed: u64,
    default_structure: setup::TurnStructure,
) -> GameOptions {
    let rules = Rules::embedded();
    let difficulty = arg_text(args, "--difficulty", &default_difficulty());
    if !rules.difficulties.contains_key(&difficulty) {
        eprintln!(
            "unknown difficulty {difficulty:?}; choose one of {:?}",
            ladder(&rules)
        );
        std::process::exit(2);
    }
    let speed = arg_text(args, "--speed", &default_speed());
    let Some(speed_spec) = rules.speeds.get(&speed) else {
        eprintln!("unknown game speed {speed:?}; choose one of {:?}", speeds(&rules));
        std::process::exit(2);
    };
    let tactics = tactics_rules(args);
    // An explicit --turns wins; otherwise every speed brings its own stock
    // budget (Standard is 500 turns / 2050 AD). Short historical defaults
    // ended games at the turn limit before the science, culture, and
    // diplomatic lanes could finish, which handed the win to whoever was
    // ahead on score at an arbitrary cutoff.
    let turns = if args.iter().any(|a| a == "--turns") {
        arg(args, "--turns", speed_spec.turns as i64)
    } else if map_script(args).is_battlefield() {
        // A Tactics battle keeps a battle's clock rather than a game speed's
        // five hundred turns. Its own four-step ladder names the stock
        // deadline; the general `--turns` flag above still wins explicitly.
        i64::from(tactics.turn_limit)
    } else {
        speed_spec.turns as i64
    };
    let player_count = players.max(1) as usize;
    let teams_arg = arg_text(args, "--teams", "");
    let teams = if teams_arg.trim().is_empty() {
        Vec::new()
    } else {
        let parsed: Result<Vec<Option<usize>>, _> = teams_arg
            .split(',')
            .map(|team| {
                let team = team.trim();
                if team.is_empty() || team == "-" {
                    Ok(None)
                } else {
                    team.parse::<usize>().map(Some)
                }
            })
            .collect();
        let teams = parsed.unwrap_or_else(|_| {
            eprintln!("invalid --teams value {teams_arg:?}; use comma-separated team numbers or -");
            std::process::exit(2);
        });
        if teams.len() != player_count {
            eprintln!(
                "--teams needs exactly {player_count} entries (one per major player), got {}",
                teams.len()
            );
            std::process::exit(2);
        }
        teams
    };
    let leader_pool = leader_pool(args);
    GameOptions {
        base_ruleset: base_ruleset(args),
        start_era: start_era(args, seed),
        // ⚠⚠ `--victories` PARSED, VALIDATED, AND THEN REACHED ONE SUBCOMMAND.
        //
        // `victory_conditions` had exactly one caller — the `play` server's
        // session config — so `simulate`, `soak`, `odds-audit`, `benchmark`,
        // `rollouts` and `selfplay` all took `GameOptions::new`'s default of
        // all six on, whatever the command line said. The flag is parsed early
        // and `exit(2)`s on a bad name, so it looked like it worked: a run
        // asking for a two-lane game was refused for a typo and then silently
        // given six lanes.
        //
        // That is the expensive shape of a footgun rather than the cheap one.
        // A soak restricted to score and science measures a different game from
        // one where religion can end it at turn 90 — `victory_eval` at the
        // ladder profile finishes religion in 8 of 16 games — so the difference
        // is not cosmetic, and nothing in the output said which game had been
        // played.
        victory_conditions: victory_conditions(args),
        // The arena's economy, so a headless sweep can vary the thing it is
        // training against without going through a lobby. Ignored on a world.
        tactics,
        future_era: future_era(args),
        map_script: map_script(args),
        map_topology: map_topology(args),
        map_poles: map_poles(args),
        difficulty,
        speed,
        // A headless game has nobody at the keyboard, so the difficulty only
        // reaches the AI side of the ladder unless a seat is named human.
        human_seats: arg_text(args, "--human-seats", "")
            .split(',')
            .filter_map(|seat| seat.trim().parse().ok())
            .collect(),
        teams,
        leader_pool,
        // Who the player is. `--civ Egypt` seats Egypt at seat 0; `--civs
        // Egypt,Rome` names the leading seats in order. Anything unnamed
        // falls back to the selected stock roster, and a name outside that
        // roster is refused here rather than silently ignored downstream.
        civs: {
            let named = arg_text(args, "--civs", &arg_text(args, "--civ", ""));
            let chosen: Vec<String> = named
                .split(',')
                .map(|civ| civ.trim().to_string())
                .filter(|civ| !civ.is_empty())
                .collect();
            for civ in &chosen {
                if !leader_roster::entry(civ).is_some_and(|entry| {
                    entry.available && entry.pool == leader_pool
                }) {
                    let mut known: Vec<&str> = leader_pool
                        .entries()
                        .map(|entry| entry.civ.as_str())
                        .collect();
                    known.sort_unstable();
                    eprintln!(
                        "civilization {civ:?} is not available in {}: choose one of {known:?}",
                        leader_pool.name()
                    );
                    std::process::exit(2);
                }
            }
            chosen
        },
        // Gathering Storm's lobby slider: 0 turns random disasters off,
        // 4 is Hyperreal. Sea-level rise follows CO2 either way.
        disaster_intensity: {
            let intensity = arg(args, "--disasters", i64::from(DEFAULT_DISASTER_INTENSITY));
            if !(0..=4).contains(&intensity) {
                eprintln!("--disasters takes 0 (none) to 4 (hyperreal), got {intensity}");
                std::process::exit(2);
            }
            intensity as u8
        },
        // A lobby checkbox like any other: competitive team events play with
        // barbarians off, so a preset has to be able to say so.
        barbarians: arg_toggle(args, "--barbarians", true),
        // Off is the stock Gathering Storm ruleset and what every tournament
        // lobby plays; naming a mode is an opt-in to New Frontier content.
        game_modes: {
            let requested = arg_text(args, "--game-modes", "");
            let modes: BTreeSet<String> = requested
                .split(',')
                .map(str::trim)
                .filter(|mode| !mode.is_empty())
                .map(str::to_string)
                .collect();
            for mode in &modes {
                if !GAME_MODES.contains(&mode.as_str()) {
                    eprintln!("unknown game mode {mode:?}; choose from {GAME_MODES:?}");
                    std::process::exit(2);
                }
            }
            modes
        },
        turn_structure: turn_structure(args, default_structure),
        ..GameOptions::new(
            player_count,
            auto_dimension(args, "--width", players, true),
            auto_dimension(args, "--height", players, false),
            seed,
            turns as u32,
            auto_cs(args, players),
        )
    }
}

fn ladder(rules: &Rules) -> Vec<&str> {
    let mut names: Vec<&str> = rules.difficulties.keys().map(|k| k.as_str()).collect();
    names.sort_by_key(|name| rules.difficulties[*name].order);
    names
}

fn speeds(rules: &Rules) -> Vec<&str> {
    let mut names: Vec<&str> = rules.speeds.keys().map(|k| k.as_str()).collect();
    names.sort_by_key(|name| rules.speeds[*name].order);
    names
}

fn standings(g: &Game) {
    if g.is_draw() {
        println!("Draw: turn limit reached on turn {}", g.reported_turn());
    }
    match g.winner {
        Some(winner) => {
            let w = &g.players[winner];
            // The label, not the bare type: this line is how a played game
            // announces its result, and a Mercy Rule ending has a lane to
            // name. The fixed-width victory columns in the batch and audit
            // tables below keep the type — they are a tabulation to scan, and
            // one of them is a key things are counted under.
            println!(
                "Winner: {} (player {}) by {} on turn {}",
                w.civ,
                w.id,
                g.victory_label().unwrap_or_default(),
                g.reported_turn()
            );
        }
        None if !g.is_draw() => println!(
            "No winner: turn {} of {}, and no enabled victory was achieved",
            g.turn, g.max_turns
        ),
        None => {}
    }
    let mut majors: Vec<usize> = g
        .players
        .iter()
        .filter(|p| !p.is_minor)
        .map(|p| p.id)
        .collect();
    majors.sort_by_key(|pid| -g.score(*pid));
    for pid in majors {
        let p = &g.players[pid];
        let cities = g.player_city_ids(pid);
        let pop: i32 = cities.iter().map(|c| g.cities[c].pop).sum();
        // The army roster is the one part of an empire the score never shows,
        // and the place where a missing rule hides longest.
        let mut roster: BTreeMap<&str, usize> = BTreeMap::new();
        for unit in g.units.values() {
            if unit.owner == pid && g.rules.units[unit.kind].class == "military" {
                *roster.entry(unit.kind.as_str()).or_default() += 1;
            }
        }
        let mut army: Vec<(&str, usize)> = roster.into_iter().collect();
        army.sort_by_key(|(kind, count)| (std::cmp::Reverse(*count), *kind));
        let army: Vec<String> = army
            .iter()
            .map(|(kind, count)| {
                let stale = if g.unit_is_obsolete(pid, civvis::name::Name::new(kind)) { "*" } else { "" };
                format!("{count}x{kind}{stale}")
            })
            .collect();
        println!(
            "  {:<10} score={:<4} cities={} pop={} techs={} {}",
            p.civ,
            g.score(pid),
            cities.len(),
            pop,
            p.techs.len(),
            if p.alive { "" } else { "(eliminated)" }
        );
        if !army.is_empty() {
            println!("             army: {}", army.join(" "));
        }
    }
    let minors: Vec<&str> = g
        .players
        .iter()
        .filter(|p| p.is_minor && !p.is_barbarian)
        .map(|p| p.civ.as_str())
        .collect();
    if !minors.is_empty() {
        println!("  City-states: {}", minors.join(", "));
    }
}

/// Available batch workers default to one per core. An explicit `--jobs`
/// always wins, while a single simulation has its own bounded default below.
fn jobs_arg(args: &[String]) -> usize {
    let requested = arg(args, "--jobs", 0);
    if requested > 0 {
        requested as usize
    } else {
        civvis::parallel::default_jobs()
    }
}

/// Independent frontiers inside one simulation share cloned worlds, so their
/// useful parallelism reaches its measured knee before every host core. Keep
/// an explicit `--jobs` authoritative and leave outer multi-game workloads on
/// [`jobs_arg`]'s one-core-per-job default.
const SINGLE_SIMULATION_DEFAULT_MAX_JOBS: usize = 4;

fn single_simulation_jobs_arg(args: &[String]) -> usize {
    let requested = arg(args, "--jobs", 0);
    if requested > 0 {
        requested as usize
    } else {
        civvis::parallel::default_jobs().min(SINGLE_SIMULATION_DEFAULT_MAX_JOBS)
    }
}

/// Split one process-wide worker budget across a simultaneous soak's active
/// games. The first `extra` game indices receive one additional seat planner,
/// making the total exact without letting nested game and seat fan-outs
/// oversubscribe the host.
fn simultaneous_soak_job_split(games: usize, jobs: usize) -> (usize, usize, usize) {
    let jobs = jobs.max(1);
    let concurrent_games = games.max(1).min(jobs);
    (
        concurrent_games,
        jobs / concurrent_games,
        jobs % concurrent_games,
    )
}

/// ★★★★★ A FLAG NOTHING HONOURS IS REFUSED, NOT IGNORED.
///
/// `--ais` seated the entrants of `civvis tournament`, and that command is
/// retired with the Elo ledger (#2357). The flag used to read as global
/// because it sat in the one shared usage line, and `civvis simulate --ais
/// basic,basic,…` played a full game with `AdvancedAi` in every seat and
/// printed a winner with no indication that the instruction had been dropped
/// — it answered a different question and said nothing, which cost a whole
/// line of investigation once. So it still fails loudly.
fn refuse_unhonoured_entrant_flag(cmd: &str, args: &[String]) {
    if !args.iter().any(|arg| arg == "--ais") {
        return;
    }
    eprintln!(
        "--ais is not honoured by `{cmd}`: it seats every chair with the default controller. \
         The tournament that read it is retired (#2357); remove the flag."
    );
    std::process::exit(2);
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("help");
    // Mods replace the ruleset for the whole process, so they have to be
    // installed before anything reads it.
    let mod_paths = civvis::mods::parse_arg(&arg_text(&args, "--mods", ""));
    if !mod_paths.is_empty() {
        match civvis::mods::activate(&mod_paths) {
            Ok(loaded) => {
                for info in loaded {
                    let about = if info.description.is_empty() {
                        String::new()
                    } else {
                        format!(" — {}", info.description)
                    };
                    println!("mod: {} ({}){about}", info.name, info.files.join(", "));
                }
            }
            Err(error) => {
                eprintln!("{error}");
                std::process::exit(2);
            }
        }
    }
    refuse_unhonoured_entrant_flag(cmd, &args);
    match cmd {
        "simulate" => {
            let players = arg(&args, "--players", 4);
            let g0 = Instant::now();
            // The product is hard-committed to sequential turns, so every
            // command defaults to the regime the shipped game plays;
            // `--turn-structure simultaneous` remains the explicit research
            // escape hatch into the retained driver.
            let mut g = Game::new_with(game_options(
                &args,
                players,
                arg(&args, "--seed", 0) as u64,
                setup::TurnStructure::Sequential,
            ));
            // ⚠ Set here rather than carried in `GameOptions`, deliberately.
            // This is a staged rules mechanism, not a setup-screen choice: it
            // ships off, and either becomes unconditional on promotion — at
            // which point the flag and this line both go — or is removed.
            // Threading it through every `GameOptions` construction would be
            // churn in four files that promotion has to undo. See
            // `Game::native_competitions`.
            g.native_competitions = args.iter().any(|a| a == "--native-competitions");
            // The two regimes want opposite parallelism. Sequential seats
            // cannot deliberate concurrently, so `--jobs` feeds the clone-
            // heavy WorkPool frontiers inside one seat's turn, whose measured
            // knee caps the default at four. Simultaneous seats deliberate
            // independently by construction, so `--jobs` fans whole seats out
            // instead — one clone buys a whole turn of deliberation, past the
            // knee — and the AIs skip the inner pool rather than stack the
            // two layers.
            let census = if g.turn_structure == setup::TurnStructure::Simultaneous {
                let jobs = jobs_arg(&args);
                let mut ais = AdvancedAi::fleet(&g);
                civvis::simultaneous::run_structured_jobs(&mut g, &mut ais, jobs)
            } else {
                let jobs = single_simulation_jobs_arg(&args);
                let mut ais = AdvancedAi::fleet_parallel(&g, jobs);
                civvis::simultaneous::run_structured(&mut g, &mut ais)
            };
            println!("[{:.3}s]", g0.elapsed().as_secs_f64());
            if let Some(census) = census {
                println!("{}", census.summary());
            }
            standings(&g);
        }
        "soak" => {
            let players = arg(&args, "--players", 4);
            let games = arg(&args, "--games", 10);
            let start = arg(&args, "--start-seed", 0);
            let jobs = jobs_arg(&args);
            let simultaneous = turn_structure(&args, setup::TurnStructure::Sequential)
                == setup::TurnStructure::Simultaneous;
            // A sequential soak has only one useful frontier: independent
            // games. Simultaneous games have a second one inside each game —
            // every ready civilization can plan at once. Keep the total
            // worker budget bounded by splitting it across the games that can
            // be live at once, then hand each game's share to its persistent
            // seat-planning fleet. With one large simultaneous game, this
            // therefore reaches every requested core instead of treating the
            // one outer job as a reason to run all of its seats serially.
            let (concurrent_games, jobs_per_game, extra_seat_workers) = if simultaneous {
                simultaneous_soak_job_split(games as usize, jobs)
            } else {
                (jobs, 1, 0)
            };
            // A Tactics match: the same two civilizations over a series of
            // battles, sides swapped between them. `--games` is how many
            // battles are actually played, so a best-of-5 run short of five
            // games simply reports the score it reached.
            let arena = map_script(&args).is_battlefield();
            let match_rules = tactics_rules(&args);
            let contenders = (arena && match_rules.best_of > 1)
                .then(|| match_contenders(&args, players, &named_civs(&args)));
            // Each game is played on an outer worker, then described on the
            // main one, so a soak reads exactly as it did when it was serial.
            let lines = civvis::parallel::map(games as usize, concurrent_games, |index| {
                let seed = start + index as i64;
                let t0 = Instant::now();
                let contenders = contenders.clone();
                let result = std::panic::catch_unwind(|| {
                    let mut options = game_options(
                        &args,
                        players,
                        seed as u64,
                        setup::TurnStructure::Sequential,
                    );
                    if let Some(contenders) = contenders {
                        // The sides change ends at half time, and every
                        // battle after it. Otherwise a series measures the
                        // corner one civilization kept sitting in as much as
                        // it measures the civilization.
                        options.civs = contenders;
                        if index % 2 == 1 {
                            options.civs.reverse();
                        }
                    }
                    let mut g = Game::new_with(options);
                    let mut ais = AdvancedAi::fleet(&g);
                    let simultaneous = if g.turn_structure == setup::TurnStructure::Simultaneous {
                        // Spread a non-divisible budget across the first live
                        // games; later replacements get the base share, so
                        // the running total never exceeds `--jobs`.
                        let seat_jobs = jobs_per_game + usize::from(index < extra_seat_workers);
                        civvis::simultaneous::run_structured_jobs(&mut g, &mut ais, seat_jobs)
                    } else {
                        civvis::simultaneous::run_structured(&mut g, &mut ais)
                    };
                    // Every major's turns, pooled: what the empires in this game
                    // actually spent the game doing.
                    let mut census = civvis::ai::StrategyCensus::default();
                    for ai in ais.iter().take(g.players.iter().filter(|p| !p.is_minor).count()) {
                        census.absorb(&ai.strategy_census());
                    }
                    (g, census, simultaneous)
                });
                match result {
                    Ok((g, census, simultaneous)) => {
                        let majors: Vec<_> = g.players.iter().filter(|p| !p.is_minor).collect();
                        let minors: Vec<_> = g
                            .players
                            .iter()
                            .filter(|p| p.is_minor && !p.is_barbarian)
                            .collect();
                        // A soak line describes a terminal result. Tactics
                        // draws carry no winner but are finished battles.
                        let w = g.winner.map(|winner| &g.players[winner]);
                        let mut flags = String::new();
                        if let Some(simultaneous) = &simultaneous {
                            flags.push_str(&format!(
                                " SIMUL drops={}/{}{}",
                                simultaneous.dropped,
                                simultaneous.planned,
                                if simultaneous.aborted { " ABORTED" } else { "" }
                            ));
                        }
                        if majors.iter().all(|p| p.techs.len() <= 2) {
                            flags.push_str(" NO-TECH-PROGRESS");
                        }
                        if w.is_some_and(|w| w.is_minor) {
                            flags.push_str(" MINOR-WINNER");
                        }
                        if g.is_draw() {
                            flags.push_str(" DRAW");
                        } else if w.is_none() {
                            flags.push_str(" NO-WINNER");
                        }
                        // An army nobody ever modernizes is invisible in the
                        // standings and obvious on the map. Count the units
                        // still fielded after their owner retired them, and
                        // the ones three eras behind the world besides.
                        let unit_era = |kind: &str| -> usize {
                            let spec = &g.rules.units[kind];
                            let tech = spec
                                .tech
                                .as_deref()
                                .and_then(|node| g.rules.techs.get(node))
                                .map(|node| node.era);
                            let civic = spec
                                .civic
                                .as_deref()
                                .and_then(|node| g.rules.civics.get(node))
                                .map(|node| node.era);
                            tech.or(civic).unwrap_or(0)
                        };
                        let (obsolete, ancient, army) = majors
                            .iter()
                            .filter(|p| p.alive)
                            .flat_map(|p| {
                                g.units.values().filter(move |unit| unit.owner == p.id)
                            })
                            .filter(|unit| g.rules.units[unit.kind].class == "military")
                            .fold((0, 0, 0), |(obsolete, ancient, army), unit| {
                                (
                                    obsolete + g.unit_is_obsolete(unit.owner, unit.kind) as i32,
                                    ancient
                                        + (g.world_era.saturating_sub(unit_era(&unit.kind)) >= 3)
                                            as i32,
                                    army + 1,
                                )
                            });
                        flags.push_str(&format!(
                            " ARMY {army} obsolete={obsolete} ancient={ancient} era={}",
                            g.world_era
                        ));
                        // A war nobody ever wins is as invisible as an army
                        // nobody modernizes: the standings only show who was
                        // left standing, so a game where every declaration
                        // ended in a white peace reads exactly like a game of
                        // uninterrupted peace. Count what the declarations
                        // actually achieved.
                        // `Game::wars` holds only the wars still running:
                        // `close_war_record` removes a finished one and pushes
                        // it to `concluded_wars`. Reading the live map alone
                        // therefore hid every war that ended — which is every
                        // war that was *won*, and every white peace this block
                        // was written to make legible. Measured over eight
                        // six-player games it saw 6 of 39 declarations, 96 of
                        // 317 unit losses, and 0 of 13 city captures, while
                        // `ended_in_peace` could not be anything but zero.
                        let all_wars: Vec<&WarRecord> =
                            g.wars.values().chain(g.concluded_wars.iter()).collect();
                        let wars = all_wars.len();
                        let (units_lost, cities_taken) = all_wars.iter().fold(
                            (0u32, 0u32),
                            |(units, cities), war| {
                                (
                                    units + war.losses.values().map(|side| side.units).sum::<u32>(),
                                    cities
                                        + war.losses.values().map(|side| side.cities).sum::<u32>(),
                                )
                            },
                        );
                        let capitals_taken = all_wars
                            .iter()
                            .flat_map(|war| war.highlights.iter())
                            .filter(|highlight| highlight.kind == "capital_captured")
                            .count();
                        // How long the declarations lasted, because a war that
                        // ends in a handful of turns cannot take a walled city
                        // whatever army was pointed at it.
                        let (turns_at_war, ended) = all_wars.iter().fold(
                            (0u32, 0usize),
                            |(turns, ended), war| {
                                let stop = war.ended.unwrap_or(g.turn);
                                (
                                    turns + stop.saturating_sub(war.started),
                                    ended + war.ended.is_some() as usize,
                                )
                            },
                        );
                        let mean_war = if wars > 0 {
                            turns_at_war as f64 / wars as f64
                        } else {
                            0.0
                        };
                        // Every kind of thing the declarations produced, so a war
                        // that only ever produces its own declaration is
                        // legible as exactly that.
                        let mut events: BTreeMap<&str, usize> = BTreeMap::new();
                        for highlight in all_wars.iter().flat_map(|war| war.highlights.iter()) {
                            *events.entry(highlight.kind.as_str()).or_default() += 1;
                        }
                        let events: Vec<String> = events
                            .iter()
                            .map(|(kind, count)| format!("{kind}:{count}"))
                            .collect();
                        flags.push_str(&format!(
                            " WAR {wars} units_lost={units_lost} cities_taken={cities_taken} \
                             capitals_taken={capitals_taken} mean_turns={mean_war:.0} \
                             ended_in_peace={ended} events=[{}]",
                            events.join(" ")
                        ));
                        // A war is only prosecuted if somebody chose to
                        // prosecute it. Recovery is the defensive posture, so
                        // turns spent there are turns nobody was besieging
                        // anything.
                        let total = census.total().max(1);
                        let share = |turns: u32| 100 * turns / total;
                        flags.push_str(&format!(
                            " PLAN conquest={}% recovery={}% expansion={}% science={}% \
                             culture={}% religion={}% diplomacy={}%",
                            share(census.conquest),
                            share(census.recovery),
                            share(census.expansion),
                            share(census.science),
                            share(census.culture),
                            share(census.religion),
                            share(census.diplomacy),
                        ));
                        let posture_total = census.posture_total().max(1);
                        let pshare = |turns: u32| 100 * turns / posture_total;
                        flags.push_str(&format!(
                            " FORCE engage={}% advance={}% hold={}% muster={}% recover={}%",
                            pshare(census.engage),
                            pshare(census.advance),
                            pshare(census.hold),
                            pshare(census.muster),
                            pshare(census.recover),
                        ));
                        flags.push_str(&format!(
                            " SIEGE blows={} damage={} walls_breached={} cities_reduced={} \
                             left_depleted={} taker_ready={} melee_was_there={}",
                            g.siege.blows,
                            g.siege.damage,
                            g.siege.walls_breached,
                            g.siege.cities_reduced,
                            g.siege.left_depleted,
                            g.siege.depleted_with_a_taker_ready,
                            g.siege.reduced_with_melee_adjacent,
                        ));
                        let held = (census.hold_threatened + census.hold_weak).max(1);
                        flags.push_str(&format!(
                            " HELD_BY threatened_city={}% locally_weak={}%",
                            100 * census.hold_threatened / held,
                            100 * census.hold_weak / held,
                        ));
                        if census.step_reassessed > 0 {
                            flags.push_str(&format!(
                                " REASSESS blind_cuts={}",
                                census.step_reassessed,
                            ));
                        }
                        Some((w.map(|w| w.civ.clone()), format!(
                            "seed {:3}  t{:<4} {:<10} {:<8} majors_alive={}/{} cities={:<2} cs_alive={}/{} [{:.2}s]{}",
                            seed,
                            g.reported_turn(),
                            g.victory_type.clone().unwrap_or_default(),
                            w.map_or("-", |w| w.civ.as_str()),
                            majors.iter().filter(|p| p.alive).count(),
                            majors.len(),
                            g.cities.len(),
                            minors.iter().filter(|p| p.alive).count(),
                            minors.len(),
                            t0.elapsed().as_secs_f64(),
                            flags
                        )))
                    }
                    Err(_) => None,
                }
            });
            let mut fails = 0;
            let mut series = contenders
                .map(|civs| setup::MatchSeries::new(match_rules.best_of, civs));
            for (index, line) in lines.into_iter().enumerate() {
                match line {
                    Some((winner, line)) => {
                        println!("{line}");
                        if let Some(series) = series.as_mut() {
                            // A match stops at the battle that settles it;
                            // the rest of `--games` is a dead rubber and is
                            // reported as unplayed rather than counted.
                            if !series.decided() {
                                series.record(winner.as_deref());
                            }
                        }
                    }
                    None => {
                        fails += 1;
                        println!("seed {:3}  CRASH (panic)", start + index as i64);
                    }
                }
            }
            println!("\n{}/{} games completed", games - fails, games);
            if let Some(series) = series {
                let verdict = match series.winner() {
                    Some(civ) => format!("{civ} takes the match"),
                    None if series.played() >= series.best_of => "match drawn".to_string(),
                    None => format!(
                        "match unfinished: {} of {} battles played",
                        series.played(),
                        series.best_of
                    ),
                };
                println!("best of {}: {} — {verdict}", series.best_of, series.scoreline());
            }
            if fails > 0 {
                std::process::exit(1);
            }
        }
        // Would ending decided games early change who wins? 62% of audited
        // league games ran to the turn cap, where the winner is whoever is
        // biggest — most of that tail is compute spent on a settled outcome.
        // The spectator ribbon already carries a calibrated live win
        // probability (`odds::table`, Brier- and log-loss-checked at three
        // phases of completed games). This audit plays every game to its real
        // end while recording, per threshold, the first world turn the odds
        // leader crossed it — so agreement between the crossing pick and the
        // played-out winner, and the turns adjudication would have saved, are
        // measured exactly rather than estimated. It changes no outcome and
        // is the pre-registered evidence gate for enabling adjudication in
        // any loop (docs/ADJUDICATION.md).
        "odds-audit" => {
            let players = arg(&args, "--players", 6);
            let games = arg(&args, "--games", 40);
            let start = arg(&args, "--start-seed", 0);
            let jobs = jobs_arg(&args);
            let start_turn = arg(&args, "--adjudicate-start", 100) as u32;
            let every = (arg(&args, "--every", 5).max(1)) as u32;
            let thresholds: Vec<f64> = arg_text(&args, "--thresholds", "0.90,0.95,0.98,0.995")
                .split(',')
                .map(|t| t.trim().parse::<f64>().expect("--thresholds wants numbers"))
                .collect();
            println!(
                "odds-audit: {games} games, {players} players, sampling from turn \
                 {start_turn} every {every}, thresholds {thresholds:?}"
            );
            type GameAudit = (Option<usize>, Option<String>, u32, Vec<Option<(usize, u32)>>);
            let results: Vec<Option<GameAudit>> =
                civvis::parallel::map(games as usize, jobs, |index| {
                    let seed = start + index as i64;
                    std::panic::catch_unwind(|| {
                        let mut g = Game::new_with(game_options(
                            &args,
                            players,
                            seed as u64,
                            setup::TurnStructure::Sequential,
                        ));
                        let mut ais = AdvancedAi::fleet(&g);
                        // Same display-state elisions as `run_game`: this is a
                        // headless rollout and the odds read none of them.
                        g.set_fog_memory(false);
                        g.set_war_ledger(false);
                        let mut crossings: Vec<Option<(usize, u32)>> =
                            vec![None; thresholds.len()];
                        let mut last_turn = g.turn;
                        while g.winner.is_none() && g.turn <= g.max_turns {
                            let pid = g.current;
                            ais[pid].take_turn(&mut g, pid);
                            if g.winner.is_none() && g.current == pid {
                                let _ = g.apply(pid, &civvis::game::Action::EndTurn);
                            }
                            if g.turn == last_turn {
                                continue;
                            }
                            last_turn = g.turn;
                            let due = g.turn >= start_turn && (g.turn - start_turn) % every == 0;
                            if !due || g.winner.is_some() || crossings.iter().all(Option::is_some)
                            {
                                continue;
                            }
                            // A flat 1500 prior for every seat: the audit runs
                            // stock fleets with no roster, so only the board
                            // terms — score, military, cities, victory races,
                            // and the clock — separate the table.
                            let table = civvis::odds::table(&g, |_pid| 1500.0f64);
                            let Some((leader, seat)) =
                                table.iter().max_by(|a, b| a.1.now.total_cmp(&b.1.now))
                            else {
                                continue;
                            };
                            for (slot, threshold) in thresholds.iter().enumerate() {
                                if crossings[slot].is_none() && seat.now >= *threshold {
                                    crossings[slot] = Some((*leader, g.turn));
                                }
                            }
                        }
                        let end_turn = g.turn.min(g.max_turns);
                        (g.winner, g.victory_type.clone(), end_turn, crossings)
                    })
                    .ok()
                });
            let mut crashes = 0;
            let mut finished: Vec<(i64, GameAudit)> = Vec::new();
            for (index, result) in results.into_iter().enumerate() {
                let seed = start + index as i64;
                match result {
                    Some(audit) => finished.push((seed, audit)),
                    None => {
                        crashes += 1;
                        println!("seed {seed:4}  CRASH (panic)");
                    }
                }
            }
            for (seed, (winner, victory, end_turn, crossings)) in &finished {
                let victory = victory.as_deref().unwrap_or("none");
                let verdicts: Vec<String> = crossings
                    .iter()
                    .zip(&thresholds)
                    .map(|(crossing, threshold)| match crossing {
                        Some((pid, turn)) => format!(
                            "{threshold}:t{turn}{}",
                            if Some(*pid) == *winner { "=" } else { "!" }
                        ),
                        None => format!("{threshold}:-"),
                    })
                    .collect();
                println!(
                    "seed {seed:4}  t{end_turn:<4} {victory:<10} {}",
                    verdicts.join(" ")
                );
            }
            // The turn cap awards the score victory, so `score` endings are
            // truncations of an undecided board and every other ending is a
            // game the rules finished. Agreement is reported for both because
            // they answer different questions: natural endings are ground
            // truth, cap endings are agreement with the truncation rule that
            // adjudication would replace.
            println!(
                "\nthreshold  crossed  agree-all      agree-natural  agree-cap      \
                 mean-saved  saved-share"
            );
            for (slot, threshold) in thresholds.iter().enumerate() {
                let (mut crossed, mut agree, mut nat, mut nat_agree, mut cap, mut cap_agree) =
                    (0u32, 0u32, 0u32, 0u32, 0u32, 0u32);
                let (mut saved, mut total) = (0u64, 0u64);
                for (_, (winner, victory, end_turn, crossings)) in &finished {
                    total += u64::from(*end_turn);
                    let Some((pid, turn)) = crossings[slot] else {
                        continue;
                    };
                    crossed += 1;
                    saved += u64::from(end_turn.saturating_sub(turn));
                    let hit = *winner == Some(pid);
                    agree += u32::from(hit);
                    if victory.as_deref() == Some("score") {
                        cap += 1;
                        cap_agree += u32::from(hit);
                    } else {
                        nat += 1;
                        nat_agree += u32::from(hit);
                    }
                }
                let pct = |num: u32, den: u32| {
                    if den == 0 {
                        "     -".to_string()
                    } else {
                        format!("{:5.1}%", 100.0 * f64::from(num) / f64::from(den))
                    }
                };
                println!(
                    "{threshold:<9}  {crossed:3}/{:<3}  {}({agree:3}/{crossed:<3})  \
                     {}({nat_agree:2}/{nat:<2})  {}({cap_agree:2}/{cap:<2})  {:8.1}    {:5.1}%",
                    finished.len(),
                    pct(agree, crossed),
                    pct(nat_agree, nat),
                    pct(cap_agree, cap),
                    if crossed == 0 {
                        0.0
                    } else {
                        saved as f64 / f64::from(crossed)
                    },
                    if total == 0 {
                        0.0
                    } else {
                        100.0 * saved as f64 / total as f64
                    },
                );
            }
            if crashes > 0 {
                println!("\n{crashes} of {games} games crashed");
                std::process::exit(1);
            }
        }
        "benchmark" => {
            let games = arg(&args, "--games", 50);
            let turns = arg(&args, "--turns", 100) as u32;
            let jobs = jobs_arg(&args);
            let t0 = Instant::now();
            let played = civvis::parallel::map(games as usize, jobs, |seed| {
                let mut g = Game::new(2, 20, 14, seed as u64, turns, 0);
                let mut ais = AdvancedAi::fleet(&g);
                run_game(&mut g, &mut ais);
                g.turn as u64
            });
            let total_turns: u64 = played.iter().sum();
            let dt = t0.elapsed().as_secs_f64();
            println!(
                "{} games, {} turns in {:.2}s = {:.0} turns/sec \
                 (2 players, 20x14, {jobs} at a time)",
                games,
                total_turns,
                dt,
                total_turns as f64 / dt
            );
        }
        // What an agent that searches actually does: take a position and roll
        // it forward, over and over. Cloning a position dominates that, and
        // nothing else here measured it.
        "rollouts" => {
            let players = arg(&args, "--players", 6);
            let warmup = arg(&args, "--turns", 150) as u32;
            let samples = arg(&args, "--samples", 5000) as usize;
            let mut g = Game::new_with(game_options(
                &args,
                players,
                arg(&args, "--seed", 0) as u64,
                setup::TurnStructure::Sequential,
            ));
            let mut ais = AdvancedAi::fleet(&g);
            // Play in to the requested turn first: an empty map clones far
            // faster than a settled one, and a settled one is what an agent
            // searches from.
            while g.turn < warmup && g.winner.is_none() {
                let pid = g.current;
                ais[pid].take_turn(&mut g, pid);
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &civvis::game::Action::EndTurn);
                }
            }
            let clone_start = Instant::now();
            let mut sink = 0usize;
            for _ in 0..samples {
                sink += g.clone().units.len();
            }
            let clone_us = clone_start.elapsed().as_secs_f64() / samples as f64 * 1e6;
            let speculative_start = Instant::now();
            for _ in 0..samples {
                sink += g.speculative_clone().units.len();
            }
            let speculative_us =
                speculative_start.elapsed().as_secs_f64() / samples as f64 * 1e6;
            // A searching agent mostly applies ordinary moves and only
            // occasionally ends a turn, and the two cost wildly different
            // amounts, so both are reported.
            let seat = g.current;
            let mut mover = None;
            for action in g.legal_actions(seat) {
                if let civvis::game::Action::Move { .. } = action {
                    mover = Some(action);
                    break;
                }
            }
            let move_us = mover.as_ref().map(|action| {
                let start = Instant::now();
                for _ in 0..samples {
                    let mut branch = g.clone();
                    let _ = branch.apply(seat, action);
                    sink += branch.units.len();
                }
                start.elapsed().as_secs_f64() / samples as f64 * 1e6
            });
            let fast = g.speculative_clone();
            let end_start = Instant::now();
            for _ in 0..samples {
                let mut branch = g.clone();
                let _ = branch.apply(seat, &civvis::game::Action::EndTurn);
                sink += branch.units.len();
            }
            let end_us = end_start.elapsed().as_secs_f64() / samples as f64 * 1e6;
            let fast_end_start = Instant::now();
            for _ in 0..samples {
                let mut branch = fast.speculative_clone();
                let _ = branch.apply(seat, &civvis::game::Action::EndTurn);
                sink += branch.units.len();
            }
            let fast_end_us = fast_end_start.elapsed().as_secs_f64() / samples as f64 * 1e6;
            // The same move on a position that is not maintaining fogged
            // memory — what a search that never observes mid-rollout pays.
            let fast_us = mover.as_ref().map(|action| {
                let start = Instant::now();
                for _ in 0..samples {
                    let mut branch = fast.speculative_clone();
                    let _ = branch.apply(seat, action);
                    sink += branch.units.len();
                }
                start.elapsed().as_secs_f64() / samples as f64 * 1e6
            });
            println!(
                "turn {} · {} seats · {} cities · {} units",
                g.turn,
                g.players.len(),
                g.cities.len(),
                g.units.len(),
            );
            println!("clone            {clone_us:8.1} us  = {:.0}/sec", 1e6 / clone_us);
            println!(
                "speculative clone {speculative_us:7.1} us  = {:.0}/sec",
                1e6 / speculative_us
            );
            match move_us {
                Some(us) => println!("clone + move     {us:8.1} us  = {:.0} rollouts/sec", 1e6 / us),
                None => println!("clone + move          n/a  (no legal move for this seat)"),
            }
            println!("clone + end turn {end_us:8.1} us  = {:.0}/sec", 1e6 / end_us);
            if let Some(us) = fast_us {
                println!(
                    "clone + move (no fog){us:6.1} us  = {:.0} rollouts/sec",
                    1e6 / us
                );
            }
            println!(
                "clone + end (no fog) {fast_end_us:6.1} us  = {:.0}/sec",
                1e6 / fast_end_us
            );
            let _ = sink;
        }
        #[cfg(not(feature = "closed-experiments"))]
        "selfplay" => {
            eprintln!(
                "selfplay is part of the closed training-data lane; rebuild with \
                 --features closed-experiments to run the exporter"
            );
            std::process::exit(1);
        }
        #[cfg(feature = "closed-experiments")]
        "selfplay" => {
            let players = arg(&args, "--players", 4).max(2);
            let options = game_options(
                &args,
                players,
                arg(&args, "--seed", 0) as u64,
                setup::TurnStructure::Sequential,
            );
            let counterfactual = args.iter().any(|arg| arg == "--counterfactual");
            let cfg = civvis::selfplay::SelfPlayCfg {
                games: arg(&args, "--games", 20) as usize,
                players: players as usize,
                width: options.width,
                height: options.height,
                city_states: options.city_states,
                max_turns: options.max_turns,
                seed: arg(&args, "--seed", 0) as u64,
                every: arg(&args, "--every", if counterfactual { 40 } else { 10 }).max(1) as u32,
                ai: arg_text(
                    &args,
                    "--ai",
                    if counterfactual {
                        "strategic_score"
                    } else {
                        "advanced"
                    },
                ),
                out: arg_text(&args, "--out", "selfplay"),
                scalar_only: args.iter().any(|arg| arg == "--scalar-only"),
                counterfactual,
                counterfactual_roots: arg(&args, "--counterfactual-roots", 0).max(0) as usize,
                decision_features: args.iter().any(|arg| arg == "--decision-features"),
                jobs: jobs_arg(&args),
            };
            match civvis::selfplay::export(&cfg) {
                Ok(stats) => println!(
                    "
{} samples from {} games ({} decisive) -> {}",
                    stats.samples, stats.games, stats.decisive, cfg.out
                ),
                Err(error) => {
                    eprintln!("selfplay export failed: {error}");
                    std::process::exit(1);
                }
            }
        }
        "evolve" => {
            let players = arg(&args, "--players", 4);
            civvis::evolve::evolve(&civvis::evolve::EvoCfg {
                generations: arg(&args, "--generations", 1_000_000) as u32,
                pop: arg(&args, "--pop", 16) as usize,
                games: arg(&args, "--games", 8) as usize,
                players: players as usize,
                width: auto_dimension(&args, "--width", players, true),
                height: auto_dimension(&args, "--height", players, false),
                // Selection reads continuous score and combat shares, but the
                // separate promotion SPRT still decides on outright wins. At
                // 160 turns almost nothing reaches a victory, so confirmation
                // would judge arbitrary cutoffs rather than completed games.
                // See `stock_turns`.
                max_turns: arg(&args, "--turns", stock_turns(&args)) as u32,
                seed: arg(&args, "--seed", 1) as u64,
                threads: arg(&args, "--threads", 8) as usize,
                dir: arg_text(&args, "--dir", "evolved"),
                // `--turns` already resolves through `stock_turns(&args)`,
                // which reads this flag; until now the game itself did not,
                // so `--speed online` bred truncated Standard games.
                speed: arg_text(&args, "--speed", &default_speed()),
            });
        }
        "play" => {
            // The stock game: four civilizations on a Tiny world, which is
            // `MapSize::for_players(4)`. The map script default lives in
            // `game_options` so the headless arms open the same world.
            let players = arg(&args, "--players", 4);
            // `--mirror <run-dir>`: show the board a Civilization VI seat can
            // actually see, rebuilt as a CIVVIS game, instead of generating one.
            //
            // This is what makes the two windows one game rather than two. The
            // control mod exports only revealed plots, so what appears here is
            // what the seat has earned and nothing more.
            //
            // Unrevealed ground remains explicit `unknown` terrain underneath
            // the fog; see `mirror::rebuild_game` for the separate traversable
            // frontier prior used by the live decider.
            let mirrored: Option<Game> = args
                .iter()
                .position(|value| value == "--mirror")
                .and_then(|index| args.get(index + 1))
                .map(|dir| {
                    let events = std::path::Path::new(dir).join("events.jsonl");
                    let snapshot = civvis::mirror::snapshot_from_events(&events)
                        .unwrap_or_else(|error| {
                            eprintln!("cannot read {}: {error}", events.display());
                            std::process::exit(2);
                        });
                    if snapshot.revealed_count() == 0 {
                        eprintln!(
                            "{} has no tiles to mirror — the run needs --export-state, \
                             and before the PlayersVisibility fix the export emitted nothing",
                            events.display()
                        );
                        std::process::exit(2);
                    }
                    println!(
                        "mirroring {} revealed plots of a {}x{} world at turn {}",
                        snapshot.revealed_count(),
                        snapshot.width,
                        snapshot.height,
                        snapshot.turn
                    );
                    // ★★★★ MIRROR THE EMPIRE, NOT JUST THE GROUND. `rebuild_game`
                    // returns terrain only, so this window read "Ancient Age TURN 1"
                    // with an empty world while Civilization VI sat at turn 7 with a
                    // revealed continent, two cities and an army. Side by side that is
                    // worse than no mirror: the operator is asked to verify the two
                    // match and shown a board that cannot match by construction.
                    //
                    // `rebuild_from_state` places both empires' cities, our units and
                    // every visible rival unit, and sets the turn — the same
                    // reconstruction `civvis-orders` decides from, so what is on screen
                    // is what CIVVIS is actually reasoning about.
                    // ★ MOCK MODE. `--dump-state <file>` writes the observed board out
                    // as JSON; `--state <file>` merges a file back over it before the
                    // reconstruction runs. Together they give a round trip — capture the
                    // real Civilization VI position, edit anything, replay it — which is
                    // what makes a disagreement between the two screens reproducible
                    // instead of a thing you have to catch live.
                    let flag = |name: &str| {
                        args.iter()
                            .position(|value| value == name)
                            .and_then(|at| args.get(at + 1))
                            .cloned()
                    };
                    let mut observed = civvis::mirror::state_value_from_events(&events, None);
                    // ⚠ DUMP BEFORE MERGE. Writing the file after the override records
                    // the mock, not the observation, so using both flags at once would
                    // overwrite the very board you were trying to capture — and the
                    // second run would silently start from the edit.
                    if let (Some(state), Some(path)) = (observed.as_ref(), flag("--dump-state")) {
                        match serde_json::to_string_pretty(state)
                            .map_err(|e| e.to_string())
                            .and_then(|text| std::fs::write(&path, text).map_err(|e| e.to_string()))
                        {
                            Ok(()) => println!("  observed state written to {path}"),
                            Err(why) => println!("  ⚠ could not write --dump-state {path}: {why}"),
                        }
                    }
                    if let (Some(state), Some(path)) = (observed.as_mut(), flag("--state")) {
                        match std::fs::read_to_string(&path)
                            .ok()
                            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok())
                        {
                            Some(patch) => {
                                civvis::mirror::merge_state(state, &patch);
                                println!("  state overridden from {path}");
                            }
                            // Loud, because silently mirroring the real board when the
                            // operator asked for a mocked one is a wrong answer that
                            // looks exactly like a right one.
                            None => println!("  ⚠ could not read --state {path}: using observed board"),
                        }
                    }
                    let from_value = observed
                        .as_ref()
                        .and_then(|v| serde_json::from_value::<civvis::mirror::StateSnapshot>(v.clone()).ok());
                    match from_value.or_else(|| civvis::mirror::state_from_events(&events, None)) {
                        Some(state) => {
                            let rebuilt = civvis::mirror::rebuild_from_state(
                                &snapshot, &state, players as usize, 1, 250, 6,
                            );
                            println!(
                                "  empire: {} cities, {} units, {} rival cities, \
                                 {} rival units at turn {}",
                                rebuilt.placed_cities,
                                rebuilt.placed_units,
                                rebuilt.placed_rival_cities,
                                rebuilt.placed_rival_units,
                                state.turn
                            );
                            if !rebuilt.unmapped.is_empty() {
                                println!("  untranslatable: {}", rebuilt.unmapped.join(","));
                            }
                            rebuilt.game
                        }
                        // No `state` event means the run is not exporting one; terrain
                        // alone is still worth showing, and saying so beats implying
                        // the empty empire is real.
                        None => {
                            println!("  no `state` event: terrain only, no cities or units");
                            civvis::mirror::rebuild_game(&snapshot, players as usize, 1)
                        }
                    }
                });
            let resumed: Option<Game> = args
                .iter()
                .position(|value| value == "--resume")
                .and_then(|index| args.get(index + 1))
                .map(|path| {
                    let raw = std::fs::read_to_string(path).unwrap_or_else(|error| {
                        eprintln!("cannot read checkpoint {path}: {error}");
                        std::process::exit(2);
                    });
                    let value: serde_json::Value =
                        serde_json::from_str(&raw).unwrap_or_else(|error| {
                            eprintln!("cannot load checkpoint {path}: {error}");
                            std::process::exit(2);
                        });
                    let game: Game =
                        civvis::protocol::game_from_save(value).unwrap_or_else(|error| {
                            eprintln!("cannot load checkpoint {path}: {error}");
                            std::process::exit(2);
                        });
                    // A save records the mods it was played under. Resuming
                    // under a different set silently changes the rules
                    // mid-game, so say so rather than pretend otherwise.
                    let active = civvis::mods::active_names();
                    if game.mods != active {
                        eprintln!(
                            "warning: {path} was played with mods {:?} but this process has {:?}",
                            game.mods, active
                        );
                    }
                    // Stepping a simultaneous save one seat at a time would
                    // silently change its regime mid-game; say so instead. A
                    // spectated table plays whole planned turns, so only a
                    // resume that seats a human refuses it.
                    if game.turn_structure == setup::TurnStructure::Simultaneous
                        && !args.iter().any(|a| a == "--spectate" || a == "--watch")
                    {
                        eprintln!(
                            "{path} is a simultaneous-turns game; a played game is \
                             sequential by construction — resume it with --spectate"
                        );
                        std::process::exit(2);
                    }
                    game
                });
            let seed = {
                let s = arg(&args, "--seed", -1);
                if s >= 0 {
                    s as u64
                } else {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .subsec_nanos() as u64
                }
            };
            // A played game consults the human seat live, one seat at a
            // time, which is the sequential regime by construction. A
            // spectated table has nobody at the keyboard, so it plays the
            // simultaneous regime as one whole planned turn per pace tick —
            // and defaults to it, like the rest of the automated surfaces.
            // Refuse the combination that cannot be honoured rather than
            // quietly playing a different game than the flag asked for;
            // `simulate` and `soak` play it headless either way.
            let spectate = args.iter().any(|a| a == "--spectate" || a == "--watch");
            let play_options = game_options(
                &args,
                players,
                seed,
                setup::TurnStructure::Sequential,
            );
            if !spectate && play_options.turn_structure == setup::TurnStructure::Simultaneous {
                eprintln!(
                    "a played game is sequential by construction; simultaneous \
                     turns need --spectate, or `civvis simulate --turn-structure \
                     simultaneous`"
                );
                std::process::exit(2);
            }
            let map_script = play_options.map_script;
            let map_topology = play_options.map_topology;
            let map_poles = play_options.map_poles;
            let game_speed = GameSpeed::from_id(&play_options.speed).unwrap_or(GameSpeed::Standard);
            civvis::server::serve_with_game(
                arg(&args, "--port", 8765) as u16,
                !args.iter().any(|a| a == "--no-open"),
                civvis::server::Params {
                    num_players: players as usize,
                    width: auto_dimension(&args, "--width", players, true),
                    height: auto_dimension(&args, "--height", players, false),
                    seed,
                    base_ruleset: play_options.base_ruleset,
                    start_era: play_options.start_era,
                    future_era: play_options.future_era,
                    turn_structure: play_options.turn_structure,
                    map_script,
                    map_topology,
                    map_poles,
                    game_speed,
                    max_turns: play_options.max_turns,
                    victory_conditions: victory_conditions(&args),
                    // The engine's stock setup leaves mercy off. The lobby
                    // can still opt into any listed threshold after launch.
                    mercy_rule: play_options.mercy_rule,
                    required_victory_types: 1,
                    // The lobby can still change these mid-session; this is
                    // what the launch itself asked for.
                    tactics: tactics_rules(&args),
                    num_city_states: auto_cs(&args, players),
                    spectate,
                    difficulty: play_options.difficulty,
                    speed: play_options.speed,
                    teams: play_options.teams,
                    leader_pool: play_options.leader_pool,
                    civs: play_options.civs,
                    supervised: args.iter().any(|a| a == "--supervised"),
                },
                mirrored.or(resumed),
                args.iter().any(|a| a == "--paused"),
            );
        }
        "pedia" => {
            // Everything after the command that is not a flag is the query.
            let query = args
                .iter()
                .skip(1)
                .take_while(|arg| !arg.starts_with("--"))
                .cloned()
                .collect::<Vec<_>>()
                .join(" ");
            let rules = Rules::embedded();
            let found = civvis::pedia::search(&rules, &query);
            if found.is_empty() {
                println!("nothing in the ruleset matches {query:?}");
                std::process::exit(1);
            }
            print!("{}", civvis::pedia::render(&found));
            println!("
{} entries", found.len());
        }
        "validate" => {
            let findings = civvis::validate::validate(&Rules::embedded());
            let (text, clean) = civvis::validate::report(&findings);
            print!("{text}");
            let strict = args.iter().any(|a| a == "--strict");
            if !clean || (strict && !findings.is_empty()) {
                std::process::exit(1);
            }
        }
        _ => {
            println!(
                "usage: civvis <simulate|soak|odds-audit|benchmark|play|evolve|validate|pedia> \
                      [--players N] [--seed N] [--turns N] [--width N] [--height N] \
                      [--city-states N] [--games N] [--port N] [--no-open] \
                      [--map land_only|lakes|inland_sea|tenins_ball|grand_canals|grand_canals_2|pangaea|earth|true_start_earth|continents|small_continents|fjords|islands|water_world|battlefield|tactics_planet|tactics_ocean|trafalgar] \
                      [--shape flat|planet] [--poles poles|randomized] \
                      [--difficulty settler|chieftain|warlord|prince|king|emperor|immortal|deity] \
                      [--speed online|quick|standard|epic|marathon] \
                      [--disasters 0|1|2|3|4] [--barbarians on|off] \
                      [--turn-structure sequential|simultaneous (everything defaults to \
                       sequential; simultaneous is a research regime)] \
                      [--game-modes apocalypse,secret_societies] \
                      [--leader-pool civ6|historical|today] \
                      [--human-seats 0,1] [--teams 0,0,1,1] [--mods path/to/mod,path/to/other] \
                      [--victories science,culture,religious,diplomatic,domination,score] \
                      [--native-competitions] [--spectate] [--supervised] [--resume checkpoint.json] [--strict] \
                      [--generations N] [--pop N]"
            );
        }
    }
}

#[cfg(test)]
mod tests {

    use super::{
        game_options, jobs_arg, map_topology, simultaneous_soak_job_split,
        single_simulation_jobs_arg, start_era, tactics_rules, turn_structure, ANCHOR_BEHAVIOUR_FNV,
        ANCHOR_DECISIONS, SINGLE_SIMULATION_DEFAULT_MAX_JOBS,
    };
    use civvis::ai::AdvancedAi;
    use civvis::game::{Action, Game};
    use civvis::setup::{GameSpeed, MapScript, MapSize, MapTopology, TurnStructure};

    /// `--start-era random` spreads a sweep over the whole unit roster
    /// instead of teaching one era's matchups, and does it reproducibly: the
    /// roll comes from the game's own seed, so a soak replayed with the same
    /// `--start-seed` opens in the same eras. Consecutive seeds must not walk
    /// the ladder in lockstep, which is what the mix in `start_era` is for.
    /// Every launch path reads the arena flags through one function, because
    /// each path that grew its own copy accepted them and silently ignored
    /// them: `tournament` rated three "different" experiments identically,
    /// and `play` launched the stock arena however it was asked. A shared
    /// reader is the fix, so this pins that it reads what it is given and
    /// clamps what it cannot play.
    #[test]
    fn the_arena_flags_are_read_once_for_every_launch_path() {
        let stock = civvis::setup::TacticsRules::default();
        assert_eq!(tactics_rules(&[]), stock, "no flags is the stock arena");

        let asked = [
            "--map".to_string(), "battlefield".to_string(),
            "--tactics-cities".to_string(), "0".to_string(),
            "--tactics-production".to_string(), "120".to_string(),
            "--tactics-gold".to_string(), "0".to_string(),
            "--tactics-turns-per-tech".to_string(), "0".to_string(),
            "--tactics-turn-limit".to_string(), "150".to_string(),
        ];
        let rules = tactics_rules(&asked);
        assert_eq!(rules.cities, 0);
        assert_eq!(rules.production, 120);
        assert_eq!(rules.gold, 0);
        assert_eq!(rules.turns_per_tech, 0);
        assert_eq!(rules.turn_limit, 150);

        // The flag objective travels the same shared reader, and drags the
        // city out of the battle the way every other surface does.
        let flagged = [
            "--map".to_string(), "battlefield".to_string(),
            "--tactics-flag".to_string(),
            "--tactics-cities".to_string(), "1".to_string(),
        ];
        let rules = tactics_rules(&flagged);
        assert!(rules.flag);
        assert_eq!(rules.cities, 0, "the flag replaces the city objective");

        // Clamped, not trusted: these reach the same sanitiser the server uses.
        let silly = [
            "--tactics-cities".to_string(), "9".to_string(),
            "--tactics-production".to_string(), "100000".to_string(),
            "--tactics-turns-per-tech".to_string(), "100000".to_string(),
        ];
        let rules = tactics_rules(&silly);
        assert_eq!(rules.cities, 1, "an arena seats at most one city a side");
        assert_eq!(rules.production, civvis::setup::TacticsRules::MAX_YIELD);
        assert_eq!(rules.turns_per_tech, civvis::setup::TacticsRules::MAX_TURNS_PER_TECH);

        // And the world path carries the same answer, so a `play` launch and a
        // `soak` launch of the same flags are the same arena.
        let options = game_options(&asked, 2, 7, TurnStructure::Sequential);
        assert_eq!(options.tactics, tactics_rules(&asked));
        assert_eq!(options.max_turns, 150, "the arena uses its selected deadline");

        let explicit = [
            "--map".to_string(), "battlefield".to_string(),
            "--tactics-turn-limit".to_string(), "200".to_string(),
            "--turns".to_string(), "73".to_string(),
        ];
        assert_eq!(
            game_options(&explicit, 2, 8, TurnStructure::Sequential).max_turns,
            73,
            "the general explicit turn flag still overrides the Tactics menu"
        );
    }

    /// ⚠⚠ `--victories` WAS PARSED, VALIDATED, AND THEN DROPPED BY EVERY
    /// COMMAND BUT ONE.
    ///
    /// `victory_conditions()` had a single caller — the `play` server's setup —
    /// so `simulate`, `soak`, `odds-audit`, `benchmark`, `rollouts` and
    /// `selfplay` took `GameOptions::new`'s all-six default whatever was asked
    /// for. The flag `exit(2)`s on a bad name, so a run was refused for a typo
    /// and then silently handed the wrong game.
    ///
    /// This asserts the shared builder carries it, which is what makes every
    /// one of those commands honour it.
    #[test]
    fn the_victories_flag_reaches_the_shared_game_builder() {
        let asked = ["--victories".to_string(), "science,score".to_string()];
        let options = game_options(&asked, 2, 11, TurnStructure::Sequential);
        assert!(options.victory_conditions.science);
        assert!(options.victory_conditions.score);
        assert!(
            !options.victory_conditions.religious,
            "religion was not asked for"
        );
        assert!(!options.victory_conditions.culture);
        assert!(!options.victory_conditions.diplomatic);
        assert!(!options.victory_conditions.domination);

        // And the game built from those options plays by them, rather than by
        // the struct default `Game::new_with` used to hardcode.
        let game =
            civvis::game::Game::new_with(game_options(&asked, 2, 11, TurnStructure::Sequential));
        assert_eq!(game.victory_conditions, options.victory_conditions);
        assert!(!game.victory_conditions.religious);
    }

    /// Saying nothing still means the lobby's full set, so no existing command
    /// line changes meaning.
    #[test]
    fn a_command_line_without_the_flag_keeps_every_condition() {
        let options = game_options(&[], 2, 12, TurnStructure::Sequential);
        assert_eq!(
            options.victory_conditions,
            civvis::game::VictoryConditions::default()
        );
    }

    #[test]
    fn a_random_start_era_is_seeded_scattered_and_playable() {
        let args = ["--start-era".to_string(), "random".to_string()];
        let playable: Vec<usize> =
            civvis::setup::playable_start_eras().filter_map(|spec| spec.era).collect();
        let rolled: Vec<usize> = (0..64).map(|seed| start_era(&args, seed)).collect();

        for era in &rolled {
            assert!(playable.contains(era), "rolled an era nobody can open in: {era}");
        }
        let replay: Vec<usize> = (0..64).map(|seed| start_era(&args, seed)).collect();
        assert_eq!(rolled, replay, "the same seed must replay the same era");

        let distinct: std::collections::BTreeSet<usize> = rolled.iter().copied().collect();
        assert!(
            distinct.len() >= playable.len().min(5),
            "64 seeds reached only {} of {} eras: {distinct:?}",
            distinct.len(),
            playable.len()
        );
        // Lockstep would make each seed's era one past its neighbour's.
        let marching = rolled
            .windows(2)
            .filter(|pair| (pair[1] + playable.len() - pair[0]) % playable.len() == 1)
            .count();
        assert!(marching < rolled.len() / 2, "the eras march with the seed");

        // Without the flag the ladder is untouched and the seed is ignored.
        assert_eq!(start_era(&[], 1), start_era(&[], 999_999));
    }

    #[test]
    fn omitted_map_shape_defaults_to_planet() {
        assert_eq!(map_topology(&[]), MapTopology::Planet);

        let options = game_options(&[], 2, 71_004, TurnStructure::Sequential);
        let size = MapSize::for_players(2);
        assert_eq!(options.map_topology, MapTopology::Planet);
        assert_eq!(
            (options.width, options.height),
            size.dimensions(MapTopology::Planet)
        );

        let flat = vec!["--shape".to_string(), "flat".to_string()];
        assert_eq!(map_topology(&flat), MapTopology::Flat);
    }

    /// The turn-structure default is per command: the surfaces that exist
    /// for throughput (simulate, soak, a spectated table) hand this helper
    /// `Simultaneous`, the rating instruments and played games hand it
    /// `Sequential`, and an explicit flag always wins over either. The
    /// anchor `TurnStructure::default()` stays `Sequential` for saves and
    /// the setup contract — that half of the promise is asserted in
    /// `simultaneous.rs`.
    #[test]
    fn the_turn_structure_default_is_the_callers_and_the_flag_still_wins() {
        assert_eq!(
            turn_structure(&[], TurnStructure::Simultaneous),
            TurnStructure::Simultaneous
        );
        assert_eq!(
            turn_structure(&[], TurnStructure::Sequential),
            TurnStructure::Sequential
        );
        let sequential = vec!["--turn-structure".to_string(), "sequential".to_string()];
        assert_eq!(
            turn_structure(&sequential, TurnStructure::Simultaneous),
            TurnStructure::Sequential
        );
        let simultaneous = vec!["--turn-structure".to_string(), "simultaneous".to_string()];
        assert_eq!(
            turn_structure(&simultaneous, TurnStructure::Sequential),
            TurnStructure::Simultaneous
        );
        // The default threads through a whole options build unchanged.
        assert_eq!(
            game_options(&[], 4, 71_006, TurnStructure::Simultaneous).turn_structure,
            TurnStructure::Simultaneous
        );
        assert_eq!(
            game_options(&sequential, 4, 71_006, TurnStructure::Simultaneous).turn_structure,
            TurnStructure::Sequential
        );
    }

    /// The stock game somebody gets by asking for nothing: a Tennis Ball
    /// world. The four-seat half of the promise lives in the `play` arm's
    /// `--players` default, and the serde default stays Pangaea so a client
    /// that has never been taught the setting is unmoved.
    #[test]
    fn omitted_map_defaults_to_the_tennis_ball() {
        use civvis::setup::MapScript;

        let options = game_options(&[], 4, 71_005, TurnStructure::Sequential);
        assert_eq!(options.map_script, MapScript::TeninsBall);
        assert_eq!(
            options.mercy_rule, None,
            "a command-line game starts without mercy"
        );

        // An explicit choice still wins, under either accepted spelling.
        for (asked, chosen) in [
            ("pangaea", MapScript::Pangaea),
            ("tennis_ball", MapScript::TeninsBall),
            ("tenins_ball", MapScript::TeninsBall),
            ("tactics_planet", MapScript::TacticsPlanet),
        ] {
            let args = vec!["--map".to_string(), asked.to_string()];
            assert_eq!(
                game_options(&args, 4, 71_005, TurnStructure::Sequential).map_script,
                chosen,
                "{asked}"
            );
        }
        let planet = vec!["--map".to_string(), "tactics_planet".to_string()];
        let options = game_options(&planet, 2, 71_005, TurnStructure::Sequential);
        assert_eq!((options.width, options.height), (40, 18));
    }

    #[test]
    fn implicit_single_simulation_workers_are_bounded_without_changing_batches() {
        let implicit = Vec::new();
        let host_default = civvis::parallel::default_jobs();
        assert_eq!(jobs_arg(&implicit), host_default);
        assert_eq!(
            single_simulation_jobs_arg(&implicit),
            host_default.min(SINGLE_SIMULATION_DEFAULT_MAX_JOBS)
        );

        let explicit = vec!["simulate".to_string(), "--jobs".to_string(), "9".to_string()];
        assert_eq!(jobs_arg(&explicit), 9);
        assert_eq!(single_simulation_jobs_arg(&explicit), 9);
    }

    #[test]
    fn simultaneous_soak_uses_idle_batch_budget_for_seat_planning() {
        assert_eq!(simultaneous_soak_job_split(1, 128), (1, 128, 0));
        assert_eq!(simultaneous_soak_job_split(3, 128), (3, 42, 2));
        assert_eq!(simultaneous_soak_job_split(64, 8), (8, 1, 0));

        for (games, jobs) in [(1, 1), (2, 3), (3, 8), (8, 8), (20, 8)] {
            let (concurrent, per_game, extra) = simultaneous_soak_job_split(games, jobs);
            assert_eq!(concurrent * per_game + extra, jobs);
            assert!(extra < concurrent);
            assert!(concurrent <= games.max(1));
        }
    }

    /// The frozen anchor never reaches `settler_commit`, which every default
    /// constructor leaves off — the claim behind several "free" re-pins, and
    /// prose does not hold — check it.
    #[test]
    fn the_anchor_never_reaches_the_settler_commit_path() {
        // ⚠ THE ANCHOR IS `legacy()`, NOT `new()` — `builtin_ai` maps
        // "advanced_v1" => AdvancedAi::legacy(). I first asserted this on `new()`,
        // which sets `settler_commit = true`, and this test failed and corrected me.
        // That is the whole reason the claim is checked rather than written down.
        assert!(
            !civvis::ai::AdvancedAi::legacy().settler_commit,
            "advanced_v1 is legacy(); if it ever reaches the settler_commit path the \
             frozen anchor moves and the fingerprint below says so"
        );
        // The global Recovery front hold is likewise a production-only branch.
        // If the anchor ever enables it, its campaign movement may change and a
        // source re-pin alone would be invalid.
        assert!(
            !civvis::ai::AdvancedAi::legacy().bounded_recovery,
            "advanced_v1 is legacy(); if it ever carries bounded Recovery the global \
             front-hold branch reaches the anchor and the fingerprint below moves"
        );
        // ⚠ And record the other half honestly: `advanced` DOES set it, so that
        // entrant's settler pipeline genuinely changes. The anchor pins the scale
        // and is untouched, which is what this guard asks about — but v5 rows for
        // `advanced` straddle this change.
        assert!(civvis::ai::AdvancedAi::new().settler_commit);
        // ⚠ SAME QUESTION, ASKED AGAIN FOR THE GARRISON-LOYALTY ARM.
        //
        // The `limitanei` portfolio insert in `strategic_policies` is guarded by
        // `self.garrison_loyalty_policy`, and BOTH the anchor and the stock
        // entrant leave it false — only the eval-only arm
        // `advanced_garrison_loyalty` turns it on. So the source fingerprint
        // moved while the legacy path did not, and the re-pin below is free.
        //
        // Checked rather than asserted in a comment, because the last time this
        // was written down instead of tested the written claim was wrong.
        assert!(
            !civvis::ai::AdvancedAi::legacy().garrison_loyalty_policy,
            "advanced_v1 must not slot limitanei; if it ever does, the frozen \
             anchor moves and the fingerprint below says so"
        );
        assert!(
            !civvis::ai::AdvancedAi::new().garrison_loyalty_policy,
            "the stock entrant must not slot limitanei either — the arm measured \
             a null and ships OFF"
        );
        // ⚠ SAME QUESTION, ASKED AGAIN FOR THE NUCLEAR LANE.
        //
        // The wmd-strike doctrine's wider gate (Recovery/threatened besides
        // Conquest) lives behind `advanced_command_actions`, and the new
        // nuclear tech beeline in `tech_value` is explicitly gated on
        // `victory_planning` — both paths the anchor never enters, because
        // legacy() constructs with victory_planning = false. So the source
        // fingerprint moved while the legacy path did not, and the re-pin
        // below is free.
        assert!(
            !civvis::ai::AdvancedAi::legacy().coordinates_forces(),
            "advanced_v1 must not victory-plan; if it ever does, the nuclear \
             beeline and strike doctrine reach the anchor and the frozen \
             anchor moves and the fingerprint below says so"
        );
        assert!(
            civvis::ai::AdvancedAi::new().coordinates_forces(),
            "the stock entrant does victory-plan, so `advanced` rows straddle \
             the nuclear-lane change — recorded here honestly"
        );
        let headless = Game::new(2, 24, 16, 71_032, 200, 0);
        assert!(
            headless
                .live_great_person_offer_blocker(0, "scientist")
                .is_none(),
            "the frozen headless anchor has no Firaxis named-offer export; if this \
             becomes populated, the live-only GPP gate can alter its ledger"
        );
        assert!(
            headless.players[0].live_great_person_offers.is_none()
                && headless.great_person_class_offered_now(0, "scientist"),
            "the frozen headless anchor has no Firaxis Great People screen, so its native \
             roster remains available; if this changes, the source re-pin is not free"
        );
        assert!(
            headless.players[0]
                .live_great_person_activation_needs
                .is_empty(),
            "the frozen headless anchor has no physical Firaxis Great Person units; \
             if this becomes populated, live activation infrastructure can alter \
             its ledger"
        );
    }

    /// The four-plus-one profiles the frozen anchor's decision stream is pinned
    /// over. Small duel through the six-player deployment shape, plus an
    /// archipelago so the embarkation paths a land map never reaches are
    /// covered — see `ANCHOR_BEHAVIOUR_FNV` for why that last one is here.
    const ANCHOR_PROFILES: [(usize, i32, i32, u64, u32, usize, MapScript); 5] = [
        (2, 20, 14, 73, 60, 1, MapScript::Pangaea),
        (4, 34, 22, 31337, 80, 2, MapScript::Pangaea),
        (4, 44, 28, 95_000_000, 100, 3, MapScript::Continents),
        (6, 54, 34, 424_242, 120, 4, MapScript::Pangaea),
        (4, 38, 26, 8_675_309, 90, 2, MapScript::Islands),
    ];

    /// FNV-1a over every action `advanced_v1` applies across `ANCHOR_PROFILES`.
    fn anchor_behaviour_fingerprint() -> (u64, usize) {
        let mut fingerprint = 0xcbf2_9ce4_8422_2325u64;
        let mut decisions = 0usize;
        for (players, width, height, seed, turns, city_states, script) in ANCHOR_PROFILES {
            let mut game = Game::new_with_setup(
                players,
                width,
                height,
                seed,
                turns,
                city_states,
                script,
                GameSpeed::Standard,
                true,
            );
            let mut fleet: Vec<AdvancedAi> = (0..game.players.len())
                .map(|_| AdvancedAi::legacy())
                .collect();
            civvis::ai::run_game(&mut game, &mut fleet);
            for (seat, action) in game.log.iter() {
                let rendered = format!(
                    "{seat}:{}",
                    serde_json::to_string(action).expect("an Action always serializes")
                );
                for byte in rendered.as_bytes() {
                    fingerprint ^= u64::from(*byte);
                    fingerprint = fingerprint.wrapping_mul(0x0100_0000_01b3);
                }
                decisions += 1;
            }
            fingerprint ^= 0xff;
            fingerprint = fingerprint.wrapping_mul(0x0100_0000_01b3);
        }
        (fingerprint, decisions)
    }

    #[test]
    fn advanced_v1_plays_the_same_game_it_always_did() {
        let (fingerprint, decisions) = anchor_behaviour_fingerprint();
        assert_eq!(
            decisions, ANCHOR_DECISIONS,
            "the anchor made a different number of decisions ({decisions} rather \
             than {ANCHOR_DECISIONS}), so its play changed. See the note on \
             ANCHOR_BEHAVIOUR_FNV: find the gene that should have kept this away \
             from legacy(), or — for a deliberate change to the shared engine — \
             re-pin and say so in docs/ELO_REPINS.md"
        );
        assert_eq!(
            fingerprint,
            ANCHOR_BEHAVIOUR_FNV,
            "advanced_v1 chose differently somewhere in {decisions} decisions \
             across {} profiles. This is a real behaviour change to the frozen \
             anchor, not a formatting one: find the gene that should have kept \
             the change away from legacy(), or — for a deliberate change to the \
             shared engine — re-pin and say so in docs/ELO_REPINS.md. Do NOT \
             re-pin to make the test pass",
            ANCHOR_PROFILES.len()
        );
    }

    /// The sea's live-only reconnaissance arm is sourced from the same AI
    /// files as the frozen anchor, so pin its gate independently of the
    /// broader repair bundle.
    #[test]
    fn naval_recon_cannot_reach_the_frozen_anchor() {
        assert!(
            !civvis::ai::AdvancedAi::legacy().naval_recon(),
            "advanced_v1 carries live-only naval reconnaissance: the \
             source-contract re-pin is valid only while this arm stays \
             unreachable from the frozen rating anchor"
        );
        // Production sails since the 2026-08-17 recon-fleet promotion
        // (corrected-gate matrix PASS, Elo +35, CI +1..+69; see
        // `promoted_policy_envoy`). The frozen anchor above is the rating
        // that must not move; `advanced` re-fits whenever it changes.
        assert!(civvis::ai::AdvancedAi::new().naval_recon());
    }

    /// The re-pin above claims the engine-repair bundle cannot reach the
    /// frozen anchor. A comment claiming that is worth exactly as much as the
    /// comment that claimed native games leave `bounded_recovery` disabled —
    /// which was wrong, and is one of the re-pins listed above.
    ///
    /// So assert it on the constructors instead. `advanced_v1` is
    /// `AdvancedAi::legacy()` and the production incumbent is
    /// `AdvancedAi::new()`; neither may carry a repair. Only the three
    /// `advanced_synergy*` evaluator arms turn these on, and if that ever
    /// stops being true this fails before a rating anchor moves under a
    /// ledger that cannot see it.
    #[test]
    fn the_repair_bundle_cannot_reach_the_frozen_anchor() {
        for (name, ai) in [
            ("advanced_v1", civvis::ai::AdvancedAi::legacy()),
            ("advanced", civvis::ai::AdvancedAi::new()),
        ] {
            for (flag, on) in [
                ("war_economy", ai.war_economy),
                ("war_reinforcement", ai.war_reinforcement),
                ("war_patience", ai.war_patience),
                ("deny_while_targeted", ai.deny_while_targeted),
                ("stock_denial_lead_time", ai.stock_denial_lead_time),
                ("endgame_war_runway", ai.endgame_war_runway),
                ("siege_commitment", ai.siege_commitment),
                ("relief_targets_the_siege", ai.relief_targets_the_siege),
                ("blind_objective_units", ai.blind_objective_units),
                ("blind_objective_strength", ai.blind_objective_strength),
                ("siege_tracks_the_wall", ai.siege_tracks_the_wall),
                ("army_target_weighs_the_enemy", ai.army_target_weighs_the_enemy),
                ("peacetime_deterrence", ai.peacetime_deterrence),
                ("strike_opening", ai.strike_opening),
                // Evaluator-only like the rest of this list: the stalled-settler
                // fallback must reach neither the anchor nor production until it
                // has a number.
                (
                    "settler_founds_when_stalled",
                    ai.settler_founds_when_stalled,
                ),
                ("fortify_idle_units", ai.fortify_idle_units()),
                ("amenity_project_preemption", ai.amenity_project_preemption),
            ] {
                assert!(
                    !on,
                    "{name} carries the engine repair {flag}: the bundle measured \
                     a confirmed -108 Elo at deployment, and the re-pin that \
                     let it into the hashed sources was justified by this arm \
                     being unreachable from the anchor"
                );
            }
        }
    }

    /// The soak's WAR block folds over the wars it can see, and
    /// `close_war_record` moves a finished war out of `Game::wars` into
    /// `Game::concluded_wars`. Reading the live map alone therefore hides
    /// every war that ended — which is every war that was *won*, and every
    /// white peace the block exists to make legible — and makes
    /// `ended_in_peace` structurally impossible to observe.
    #[test]
    fn a_settled_war_leaves_the_live_map_and_must_still_be_counted() {
        let mut game = Game::new(2, 24, 16, 5150, 400, 0);
        game.current = 0;
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
        game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
        assert_eq!(game.wars.len(), 1, "the declaration must open a war");
        assert!(game.concluded_wars.is_empty());

        // Peace is gated behind a mandatory war duration; wait it out.
        while let Some(until) = game.peace_available_at(0, 1) {
            assert!(until > game.turn, "the gate must advance the clock");
            game.turn = until;
        }
        game.apply(0, &Action::MakePeace { player: 1 }).unwrap();

        assert!(
            game.wars.is_empty(),
            "a settled war must not remain in the live map"
        );
        assert_eq!(
            game.concluded_wars.len(),
            1,
            "the settled war has to be read from concluded_wars or it is \
             invisible to every count in the soak line"
        );
        assert!(game.concluded_wars[0].ended.is_some());
    }
}
