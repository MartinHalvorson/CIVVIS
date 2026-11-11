//! Siege replay: play a live board forward from a stuck siege.
//!
//! The live Emperor seat converts its wars badly: across the 2026-10-18 games
//! about a third of our declarations took a city within 40 turns, whether the
//! target was walled or not and whether a breaker was at hand. A live game
//! costs twenty minutes and its map is never repeated, so no siege change can
//! be measured on the board where it failed. This rebuilds a recorded turn of a
//! live run (`mirror::rebuild_from_state`, the reconstruction `civvis-orders`
//! decides from), seats our Domination agent on the live genome and the
//! engine's own agent in every other chair, and plays it forward, reporting
//! when the named city's walls fell and when the city did.
//!
//! The rivals are CIVVIS agents, not the host's, and only what the seat could
//! see of them is on the board, so a replay is a fixed, repeatable siege
//! problem rather than a forecast of the live game. Compare arms on the same
//! scenarios and seeds; do not read the absolute numbers as live ones.
//!
//! ```bash
//! cargo run --release --features developer-tools --bin siege_replay -- \
//!     --run ~/civvis-civ6-runs/control/civvis-20261019T003348Z --turn 90 \
//!     --target Rustavi --turns 40 --seeds 8 \
//!     --with-file ~/.civvis-live-force-on [--with tag,tag] [--without tag,tag]
//! ```
//!
//! `--force-target` aims the campaign at the target city's owner
//! (`AdvancedAi::retarget_against`) instead of letting the plan choose: the
//! feasibility probe for a war the seat would not open on its own, such as a
//! nearby city-state. `--lane <science|culture|religious|domination>` seats
//! our agent on that victory lane instead of Domination. Each seed's row also
//! says when we were first at war with the target's owner (`war_turn`), how
//! many of the owner's cities we took (`owner_cities_taken`) and how many of
//! ours we lost (`cities_lost`). `--why-match a,b` adds headline needles to
//! `--why`'s journal print (seed 0).

use civvis::ai::{AdvancedAi, Ai, VictoryTarget};
use civvis::game::{Action, Game};

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
            list.split(',')
                .map(|tag| tag.trim().to_string())
                .filter(|tag| !tag.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// What one replay seed did to the target.
struct Outcome {
    walls_down: Option<u32>,
    captured: Option<u32>,
    min_hp: i32,
    min_walls: i32,
    units_lost: usize,
    turns: u32,
    /// Cities of ours at the end that were not ours at the start, anywhere.
    any_captures: usize,
    /// Turns after the start at which we were first at war with the target
    /// city's owner, `Some(0)` when the war was already running.
    war_turn: Option<u32>,
    /// Cities the target's owner held at the start that are ours at the end.
    owner_cities_taken: usize,
    /// Cities of ours at the start that are not ours at the end.
    cities_lost: usize,
    /// Our empire at the end: summed city science, production and population.
    science: f64,
    production: f64,
    population: i32,
}

fn military_units(g: &Game, pid: usize) -> usize {
    g.units
        .values()
        .filter(|unit| unit.owner == pid && g.rules.units[unit.kind].class == "military")
        .count()
}

#[allow(clippy::too_many_arguments)]
fn replay(
    base: &Game,
    seed: u64,
    forced: &[&str],
    without: &[String],
    target_pos: (i32, i32),
    turns: u32,
    why: Option<&str>,
    force_owner: Option<usize>,
    lane: VictoryTarget,
) -> Outcome {
    let mut g = base.clone();
    g.rng = civvis::rng::Rng::new(seed);
    let start = g.turn;
    g.max_turns = g.max_turns.max(start + turns + 1);
    let journal = civvis::reasoning::Journal::recording();
    let mut ais: Vec<Box<dyn Ai>> = Vec::new();
    for pid in 0..g.players.len() {
        if pid == 0 {
            let mut ai = AdvancedAi::targeting(lane);
            if let Some(owner) = force_owner {
                ai.retarget_against(lane, owner);
            }
            ai.enable_live_bridge_universe();
            ai.apply_gene_ledger_with_forced_live(forced);
            for tag in without {
                if let Some(gene) = civvis::ai::gene(tag) {
                    (gene.disable)(&mut ai);
                }
            }
            if why.is_some() {
                ai.attach_journal(journal.handle());
            }
            ais.push(Box::new(ai));
        } else {
            ais.push(Box::new(AdvancedAi::new()));
        }
    }
    let units_at_start = military_units(&g, 0);
    let mut out = Outcome {
        walls_down: None,
        captured: None,
        min_hp: i32::MAX,
        min_walls: i32::MAX,
        units_lost: 0,
        turns: 0,
        any_captures: 0,
        war_turn: None,
        owner_cities_taken: 0,
        cities_lost: 0,
        science: 0.0,
        production: 0.0,
        population: 0,
    };
    let ours_at_start: std::collections::BTreeSet<(i32, i32)> = g
        .cities
        .values()
        .filter(|city| city.owner == 0)
        .map(|city| city.pos)
        .collect();
    let owner = g
        .cities
        .values()
        .find(|city| city.pos == target_pos)
        .map(|city| city.owner);
    let owners_at_start: std::collections::BTreeSet<(i32, i32)> = g
        .cities
        .values()
        .filter(|city| Some(city.owner) == owner)
        .map(|city| city.pos)
        .collect();
    while g.winner.is_none() && g.turn < start + turns {
        let pid = g.current;
        ais[pid].take_turn(&mut g, pid);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        if out.war_turn.is_none() && owner.is_some_and(|owner| g.is_at_war(0, owner)) {
            out.war_turn = Some(g.turn - start);
        }
        if let Some(city) = g.cities.values().find(|city| city.pos == target_pos) {
            out.min_hp = out.min_hp.min(city.hp);
            out.min_walls = out.min_walls.min(city.wall_hp);
            if city.wall_hp <= 0 && out.walls_down.is_none() {
                out.walls_down = Some(g.turn - start);
            }
            if city.owner == 0 && out.captured.is_none() {
                out.captured = Some(g.turn - start);
            }
        } else if out.captured.is_none() {
            // Razed or otherwise gone: count it as taken when we were the
            // last to hold the ground.
            out.captured = Some(g.turn - start);
        }
    }
    out.turns = g.turn - start;
    out.units_lost = units_at_start.saturating_sub(military_units(&g, 0));
    out.any_captures = g
        .cities
        .values()
        .filter(|city| city.owner == 0 && !ours_at_start.contains(&city.pos))
        .count();
    out.owner_cities_taken = g
        .cities
        .values()
        .filter(|city| city.owner == 0 && owners_at_start.contains(&city.pos))
        .count();
    out.cities_lost = ours_at_start
        .iter()
        .filter(|pos| g.city_at(**pos).is_none_or(|cid| g.cities[&cid].owner != 0))
        .count();
    for cid in g.player_city_ids(0) {
        let yields = g.city_yields(cid);
        out.science += yields.science;
        out.production += yields.production;
        out.population += g.cities[&cid].pop;
    }
    // `--why`: our seat's journal lines that name the target (or any
    // `--why-match` needle), to read why a siege held where it did.
    if let Some(names) = why {
        let needles: Vec<String> = names.split(',').map(str::to_lowercase).collect();
        for thought in journal.since(0).thoughts {
            // The grand strategy too, so a war's posture cost is visible.
            let headline = thought.headline.to_lowercase();
            if thought.player == 0
                && (needles.iter().any(|needle| headline.contains(needle.as_str()))
                    || thought.headline.starts_with("Grand strategy"))
            {
                let detail: String = thought.detail.chars().take(220).collect();
                eprintln!("  t{} {} | {}", thought.turn, thought.headline, detail);
            }
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(run) = text(&args, "--run") else {
        eprintln!("usage: siege_replay --run <run dir> --turn T --target <city> [--turns N] [--seeds K] [--with-file F] [--with a,b] [--without a,b] [--force-target] [--lane L] [--why] [--why-match a,b]");
        std::process::exit(2);
    };
    let turn = number(&args, "--turn", 0) as u32;
    let target = text(&args, "--target").unwrap_or_default().to_lowercase();
    let turns = number(&args, "--turns", 40) as u32;
    let seeds = number(&args, "--seeds", 8);
    let events = std::path::Path::new(&run).join("events.jsonl");
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
    let Some(target_city) = base
        .cities
        .values()
        .find(|city| city.name.to_lowercase() == target)
    else {
        let names: Vec<String> = base.cities.values().map(|city| city.name.clone()).collect();
        eprintln!(
            "no city named {target:?} at turn {}; cities: {}",
            base.turn,
            names.join(", ")
        );
        std::process::exit(2);
    };
    let target_pos = target_city.pos;
    eprintln!(
        "{} at turn {}: {} owned by player {}, walls {}/{}, hp {}; our military {}",
        run,
        base.turn,
        target_city.name,
        target_city.owner,
        target_city.wall_hp,
        base.city_max_wall_hp(target_city),
        target_city.hp,
        military_units(&base, 0)
    );
    let mut forced_tags: Vec<String> = text(&args, "--with-file")
        .map(|path| {
            let path = path.replacen('~', &std::env::var("HOME").unwrap_or_default(), 1);
            tags(std::fs::read_to_string(path).ok())
        })
        .unwrap_or_default();
    forced_tags.extend(tags(text(&args, "--with")));
    let forced: Vec<&str> = forced_tags.iter().map(String::as_str).collect();
    let without = tags(text(&args, "--without"));
    let why = args.iter().any(|arg| arg == "--why");
    // `--why-match`: more headline needles for `--why`, comma separated.
    let mut why_needles = target.clone();
    for needle in tags(text(&args, "--why-match")) {
        why_needles.push(',');
        why_needles.push_str(&needle);
    }
    let force_owner = args
        .iter()
        .any(|arg| arg == "--force-target")
        .then_some(target_city.owner);
    let lane: VictoryTarget = text(&args, "--lane")
        .map(|lane| {
            lane.parse().unwrap_or_else(|_| {
                eprintln!("unknown --lane {lane:?}");
                std::process::exit(2);
            })
        })
        .unwrap_or(VictoryTarget::Domination);
    let mut captured = 0;
    let mut breached = 0;
    for seed in 0..seeds {
        let name = (why && seed == 0).then_some(why_needles.as_str());
        let out = replay(
            &base,
            9_100 + seed,
            &forced,
            &without,
            target_pos,
            turns,
            name,
            force_owner,
            lane,
        );
        captured += usize::from(out.captured.is_some());
        breached += usize::from(out.walls_down.is_some());
        println!(
            "{{\"seed\":{seed},\"walls_down\":{},\"captured\":{},\"min_hp\":{},\"min_walls\":{},\"units_lost\":{},\"turns\":{},\"any_captures\":{},\"war_turn\":{},\"owner_cities_taken\":{},\"cities_lost\":{},\"science\":{:.1},\"production\":{:.1},\"population\":{}}}",
            out.walls_down.map_or("null".into(), |t| t.to_string()),
            out.captured.map_or("null".into(), |t| t.to_string()),
            out.min_hp,
            out.min_walls,
            out.units_lost,
            out.turns,
            out.any_captures,
            out.war_turn.map_or("null".into(), |t| t.to_string()),
            out.owner_cities_taken,
            out.cities_lost,
            out.science,
            out.production,
            out.population
        );
    }
    eprintln!("{seeds} seeds: walls down in {breached}, captured in {captured}");
}
