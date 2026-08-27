//! Where a Settler's turns go, counted rather than inferred.
//!
//! Operator, 2026-08-27: *"our settlers often just sit around in the capital
//! or other cities. this is a huge mistake. investigate how this was allowed
//! to happen."* `advanced_settler_step` has, by this census's count, more
//! than a dozen branches that leave a Settler exactly where it stands —
//! waiting out a forecast, a safe-step guard that refuses every neighbour,
//! a guard that has not arrived, a barbarian reach it will not enter, a
//! target search that came back empty. Each was added against one live run's
//! anecdote and none was ever charged for the turns it costs. This census
//! plays whole games and, for every Settler a major owns, records each turn
//! as a MOVE, a FOUNDING or an IDLE turn; for the idle turns it records where
//! the unit stood (an own city tile or open ground) and what the controller's
//! own journal said the reason was, so the hold branches can be ranked by the
//! turns they actually cost instead of by the anecdote that motivated them.
//!
//! ⚠ A census, not an assertion. `#[ignore]`d; run with
//!
//! ```text
//! CIVVIS_CENSUS_MAPS=4 CIVVIS_CENSUS_ARMS=deployment,live \
//!   cargo test --profile ci --lib settler_idle_census -- --ignored --nocapture
//! ```
//!
//! Arms: `deployment` is `enable_engine_repairs()` — the genome a native
//! game ships. `live` is `enable_live_bridge()` — the Civilization VI seat's
//! genome, host-only genes included, given a native board. Those genes were
//! written for the live seat and are never screened natively, so the `live`
//! arm says what the seat's settler logic does when it is handed a board, not
//! what the native game ships; the operator watches the live seat.
//! `CIVVIS_CENSUS_OPT_INS=tag,tag` turns further opt-in genes on in every arm.

use super::*;
use crate::ai::Ai;
use crate::game::{Action, Game, GameOptions};
use crate::reasoning::{Journal, Thought};
use std::collections::{BTreeMap, BTreeSet};

/// One Settler from the turn it was first seen to the turn it was gone.
struct Life {
    owner: usize,
    born: u32,
    birth_tile: Pos,
    turns: u32,
    moved: u32,
    idle: u32,
    idle_in_city: u32,
    first_move: Option<u32>,
    streak: u32,
    longest_city_streak: u32,
    city_streak: u32,
    /// Turn, standing-in-city, and the reason label of every idle turn.
    idle_reasons: Vec<(u32, bool, String)>,
    outcome: Outcome,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Walking,
    Founded,
    Lost,
    AliveAtEnd,
}

/// The most specific settler line the journal wrote this turn, as a short
/// label. Lower is more specific; the census keeps the lowest.
fn reason_of(thought: &Thought) -> Option<(u8, String)> {
    let h = thought.headline.as_str();
    let d = thought.detail.as_str();
    let after_dash =
        |text: &str| -> String { text.rsplit(" — ").next().unwrap_or(text).trim().to_string() };
    if h.starts_with("Settler HELD short") {
        let why = after_dash(d);
        // Fold the unit kind out of the two occupant reasons so the table
        // ranks the shape, not the roster.
        let why = if why.starts_with("our own ") && why.ends_with("is standing on the next tile") {
            "our own unit is standing on the next tile".to_string()
        } else if why.starts_with("a foreign ") && why.ends_with("holds the next tile") {
            "a foreign unit holds the next tile".to_string()
        } else {
            why
        };
        return Some((0, format!("HELD short of its target: {why}")));
    }
    if h.starts_with("Settler waits outside a barbarian's reach") {
        return Some((
            0,
            "waits outside a barbarian's reach (civilian-out-of-reach)".into(),
        ));
    }
    if h.starts_with("Settler holds inside a barbarian's reach") {
        return Some((
            0,
            "holds inside a barbarian's reach: no reachable tile is better (flee step)".into(),
        ));
    }
    if h.starts_with("Settler is stranded") {
        return Some((
            0,
            "STRANDED (named): no legal site is reachable and it cannot found here".into(),
        ));
    }
    if h.starts_with("Settler holds at") {
        return Some((
            0,
            "watchdog holds: every step toward the target is in a hostile's reach".into(),
        ));
    }
    if h.starts_with("Settler takes ") || h.starts_with("Settler skips a doomed site") {
        return Some((3, "exhaustion search retargeted, no step".into()));
    }
    if h.starts_with("Settler waits for its guard") {
        return Some((0, "waits for its guard (stacked escort, live seat)".into()));
    }
    if h.starts_with("Settler stops waiting for its guard") {
        return Some((4, "stops waiting for its guard, then no move".into()));
    }
    if h.starts_with("Settler falls back toward its guard") {
        return Some((3, "falls back toward its guard but no move".into()));
    }
    if h.starts_with("Settler sets ") {
        let why = d
            .split_once('(')
            .and_then(|(_, rest)| rest.split_once(')'))
            .map(|(why, _)| why.to_string())
            .unwrap_or_default();
        return Some((1, format!("sets its target aside (hysteresis): {why}")));
    }
    if h.starts_with("Settler refuses ") {
        return Some((1, "refuses a site before walking (loyalty forecast)".into()));
    }
    if h.starts_with("Settler abandons loyalty-doomed") {
        return Some((1, "abandons a loyalty-doomed arrival".into()));
    }
    if h.starts_with("Settler abandons ") {
        return Some((1, "abandons a target it stands on and cannot found".into()));
    }
    if h.starts_with("Settler gives up") {
        return Some((1, "gives up on a target it kept retreating from".into()));
    }
    if h.starts_with("Settler declines") {
        return Some((1, "declines the stalled fallback founding".into()));
    }
    if h.starts_with("Founding refused") {
        return Some((1, "the engine refused the founding".into()));
    }
    if h.starts_with("Settler advancing with its escort") {
        return Some((
            2,
            "escort link reports 'advancing' (the formation did not move)".into(),
        ));
    }
    if h.starts_with("Settler detours") {
        return Some((
            3,
            "detours around a visible threat (retargeted, no step)".into(),
        ));
    }
    if h.starts_with("Settler falls back") {
        return Some((
            3,
            "falls back to a nearby safe site (retargeted, no step)".into(),
        ));
    }
    if h.starts_with("Settler retreats") {
        return Some((3, "retreat line but no move".into()));
    }
    if h.starts_with("Settler sidesteps") {
        return Some((3, "sidestep line but no move".into()));
    }
    if h.starts_with("Settler marching to") {
        return Some((5, "'marching' line, no HELD line, no move".into()));
    }
    if h.starts_with("Walking the first settler") {
        return Some((5, "opening walk line, no move".into()));
    }
    None
}

fn is_settler_thought(thought: &Thought) -> bool {
    let h = thought.headline.as_str();
    h.starts_with("Settler ")
        || h.starts_with("Walking the first settler")
        || h.starts_with("Founding")
        || h.starts_with("The settler's guard")
}

fn env_list(name: &str, default: &[&str]) -> Vec<String> {
    std::env::var(name)
        .ok()
        .map(|text| {
            text.split(',')
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_else(|| default.iter().map(|s| s.to_string()).collect())
}

fn percentile(sorted: &[u32], p: f64) -> u32 {
    if sorted.is_empty() {
        return 0;
    }
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn mean(values: &[u32]) -> f64 {
    values.iter().map(|&v| v as f64).sum::<f64>() / values.len().max(1) as f64
}

fn run_arm(arm: &str, maps: u64, opt_ins: &[String]) {
    let mut lives: BTreeMap<(u64, u32), Life> = BTreeMap::new();
    let mut truncated_turns = 0u64;
    for map in 0..maps {
        let seed = 98_000_000 + map;
        let mut game = Game::new_with(GameOptions {
            speed: "online".to_string(),
            randomize_civs: true,
            ..GameOptions::new(6, 60, 38, seed, 250, 6)
        });
        game.set_fog_memory(false);
        game.set_war_ledger(false);
        let majors: Vec<usize> = (0..game.players.len())
            .filter(|&pid| !game.players[pid].is_minor && !game.players[pid].is_barbarian)
            .collect();
        let journals: Vec<Journal> = (0..game.players.len())
            .map(|_| Journal::recording())
            .collect();
        let mut cursors: Vec<u64> = vec![0; game.players.len()];
        let mut ais: Vec<AdvancedAi> = (0..game.players.len())
            .map(|pid| {
                let mut ai = AdvancedAi::new();
                if majors.contains(&pid) {
                    match arm {
                        "deployment" => ai.enable_engine_repairs(),
                        "live" => ai.enable_live_bridge(),
                        other => panic!("unknown arm {other}: deployment or live"),
                    }
                    for tag in opt_ins {
                        let enable = GENES
                            .iter()
                            .find(|gene| gene.tag == *tag && gene.opt_in())
                            .unwrap_or_else(|| panic!("{tag} is not an opt-in gene"))
                            .enable;
                        enable(&mut ai);
                    }
                    ai.attach_journal(journals[pid].handle());
                }
                ai
            })
            .collect();
        let mut known_cities: BTreeSet<u32> = game.cities.keys().copied().collect();
        while game.winner.is_none() && game.turn <= game.max_turns {
            let pid = game.current;
            let major = majors.contains(&pid);
            // (uid, position, target before the turn)
            let mut before: Vec<(u32, Pos, Option<Pos>)> = Vec::new();
            if major {
                for uid in game.player_unit_ids(pid) {
                    let unit = &game.units[&uid];
                    if unit.kind != "settler" {
                        continue;
                    }
                    before.push((uid, unit.pos, ais[pid].settler_targets.get(&uid).copied()));
                    lives.entry((seed, uid)).or_insert(Life {
                        owner: pid,
                        born: game.turn,
                        birth_tile: unit.pos,
                        turns: 0,
                        moved: 0,
                        idle: 0,
                        idle_in_city: 0,
                        first_move: None,
                        streak: 0,
                        longest_city_streak: 0,
                        city_streak: 0,
                        idle_reasons: Vec::new(),
                        outcome: Outcome::Walking,
                    });
                }
            }
            let turn = game.turn;
            ais[pid].take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
            if !major {
                continue;
            }
            let new_cities: Vec<Pos> = game
                .cities
                .iter()
                .filter(|(cid, city)| city.owner == pid && !known_cities.contains(cid))
                .map(|(_, city)| city.pos)
                .collect();
            known_cities.extend(game.cities.keys().copied());
            let delta = journals[pid].since(cursors[pid]);
            cursors[pid] = delta.cursor;
            truncated_turns = truncated_turns.max(delta.truncated_turns);
            let settler_thoughts: Vec<&Thought> = delta
                .thoughts
                .iter()
                .filter(|t| is_settler_thought(t))
                .collect();
            let several = before.len() > 1;
            for (uid, pos, target_before) in &before {
                let life = lives.get_mut(&(seed, *uid)).expect("inserted above");
                life.turns += 1;
                let in_city = game
                    .city_at(*pos)
                    .is_some_and(|cid| game.cities[&cid].owner == pid);
                match game.units.get(uid) {
                    None => {
                        // Consumed by a founding (a step-and-settle founds up
                        // to a walk away from where it stood), or taken.
                        let founded = new_cities.iter().any(|city| game.wdist(*city, *pos) <= 2);
                        life.outcome = if founded {
                            Outcome::Founded
                        } else {
                            Outcome::Lost
                        };
                        continue;
                    }
                    Some(unit) if unit.owner != pid => {
                        life.outcome = Outcome::Lost;
                        continue;
                    }
                    Some(unit) => {
                        if unit.pos != *pos {
                            life.moved += 1;
                            life.first_move.get_or_insert(turn);
                            life.streak = 0;
                            life.city_streak = 0;
                            continue;
                        }
                    }
                }
                // Idle: still ours, still on the same tile.
                life.idle += 1;
                life.streak += 1;
                if in_city {
                    life.idle_in_city += 1;
                    life.city_streak += 1;
                    life.longest_city_streak = life.longest_city_streak.max(life.city_streak);
                } else {
                    life.city_streak = 0;
                }
                let target_after = ais[pid].settler_targets.get(uid).copied();
                let mine = settler_thoughts.iter().filter(|t| {
                    !several
                        || t.focus == Some(*pos)
                        || (target_before.is_some() && t.focus == *target_before)
                        || (target_after.is_some() && t.focus == target_after)
                });
                let mut best: Option<(u8, String)> = None;
                for thought in mine {
                    if let Some((rank, label)) = reason_of(thought) {
                        if best.as_ref().is_none_or(|(b, _)| rank < *b) {
                            best = Some((rank, label));
                        }
                    }
                }
                let label = match best {
                    Some((_, label)) => label,
                    None => {
                        let unit = &game.units[uid];
                        let full = unit.moves_left >= game.unit_max_moves(*uid) - 1e-9;
                        if ais[pid].guard_wait.contains_key(uid) {
                            "SILENT: waiting for a guard".to_string()
                        } else if target_after.is_none() {
                            // Split "no target" by what the board offered:
                            // the preferred picker's answer, then whether
                            // any legal site exists at all.
                            let preferred = ais[pid].best_settler_target(&game, pid, *uid, 8, None);
                            let base_target = ais[pid].base.settler_targets.get(uid).copied();
                            let any = ais[pid].any_settle_site(&game, pid)
                                || ais[pid].base.has_practical_settle_site(&game, pid);
                            if preferred.is_some() {
                                "NO TARGET although the picker offers a site: the loyalty verdict / hold branch refused it"
                                    .to_string()
                            } else if base_target.is_some() {
                                "NO TARGET in the advanced picker; the baseline picker holds one and did not step"
                                    .to_string()
                            } else if any {
                                "NO TARGET: a legal site exists near a city but none is reachable/ranked for this settler"
                                    .to_string()
                            } else {
                                "NO TARGET: no legal site anywhere in reach (the map is full for this seat)"
                                    .to_string()
                            }
                        } else if full {
                            "SILENT: holds a target and never stepped".to_string()
                        } else {
                            "SILENT: holds a target, spent movement, same tile".to_string()
                        }
                    }
                };
                life.idle_reasons.push((turn, in_city, label));
            }
        }
        for life in lives.values_mut() {
            if life.outcome == Outcome::Walking {
                life.outcome = Outcome::AliveAtEnd;
            }
        }
    }

    let lives: Vec<&Life> = lives.values().collect();
    let n = lives.len();
    let founded = lives
        .iter()
        .filter(|l| l.outcome == Outcome::Founded)
        .count();
    let lost = lives.iter().filter(|l| l.outcome == Outcome::Lost).count();
    let alive = lives
        .iter()
        .filter(|l| l.outcome == Outcome::AliveAtEnd)
        .count();
    let turns: u32 = lives.iter().map(|l| l.turns).sum();
    let moved: u32 = lives.iter().map(|l| l.moved).sum();
    let idle: u32 = lives.iter().map(|l| l.idle).sum();
    let idle_in_city: u32 = lives.iter().map(|l| l.idle_in_city).sum();
    let pct = |a: u32, b: u32| a as f64 / b.max(1) as f64 * 100.0;
    println!(
        "\n=== settler idle census [{arm}{}]: {n} settlers over {maps} maps (6p 60x38 online) ===",
        if opt_ins.is_empty() {
            String::new()
        } else {
            format!(" + {}", opt_ins.join(","))
        }
    );
    println!("  outcomes: founded {founded}   lost {lost}   alive at the end {alive}");
    println!(
        "  settler-turns {turns}: moved {moved} ({:.1}%)   founded {founded}   IDLE {idle} ({:.1}%)",
        pct(moved, turns),
        pct(idle, turns)
    );
    println!(
        "  idle turns standing on an own city tile: {idle_in_city} ({:.1}% of idle, {:.1}% of all settler-turns)",
        pct(idle_in_city, idle),
        pct(idle_in_city, turns)
    );
    for bound in [1u32, 3, 6, 10] {
        let k = lives.iter().filter(|l| l.idle_in_city >= bound).count();
        println!(
            "    settlers with >= {bound:>2} idle turns in a city: {k:>4} ({:.1}%)",
            pct(k as u32, n as u32)
        );
    }
    let mut streaks: Vec<u32> = lives.iter().map(|l| l.longest_city_streak).collect();
    streaks.sort_unstable();
    println!(
        "  longest idle-in-city streak per settler: mean {:.1}   p50 {}   p90 {}   max {}",
        mean(&streaks),
        percentile(&streaks, 0.5),
        percentile(&streaks, 0.9),
        streaks.last().copied().unwrap_or(0)
    );
    let at_birth: usize = lives
        .iter()
        .filter(|l| {
            l.idle_reasons
                .iter()
                .any(|(t, in_city, _)| *in_city && *t == l.born)
        })
        .count();
    let mut to_first: Vec<u32> = lives
        .iter()
        .filter_map(|l| l.first_move.map(|t| t.saturating_sub(l.born)))
        .collect();
    to_first.sort_unstable();
    // A Settler that stepped and founded in one turn is gone before its
    // move is seen; only one still alive at the end never moved.
    let never = lives
        .iter()
        .filter(|l| l.first_move.is_none() && l.outcome == Outcome::AliveAtEnd)
        .count();
    println!(
        "  idle on the tile it was built on, the turn it was built: {at_birth} of {n} ({:.1}%)",
        pct(at_birth as u32, n as u32)
    );
    println!(
        "  turns from build to first move: mean {:.1}   p50 {}   p90 {}   max {}   alive at the end without ever moving: {never}",
        mean(&to_first),
        percentile(&to_first, 0.5),
        percentile(&to_first, 0.9),
        to_first.last().copied().unwrap_or(0)
    );
    let era = |t: u32| {
        if t < 50 {
            0
        } else if t < 150 {
            1
        } else {
            2
        }
    };
    let mut by_era = [(0u32, 0u32); 3];
    for life in &lives {
        for (t, in_city, _) in &life.idle_reasons {
            by_era[era(*t)].0 += 1;
            if *in_city {
                by_era[era(*t)].1 += 1;
            }
        }
    }
    println!(
        "  idle turns by game turn (all / in a city): t<50 {}/{}   t50-149 {}/{}   t150+ {}/{}",
        by_era[0].0, by_era[0].1, by_era[1].0, by_era[1].1, by_era[2].0, by_era[2].1
    );
    let mut reasons: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
    for life in &lives {
        for (_, in_city, label) in &life.idle_reasons {
            let entry = reasons.entry(label.as_str()).or_insert((0, 0));
            entry.0 += 1;
            if *in_city {
                entry.1 += 1;
            }
        }
    }
    let mut rows: Vec<(&str, u32, u32)> = reasons
        .into_iter()
        .map(|(label, (all, city))| (label, all, city))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    println!("  why the settler stood still            idle turns   of which in a city");
    for (label, all, city) in rows {
        println!(
            "    {all:>6} ({:>5.1}%)   {city:>6}   {label}",
            pct(all, idle)
        );
    }
    // The worst lives, so a reader can go and look at one.
    let mut worst: Vec<&Life> = lives.to_vec();
    worst.sort_by(|a, b| {
        b.idle_in_city
            .cmp(&a.idle_in_city)
            .then(b.idle.cmp(&a.idle))
    });
    println!("  the five settlers that idled longest in a city:");
    for life in worst.iter().take(5) {
        let reasons: BTreeMap<&str, u32> = life
            .idle_reasons
            .iter()
            .filter(|(_, in_city, _)| *in_city)
            .fold(BTreeMap::new(), |mut acc, (_, _, label)| {
                *acc.entry(label.as_str()).or_insert(0) += 1;
                acc
            });
        let mut reasons: Vec<(&str, u32)> = reasons.into_iter().collect();
        reasons.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        println!(
            "    seat {} born t{} at {:?}: {} idle-in-city of {} turns, first move {:?}, {:?}; {}",
            life.owner,
            life.born,
            life.birth_tile,
            life.idle_in_city,
            life.turns,
            life.first_move.map(|t| t.saturating_sub(life.born)),
            match life.outcome {
                Outcome::Founded => "founded",
                Outcome::Lost => "lost",
                Outcome::AliveAtEnd => "alive at the end",
                Outcome::Walking => "walking",
            },
            reasons
                .iter()
                .take(3)
                .map(|(label, k)| format!("{k}x {label}"))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    println!("  journal turns truncated by the per-turn budget: {truncated_turns}");
    println!();
}

/// Where a Settler's turns go, at the deployment genome and at the live
/// seat's genome. Run explicitly with `--ignored --nocapture`.
#[test]
#[ignore = "census, not an assertion; run explicitly with --nocapture"]
fn settler_idle_census() {
    let maps = std::env::var("CIVVIS_CENSUS_MAPS")
        .ok()
        .and_then(|text| text.parse::<u64>().ok())
        .unwrap_or(8);
    let arms = env_list("CIVVIS_CENSUS_ARMS", &["deployment", "live"]);
    let opt_ins = env_list("CIVVIS_CENSUS_OPT_INS", &[]);
    for arm in &arms {
        run_arm(arm, maps, &opt_ins);
    }
}
