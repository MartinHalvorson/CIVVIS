//! CLI: simulate / soak / benchmark (mirrors the Python CLI outputs).
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use civvis::ai::{run_game, AdvancedAi, Ai};
use civvis::game::{
    default_difficulty, default_speed, Game, GameOptions, LeaderPool, VictoryConditions,
    WarRecord, DEFAULT_DISASTER_INTENSITY, GAME_MODES,
};
use civvis::rules::Rules;
use civvis::setup::{
    BaseRuleset, GameSpeed, MapPoles, MapScript, MapSize, MapTopology, StartEon, START_EONS,
};

fn arg(args: &[String], key: &str, default: i64) -> i64 {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn arg_f64(args: &[String], key: &str, default: f64) -> f64 {
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
    let size = MapSize::for_players(players.max(1) as usize);
    // A globe stores itself in a rectangle of its own shape, so the size's
    // default dimensions depend on which world shape was asked for.
    let (default_width, default_height) = size.dimensions(map_topology(args));
    arg(
        args,
        key,
        if width { default_width } else { default_height } as i64,
    ) as i32
}

/// The world's shape, which is asked for separately from what fills it.
/// Fixed geography changes where the land comes from, not which shape it is
/// sampled onto: even True Start Earth can be a flat atlas or a globe.
fn map_topology(args: &[String]) -> MapTopology {
    // `--map planet` named a world type before the globe became a shape of its
    // own, and still means both halves of what it meant then.
    let default = if arg_text(args, "--map", "pangaea") == "planet" {
        MapTopology::Planet
    } else {
        MapTopology::Flat
    };
    MapTopology::from_id(&arg_text(args, "--shape", default.id())).unwrap_or(default)
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

/// The sweep of time the game is played through, and how far into it it opens.
///
/// The two are one question: an era id is only meaningful inside its own eon,
/// so they are resolved together. An eon that is declared but not yet playable
/// is refused here rather than quietly played as human history.
fn start_eon_and_era(args: &[String]) -> (StartEon, usize) {
    let id = arg_text(args, "--eon", StartEon::default().id());
    let eon = StartEon::from_id(&id)
        .filter(|eon| eon.is_playable())
        .unwrap_or_else(|| {
            let playable: Vec<&str> = START_EONS
                .iter()
                .filter(|spec| spec.playable)
                .map(|spec| spec.id)
                .collect();
            eprintln!("cannot play the {id:?} eon yet; choose one of: {}", playable.join(", "));
            std::process::exit(2);
        });
    let default = eon.era_id(eon.default_era()).to_string();
    let era = arg_text(args, "--start-era", &default);
    let index = eon.era_from_id(&era).unwrap_or_else(|| {
        let ladder: Vec<&str> = eon.eras().iter().map(|spec| spec.id).collect();
        eprintln!(
            "unknown start era {era:?} for the {} eon; choose one of: {}",
            eon.id(),
            ladder.join(", ")
        );
        std::process::exit(2);
    });
    (eon, index)
}

/// Difficulty and speed are chosen the same way everywhere: by name, against
/// the shipped ruleset, with the stock levels as defaults.
fn game_options(args: &[String], players: i64, seed: u64) -> GameOptions {
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
    // An explicit --turns wins; otherwise every speed brings its own stock
    // budget (Standard is 500 turns / 2050 AD). Short historical defaults
    // ended games at the turn limit before the science, culture, and
    // diplomatic lanes could finish, which handed the win to whoever was
    // ahead on score at an arbitrary cutoff.
    let turns = if args.iter().any(|a| a == "--turns") {
        arg(args, "--turns", speed_spec.turns as i64)
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
    let (start_eon, start_era) = start_eon_and_era(args);
    GameOptions {
        base_ruleset: base_ruleset(args),
        start_eon,
        start_era,
        map_script: MapScript::from_id(&arg_text(args, "--map", "pangaea"))
            .unwrap_or(MapScript::Pangaea),
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
        leader_pool: {
            let id = arg_text(args, "--leader-pool", LeaderPool::default().id());
            LeaderPool::from_id(&id).unwrap_or_else(|| {
                eprintln!("unknown leader pool {id:?}; choose civ6 or expanded");
                std::process::exit(2);
            })
        },
        // Who the player is. `--civ Egypt` seats Egypt at seat 0; `--civs
        // Egypt,Rome` names the leading seats in order. Anything unnamed
        // falls back to the stock roster, and a name the ruleset does not
        // know is refused here rather than silently ignored downstream.
        civs: {
            let named = arg_text(args, "--civs", &arg_text(args, "--civ", ""));
            let chosen: Vec<String> = named
                .split(',')
                .map(|civ| civ.trim().to_string())
                .filter(|civ| !civ.is_empty())
                .collect();
            for civ in &chosen {
                if !rules.civs.contains_key(civ) {
                    let mut known: Vec<&str> = rules.civs.keys().map(String::as_str).collect();
                    known.sort_unstable();
                    eprintln!("unknown civilization {civ:?}; choose one of {known:?}");
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
    // A game can legitimately end with nobody having won: a lobby that pins
    // `--victories` without `score` has no turn-limit tiebreak, so the limit
    // arrives and no enabled path has been achieved. That is a result to
    // report, not a reason to abort before printing the standings that say
    // what actually happened.
    match g.winner {
        Some(winner) => {
            let w = &g.players[winner];
            println!(
                "Winner: {} (player {}) by {} on turn {}",
                w.civ,
                w.id,
                g.victory_type.clone().unwrap_or_default(),
                g.turn
            );
        }
        None => println!(
            "No winner: turn {} of {}, and no enabled victory was achieved",
            g.turn, g.max_turns
        ),
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
            if unit.owner == pid && g.rules.units[unit.kind.as_str()].class == "military" {
                *roster.entry(unit.kind.as_str()).or_default() += 1;
            }
        }
        let mut army: Vec<(&str, usize)> = roster.into_iter().collect();
        army.sort_by_key(|(kind, count)| (std::cmp::Reverse(*count), *kind));
        let army: Vec<String> = army
            .iter()
            .map(|(kind, count)| {
                let stale = if g.unit_is_obsolete(pid, kind) { "*" } else { "" };
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

/// How many games to play at once. Defaults to one per core; `--jobs 1`
/// restores the strictly serial run, which is what timing one game wants.
fn jobs_arg(args: &[String]) -> usize {
    let requested = arg(args, "--jobs", 0);
    if requested > 0 {
        requested as usize
    } else {
        civvis::parallel::default_jobs()
    }
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
    match cmd {
        "simulate" => {
            let players = arg(&args, "--players", 4);
            let g0 = Instant::now();
            let mut g = Game::new_with(game_options(
                &args,
                players,
                arg(&args, "--seed", 0) as u64,
            ));
            let mut ais = AdvancedAi::fleet(&g);
            run_game(&mut g, &mut ais);
            println!("[{:.3}s]", g0.elapsed().as_secs_f64());
            standings(&g);
        }
        "soak" => {
            let players = arg(&args, "--players", 4);
            let games = arg(&args, "--games", 10);
            let start = arg(&args, "--start-seed", 0);
            let jobs = jobs_arg(&args);
            // Each game is played on its own thread, then described on the
            // main one, so a soak reads exactly as it did when it was serial.
            let lines = civvis::parallel::map(games as usize, jobs, |index| {
                let seed = start + index as i64;
                let t0 = Instant::now();
                let result = std::panic::catch_unwind(|| {
                    let mut g = Game::new_with(game_options(&args, players, seed as u64));
                    let mut ais = AdvancedAi::fleet(&g);
                    run_game(&mut g, &mut ais);
                    // Every major's turns, pooled: what the empires in this game
                    // actually spent the game doing.
                    let mut census = civvis::ai::StrategyCensus::default();
                    for ai in ais.iter().take(g.players.iter().filter(|p| !p.is_minor).count()) {
                        census.absorb(&ai.strategy_census());
                    }
                    (g, census)
                });
                match result {
                    Ok((g, census)) => {
                        let majors: Vec<_> = g.players.iter().filter(|p| !p.is_minor).collect();
                        let minors: Vec<_> = g
                            .players
                            .iter()
                            .filter(|p| p.is_minor && !p.is_barbarian)
                            .collect();
                        // A soak line describes a finished game, and a game
                        // whose turn limit arrived with no enabled victory
                        // achieved is finished too. Report it as one rather
                        // than taking the whole run down.
                        let w = g.winner.map(|winner| &g.players[winner]);
                        let mut flags = String::new();
                        if majors.iter().all(|p| p.techs.len() <= 2) {
                            flags.push_str(" NO-TECH-PROGRESS");
                        }
                        if w.is_some_and(|w| w.is_minor) {
                            flags.push_str(" MINOR-WINNER");
                        }
                        if w.is_none() {
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
                            .filter(|unit| g.rules.units[unit.kind.as_str()].class == "military")
                            .fold((0, 0, 0), |(obsolete, ancient, army), unit| {
                                (
                                    obsolete + g.unit_is_obsolete(unit.owner, &unit.kind) as i32,
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
                        Some(format!(
                            "seed {:3}  t{:<4} {:<10} {:<8} majors_alive={}/{} cities={:<2} cs_alive={}/{} [{:.2}s]{}",
                            seed,
                            g.turn,
                            g.victory_type.clone().unwrap_or_default(),
                            w.map_or("-", |w| w.civ.as_str()),
                            majors.iter().filter(|p| p.alive).count(),
                            majors.len(),
                            g.cities.len(),
                            minors.iter().filter(|p| p.alive).count(),
                            minors.len(),
                            t0.elapsed().as_secs_f64(),
                            flags
                        ))
                    }
                    Err(_) => None,
                }
            });
            let mut fails = 0;
            for (index, line) in lines.into_iter().enumerate() {
                match line {
                    Some(line) => println!("{line}"),
                    None => {
                        fails += 1;
                        println!("seed {:3}  CRASH (panic)", start + index as i64);
                    }
                }
            }
            println!("\n{}/{} games completed", games - fails, games);
            if fails > 0 {
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
            let mut g = Game::new_with(game_options(&args, players, arg(&args, "--seed", 0) as u64));
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
            let mut fast = g.clone();
            fast.set_fog_memory(false);
            let end_start = Instant::now();
            for _ in 0..samples {
                let mut branch = g.clone();
                let _ = branch.apply(seat, &civvis::game::Action::EndTurn);
                sink += branch.units.len();
            }
            let end_us = end_start.elapsed().as_secs_f64() / samples as f64 * 1e6;
            let fast_end_start = Instant::now();
            for _ in 0..samples {
                let mut branch = fast.clone();
                let _ = branch.apply(seat, &civvis::game::Action::EndTurn);
                sink += branch.units.len();
            }
            let fast_end_us = fast_end_start.elapsed().as_secs_f64() / samples as f64 * 1e6;
            // The same move on a position that is not maintaining fogged
            // memory — what a search that never observes mid-rollout pays.
            let fast_us = mover.as_ref().map(|action| {
                let start = Instant::now();
                for _ in 0..samples {
                    let mut branch = fast.clone();
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
            println!("clone + end (no fog) {fast_end_us:6.1} us  = {:.0}/sec", 1e6 / fast_end_us);
            let _ = sink;
        }
        "tournament" => {
            let names: Vec<String> = args
                .iter()
                .position(|a| a == "--ais")
                .and_then(|i| args.get(i + 1))
                .map(|s| s.split(',').map(|x| x.trim().to_string()).collect())
                .unwrap_or_else(|| vec!["advanced".to_string(), "basic".to_string()]);
            for n in &names {
                if !civvis::elo::BUILTIN_AIS.contains(&n.as_str()) {
                    eprintln!(
                        "unknown AI {n:?}; builtin: {:?} (custom bots: \
                              use civvis::elo::run_tournament from Rust)",
                        civvis::elo::BUILTIN_AIS
                    );
                    std::process::exit(1);
                }
            }
            let cfg = civvis::elo::TourneyCfg {
                games: arg(&args, "--games", 20) as u32,
                players_per_game: arg(&args, "--players", 4) as usize,
                width: auto_dimension(&args, "--width", arg(&args, "--players", 4), true),
                height: auto_dimension(&args, "--height", arg(&args, "--players", 4), false),
                // A tournament writes the project's persistent Elo, so it has
                // to rank on whole games; see `stock_turns`.
                max_turns: arg(&args, "--turns", stock_turns(&args)) as u32,
                num_city_states: auto_cs(&args, arg(&args, "--players", 4)),
                seed: arg(&args, "--seed", 0) as u64,
                k: arg(&args, "--k", 24) as f64,
                verbose: !args.iter().any(|a| a == "--quiet"),
                jobs: jobs_arg(&args),
            };
            let ratings_path = arg_text(&args, "--ratings", civvis::elo::DEFAULT_RATINGS_PATH);
            match civvis::elo::run_persistent_tournament(
                &names,
                civvis::elo::builtin_ai,
                &cfg,
                &ratings_path,
            ) {
                Ok(pool) => {
                    println!();
                    print!("{}", civvis::elo::leaderboard(&pool));
                    println!("ratings checkpointed to {ratings_path}");
                }
                Err(error) => {
                    eprintln!("Elo tournament failed: {error}");
                    std::process::exit(1);
                }
            }
        }
        "selfplay" => {
            let players = arg(&args, "--players", 4).max(2);
            let options = game_options(&args, players, arg(&args, "--seed", 0) as u64);
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
        "league" => {
            let players = arg(&args, "--players", 4).max(2);
            let defaults = civvis::league::LeagueCfg::default();
            let shared_dir =
                std::env::var("CIVVIS_LEAGUE_DIR").unwrap_or_else(|_| defaults.dir.clone());
            let cfg = civvis::league::LeagueCfg {
                rounds: arg(&args, "--rounds", 10).max(0) as u32,
                games_per_round: arg(&args, "--games", 16).max(1) as u32,
                players_per_game: players as usize,
                width: auto_dimension(&args, "--width", players, true),
                height: auto_dimension(&args, "--height", players, false),
                max_turns: arg(&args, "--turns", i64::from(defaults.max_turns)).max(1) as u32,
                num_city_states: auto_cs(&args, players),
                seed: arg(&args, "--seed", 1) as u64,
                jobs: jobs_arg(&args),
                dir: arg_text(&args, "--dir", &shared_dir),
                evolve_every: arg(&args, "--evolve-every", 4).max(0) as u32,
                max_pop: arg(&args, "--pop", 12).max(1) as usize,
                verbose: !args.iter().any(|a| a == "--quiet"),
                worker_id: arg_text(&args, "--worker", &defaults.worker_id),
                lease_seconds: arg(&args, "--lease-seconds", defaults.lease_seconds as i64).max(1)
                    as u64,
            };
            let civ = arg_text(&args, "--civ", "");
            if args.iter().any(|a| a == "--standings") || !civ.is_empty() {
                match civvis::league::load_league(&cfg.dir) {
                    Some(league) => {
                        if !civ.is_empty() {
                            print!("{}", civvis::league::civ_standings(&league, &civ));
                        } else if args.iter().any(|a| a == "--civs") {
                            print!("{}", civvis::league::civ_summary(&league));
                        } else {
                            print!("{}", civvis::league::standings(&league));
                        }
                    }
                    None => {
                        eprintln!("no league at {}/league.json", cfg.dir);
                        std::process::exit(1);
                    }
                }
            } else {
                if let Err(error) = civvis::league::try_run_league(&cfg) {
                    eprintln!("league failed: {error}");
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
                // `eval_game` pays 100 for an outright win on top of a ~50
                // average score share, but at 160 turns almost nothing reaches
                // a victory, so the win term that is supposed to decide
                // champion promotion almost never fired and fitness was score
                // at an arbitrary cutoff. See `stock_turns`.
                max_turns: arg(&args, "--turns", stock_turns(&args)) as u32,
                seed: arg(&args, "--seed", 1) as u64,
                threads: arg(&args, "--threads", 8) as usize,
                dir: arg_text(&args, "--dir", "evolved"),
            });
        }
        "play" => {
            let players = arg(&args, "--players", 4);
            let resumed: Option<Game> = args
                .iter()
                .position(|value| value == "--resume")
                .and_then(|index| args.get(index + 1))
                .map(|path| {
                    let raw = std::fs::read_to_string(path).unwrap_or_else(|error| {
                        eprintln!("cannot read checkpoint {path}: {error}");
                        std::process::exit(2);
                    });
                    let game: Game = serde_json::from_str(&raw).unwrap_or_else(|error| {
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
            let play_options = game_options(&args, players, seed);
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
                    start_eon: play_options.start_eon,
                    start_era: play_options.start_era,
                    map_script,
                    map_topology,
                    map_poles,
                    game_speed,
                    max_turns: play_options.max_turns,
                    victory_conditions: victory_conditions(&args),
                    num_city_states: auto_cs(&args, players),
                    spectate: args.iter().any(|a| a == "--spectate" || a == "--watch"),
                    difficulty: play_options.difficulty,
                    speed: play_options.speed,
                    teams: play_options.teams,
                    leader_pool: play_options.leader_pool,
                    civs: play_options.civs,
                    supervised: args.iter().any(|a| a == "--supervised"),
                    // Ten seconds of result screen. The server bounds this
                    // either way; the floor here is what keeps a negative or
                    // missing value from becoming an enormous `u64`.
                    restart_ms: arg(&args, "--restart-ms", 10_000).max(10_000) as u64,
                    league_dir: {
                        let dir = arg_text(&args, "--league", "");
                        (!dir.is_empty()).then_some(dir)
                    },
                    league_record: args.iter().any(|a| a == "--league-record"),
                },
                resumed,
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
        "rating" => {
            let dir = arg_text(
                &args,
                "--dir",
                &std::env::var("CIVVIS_LEAGUE_DIR").unwrap_or_else(|_| "league".into()),
            );
            let mut history = match civvis::rating::load_history(&dir) {
                Ok(history) if history.len() >= 2 => history,
                Ok(_) => {
                    eprintln!("{dir}/matches.csv has no finished games to rate");
                    std::process::exit(1);
                }
                Err(error) => {
                    eprintln!("cannot read {dir}/matches.csv: {error}");
                    std::process::exit(1);
                }
            };
            // A league directory can hold games of several table sizes; a
            // single size is the cleaner slice to reason about.
            let want_seats = arg(&args, "--seats", 0).max(0) as usize;
            if want_seats > 0 {
                history.retain(|m| m.seats.len() == want_seats);
                if history.len() < 2 {
                    eprintln!("{dir}/matches.csv has fewer than 2 games with {want_seats} seats");
                    std::process::exit(1);
                }
            }
            let seats = history.iter().map(|m| m.seats.len()).sum::<usize>() as f64
                / history.len() as f64;
            let burn_in = arg_f64(&args, "--burn-in", 0.3).clamp(0.0, 0.95);
            let mut cfg = civvis::rating::RatingCfg {
                stage_decay: arg_f64(&args, "--stage-decay", 0.5).clamp(0.0, 1.0),
                beta: arg_f64(&args, "--beta", 0.9).max(1e-3),
                ..civvis::rating::RatingCfg::default()
            };
            for anchor in arg_text(&args, "--anchors", "advanced,basic").split(',') {
                let anchor = anchor.trim();
                if !anchor.is_empty() {
                    cfg.anchors.insert(anchor.to_string());
                }
            }
            // Explicit per-stage credit, e.g. `--stage-credit 1,0.5,0.25,0`
            // to keep the geometric shape but silence an anti-informative
            // last stage. Overrides --stage-decay.
            let credit = arg_text(&args, "--stage-credit", "");
            if !credit.is_empty() {
                let parsed: Vec<f64> = credit
                    .split(',')
                    .filter_map(|x| x.trim().parse::<f64>().ok())
                    .collect();
                if parsed.is_empty() {
                    eprintln!("--stage-credit needs comma-separated numbers");
                    std::process::exit(1);
                }
                cfg.stage_credit = Some(parsed);
            }
            println!("{} games from {dir}/matches.csv\n", history.len());
            if args.iter().any(|a| a == "--stages") {
                let info = civvis::rating::fit_stage_weights(&history, burn_in);
                println!("information carried by each placement stage (nats, measured)");
                println!("  a stage at or below zero is noise and should not move a rating\n");
                for (k, nats) in info.iter().enumerate() {
                    let bar = "#".repeat(((nats.max(0.0)) * 60.0) as usize);
                    println!("  stage {:<3} {:+8.4}  {bar}", k + 1, nats);
                }
            } else if args.iter().any(|a| a == "--sweep") {
                println!(
                    "{:<14}{:>12}{:>10}{:>12}",
                    "stage decay", "winner LL", "accuracy", "info/game"
                );
                for step in 0..=10 {
                    let decay = step as f64 / 10.0;
                    let mut model = civvis::rating::ContextualRating::new(
                        civvis::rating::RatingCfg {
                            stage_decay: decay,
                            ..cfg.clone()
                        },
                    );
                    let m = civvis::rating::evaluate(&mut model, &history, burn_in);
                    println!(
                        "{decay:<14.1}{:>12.4}{:>9.1}%{:>12.4}",
                        m.win_log_loss,
                        100.0 * m.win_accuracy,
                        m.information
                    );
                }
            } else if args.iter().any(|a| a == "--backtest") {
                let rows = civvis::rating::backtest(&history, burn_in, &cfg);
                print!("{}", civvis::rating::backtest_report(&rows, seats));
            } else {
                let rating = civvis::rating::rate_history(&history, &cfg);
                print!("{}", rating.standings());
            }
        }
        _ => {
            println!(
                "usage: civvis <simulate|soak|benchmark|tournament|league|rating|play|evolve|validate|pedia> \
                      [--players N] [--seed N] [--turns N] [--width N] [--height N] \
                      [--city-states N] [--games N] [--ais a,b] [--ratings path] [--port N] [--no-open] \
                      [--map land_only|lakes|inland_sea|pangaea|continents|small_continents|islands|water_world|true_start_earth] \
                      [--shape flat|planet] [--poles poles|no_poles|randomized] \
                      [--difficulty settler|chieftain|warlord|prince|king|emperor|immortal|deity] \
                      [--speed online|quick|standard|epic|marathon] \
                      [--disasters 0|1|2|3|4] [--barbarians on|off] \
                      [--game-modes apocalypse,secret_societies] \
                      [--leader-pool civ6|expanded] \
                      [--human-seats 0,1] [--teams 0,0,1,1] [--mods path/to/mod,path/to/other] \
                      [--victories science,culture,religious,diplomatic,domination,score] \
                      [--spectate] [--supervised] [--restart-ms N] [--resume checkpoint.json] [--strict] \
                      [--league dir] [--league-record] [--standings [--civ Rome | --civs]] [--rounds N] \
                      [--evolve-every N] [--pop N] [--worker ID] [--lease-seconds N] \
                      [rating: --dir league/ --backtest|--sweep|--stages --burn-in F --stage-decay F --anchors a,b]"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use civvis::game::{Action, Game};

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
