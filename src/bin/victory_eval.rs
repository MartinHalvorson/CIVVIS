//! End-to-end victory-condition evaluator.
//!
//! Every major in each game is given the same explicit victory target. A run
//! only passes when the real game loop ends with that victory type; no state
//! is injected and no victory check is called directly.
//!
//! ⚠⚠ THIS BINARY WAS DELETED AS UNREFERENCED IN #1278 AND IT WAS NOT
//! UNREFERENCED. The audit looked for callers in the tree and found none, which
//! is true and was the wrong question: everything that cites this tool cites it
//! in prose, and all of it stayed behind.
//!
//! - `docs/EVAL.md` lists `victory_eval --games 2 --players 2` in the battery a
//!   contributor is told to re-run after any AI or rules batch.
//! - `docs/AI_GUIDE.md` gives a full invocation and documents `--target`.
//! - `src/elo.rs` cites its per-target turn limits.
//! - `tools/civ6_civvis_climb.py`'s ★★★★★ note derives the deployed victory
//!   objective from a measurement taken with it.
//!
//! So for eleven days the repository instructed people to run a command that
//! did not exist, and the standing justification for which victory the live
//! agent plays for could not be reproduced or challenged. A binary with no
//! caller in the tree is not the same thing as a binary nothing depends on;
//! the tests below give this one a caller so the audit's question and the real
//! one give the same answer.
//!
//! ## What it measured on restoration (2026-08-17, main `a19120d9`)
//!
//! At the live ladder's profile — `--games 8 --players 6 --turns 250 --speed
//! online` — over two disjoint seed streams (21000000-21000007 and
//! 23000000-23000007):
//!
//! | target | stream A | stream B | total | winning turns |
//! |---|---|---|---|---|
//! | science | 0/8 | 0/8 | **0/16** | — |
//! | culture | 6/8 | 6/8 | **12/16** | 133-247 |
//! | religious | 5/8 | 3/8 | **8/16** | 60-132 |
//! | diplomatic | 7/8 | 7/8 | **14/16** | 205-247 |
//! | domination | 0/8 | 2/8 | **2/16** | 72, 230 |
//! | score | 8/8 | 7/8 | 15/16 | 250 (the clock) |
//!
//! Four of the five named conditions complete inside the ladder's clock, and
//! the ordering — diplomatic > culture > religious > domination > science —
//! is the same ordering the host produces. `docs/CIV6_LADDER.md`'s census of
//! 199 terminal events in real Settler games ranks the indices
//! 6 (41 games) > 3 (24) > 4 (5) > 5 (3) > 2 (1). Nothing was fitted to make
//! those agree; it is the first cross-check this repository has that the
//! engine's victory pacing tracks Civilization VI's at the profile the ladder
//! actually plays.
//!
//! **Science is the lane that never lands — and it is the deployed default**
//! (`tools/civ6_civvis_climb.py:49`). Culture and Diplomacy, the two that land
//! most, could not be selected at all until #1871.
//!
//! ⚠⚠ THE FIRST RUN OF THIS GOT IT BACKWARDS, and the reason is the flag
//! directly below. Without `--speed`, `--turns 250` is a **Standard** game
//! stopped halfway, not the Online game the ladder plays: Standard/250 reported
//! religious 6/8 and culture, diplomatic, science and domination all 0/8, which
//! is a reading of the clock rather than of the agent. Online prices everything
//! at 50% of Standard, so Online/250 and Standard/500 are the same race. Quote
//! no number out of this tool without the speed beside it.
//!
//! ## `--without <treatment>`
//!
//! ⚠ A LANE TABLE WITH NO CONTROL ARM IS A DESCRIPTION, NOT A MEASUREMENT. The
//! table above says how often each victory lands. It could not say what any one
//! behaviour contributed to that, because every run it has ever taken built the
//! same agent — so a change to the controller moved these counts and nothing
//! here could attribute the movement.
//!
//! `--without <gene>` withholds a registered gene from every seat, so the same
//! seeds replay with one behaviour removed and the lane counts compare
//! directly. This includes opt-ins selected by the deployment ledger, such as
//! `science-victory-drive`. Repeat the flag to withhold more than one; an
//! unknown name lists what is available rather than failing quietly. The
//! fieldless default path is unchanged, so every number above still
//! reproduces.
//!
//! `--deployment` starts each targeted seat from the deployed bridge profile
//! (`AdvancedAi::enable_live_bridge`) before applying `--without` and `--with`.
//! This is the profile to use when asking whether a shipped live treatment
//! matters: the ordinary targeted profile is intentionally kept as the
//! historical native baseline, and does not carry the live bridge's repair
//! genes. Without this switch, withholding a live-only gene from a targeted
//! seat is a no-op rather than a control arm.
//!
//! ## `--denial-pair DEFENDER:RIVAL`
//!
//! A targeted controller used to have no native control arm for
//! `deny-while-targeted`: the ordinary evaluator made every major pursue the
//! same lane, and an adaptive `live` arm never reached the targeted-only
//! gate. This paired mode gives major seat zero an assigned `DEFENDER` lane,
//! pins every other major to `RIVAL`, and plays the same seed twice. Both
//! legs use the live bridge; only seat zero's `deny-while-targeted` flag
//! differs. It reports the selected denial responses as well as outcomes and
//! fails if the enabled leg never selects one, so a zero-width comparison
//! cannot be mistaken for evidence.
use civvis::ai::{run_game, AdvancedAi, Ai, VictoryTarget};
use civvis::game::{Action, Game};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

/// One seat's government, as `id=government:slotted/slots`.
///
/// ⚠ `none` and `chiefdom` are different answers and both are worth seeing. A
/// seat still on `none` has never adopted a government at all — zero slots, so
/// every policy card it ever unlocked is unseated — and at the turn limit that
/// is a defect, not a phase. Anarchy is called out by name because `gov_slots`
/// returns nothing during it: a seat mid-switch runs those turns on an empty
/// deck, and the count alone would read as a seat that simply had no cards.
fn government_cell(
    id: usize,
    government: Option<&str>,
    anarchy_turns: u32,
    seated: usize,
    slots: i64,
) -> String {
    format!(
        "{}={}{}:{}/{}",
        id,
        government.unwrap_or("none"),
        if anarchy_turns > 0 {
            format!("(anarchy {anarchy_turns})")
        } else {
            String::new()
        },
        seated,
        slots,
    )
}

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn deployment_profile(args: &[String]) -> bool {
    args.iter().any(|arg| arg == "--deployment")
}

fn selected_targets(args: &[String]) -> Result<Vec<VictoryTarget>, String> {
    let Some(index) = args.iter().position(|arg| arg == "--target") else {
        return Ok(VictoryTarget::ALL.to_vec());
    };
    let raw = args
        .get(index + 1)
        .ok_or_else(|| "--target requires a value".to_string())?;
    if raw == "all" {
        return Ok(VictoryTarget::ALL.to_vec());
    }
    raw.split(',').map(str::parse).collect()
}

/// The game speed to play at, defaulting to the engine's own default.
///
/// ⚠⚠ ADDED BECAUSE ITS ABSENCE MADE THIS TOOL MEASURE THE WRONG GAME, and the
/// mistake is easy to repeat: `Game::new_full` does not take a speed, so every
/// run this binary has ever done was `GameSpeed::Standard` — the enum's
/// `#[default]`. Ask it for "the live ladder's profile" by passing `--turns 250`
/// and you get a **Standard game stopped halfway**, not the Online game the
/// ladder plays. `GameSpeed::Online` prices everything at 50% of Standard and
/// its own `turn_limit` is 250, so an Online 250 and a Standard 500 are the same
/// race; a Standard 250 is half of one, and a science lane read from it is
/// reporting the clock, not the agent.
fn selected_speed(args: &[String]) -> Result<civvis::setup::GameSpeed, String> {
    let Some(index) = args.iter().position(|arg| arg == "--speed") else {
        return Ok(civvis::setup::GameSpeed::default());
    };
    let raw = args
        .get(index + 1)
        .ok_or_else(|| "--speed requires a value".to_string())?;
    civvis::setup::GameSpeed::from_id(raw)
        .ok_or_else(|| format!("unknown --speed {raw:?}; use online|quick|standard|epic|marathon"))
}

/// The defender's assigned lane and the lane every rival is pinned to for a
/// matched targeted-denial evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DenialPair {
    defender: VictoryTarget,
    rival: VictoryTarget,
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

/// Parse `--denial-pair science:diplomatic` without giving a malformed or
/// symmetric field a chance to become a silent no-op.
fn selected_denial_pair(args: &[String]) -> Result<Option<DenialPair>, String> {
    let Some(index) = args.iter().position(|arg| arg == "--denial-pair") else {
        return Ok(None);
    };
    if has_flag(args, "--target") {
        return Err("--denial-pair supplies its own lanes; do not also pass --target".into());
    }
    if has_flag(args, "--with") || has_flag(args, "--without") {
        return Err(
            "--denial-pair owns the deny-while-targeted treatment/control; do not also pass --with or --without"
                .into(),
        );
    }
    let raw = args
        .get(index + 1)
        .ok_or_else(|| "--denial-pair requires DEFENDER:RIVAL".to_string())?;
    let (defender, rival) = raw.split_once(':').ok_or_else(|| {
        "--denial-pair must be DEFENDER:RIVAL, for example science:diplomatic".to_string()
    })?;
    let defender = defender.parse::<VictoryTarget>()?;
    let rival = rival.parse::<VictoryTarget>()?;
    if defender == rival {
        return Err(
            "--denial-pair needs distinct lanes: identical targets recreate the old symmetric no-op"
                .into(),
        );
    }
    Ok(Some(DenialPair { defender, rival }))
}

/// What one leg of a matched targeted-denial pair actually did.
#[derive(Debug)]
struct DenialTrial {
    defender_won: bool,
    victory: String,
    turn: u32,
    selections: u32,
    first_selection: Option<(u32, usize, &'static str)>,
}

/// Build the two-arm experiment from the same deployment profile. Major zero
/// is the measured defender; all other majors are fixed lane pursuers and
/// therefore contribute no treatment variance. Minor and barbarian seats keep
/// their ordinary controller, just as they do in the normal evaluator.
fn denial_fleet(game: &Game, pair: DenialPair, treatment: bool) -> Vec<AdvancedAi> {
    game.players
        .iter()
        .map(|player| {
            if player.is_minor || player.is_barbarian {
                return AdvancedAi::new();
            }
            let mut ai = AdvancedAi::targeting(if player.id == 0 {
                pair.defender
            } else {
                pair.rival
            });
            // This probe exists for the live-only gate. Both legs must carry
            // the deployment bundle before the one flag is withheld.
            ai.enable_live_bridge();
            if player.id == 0 && !treatment {
                ai.disable_deny_while_targeted();
            }
            ai
        })
        .collect()
}

/// Run one leg with the same headless loop as [`run_game`], observing the
/// defender immediately before each of its turns. `denial_target` is the
/// exact actionable selector the planner will use, so counting it does not
/// invent a second definition of "the treatment fired".
fn run_denial_trial(
    players: usize,
    width: i32,
    height: i32,
    speed: civvis::setup::GameSpeed,
    turns: u32,
    seed: u64,
    pair: DenialPair,
    treatment: bool,
) -> DenialTrial {
    let city_states = if matches!(pair.defender, VictoryTarget::Diplomacy)
        || matches!(pair.rival, VictoryTarget::Diplomacy)
    {
        (players + 1).max(3)
    } else {
        0
    };
    let mut game = Game::new_with(civvis::game::GameOptions {
        barbarians: false,
        speed: speed.id().to_string(),
        ..civvis::game::GameOptions::new(players, width, height, seed, turns, city_states)
    });
    let mut ais = denial_fleet(&game, pair, treatment);
    // Keep the mechanics identical to `run_game`; the only addition is the
    // read-only diagnostic seam before the defender decides.
    game.set_fog_memory(false);
    game.set_war_ledger(false);
    let mut selections = 0;
    let mut first_selection = None;
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        if pid == 0 {
            if let Some((target, counter)) = ais[pid].denial_target(&game, pid) {
                selections += 1;
                first_selection.get_or_insert((game.turn, target, counter.as_str()));
            }
        }
        ais[pid].take_turn(&mut game, pid);
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    // This evaluator leaves score enabled, so the public EndTurn path awards
    // the turn-limit winner before the bound can expire. `run_game` also has
    // a crate-private fallback for score-disabled worlds; this probe does not
    // offer a victory-mask switch that could need it.
    let turn = game.reported_turn();
    let victory = game.victory_type.unwrap_or_else(|| "none".to_string());
    DenialTrial {
        defender_won: game.winner == Some(0),
        victory,
        turn,
        selections,
        first_selection,
    }
}

fn trial_cell(trial: &DenialTrial) -> String {
    let first = trial.first_selection.map_or_else(
        || "none".to_string(),
        |(turn, target, counter)| format!("t{turn}/p{target}/{counter}"),
    );
    format!(
        "defender={} victory={} t{} selections={} first={first}",
        if trial.defender_won { "win" } else { "loss" },
        trial.victory,
        trial.turn,
        trial.selections,
    )
}

/// A pair is evidence only when the enabled treatment actually reached its
/// decision seam. The withheld control must be silent because a targeted
/// planner short-circuits before it can choose denial without this flag.
fn validate_denial_exposure(
    treatment_selections: u32,
    control_selections: u32,
) -> Result<(), String> {
    if control_selections != 0 {
        return Err(format!(
            "invalid control: targeted defender selected {control_selections} denial responses with deny-while-targeted withheld"
        ));
    }
    if treatment_selections == 0 {
        return Err(
            "inert denial pair: the enabled defender selected no actionable denial response; do not read its outcome contrast as a treatment measurement"
                .into(),
        );
    }
    Ok(())
}

/// Run the live treatment and its withheld control on every requested seed.
/// Unlike ordinary lane verification, a defender loss is an observation, not
/// a command failure: the paired contrast is the result.
fn run_denial_pair(args: &[String], pair: DenialPair) -> Result<(), String> {
    let speed = selected_speed(args)?;
    let games = number(args, "--games", 3);
    let start_seed = number(args, "--start-seed", 9_000) as u64;
    let players = number(args, "--players", 2).clamp(2, 8);
    let width = number(args, "--width", 24).max(16) as i32;
    let height = number(args, "--height", 16).max(12) as i32;
    let turns = args
        .iter()
        .position(|arg| arg == "--turns")
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or_else(|| default_turn_limit(pair.defender).max(default_turn_limit(pair.rival)));
    let started = Instant::now();
    let mut treatment_wins = 0;
    let mut control_wins = 0;
    let mut treatment_selections = 0;
    let mut control_selections = 0;
    let mut changed_outcomes = 0;

    println!(
        "targeted denial pair: defender={} rival={} (deployment bridge; treatment vs withheld deny-while-targeted)",
        pair.defender.as_str(),
        pair.rival.as_str(),
    );
    for game in 0..games {
        let seed = start_seed + game as u64;
        let treatment = run_denial_trial(players, width, height, speed, turns, seed, pair, true);
        let control = run_denial_trial(players, width, height, speed, turns, seed, pair, false);
        treatment_wins += usize::from(treatment.defender_won);
        control_wins += usize::from(control.defender_won);
        treatment_selections += treatment.selections;
        control_selections += control.selections;
        changed_outcomes += usize::from(
            treatment.defender_won != control.defender_won
                || treatment.victory != control.victory
                || treatment.turn != control.turn,
        );
        println!(
            "seed={seed} treatment=[{}] control=[{}]",
            trial_cell(&treatment),
            trial_cell(&control),
        );
    }

    println!(
        "\n{} paired seeds in {:.2}s: defender wins treatment={}/{} control={}/{}; actionable selections treatment={} control={}; changed outcomes={}",
        games,
        started.elapsed().as_secs_f64(),
        treatment_wins,
        games,
        control_wins,
        games,
        treatment_selections,
        control_selections,
        changed_outcomes,
    );
    validate_denial_exposure(treatment_selections, control_selections)
}

fn default_turn_limit(target: VictoryTarget) -> u32 {
    match target {
        VictoryTarget::Religion => 450,
        VictoryTarget::Domination => 650,
        VictoryTarget::Diplomacy => 750,
        VictoryTarget::Culture => 1_500,
        VictoryTarget::Science => 1_300,
        VictoryTarget::Score => 300,
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let denial_pair = selected_denial_pair(&args).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    if let Some(pair) = denial_pair {
        if let Err(error) = run_denial_pair(&args, pair) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }
    let targets = selected_targets(&args).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let speed = selected_speed(&args).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    });
    let games = number(&args, "--games", 3);
    let deployment = deployment_profile(&args);
    let start_seed = number(&args, "--start-seed", 9_000) as u64;
    let players = number(&args, "--players", 2).clamp(2, 8);
    let width = number(&args, "--width", 24).max(16) as i32;
    let height = number(&args, "--height", 16).max(12) as i32;
    let override_turns = args
        .iter()
        .position(|arg| arg == "--turns")
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse::<u32>().ok());
    // ⚠ A LANE TABLE WITH NO CONTROL ARM IS A DESCRIPTION, NOT A MEASUREMENT.
    // The table in this file's header says how often each victory lands; it
    // could not say what any one behaviour contributed to that, because every
    // run built the same agent. `--without <gene>` withholds a live or
    // production gene from every seat, so the same seeds can be replayed
    // with one behaviour removed and the lane counts compared directly.
    // Repeat the flag to withhold more than one.
    let withheld: Vec<&'static civvis::ai::Gene> = {
        let mut rows = Vec::new();
        for (index, arg) in args.iter().enumerate() {
            if arg != "--without" {
                continue;
            }
            let Some(name) = args.get(index + 1) else {
                eprintln!("--without requires a treatment name");
                std::process::exit(2);
            };
            // Every registry row has a paired disable function. Opt-ins are
            // included because the deployment ledger can seat one before the
            // ablation is applied; filtering them out made a deployed opt-in
            // impossible to measure.
            match civvis::ai::GENES
                .iter()
                .find(|gene| gene.field == name || gene.tag == name)
            {
                Some(row) => rows.push(row),
                None => {
                    eprintln!("unknown gene {name:?}; known names:");
                    for gene in civvis::ai::GENES.iter() {
                        eprintln!("  {} ({})", gene.tag, gene.field);
                    }
                    std::process::exit(2);
                }
            }
        }
        rows
    };
    if !withheld.is_empty() {
        println!(
            "withholding: {}",
            withheld
                .iter()
                .map(|gene| gene.tag)
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    // The complement of `--without`: an off-by-default production arm can be
    // seated by name, so the targeted regime it exists for can be measured
    // before any promotion question is asked. Rows are the registry's
    // opt-ins; repeat the flag to enable more than one.
    let opted_in: Vec<&'static civvis::ai::Gene> = {
        let mut rows = Vec::new();
        for (index, arg) in args.iter().enumerate() {
            if arg != "--with" {
                continue;
            }
            let Some(name) = args.get(index + 1) else {
                eprintln!("--with requires a treatment name");
                std::process::exit(2);
            };
            match civvis::ai::GENES
                .iter()
                .filter(|gene| gene.opt_in())
                .find(|gene| gene.field == name || gene.tag == name)
            {
                Some(row) => rows.push(row),
                None => {
                    eprintln!("unknown opt-in {name:?}; known names:");
                    for gene in civvis::ai::GENES.iter().filter(|gene| gene.opt_in()) {
                        eprintln!("  {} ({})", gene.tag, gene.field);
                    }
                    std::process::exit(2);
                }
            }
        }
        rows
    };
    if !opted_in.is_empty() {
        println!(
            "opting in: {}",
            opted_in
                .iter()
                .map(|gene| gene.tag)
                .collect::<Vec<_>>()
                .join(",")
        );
    }
    let mut failures = 0;
    let mut winners: BTreeMap<&'static str, BTreeSet<usize>> = BTreeMap::new();
    let started = Instant::now();

    for target in targets.iter().copied() {
        for game_index in 0..games {
            let seed = start_seed + game_index as u64;
            let city_states = if target == VictoryTarget::Diplomacy {
                (players + 1).max(3)
            } else {
                0
            };
            let turns = override_turns.unwrap_or_else(|| default_turn_limit(target));
            let game_started = Instant::now();
            let mut game = Game::new_with(civvis::game::GameOptions {
                barbarians: false,
                speed: speed.id().to_string(),
                ..civvis::game::GameOptions::new(players, width, height, seed, turns, city_states)
            });
            let mut ais = AdvancedAi::fleet_targeting(&game, target);
            for ai in ais.iter_mut() {
                if deployment {
                    // Match the live bridge before applying arms. In
                    // particular, this seats ledger-selected genes such as
                    // science-victory-drive, and makes --without a real
                    // ablation instead of disabling an already-false field.
                    ai.enable_live_bridge();
                }
                for gene in &withheld {
                    (gene.disable)(ai);
                }
                for gene in &opted_in {
                    (gene.enable)(ai);
                }
            }
            run_game(&mut game, &mut ais);

            let actual = game.victory_type.as_deref().unwrap_or("none");
            let winner = game.winner.unwrap_or(usize::MAX);
            let major_progress: Vec<(usize, bool, usize, usize, usize)> =
                game.players
                    .iter()
                    .filter(|player| !player.is_minor && !player.is_barbarian)
                    .map(|player| {
                        let era = player
                            .techs
                            .iter()
                            .filter_map(|node| game.rules.techs.get(node).map(|spec| spec.era))
                            .chain(player.civics.iter().filter_map(|node| {
                                game.rules.civics.get(node).map(|spec| spec.era)
                            }))
                            .max()
                            .unwrap_or(0);
                        (
                            player.id,
                            player.alive,
                            era,
                            game.player_city_ids(player.id).len(),
                            player.techs.len(),
                        )
                    })
                    .collect();
            // ★★★★★ WHICH WONDERS THE LANE ACTUALLY FINISHED, WHICH IS NOT
            // WHAT ITS VALUATION SAYS IT WANTS.
            //
            // The `Item::Wonder` valuation is a claim about intent; a wonder on
            // the map is the artifact. Without this line the two are impossible
            // to tell apart, and they came apart immediately: over 32
            // 250-turn games a Diplomacy-targeted agent finishes a wonder in
            // **one** of them, against a Culture agent's three in a single game
            // and a Score agent's eight. Seven of the twenty points a
            // diplomatic victory needs are wonders, so that is not a pricing
            // result — it is a reachability one, and pricing cannot fix it. The
            // Mahabodhi Temple needs a founded religion, a Holy Site and a
            // Temple; the Statue of Liberty needs a Harbor and Civil
            // Engineering. A diplomatic empire builds those once in 32 games.
            //
            // Owner-tagged, because "somebody built the Great Library" and "the
            // seat we are measuring built the Great Library" are different
            // facts and the lane result only follows from the second.
            let wonders: Vec<String> = game
                .cities
                .values()
                .filter(|city| !game.players[city.owner].is_minor)
                .flat_map(|city| {
                    city.wonders
                        .keys()
                        .map(move |wonder| format!("{}:{wonder}", city.owner))
                })
                .collect();
            // ★★★★ WHAT THE EMPIRE WAS GOVERNED BY, WHICH THIS TOOL NEVER SAID.
            //
            // A verification game reported eras, cities and techs — the outputs
            // — and nothing about the one empire-wide choice that multiplies
            // all three. A government is four to six policy slots and the cards
            // in them, and the difference between an empire under Monarchy with
            // four cards seated and one still under `none` with zero is not a
            // small one: `gov_slots` returns nothing at all in Anarchy, so a
            // seat mid-switch is running the whole turn on an empty deck. Read
            // a lane table without it and a lane that never lands looks like a
            // pacing problem rather than an empire that spent forty turns
            // ungoverned.
            //
            // Every major, because the interesting comparison is against the
            // seats that beat this one; `slotted/slots` rather than a card list
            // per seat, because the counts are what a scan needs and the
            // winner's actual deck follows on the same line.
            let governments: Vec<String> = game
                .players
                .iter()
                .filter(|player| !player.is_minor && !player.is_barbarian)
                .map(|player| {
                    let slots = game.gov_slots(player.id);
                    government_cell(
                        player.id,
                        player.government.as_deref(),
                        player.anarchy_turns,
                        player.policies.len(),
                        slots.military + slots.economic + slots.diplomatic + slots.wildcard,
                    )
                })
                .collect();
            // The winner's actual deck. A count says a slot was filled; only
            // the names say what the empire was actually paying for, which is
            // the question a lane result raises first.
            let seated: String = game
                .players
                .get(game.winner.unwrap_or(usize::MAX))
                .map(|player| {
                    player
                        .policies
                        .iter()
                        .map(|card| card.to_string())
                        .collect::<Vec<_>>()
                        .join("+")
                })
                .filter(|cards| !cards.is_empty())
                .unwrap_or_else(|| "none".to_string());
            let passed = actual == target.as_str();
            failures += usize::from(!passed);
            if passed {
                winners.entry(target.as_str()).or_default().insert(winner);
            }
            let winner_state = game.players.get(winner);
            let progress = winner_state.map(|player| match target {
                VictoryTarget::Science => format!(
                    "techs={}/{} projects={} distance={:.0} science={:.1}",
                    player.techs.len(),
                    game.rules.techs.len(),
                    player.science_projects.len(),
                    player.exoplanet_distance,
                    game.player_city_ids(winner)
                        .into_iter()
                        .map(|city| game.city_yields(city).science)
                        .sum::<f64>()
                ),
                VictoryTarget::Culture => {
                    let target = game
                        .players
                        .iter()
                        .filter(|rival| {
                            rival.id != winner
                                && rival.alive
                                && !rival.is_minor
                                && !rival.is_barbarian
                        })
                        .map(|rival| game.domestic_tourists(rival.id))
                        .max()
                        .unwrap_or(0);
                    let cities = game.player_city_ids(winner);
                    let theaters = cities
                        .iter()
                        .filter(|city| {
                            game.cities[city].districts.contains_key(civvis::name!("theater_square"))
                                || game.cities[city].districts.contains_key(civvis::name!("acropolis"))
                        })
                        .count();
                    let tourist_improvements = cities
                        .iter()
                        .flat_map(|city| game.cities[city].owned_tiles.iter())
                        .filter_map(|position| game.map.tiles[position].improvement.as_deref())
                        .filter(|improvement| {
                            game.rules.improvements[*improvement]
                                .effects
                                .get("tourism")
                                .copied()
                                .unwrap_or(0.0)
                                > 0.0
                        })
                        .count();
                    format!(
                        "visiting={} target={} domestic={} tourism={:.1}/turn cities={} theaters={} tourist_tiles={} lifetime={:.0}",
                        game.foreign_tourists(winner),
                        target,
                        game.domestic_tourists(winner),
                        game.tourism_per_turn(winner),
                        cities.len(),
                        theaters,
                        tourist_improvements,
                        player.tourism_lifetime,
                    )
                }
                VictoryTarget::Religion => {
                    format!("religion={}", player.religion.as_deref().unwrap_or("none"))
                }
                VictoryTarget::Diplomacy => format!("dvp={}", player.dvp),
                VictoryTarget::Domination => {
                    format!("cities={}", game.player_city_ids(winner).len())
                }
                VictoryTarget::Score => format!("score={}", game.score(winner)),
            });
            println!(
                "{:<11} seed={} target={:<10} actual={:<10} winner={} turn={} world_era={} majors=(id,alive,era,cities,techs){:?} wonders=[{}] govs=[{}] policies={} {} [{:.2}s]",
                if passed { "PASS" } else { "FAIL" },
                seed,
                target.as_str(),
                actual,
                if winner == usize::MAX {
                    "none".to_string()
                } else {
                    winner.to_string()
                },
                game.reported_turn(),
                game.world_era,
                major_progress,
                wonders.join(" "),
                governments.join(" "),
                seated,
                progress.unwrap_or_default(),
                game_started.elapsed().as_secs_f64(),
            );
        }
    }

    println!(
        "\nspeed={} ({}% of Standard costs, own turn limit {})",
        speed.id(),
        speed.cost_percent(),
        speed.turn_limit()
    );
    println!(
        "profile={}",
        if deployment { "deployment" } else { "targeted" }
    );
    println!("seat winners by target:");
    for target in &targets {
        println!(
            "  {:<10} {:?}",
            target.as_str(),
            winners.get(target.as_str())
        );
    }
    println!(
        "{} games, {} failures in {:.2}s",
        targets.len() * games,
        failures,
        started.elapsed().as_secs_f64()
    );
    if failures > 0 {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three answers a seat can give, told apart.
    #[test]
    fn a_government_cell_distinguishes_ungoverned_from_anarchic_from_seated() {
        assert_eq!(
            government_cell(0, Some("monarchy"), 0, 4, 6),
            "0=monarchy:4/6",
            "an ordinary seat reads its cards against its slots"
        );
        assert_eq!(
            government_cell(3, None, 0, 0, 0),
            "3=none:0/0",
            "a seat that never adopted a government must say so rather than \
             render as an empty deck"
        );
        assert_eq!(
            government_cell(2, Some("monarchy"), 3, 2, 0),
            "2=monarchy(anarchy 3):2/0",
            "Anarchy has no slots of its own, so the seat is named as running \
             on none of the cards it holds"
        );
    }

    fn args(words: &[&str]) -> Vec<String> {
        words.iter().map(|word| (*word).to_string()).collect()
    }

    #[test]
    fn deployment_profile_is_opt_in_and_spelled_as_a_standalone_flag() {
        assert!(!deployment_profile(&args(&[])));
        assert!(deployment_profile(&args(&["--deployment"])));
        assert!(!deployment_profile(&args(&["--deployment-mode"])));
    }

    #[test]
    fn denial_pair_requires_two_distinct_named_lanes() {
        assert_eq!(
            selected_denial_pair(&args(&["--denial-pair", "science:diplomatic"])).unwrap(),
            Some(DenialPair {
                defender: VictoryTarget::Science,
                rival: VictoryTarget::Diplomacy,
            })
        );
        assert_eq!(
            selected_denial_pair(&args(&["--denial-pair", "religious:conquest"])).unwrap(),
            Some(DenialPair {
                defender: VictoryTarget::Religion,
                rival: VictoryTarget::Domination,
            })
        );
        for words in [
            vec!["--denial-pair"],
            vec!["--denial-pair", "science"],
            vec!["--denial-pair", "science:science"],
            vec!["--denial-pair", "science:diplomatic", "--target", "science"],
            vec![
                "--denial-pair",
                "science:diplomatic",
                "--without",
                "deny-while-targeted",
            ],
        ] {
            assert!(selected_denial_pair(&args(&words)).is_err(), "{words:?}");
        }
    }

    #[test]
    fn denial_pair_only_withholds_the_measured_defender() {
        let game = Game::new(3, 24, 16, 91_002, 80, 0);
        let pair = DenialPair {
            defender: VictoryTarget::Science,
            rival: VictoryTarget::Diplomacy,
        };
        let treatment = denial_fleet(&game, pair, true);
        let control = denial_fleet(&game, pair, false);
        assert_eq!(treatment[0].victory_target(), Some(pair.defender));
        assert!(treatment[0].deny_while_targeted);
        assert_eq!(control[0].victory_target(), Some(pair.defender));
        assert!(!control[0].deny_while_targeted);
        for pid in 1..3 {
            assert_eq!(treatment[pid].victory_target(), Some(pair.rival));
            assert_eq!(control[pid].victory_target(), Some(pair.rival));
            assert!(treatment[pid].deny_while_targeted);
            assert!(control[pid].deny_while_targeted);
        }
    }

    #[test]
    fn denial_pair_refuses_to_price_a_zero_exposure_arm() {
        assert!(validate_denial_exposure(1, 0).is_ok());
        assert!(validate_denial_exposure(0, 0)
            .unwrap_err()
            .contains("inert denial pair"));
        assert!(validate_denial_exposure(1, 1)
            .unwrap_err()
            .contains("invalid control"));
    }

    /// The documented one-map smoke is a real branch exposure, not merely a
    /// parser example. Both legs play the same deterministic world; only the
    /// enabled defender may select denial because it is assigned a lane.
    #[test]
    fn documented_denial_pair_smoke_exposes_only_the_enabled_defender() {
        use civvis::setup::GameSpeed;

        let pair = DenialPair {
            defender: VictoryTarget::Science,
            rival: VictoryTarget::Religion,
        };
        let treatment = run_denial_trial(2, 24, 16, GameSpeed::Online, 200, 940_000, pair, true);
        let control = run_denial_trial(2, 24, 16, GameSpeed::Online, 200, 940_000, pair, false);
        assert!(
            treatment.selections > 0,
            "the documented smoke must reach the enabled denial branch: {treatment:?}"
        );
        assert_eq!(control.selections, 0, "the withheld arm must stay silent");
        assert_eq!(
            treatment.victory, control.victory,
            "the fixture tests branch exposure, not an outcome claim"
        );
        validate_denial_exposure(treatment.selections, control.selections).unwrap();
    }

    /// ⚠ The reason this test exists is not the parser. #1278 removed this
    /// binary for having "zero tests and zero invocations", and the second half
    /// was false — `docs/EVAL.md` and `docs/AI_GUIDE.md` invoke it, in prose an
    /// audit cannot grep. The first half was true, and it is what made the
    /// second half easy to believe. This is the caller the tree lacked.
    #[test]
    fn the_default_target_set_is_every_victory_the_engine_implements() {
        assert_eq!(selected_targets(&args(&[])).unwrap(), VictoryTarget::ALL);
        assert_eq!(
            selected_targets(&args(&["--target", "all"])).unwrap(),
            VictoryTarget::ALL
        );
    }

    #[test]
    fn a_comma_separated_subset_selects_exactly_those_lanes() {
        assert_eq!(
            selected_targets(&args(&["--target", "religion,score"])).unwrap(),
            vec![VictoryTarget::Religion, VictoryTarget::Score]
        );
        // The aliases `VictoryTarget::from_str` accepts work here too, so a
        // command line copied out of `docs/EVAL.md` runs whichever spelling the
        // prose happened to use.
        assert_eq!(
            selected_targets(&args(&["--target", "religious,diplomatic,conquest"])).unwrap(),
            vec![
                VictoryTarget::Religion,
                VictoryTarget::Diplomacy,
                VictoryTarget::Domination
            ]
        );
    }

    #[test]
    fn an_unknown_target_is_refused_rather_than_silently_dropped() {
        assert!(selected_targets(&args(&["--target", "religous"])).is_err());
        assert!(selected_targets(&args(&["--target"])).is_err());
    }

    /// ★★★★★ THESE NUMBERS ARE THE ARGUMENT, so they are pinned.
    ///
    /// The live ladder runs 250 turns. Four of the six lanes are budgeted here
    /// at more than twice that and two at more than five times it, which is the
    /// cheapest available statement of why a 250-turn ladder attempt aimed at
    /// Science or Culture has never completed one — and it is a statement the
    /// tree lost entirely for eleven days when this file was deleted while
    /// `civ6_civvis_climb.py` went on citing it.
    #[test]
    fn every_lane_declares_the_budget_its_race_actually_needs() {
        let limits: Vec<(&str, u32)> = VictoryTarget::ALL
            .iter()
            .map(|target| (target.as_str(), default_turn_limit(*target)))
            .collect();
        assert_eq!(
            limits,
            vec![
                ("science", 1_300),
                ("culture", 1_500),
                ("religious", 450),
                ("diplomatic", 750),
                ("domination", 650),
                ("score", 300),
            ]
        );
        let ladder_budget = 250;
        assert!(
            limits.iter().all(|(_, turns)| *turns > ladder_budget),
            "every lane's own budget exceeds the ladder's 250 turns; the ladder \
             is not measuring these races at their own length"
        );
    }

    /// ⚠⚠ THE FLAG THAT DECIDES WHETHER THE ANSWER IS BACKWARDS.
    ///
    /// Standard/250 and Online/250 are not the same experiment and this tool
    /// reported the second while running the first, because `Game::new_full`
    /// takes no speed and the enum's default is Standard. Online costs 50% of
    /// Standard, so an Online 250-turn game is a Standard 500-turn race; read at
    /// Standard, the ladder's clock cuts every long lane off halfway and the
    /// tool reports that as the agent failing.
    #[test]
    fn the_speed_is_selectable_and_defaults_to_the_engines_own() {
        assert_eq!(
            selected_speed(&args(&[])).unwrap(),
            civvis::setup::GameSpeed::default()
        );
        assert_eq!(
            selected_speed(&args(&["--speed", "online"])).unwrap(),
            civvis::setup::GameSpeed::Online
        );
        assert!(selected_speed(&args(&["--speed", "instant"])).is_err());
        assert!(selected_speed(&args(&["--speed"])).is_err());
    }

    /// The two profiles that are the same race, stated as an assertion so the
    /// relationship survives a change to either table.
    #[test]
    fn an_online_game_is_a_standard_game_at_half_the_cost_and_half_the_clock() {
        use civvis::setup::GameSpeed;
        assert_eq!(
            GameSpeed::Online.cost_percent() * 2,
            GameSpeed::Standard.cost_percent()
        );
        assert_eq!(
            GameSpeed::Online.turn_limit() * 2,
            GameSpeed::Standard.turn_limit()
        );
        // And the ladder's clock IS Online's own limit, which is why 250 is not
        // an arbitrary harness choice: `tools/civ6_civvis_climb.py --max-turns`.
        assert_eq!(GameSpeed::Online.turn_limit(), 250);
    }

    #[test]
    fn numbers_come_off_the_command_line_and_fall_back_when_absent() {
        assert_eq!(number(&args(&["--games", "8"]), "--games", 3), 8);
        assert_eq!(number(&args(&[]), "--games", 3), 3);
        // A flag with an unparseable value takes the default rather than
        // panicking mid-battery.
        assert_eq!(number(&args(&["--games", "lots"]), "--games", 3), 3);
    }
}
