//! Sim-vs-live divergence replay: the engine's next turn against the game's.
//!
//! For every turn `t` of a recorded live seat (`<run>/events.jsonl`) that has a
//! `state` frame AND a following frame at `t+1`, this rebuilds the CIVVIS mirror
//! at `t` (`LiveMirror::new`, the same board `civvis_orders` answers from), lets
//! the engine play ONE passive turn — every seat ends its turn, nobody issues an
//! order — and pairs what CIVVIS now says against what Civilization VI exported
//! at `t+1`. The pairs are printed as JSON; `tools/live_divergence.py` turns them
//! into the per-run report and the scoreboard.
//!
//! ★★★ PROJECTION-ONLY, BY MEASUREMENT OF THE RECORD, NOT BY CHOICE. The live
//! `orders` and `turn` events carry COUNTS (`applied`, `by: {unit: 2}`) and the
//! `build` events a city and an item; no event on this machine records a unit
//! order with its target. There is nothing to replay, so the projection is the
//! engine's own one-turn evolution of the mirrored frame. Everything CIVVIS
//! derives from a frame that is not driven by an order — yields, treasury
//! deltas, loyalty, majority religion — is measured exactly by this; anything an
//! order changes (a unit's hit points after it attacked) is measured only where a
//! ledger names both sides.
//!
//! ⚠ A pair is emitted only where BOTH sides reported a number. A city the mirror
//! dropped, a yield an older mod did not export, a frame with no successor — each
//! is a gap in coverage, never a zero in the error.
//!
//! Usage:
//!     live_divergence <events.jsonl> [--turns a-b] [--players N] [--max-turns N]
//!                     [--frontier N]
//!
//! Output (stdout, one JSON document):
//!     {"run", "events", "mode": "projection", "frames", "frame_turns",
//!      "comparable_turns", "compared_turns",
//!      "subsystems": {<name>: {"unit", "pairs": [{"turn", "key", "live", "sim"}],
//!                              "note"}}}

use civvis::game::{expected_damage, Action, Game, PlanningRole};
use civvis::mirror::{self, LiveMirror, Snapshot, StateSnapshot, TilesChunk};
use civvis::rules::Yields;
use std::collections::BTreeMap;

/// One measured (live, sim) reading. `key` names the city, unit or empire
/// quantity so the report can print the worst turns with their subject.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct Pair {
    pub turn: u32,
    pub key: String,
    pub live: f64,
    pub sim: f64,
}

#[derive(Default, serde::Serialize)]
struct Subsystem {
    unit: &'static str,
    pairs: Vec<Pair>,
    note: &'static str,
}

#[derive(serde::Serialize)]
struct Report {
    run: String,
    events: String,
    mode: &'static str,
    frames: usize,
    frame_turns: Vec<u32>,
    comparable_turns: usize,
    compared_turns: usize,
    skipped: Vec<String>,
    subsystems: BTreeMap<&'static str, Subsystem>,
}

/// What the engine says about the seat after its passive turn, keyed the way
/// the live export keys the same things, so the pairing is pure arithmetic.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct SimFrame {
    /// Civilization VI city id -> (name, yields per turn).
    pub city_yields: BTreeMap<i64, (String, Yields)>,
    /// Civilization VI city id -> loyalty after the turn.
    pub city_loyalty: BTreeMap<i64, f64>,
    /// Civilization VI city id -> majority religion (Civ 6 spelling normalised
    /// through `normalize_religion`), `None` when no religion holds a majority.
    pub city_religion: BTreeMap<i64, Option<String>>,
    pub gold_delta: f64,
    pub faith_delta: f64,
    pub favor_delta: Option<f64>,
}

/// `RELIGION_CATHOLICISM`, `Catholicism` and `catholicism` are one religion.
pub fn normalize_religion(name: &str) -> String {
    let bare = name.trim();
    let bare = bare.strip_prefix("RELIGION_").unwrap_or(bare);
    bare.to_ascii_lowercase().replace([' ', '-'], "_")
}

/// Per-city yield pairs: every yield the live frame reported for a city the
/// engine still holds. Cities the mirror dropped or that changed hands produce
/// no pair — a missing city is a coverage gap, not a yield of zero.
pub fn city_yield_pairs(next: &StateSnapshot, sim: &SimFrame) -> BTreeMap<&'static str, Vec<Pair>> {
    let mut out: BTreeMap<&'static str, Vec<Pair>> = BTreeMap::new();
    for city in &next.cities {
        let Some(live) = city.yields.as_ref() else {
            continue;
        };
        let Some((name, model)) = sim.city_yields.get(&city.id) else {
            continue;
        };
        let key = if city.name.is_empty() {
            name.clone()
        } else {
            city.name.clone()
        };
        let readings: [(&'static str, f64, f64); 6] = [
            ("city_science", live.science, model.science),
            ("city_culture", live.culture, model.culture),
            ("city_gold", live.gold, model.gold),
            ("city_faith", live.faith, model.faith),
            ("city_food", live.food, model.food),
            ("city_production", live.production, model.production),
        ];
        for (name, live, sim) in readings {
            out.entry(name).or_default().push(Pair {
                turn: next.turn,
                key: key.clone(),
                live,
                sim,
            });
        }
    }
    out
}

/// Empire treasury deltas: what the live seat gained between the two frames
/// against what the engine banked over its passive turn.
pub fn empire_delta_pairs(
    prev: &StateSnapshot,
    next: &StateSnapshot,
    sim: &SimFrame,
) -> Vec<(&'static str, Pair)> {
    let mut out = vec![
        (
            "empire_gold_delta",
            Pair {
                turn: next.turn,
                key: "gold".into(),
                live: (next.gold - prev.gold) as f64,
                sim: sim.gold_delta,
            },
        ),
        (
            "empire_faith_delta",
            Pair {
                turn: next.turn,
                key: "faith".into(),
                live: (next.faith - prev.faith) as f64,
                sim: sim.faith_delta,
            },
        ),
    ];
    if let (Some(a), Some(b), Some(sim_delta)) = (prev.favor, next.favor, sim.favor_delta) {
        out.push((
            "empire_favor_delta",
            Pair {
                turn: next.turn,
                key: "favor".into(),
                live: b - a,
                sim: sim_delta,
            },
        ));
    }
    out
}

/// Loyalty pairs for every city the live frame reports loyalty on and the engine
/// still holds.
pub fn loyalty_pairs(next: &StateSnapshot, sim: &SimFrame) -> Vec<Pair> {
    next.cities
        .iter()
        .filter_map(|city| {
            let model = *sim.city_loyalty.get(&city.id)?;
            Some(Pair {
                turn: next.turn,
                key: city.name.clone(),
                live: city.loyalty,
                sim: model,
            })
        })
        .collect()
}

/// Majority-religion agreement, 1.0 when both sides name the same majority (or
/// both name none), 0.0 otherwise — recorded as a pair whose live side is 1 and
/// whose sim side is the agreement, so its mean absolute error is the
/// disagreement rate. Only cities where at least one side reports a majority
/// count; a city neither side has converted says nothing about the pressure
/// model.
pub fn religion_pairs(next: &StateSnapshot, sim: &SimFrame) -> Vec<Pair> {
    next.cities
        .iter()
        .filter_map(|city| {
            let model = sim.city_religion.get(&city.id)?;
            let live = city.religion.as_deref().map(normalize_religion);
            if live.is_none() && model.is_none() {
                return None;
            }
            Some(Pair {
                turn: next.turn,
                key: format!(
                    "{} live={} sim={}",
                    city.name,
                    live.as_deref().unwrap_or("-"),
                    model.as_deref().unwrap_or("-")
                ),
                live: 1.0,
                sim: if live == *model { 1.0 } else { 0.0 },
            })
        })
        .collect()
}

/// A combat the ledger recorded on this turn, reduced to what the engine can
/// price: both units' Civilization VI ids and the damage the defender took.
#[derive(Clone, Debug, PartialEq)]
pub struct LedgerCombat {
    pub turn: u32,
    pub attacker: i64,
    pub defender: i64,
    pub damage_to_defender: Option<f64>,
    pub attacker_kind: String,
    pub defender_kind: String,
}

/// Parse a `combat` event. Only unit-versus-unit fights with a known defender
/// damage are priced; district fights (walls, garrisons) are not modelled here.
pub fn parse_combat(value: &serde_json::Value) -> Option<LedgerCombat> {
    if value.get("kind").and_then(|k| k.as_str()) != Some("combat") {
        return None;
    }
    let side = |name: &str| -> Option<(i64, String)> {
        let side = value.get(name)?;
        if side.get("type").and_then(|t| t.as_str()) != Some("unit") {
            return None;
        }
        Some((
            side.get("id")?.as_i64()?,
            side.get("kind")
                .and_then(|k| k.as_str())
                .unwrap_or("?")
                .to_string(),
        ))
    };
    let (attacker, attacker_kind) = side("attacker")?;
    let (defender, defender_kind) = side("defender")?;
    Some(LedgerCombat {
        turn: value.get("turn")?.as_u64()? as u32,
        attacker,
        defender,
        damage_to_defender: value.get("damage_to_defender").and_then(|d| d.as_f64()),
        attacker_kind,
        defender_kind,
    })
}

/// Pair the ledger's defender damage against the engine's expected damage for
/// the same two units on the mirrored board at the start of the turn.
pub fn combat_pairs(combats: &[LedgerCombat], mirror: &LiveMirror) -> Vec<Pair> {
    let game = &mirror.game;
    combats
        .iter()
        .filter_map(|combat| {
            let live = combat.damage_to_defender?;
            let attacker = game.units.get(mirror.uid_of.get(&combat.attacker)?)?;
            let defender = game.units.get(mirror.uid_of.get(&combat.defender)?)?;
            let sim = expected_damage(
                game.unit_strength(attacker, false),
                game.unit_strength(defender, true),
            );
            Some(Pair {
                turn: combat.turn,
                key: format!("{} -> {}", combat.attacker_kind, combat.defender_kind),
                live,
                sim,
            })
        })
        .collect()
}

/// Read the engine's view of the seat after the passive turn, keyed by the
/// Civilization VI ids the mirror remembered when it placed each city.
pub fn sim_frame(mirror: &LiveMirror, before: (f64, f64, Option<f64>)) -> SimFrame {
    let game = &mirror.game;
    let mut frame = SimFrame::default();
    for (civ6, cid) in &mirror.cid_of {
        let Some(city) = game.cities.get(cid) else {
            continue;
        };
        if city.owner != 0 {
            continue;
        }
        frame
            .city_yields
            .insert(*civ6, (city.name.clone(), game.city_yields_model(*cid)));
        frame.city_loyalty.insert(*civ6, city.loyalty);
        frame
            .city_religion
            .insert(*civ6, game.city_religion(city).map(normalize_religion));
    }
    let seat = &game.players[0];
    frame.gold_delta = seat.gold - before.0;
    frame.faith_delta = seat.faith - before.1;
    frame.favor_delta = before.2.map(|favor| seat.diplomatic_favor - favor);
    frame
}

/// Let every seat end its turn once, so the engine rolls from `turn` to
/// `turn + 1` and the local seat's begin-of-turn processing has run. Returns
/// false when the game refused to roll (a finished game, a dead seat).
pub fn advance_one_turn(game: &mut Game) -> bool {
    let target = game.turn + 1;
    game.set_planning_role(PlanningRole::Off);
    game.set_fog_memory(false);
    game.set_war_ledger(false);
    let seats = game.players.len().max(1) * 2;
    for _ in 0..seats {
        if game.turn >= target && game.current == 0 {
            return true;
        }
        if game.winner.is_some() {
            return false;
        }
        let pid = game.current;
        if game.apply(pid, &Action::EndTurn).is_err() {
            return false;
        }
    }
    game.turn >= target && game.current == 0
}

fn arg_text(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn parse_turns(text: &str) -> Option<(u32, u32)> {
    let (a, b) = text.split_once('-')?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args
        .iter()
        .find(|a| !a.starts_with("--") && a.ends_with(".jsonl"))
    else {
        eprintln!("usage: live_divergence <events.jsonl> [--turns a-b] [--players N] [--max-turns N] [--frontier N]");
        std::process::exit(2);
    };
    let path = std::path::PathBuf::from(path);
    let range = arg_text(&args, "--turns").and_then(|t| parse_turns(&t));
    let players_fallback: usize = arg_text(&args, "--players")
        .and_then(|v| v.parse().ok())
        .unwrap_or(6);
    let turns_fallback: u32 = arg_text(&args, "--max-turns")
        .and_then(|v| v.parse().ok())
        .unwrap_or(250);
    let frontier: u32 = arg_text(&args, "--frontier")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(why) => {
            eprintln!("live_divergence: cannot read {}: {why}", path.display());
            std::process::exit(2);
        }
    };
    let run = path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let mut report = Report {
        run,
        events: path.display().to_string(),
        mode: "projection",
        frames: 0,
        frame_turns: Vec::new(),
        comparable_turns: 0,
        compared_turns: 0,
        skipped: Vec::new(),
        subsystems: BTreeMap::new(),
    };
    let describe: [(&'static str, &'static str, &'static str); 12] = [
        ("city_science", "yield/turn", "per-city science at t+1, live export vs engine model"),
        ("city_culture", "yield/turn", "per-city culture at t+1"),
        ("city_gold", "yield/turn", "per-city gold at t+1"),
        ("city_faith", "yield/turn", "per-city faith at t+1"),
        ("city_food", "yield/turn", "per-city food at t+1"),
        ("city_production", "yield/turn", "per-city production at t+1"),
        ("empire_gold_delta", "gold", "treasury change t -> t+1, live vs engine's passive turn; the live delta includes purchases and deal income no passive projection can see, so the MEDIAN is the income reading and the worst turns are the seat's spending"),
        ("empire_faith_delta", "faith", "faith change t -> t+1; the live delta includes faith purchases, read the median"),
        ("empire_favor_delta", "favor", "diplomatic favor change t -> t+1; needs `favor` in both frames"),
        ("city_loyalty", "loyalty", "per-city loyalty at t+1"),
        ("city_religion", "agreement", "majority religion at t+1; MAE is the disagreement rate"),
        ("combat_damage", "hp", "ledger `combat` defender damage vs engine expected damage on the t board; needs the tactical ledger"),
    ];
    for (name, unit, note) in describe {
        report.subsystems.insert(
            name,
            Subsystem {
                unit,
                pairs: Vec::new(),
                note,
            },
        );
    }
    report.subsystems.insert(
        "tourism",
        Subsystem {
            unit: "tourism/turn",
            pairs: Vec::new(),
            note: "the seat's own tourism per turn is not in the state export (only rivals' is); no pair can be formed",
        },
    );
    report.subsystems.insert(
        "deal_outcome",
        Subsystem {
            unit: "agreement",
            pairs: Vec::new(),
            note: "`deal_closed`/`peace_response` events are counted below; the engine's valuation of a live deal is not yet paired",
        },
    );

    let mut seat: Option<mirror::Seat> = None;
    let mut snapshot = Snapshot::default();
    let mut pending: Option<(StateSnapshot, Snapshot)> = None;
    let mut pending_combats: Vec<LedgerCombat> = Vec::new();
    let mut deal_events = 0usize;
    let in_range = |turn: u32| range.is_none_or(|(a, b)| turn >= a && turn <= b);

    for line in raw.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match value.get("kind").and_then(|k| k.as_str()) {
            Some("seat") => {
                if let Ok(found) = serde_json::from_value::<mirror::Seat>(value.clone()) {
                    if !found.civ.is_empty() {
                        seat = Some(found);
                    }
                }
            }
            Some("tiles") => {
                if let Ok(chunk) = serde_json::from_value::<TilesChunk>(value.clone()) {
                    if !chunk.plots.is_empty() {
                        if value.get("delta").and_then(|d| d.as_bool()) == Some(true) {
                            snapshot.merge_delta(&chunk);
                        } else {
                            snapshot.merge_sweep(&chunk);
                        }
                    }
                }
            }
            Some("improved") => {
                if let (Some(x), Some(y), Some(im)) = (
                    value.get("x").and_then(|v| v.as_i64()),
                    value.get("y").and_then(|v| v.as_i64()),
                    value.get("im").and_then(|v| v.as_str()),
                ) {
                    snapshot.set_improvement((x as i32, y as i32), im);
                }
            }
            Some("combat") => {
                if let Some(combat) = parse_combat(&value) {
                    pending_combats.push(combat);
                }
            }
            Some("deal_closed") | Some("peace_response") => deal_events += 1,
            Some("state") => {
                let Ok(mut state) = mirror::state_from_json(line) else {
                    continue;
                };
                if let Some(seat) = seat.as_ref() {
                    state.seat = seat.clone();
                }
                report.frames += 1;
                report.frame_turns.push(state.turn);
                if let Some((prev, prev_snapshot)) = pending.take() {
                    if state.turn == prev.turn + 1 && in_range(prev.turn) {
                        report.comparable_turns += 1;
                        let combats: Vec<LedgerCombat> = pending_combats
                            .drain(..)
                            .filter(|c| c.turn == prev.turn)
                            .collect();
                        match compare_turn(
                            &prev_snapshot,
                            &prev,
                            &state,
                            &combats,
                            players_fallback,
                            turns_fallback,
                            frontier,
                        ) {
                            Ok(pairs) => {
                                report.compared_turns += 1;
                                for (name, pair) in pairs {
                                    if let Some(sub) = report.subsystems.get_mut(name) {
                                        sub.pairs.push(pair);
                                    }
                                }
                            }
                            Err(why) => report.skipped.push(format!("t{}: {why}", prev.turn)),
                        }
                    }
                }
                pending_combats.clear();
                if in_range(state.turn) && snapshot.revealed_count() > 0 {
                    pending = Some((state, snapshot.clone()));
                }
            }
            _ => {}
        }
    }
    if deal_events > 0 {
        report.skipped.push(format!(
            "{deal_events} deal/peace events recorded, not priced"
        ));
    }
    println!(
        "{}",
        serde_json::to_string(&report).expect("report serialises")
    );
}

/// Mirror the frame at `t`, roll one passive turn, and pair the result against
/// the frame at `t+1`.
fn compare_turn(
    snapshot: &Snapshot,
    prev: &StateSnapshot,
    next: &StateSnapshot,
    combats: &[LedgerCombat],
    players_fallback: usize,
    turns_fallback: u32,
    frontier: u32,
) -> Result<Vec<(&'static str, Pair)>, String> {
    let players = if prev.seat.players > 0 {
        prev.seat.players
    } else {
        players_fallback
    };
    let max_turns = if prev.seat.max_turns > 0 {
        prev.seat.max_turns as u32
    } else {
        turns_fallback
    };
    let mut mirror = LiveMirror::new(
        snapshot,
        prev,
        players,
        1,
        max_turns.max(next.turn + 1),
        frontier,
    );
    let mut out: Vec<(&'static str, Pair)> = Vec::new();
    for pair in combat_pairs(combats, &mirror) {
        out.push(("combat_damage", pair));
    }
    let seat = &mirror.game.players[0];
    let before = (
        seat.gold,
        seat.faith,
        prev.favor.map(|_| seat.diplomatic_favor),
    );
    if !advance_one_turn(&mut mirror.game) {
        return Err("engine did not roll the turn".into());
    }
    let sim = sim_frame(&mirror, before);
    for (name, pairs) in city_yield_pairs(next, &sim) {
        out.extend(pairs.into_iter().map(|p| (name, p)));
    }
    out.extend(empire_delta_pairs(prev, next, &sim));
    out.extend(
        loyalty_pairs(next, &sim)
            .into_iter()
            .map(|p| ("city_loyalty", p)),
    );
    out.extend(
        religion_pairs(next, &sim)
            .into_iter()
            .map(|p| ("city_religion", p)),
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(
        turn: u32,
        gold: i64,
        faith: i64,
        favor: Option<f64>,
        cities: serde_json::Value,
    ) -> StateSnapshot {
        let mut value = serde_json::json!({
            "kind": "state", "turn": turn, "gold": gold, "faith": faith,
            "science": 1.0, "culture": 1.0, "cities": cities, "units": [], "rivals": [],
        });
        if let Some(favor) = favor {
            value["favor"] = serde_json::json!(favor);
        }
        mirror::state_from_json(&value.to_string()).expect("frame parses")
    }

    fn city(
        id: i64,
        name: &str,
        science: f64,
        loyalty: f64,
        religion: Option<&str>,
    ) -> serde_json::Value {
        let mut value = serde_json::json!({
            "id": id, "name": name, "x": 0, "y": 0, "pop": 3, "loyalty": loyalty,
            "yields": {"food": 4.0, "production": 5.0, "gold": 6.0, "science": science, "culture": 2.0, "faith": 1.0},
        });
        if let Some(religion) = religion {
            value["religion"] = serde_json::json!(religion);
        }
        value
    }

    fn sim(science: f64, loyalty: f64, religion: Option<&str>) -> SimFrame {
        let mut frame = SimFrame::default();
        frame.city_yields.insert(
            7,
            (
                "Rome".into(),
                Yields {
                    food: 4.0,
                    production: 5.5,
                    gold: 6.0,
                    science,
                    culture: 2.0,
                    faith: 1.0,
                },
            ),
        );
        frame.city_loyalty.insert(7, loyalty);
        frame
            .city_religion
            .insert(7, religion.map(normalize_religion));
        frame.gold_delta = 8.0;
        frame.faith_delta = 3.0;
        frame.favor_delta = Some(1.0);
        frame
    }

    #[test]
    fn two_frames_pair_every_reported_yield_and_skip_unknown_cities() {
        let next = frame(
            11,
            120,
            30,
            Some(5.0),
            serde_json::json!([
                city(7, "Rome", 6.5, 96.0, Some("RELIGION_CATHOLICISM")),
                city(9, "Ostia", 1.0, 100.0, None)
            ]),
        );
        let pairs = city_yield_pairs(&next, &sim(5.0, 100.0, Some("Catholicism")));
        assert_eq!(pairs.len(), 6, "one series per yield");
        let science = &pairs["city_science"];
        assert_eq!(
            science,
            &vec![Pair {
                turn: 11,
                key: "Rome".into(),
                live: 6.5,
                sim: 5.0
            }]
        );
        assert_eq!(pairs["city_production"][0].sim, 5.5);
        assert!(
            pairs.values().all(|series| series.len() == 1),
            "Ostia is not on the sim board: no pair, no zero"
        );
    }

    #[test]
    fn empire_deltas_are_frame_differences_and_favor_needs_both_frames() {
        let prev = frame(10, 100, 20, None, serde_json::json!([]));
        let next = frame(11, 112, 24, Some(5.0), serde_json::json!([]));
        let pairs = empire_delta_pairs(&prev, &next, &sim(0.0, 0.0, None));
        assert_eq!(pairs.len(), 2, "favor absent at t: no favor pair");
        assert_eq!(pairs[0].0, "empire_gold_delta");
        assert_eq!((pairs[0].1.live, pairs[0].1.sim), (12.0, 8.0));
        assert_eq!((pairs[1].1.live, pairs[1].1.sim), (4.0, 3.0));
        let prev = frame(10, 100, 20, Some(4.0), serde_json::json!([]));
        let pairs = empire_delta_pairs(&prev, &next, &sim(0.0, 0.0, None));
        assert_eq!(pairs[2].0, "empire_favor_delta");
        assert_eq!((pairs[2].1.live, pairs[2].1.sim), (1.0, 1.0));
    }

    #[test]
    fn loyalty_and_religion_pairs() {
        let next = frame(
            11,
            0,
            0,
            None,
            serde_json::json!([city(7, "Rome", 6.5, 96.0, Some("RELIGION_CATHOLICISM"))]),
        );
        let loyalty = loyalty_pairs(&next, &sim(0.0, 100.0, Some("Catholicism")));
        assert_eq!((loyalty[0].live, loyalty[0].sim), (96.0, 100.0));
        let agree = religion_pairs(&next, &sim(0.0, 0.0, Some("Catholicism")));
        assert_eq!(
            (agree[0].live, agree[0].sim),
            (1.0, 1.0),
            "same religion under two spellings agrees"
        );
        let disagree = religion_pairs(&next, &sim(0.0, 0.0, None));
        assert_eq!((disagree[0].live, disagree[0].sim), (1.0, 0.0));
        let neither = frame(
            11,
            0,
            0,
            None,
            serde_json::json!([city(7, "Rome", 6.5, 96.0, None)]),
        );
        assert!(
            religion_pairs(&neither, &sim(0.0, 0.0, None)).is_empty(),
            "no majority on either side is not a reading"
        );
    }

    #[test]
    fn combat_events_parse_units_only() {
        let event = serde_json::json!({
            "kind": "combat", "turn": 40,
            "attacker": {"type": "unit", "id": 65536, "player": 0, "kind": "UNIT_WARRIOR", "hp": 100},
            "defender": {"type": "unit", "id": 131072, "player": 3, "kind": "UNIT_SCOUT", "hp": 100},
            "damage_to_defender": 37, "damage_to_attacker": 22,
        });
        let combat = parse_combat(&event).expect("unit fight parses");
        assert_eq!(
            (combat.turn, combat.attacker, combat.defender),
            (40, 65536, 131072)
        );
        assert_eq!(combat.damage_to_defender, Some(37.0));
        let mut walls = event.clone();
        walls["defender"] =
            serde_json::json!({"type": "district", "id": 1, "player": 3, "hp": 200});
        assert!(
            parse_combat(&walls).is_none(),
            "district fights are not priced"
        );
    }

    #[test]
    fn religion_names_normalise_across_spellings() {
        assert_eq!(
            normalize_religion("RELIGION_EASTERN_ORTHODOXY"),
            "eastern_orthodoxy"
        );
        assert_eq!(normalize_religion("Eastern Orthodoxy"), "eastern_orthodoxy");
    }
}
