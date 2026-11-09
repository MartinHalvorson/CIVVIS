//! Defence replay: play a live board forward from a war declared on us.
//!
//! Early surprise wars are what eliminate the live seat. At Lv 7 (Immortal) on
//! Online speed 10 of 110 games of 2026-11-02/03 ended with our civilization
//! destroyed, and the first eight games on Standard speed lost three that way:
//! a neighbour whose city stands four or five tiles from ours declares
//! between turns 28 and 90 and takes most of our cities within fifteen turns.
//! The whole-game screen cannot price a defence against that, because its
//! rival chairs play our own genome and do not rush (0 of 32 Standard seats
//! died there), and a live game never repeats its map. This rebuilds the
//! recorded turn of a live run at which a rival is at war with us (or is about
//! to be), seats our agent on the live genome, aims the attacker's campaign at
//! us, puts the engine's own agent in every other chair, and plays it
//! forward, reporting which of our cities fell and when.
//!
//! The board is the seat's view (`mirror::rebuild_from_state`): the attacker
//! brings the units we could see at that turn, not the ones we could not, and
//! its cities exist only where we had seen them. So a replay is a fixed,
//! repeatable defence problem on the ground where we lost, not a forecast of
//! the live game. Compare arms on the same scenarios and seeds; do not read
//! the absolute numbers as live ones. The game speed is the run's own: the
//! mirror restores `game_speed` from the seat header, so a Standard run
//! replays at Standard.
//!
//! ```bash
//! cargo run --release --features developer-tools --bin defence_replay -- \
//!     --run ~/civvis-civ6-runs/control/civvis-20261103T212457Z --turn 89 \
//!     --attacker persia --turns 30 --seeds 8 \
//!     --genome <live genome header or tag list> [--with a,b] [--without a,b]
//! ```
//!
//! `--genome FILE` plays our seat on exactly the tags FILE names (a run's
//! genome header, `{"treatments": [...]}`, or a plain tag list), the way
//! `gene_screen --live-genome` does. Without it the seat plays the deployment
//! ledger plus `--with-file`/`--with`, as `siege_replay` does. `--with` and
//! `--without` then switch single genes on or off for an arm. `--lane`
//! (default science) is the victory lane our seat plays. The attacker plays
//! the deployment ledger on the Domination lane, retargeted at us
//! (`AdvancedAi::retarget_against`), and declares a surprise war on its first
//! turn if the recorded turn is still at peace. The war is held for the whole
//! replay (see `hold_war`); `--allow-peace` lets it end.
//!
//! Each seed prints one JSON line; the last stderr line sums the arm.

use civvis::ai::{AdvancedAi, Ai, VictoryTarget, GENES};
use civvis::game::{Action, Game};
use std::collections::BTreeSet;

fn text(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn number(args: &[String], key: &str, default: u64) -> u64 {
    text(args, key)
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn tags(value: Option<String>) -> Vec<String> {
    value
        .map(|list| {
            list.split(|c: char| c == ',' || c.is_whitespace())
                .map(|tag| tag.trim().to_string())
                .filter(|tag| !tag.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn home(path: &str) -> String {
    path.replacen('~', &std::env::var("HOME").unwrap_or_default(), 1)
}

/// The tags a `--genome` file names: a genome header (`{"treatments": [...]}`
/// on its first line) or a plain list separated by commas or whitespace.
fn genome_tags(text: &str) -> BTreeSet<String> {
    let text = text.trim();
    if text.starts_with('{') {
        let line = text.lines().next().unwrap_or(text);
        let header: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|error| {
            eprintln!("--genome is not a genome header: {error}");
            std::process::exit(2);
        });
        let Some(treatments) = header.get("treatments").and_then(|value| value.as_array()) else {
            eprintln!("--genome header has no `treatments` array");
            std::process::exit(2);
        };
        return treatments
            .iter()
            .filter_map(|tag| tag.as_str().map(str::to_string))
            .collect();
    }
    tags(Some(text.to_string())).into_iter().collect()
}

/// How our seat is configured for one arm.
struct Seat {
    lane: VictoryTarget,
    /// `--genome`: play exactly these tags.
    genome: Option<BTreeSet<String>>,
    /// Without `--genome`: ledger-held tags forced on, as `siege_replay` does.
    forced: Vec<String>,
    with: Vec<String>,
    without: Vec<String>,
}

impl Seat {
    fn agent(&self) -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(self.lane);
        ai.enable_live_bridge_universe();
        match &self.genome {
            Some(on) => {
                for gene in GENES {
                    if on.contains(gene.tag) {
                        (gene.enable)(&mut ai);
                    } else {
                        (gene.disable)(&mut ai);
                    }
                }
            }
            None => {
                let forced: Vec<&str> = self.forced.iter().map(String::as_str).collect();
                ai.apply_gene_ledger_with_forced_live(&forced);
            }
        }
        for tag in &self.with {
            if let Some(gene) = civvis::ai::gene(tag) {
                (gene.enable)(&mut ai);
            }
        }
        for tag in &self.without {
            if let Some(gene) = civvis::ai::gene(tag) {
                (gene.disable)(&mut ai);
            }
        }
        ai
    }
}

/// What one replay seed did to us.
struct Outcome {
    /// Our cities at the start that someone else holds at the end (or that
    /// are gone).
    lost: usize,
    /// Of those, the ones the attacker holds.
    to_attacker: usize,
    /// Our starting cities that changed hands at any point, including the
    /// ones we took back: `fell - lost` is what a counterattack recovered.
    fell: usize,
    /// Turns from the start to our first city lost.
    first_loss: Option<u32>,
    capital_held: bool,
    alive: bool,
    cities_end: usize,
    our_military_start: usize,
    our_military_end: usize,
    attacker_military_start: usize,
    attacker_military_end: usize,
    at_war_end: bool,
    declared: bool,
    /// Times a peace signed in the replay was undone by `--hold-war`.
    peace_undone: u32,
    turns: u32,
}

fn military_units(g: &Game, pid: usize) -> usize {
    g.units
        .values()
        .filter(|unit| unit.owner == pid && g.rules.units[unit.kind].class == "military")
        .count()
}

fn our_cities(g: &Game) -> BTreeSet<(i32, i32)> {
    g.cities
        .values()
        .filter(|city| city.owner == 0)
        .map(|city| city.pos)
        .collect()
}

/// Keep the war between `attacker` and us running through the replay. The
/// live attacker is the host's agent, which does not sue for peace inside its
/// own minimum war length, but a rebuilt board carries the war as a bare
/// `at_war` pair with no `WarRecord`, so `peace_available_at` finds no start
/// turn and the replayed attacker can sign peace on the next turn. Where a
/// record exists its `joint_war_until` pins the war; otherwise a peace signed
/// in the replay is undone at once.
fn hold_war(g: &mut Game, attacker: usize, until: u32) -> bool {
    let key = (0, attacker);
    if let Some(record) = g.wars.get_mut(&key) {
        record.joint_war_until = Some(record.joint_war_until.unwrap_or(0).max(until));
        return true;
    }
    false
}

struct ReplayOptions<'a> {
    attacker: usize,
    turns: u32,
    hold: bool,
    why: bool,
    attacker_ai: &'a str,
    declare_after: u32,
}

fn replay(base: &Game, seed: u64, seat: &Seat, options: ReplayOptions<'_>) -> Outcome {
    let ReplayOptions {
        attacker,
        turns,
        hold,
        why,
        attacker_ai,
        declare_after,
    } = options;
    let journal = civvis::reasoning::Journal::recording();
    let mut g = base.clone();
    g.rng = civvis::rng::Rng::new(seed);
    let start = g.turn;
    g.max_turns = g.max_turns.max(start + turns + 1);
    let key = (0, attacker);
    let recorded = hold && hold_war(&mut g, attacker, start + turns + 1);
    let mut ais: Vec<Box<dyn Ai>> = Vec::new();
    for pid in 0..g.players.len() {
        if pid == 0 {
            let mut ai = seat.agent();
            if why {
                ai.attach_journal(journal.handle());
            }
            ais.push(Box::new(ai));
        } else if pid == attacker {
            let mut ai = match attacker_ai {
                "legacy" => AdvancedAi::legacy(),
                "new" => AdvancedAi::new(),
                _ => {
                    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
                    ai.apply_gene_ledger();
                    ai
                }
            };
            ai.retarget_against(VictoryTarget::Domination, 0);
            if why {
                ai.attach_journal(journal.handle());
            }
            ais.push(Box::new(ai));
        } else {
            ais.push(Box::new(AdvancedAi::new()));
        }
    }
    let held = our_cities(&g);
    let capital = g
        .cities
        .values()
        .find(|city| city.owner == 0 && city.is_capital)
        .map(|city| city.pos);
    let mut out = Outcome {
        lost: 0,
        to_attacker: 0,
        fell: 0,
        first_loss: None,
        capital_held: true,
        alive: true,
        cities_end: 0,
        our_military_start: military_units(&g, 0),
        our_military_end: 0,
        attacker_military_start: military_units(&g, attacker),
        attacker_military_end: 0,
        at_war_end: false,
        declared: false,
        peace_undone: 0,
        turns: 0,
    };
    let mut asked = false;
    let mut fallen: BTreeSet<(i32, i32)> = BTreeSet::new();
    while g.winner.is_none() && g.turn < start + turns && g.players[0].alive {
        let pid = g.current;
        if pid == attacker && !asked && g.turn >= start + declare_after {
            asked = true;
            if !g.is_at_war(attacker, 0) {
                out.declared = g.apply(attacker, &Action::DeclareWar { player: 0 }).is_ok();
            }
        }
        ais[pid].take_turn(&mut g, pid);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        if hold && !recorded && (out.declared || base.is_at_war(attacker, 0)) {
            if !g.is_at_war(attacker, 0) {
                out.peace_undone += 1;
            }
            g.at_war.insert(key);
            g.peace_treaties.remove(&key);
        }
        let now = our_cities(&g);
        for pos in held.difference(&now) {
            if fallen.insert(*pos) && out.first_loss.is_none() {
                out.first_loss = Some(g.turn - start);
            }
        }
    }
    // `--why`: the attacker's and our own journal lines for the first seed,
    // to read why a city held or fell.
    if why {
        for thought in journal.since(0).thoughts {
            if thought.player == attacker || thought.player == 0 {
                let side = if thought.player == 0 { "us " } else { "att" };
                let detail: String = thought.detail.chars().take(160).collect();
                eprintln!(
                    "  t{} {side} {} | {}",
                    thought.turn, thought.headline, detail
                );
            }
        }
    }
    let now = our_cities(&g);
    out.fell = fallen.len();
    for pos in held.difference(&now) {
        out.lost += 1;
        if g.cities
            .values()
            .any(|city| city.pos == *pos && city.owner == attacker)
        {
            out.to_attacker += 1;
        }
    }
    out.capital_held = capital.is_none_or(|pos| now.contains(&pos));
    out.alive = g.players[0].alive;
    out.cities_end = now.len();
    out.our_military_end = military_units(&g, 0);
    out.attacker_military_end = military_units(&g, attacker);
    out.at_war_end = g.is_at_war(attacker, 0);
    out.turns = g.turn - start;
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(run) = text(&args, "--run") else {
        eprintln!(
            "usage: defence_replay --run <run dir> --turn T --attacker <civ> [--turns N] \
             [--seeds K] [--genome F | --with-file F] [--with a,b] [--without a,b] [--lane L]"
        );
        std::process::exit(2);
    };
    let turn = number(&args, "--turn", 0) as u32;
    let turns = number(&args, "--turns", 30) as u32;
    let seeds = number(&args, "--seeds", 8);
    let events = std::path::Path::new(&home(&run)).join("events.jsonl");
    let snapshot =
        civvis::mirror::snapshot_from_events_at(&events, Some(turn)).unwrap_or_else(|error| {
            eprintln!("cannot read {}: {error}", events.display());
            std::process::exit(2);
        });
    let Some(state) = civvis::mirror::state_from_events(&events, Some(turn)) else {
        eprintln!("no state at turn {turn} in {}", events.display());
        std::process::exit(2);
    };
    let players = if state.seat.players > 0 {
        state.seat.players
    } else {
        4
    };
    let max_turns = if state.seat.max_turns > 0 {
        state.seat.max_turns as u32
    } else {
        250
    };
    let rebuilt = civvis::mirror::rebuild_from_state(&snapshot, &state, players, 1, max_turns, 6);
    let base = rebuilt.game;
    let wanted = text(&args, "--attacker").unwrap_or_default().to_lowercase();
    // An unseen major can retain alive=true while the reconstruction carries
    // none of its cities or units. Such a placeholder cannot supply a replay.
    let represented = |pid: usize| {
        base.players[pid].alive
            && (base.cities.values().any(|city| city.owner == pid)
                || base.units.values().any(|unit| unit.owner == pid))
    };
    let Some(attacker) = (1..base.players.len()).find(|&pid| {
        !wanted.is_empty()
            && represented(pid)
            && base.players[pid].civ.to_lowercase().contains(&wanted)
    }) else {
        let civs: Vec<String> = base
            .players
            .iter()
            .filter(|player| represented(player.id))
            .map(|player| player.civ.clone())
            .collect();
        eprintln!(
            "no active rival matching --attacker {wanted:?}; active players: {}",
            civs.join(", ")
        );
        std::process::exit(2);
    };
    let lane: VictoryTarget = text(&args, "--lane")
        .map(|lane| {
            lane.parse().unwrap_or_else(|_| {
                eprintln!("unknown --lane {lane:?}");
                std::process::exit(2);
            })
        })
        .unwrap_or(VictoryTarget::Science);
    let genome = text(&args, "--genome").map(|path| {
        let path = home(&path);
        let body = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            eprintln!("cannot read --genome {path}: {error}");
            std::process::exit(2);
        });
        genome_tags(&body)
    });
    if let Some(on) = &genome {
        let unknown: Vec<&String> = on
            .iter()
            .filter(|tag| civvis::ai::gene(tag).is_none())
            .collect();
        if !unknown.is_empty() {
            eprintln!(
                "--genome: {} tag(s) this build does not carry (ignored)",
                unknown.len()
            );
        }
    }
    let mut forced = text(&args, "--with-file").map_or_else(Vec::new, |path| {
        let path = home(&path);
        let body = std::fs::read_to_string(&path).unwrap_or_else(|error| {
            eprintln!("cannot read --with-file {path}: {error}");
            std::process::exit(2);
        });
        tags(Some(body))
    });
    let with = tags(text(&args, "--with"));
    for tag in with.iter().chain(tags(text(&args, "--without")).iter()) {
        if civvis::ai::gene(tag).is_none() {
            eprintln!("unknown gene {tag:?}");
            std::process::exit(2);
        }
    }
    forced.extend(with.iter().cloned());
    let seat = Seat {
        lane,
        genome,
        forced,
        with,
        without: tags(text(&args, "--without")),
    };
    // `--allow-peace` lets the replayed attacker settle; by default the war
    // runs the whole replay, as the live attacker's did.
    let hold = !args.iter().any(|arg| arg == "--allow-peace");
    let why = args.iter().any(|arg| arg == "--why");
    // `--attacker-ai ledger|legacy|new`: which agent plays the attacker, all
    // retargeted at us. `ledger` (default) is the deployment genome on the
    // Domination lane; `legacy` is the frozen anchor; `new` is the engine's
    // default agent every other chair plays.
    // `--declare-after N`: start at peace and let the attacker declare N
    // turns in, so a defence that acts before the war (a garrison called
    // home, Walls) is played from the turn it would have started.
    let declare_after = number(&args, "--declare-after", 0) as u32;
    let attacker_ai = text(&args, "--attacker-ai").unwrap_or_else(|| "ledger".into());
    if !["ledger", "legacy", "new"].contains(&attacker_ai.as_str()) {
        eprintln!("unknown --attacker-ai {attacker_ai:?}");
        std::process::exit(2);
    }
    let ours = our_cities(&base).len();
    eprintln!(
        "{run} at turn {} ({}): attacker {} (player {attacker}) at war {}, our cities {ours}, \
         our military {}, theirs {}, war record {}",
        base.turn,
        base.game_speed.id(),
        base.players[attacker].civ,
        base.is_at_war(attacker, 0),
        military_units(&base, 0),
        military_units(&base, attacker),
        base.wars.contains_key(&(0, attacker)),
    );
    let (mut lost, mut fell, mut any, mut capitals, mut dead, mut crashed) = (0, 0, 0, 0, 0, 0);
    for seed in 0..seeds {
        // A seed the engine panics on is reported and skipped, not fatal: a
        // rebuilt board can carry a value the live host rounded differently.
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            replay(
                &base,
                9_100 + seed,
                &seat,
                ReplayOptions {
                    attacker,
                    turns,
                    hold,
                    why: why && seed == 0,
                    attacker_ai: &attacker_ai,
                    declare_after,
                },
            )
        }));
        let Ok(out) = run else {
            crashed += 1;
            println!("{{\"seed\":{seed},\"crashed\":true}}");
            continue;
        };
        lost += out.lost;
        fell += out.fell;
        any += usize::from(out.lost > 0);
        capitals += usize::from(!out.capital_held);
        dead += usize::from(!out.alive);
        println!(
            "{{\"seed\":{seed},\"lost\":{},\"fell\":{},\"to_attacker\":{},\"first_loss\":{},\"capital_held\":{},\
             \"alive\":{},\"cities_end\":{},\"our_military\":[{},{}],\"attacker_military\":[{},{}],\
             \"at_war_end\":{},\"declared\":{},\"peace_undone\":{},\"turns\":{}}}",
            out.lost,
            out.fell,
            out.to_attacker,
            out.first_loss.map_or("null".into(), |t| t.to_string()),
            out.capital_held,
            out.alive,
            out.cities_end,
            out.our_military_start,
            out.our_military_end,
            out.attacker_military_start,
            out.attacker_military_end,
            out.at_war_end,
            out.declared,
            out.peace_undone,
            out.turns
        );
    }
    let played = seeds - crashed;
    eprintln!(
        "{played} of {seeds} seeds: cities lost {:.2}/seed of {ours} (fell {:.2}, retaken {:.2}), any lost {any}, \
         capital lost {capitals}, dead {dead}, crashed {crashed}",
        lost as f64 / played.max(1) as f64,
        fell as f64 / played.max(1) as f64,
        (fell - lost) as f64 / played.max(1) as f64
    );
}
