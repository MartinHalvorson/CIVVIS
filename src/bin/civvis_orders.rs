//! Ask CIVVIS what to do with a real Civilization VI turn, and say it in orders.
//!
//! # The architecture this completes
//!
//! "CIVVIS is the logic engine and the harness actuates its decisions" was the
//! stated design, and it was blocked on a measured fact: the control mod had no
//! inbound channel. That fact is now false — `DB.Query("ATTACH DATABASE …")` works
//! from the mod's gameplay context, so a decision written to a SQLite file the
//! harness owns reaches a running game on the same turn.
//!
//! So this is the decider. It rebuilds the real board as a CIVVIS `Game`, runs
//! `AdvancedAi` on it, and reads back the actions that agent chose from the action
//! log. Nothing here decides anything itself; the translation layer is deliberately
//! dumb, because the moment it starts preferring one action to another it becomes
//! another hand-written heuristic wearing CIVVIS's name.
//!
//!     civvis-orders --mirror ~/civvis-civ6-runs/control/<run> --turn 42
//!
//! # What it is honest about
//!
//! ⚠ THE RECONSTRUCTION IS PARTIAL, and the partiality has a direction. Terrain,
//! both empires' cities, our units and every VISIBLE rival unit cross over. Techs,
//! civics, buildings, districts, promotions and treasuries do not. So CIVVIS is
//! deciding with a correct map and a correct order of battle, and an empty research
//! tree. Orders about ground and force are worth trusting; a `Produce` for a unit
//! this seat cannot yet build will simply be refused by Civilization VI and counted
//! as refused, which is the right failure.
//!
//! ⚠ `unmapped` is reported, not swallowed. A Civ 6 unit type with no CIVVIS
//! counterpart is a unit CIVVIS cannot see, and a half-visible army produces
//! confident orders about the wrong battle.
//!
//! ⚠ Coordinates are converted at the boundary and only there. Civilization VI
//! speaks OFFSET; CIVVIS stores AXIAL. Mixing them is silent — it once put a
//! capital on no tile at all — so every position in the emitted orders goes back
//! through `hex::axial_to_offset`.

use std::path::Path;

use civvis::ai::Ai;
#[allow(unused_imports)]
use civvis::reasoning::Journal;
use civvis::game::Action;
use civvis::mirror;

fn arg_text(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|value| value == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

/// JSON-escape the little that needs it. Order verbs are type names from the two
/// rulesets, so this is a guard rather than a general encoder.
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The Civilization VI type name for whatever CIVVIS asked a city to build.
///
/// ⚠ ONLY UNITS USED TO TRANSLATE, so every district, building and wonder CIVVIS
/// chose was dropped — its whole economic half silently became the built-in
/// ladder's decision while telemetry still read `orders_source: civvis`. On the
/// turn-190 board that was 6 of 6 city orders reduced to units.
///
/// A wonder is a BUILDING in Civilization VI, not a category of its own. Formations
/// (Corps/Armies) need a different operation than BUILD, so they stay untranslated
/// and counted rather than guessed at.
///
/// The mod resolves the final name against `GameInfo.Units`/`Buildings`/`Districts`/
/// `Projects` and refuses what it cannot find, so a wrong guess here is reported as
/// a refusal rather than acted on.
fn civ6_build_name(item: &civvis::game::Item) -> Option<String> {
    use civvis::game::Item;
    let upper = |name: &civvis::name::Name| name.as_str().to_ascii_uppercase();
    match item {
        Item::Unit { unit } => Some(format!("UNIT_{}", upper(unit))),
        Item::Building { building } => Some(format!("BUILDING_{}", upper(building))),
        Item::District { district, .. } => Some(format!("DISTRICT_{}", upper(district))),
        Item::Wonder { wonder, .. } => Some(format!("BUILDING_{}", upper(wonder))),
        Item::Project { project } => Some(format!("PROJECT_{}", upper(project))),
        _ => None,
    }
}

fn civ6_tech_name(civvis: &str) -> String {
    format!("TECH_{}", civvis.to_ascii_uppercase())
}

fn civ6_civic_name(civvis: &str) -> String {
    format!("CIVIC_{}", civvis.to_ascii_uppercase())
}

struct Order {
    kind: &'static str,
    subject: Option<i64>,
    verb: Option<String>,
    pos: Option<(i32, i32)>,
}

impl Order {
    fn to_json(&self) -> String {
        let mut parts = vec![format!("\"kind\":{}", quote(self.kind))];
        match self.subject {
            Some(value) => parts.push(format!("\"subject\":{value}")),
            None => parts.push("\"subject\":null".to_string()),
        }
        match &self.verb {
            Some(value) => parts.push(format!("\"verb\":{}", quote(value))),
            None => parts.push("\"verb\":null".to_string()),
        }
        match self.pos {
            Some((x, y)) => {
                parts.push(format!("\"x\":{x}"));
                parts.push(format!("\"y\":{y}"));
            }
            None => {
                parts.push("\"x\":null".to_string());
                parts.push("\"y\":null".to_string());
            }
        }
        format!("{{{}}}", parts.join(","))
    }
}

/// One turn's decision, against a mirror and an agent that PERSIST across turns.
///
/// ★★★★★ WHY PERSISTENCE IS THE WHOLE POINT. A fresh `AdvancedAi` on a fresh board
/// throws away its strategic plan, its force groups and every settler's destination
/// each turn, so only what is locally optimal survives — and standing still is almost
/// always locally optimal. Measured on run civvis-20260730T120107Z: 28 units at turn
/// 108 and the FURTHEST one 7 tiles from the capital, plateaued since turn 74. Nothing
/// ever went looking for the enemy, `met` stopped at 2, no rival city was ever seen,
/// and an army of 23 had nothing to attack. The settler oscillating between two tiles
/// for twenty turns is the same defect in miniature.
fn decide(
    mirror_state: &mut civvis::mirror::LiveMirror,
    ai: &mut civvis::ai::AdvancedAi,
    snapshot: &civvis::mirror::Snapshot,
    state: &civvis::mirror::StateSnapshot,
    war_from_plan: bool,
) -> String {
    let before = mirror_state.game.log.len();
    ai.take_turn(&mut mirror_state.game, 0);

    let mut orders: Vec<Order> = Vec::new();
    let mut skipped: std::collections::BTreeMap<&'static str, usize> =
        std::collections::BTreeMap::new();
    let mut note_bits: Vec<String> = Vec::new();

    for (seat, action) in mirror_state.game.log.since(before) {
        if *seat != 0 {
            continue;
        }
        let order = translate(action, mirror_state, state, &mut skipped);
        match order {
            Some(value) => orders.push(value),
            None => {
                *skipped.entry("untranslatable").or_insert(0) += 1;
            }
        }
    }

    let plan = ai.plan_report();
    if let Some(report) = &plan {
        note_bits.push(format!(
            "plan strategy={} victory={:?} target_player={:?} desired_cities={}",
            report.strategy, report.victory_target, report.target_player,
            report.desired_cities
        ));
        // ⚠⚠ RETRACTED AS A DEFAULT, AND THE REASON IS A LOST GAME.
        //
        // This used to declare war whenever CIVVIS's PLAN named a target, on the
        // reasoning that a plan rebuilt every turn never gets far enough to log a
        // `DeclareWar` of its own. But `plan_report().target_player` is who CIVVIS
        // would PREFER to fight, not "declare now" — CIVVIS's own gating had declined,
        // and overriding a decline is me making the decision, which is exactly what
        // this architecture exists to stop.
        //
        // Measured cost on run civvis-20260730T120107Z: three forced declarations
        // (t48, t144, t217) with an army of 2-6 units, the empire ground from 3 cities
        // to 2 to none, and the run ended on the DEFEAT screen at ~t220 with score 161
        // against 892. Being conquered on SETTLER is the strongest possible evidence
        // that the wars were not CIVVIS's idea.
        //
        // Kept behind `--war-from-plan` because it is the right diagnostic when plan
        // continuity is broken — but with the persistent mirror, CIVVIS should reach
        // its own declaration, and if it does not that is information, not a gap to fill.
        let already = orders.iter().any(|o| o.kind == "war");
        if war_from_plan && !already {
            if let Some(seat) = report.target_player {
                if let Some(rival) = state.rivals.get(seat.saturating_sub(1)) {
                    if !rival.at_war {
                        orders.push(Order {
                            kind: "war",
                            subject: Some(rival.player as i64),
                            verb: Some("DECLARE".to_string()),
                            pos: None,
                        });
                        note_bits.push(format!("war_from_plan={}", rival.player));
                    }
                }
            }
        }
    } else {
        note_bits.push("plan=none".to_string());
    }

    if !mirror_state.unmapped.is_empty() {
        note_bits.push(format!("unmapped: {}", mirror_state.unmapped.join(",")));
    }
    if !skipped.is_empty() {
        let text = skipped
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(" ");
        note_bits.push(format!("skipped {text}"));
    }
    // ⚠ Diagnostics for the "CIVVIS returned nothing" case, which is otherwise
    // indistinguishable from "CIVVIS chose to do nothing". Both a stopped game
    // (`winner` set) and an army with no movement produce an empty order list.
    let movable = mirror_state
        .game
        .units
        .values()
        .filter(|u| u.owner == 0 && u.moves_left > 0.0)
        .count();
    let ours = mirror_state.game.units.values().filter(|u| u.owner == 0).count();
    // Does the engine still SEE our roster? `player_unit_ids` answers from a memo,
    // and on a persistent game a stale memo would hand CIVVIS an army that no longer
    // matches the one in `units` — a mismatch that produces no error anywhere.
    note_bits.push(format!(
        "roster={} ",
        mirror_state.game.player_unit_ids(0).len()
    ));
    note_bits.push(format!(
        "movable={}/{} winner={:?} logged={}",
        movable,
        ours,
        mirror_state.game.winner,
        mirror_state.game.log.len() - before
    ));
    note_bits.push(format!(
        "synced={} units={} cities={} revealed={}",
        mirror_state.turns_synced,
        mirror_state.game.units.values().filter(|u| u.owner == 0).count(),
        mirror_state.game.cities.values().filter(|c| c.owner == 0).count(),
        snapshot.revealed_count()
    ));

    let body = orders
        .iter()
        .map(|o| o.to_json())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"turn\":{},\"orders\":[{}],\"note\":{}}}",
        state.turn,
        body,
        quote(&note_bits.join("; "))
    )
}

/// One CIVVIS action -> one Civilization VI order, or None with a counted reason.
fn translate(
    action: &Action,
    mirror_state: &civvis::mirror::LiveMirror,
    state: &civvis::mirror::StateSnapshot,
    skipped: &mut std::collections::BTreeMap<&'static str, usize>,
) -> Option<Order> {
    let civ6_of = &mirror_state.civ6_of;
    match action {
        Action::MoveTo { unit, to } | Action::Move { unit, to } => {
            civ6_of.get(unit).map(|civ6| Order {
                kind: "unit",
                subject: Some(*civ6),
                verb: Some("MOVE_TO".to_string()),
                pos: Some(civvis::hex::axial_to_offset(to.0, to.1)),
            })
        }
        // ⚠ There is NO attack operation on this build; the resolved list is only
        // MOVE_TO and RANGE_ATTACK, so a melee strike IS a move onto the defended
        // plot. That is how Civilization VI resolves it, not a hack.
        Action::Attack { unit, target } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("ATTACK".to_string()),
            pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
        }),
        Action::Ranged { unit, target } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("RANGE_ATTACK".to_string()),
            pos: Some(civvis::hex::axial_to_offset(target.0, target.1)),
        }),
        Action::FoundCity { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("FOUND_CITY".to_string()),
            pos: None,
        }),
        Action::Fortify { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("FORTIFY".to_string()),
            pos: None,
        }),
        Action::UpgradeUnit { unit } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("UPGRADE".to_string()),
            pos: None,
        }),
        // ⚠⚠ A CASUS-BELLI WAR IS STILL A WAR, AND THIS DROPPED IT ON THE FLOOR.
        // CIVVIS prefers `DeclareWarWithCasusBelli` for a major rival and keeps
        // surprise war for minors, so this variant is the one it would actually emit
        // against the civilizations domination needs — and it was falling through to
        // the `other` tally, counted as untranslatable. Civilization VI has one war
        // declaration; the grievance bookkeeping is a CIVVIS rule with no counterpart,
        // so the casus belli is dropped and the war is kept.
        Action::DeclareWarWithCasusBelli { player, .. }
        | Action::DeclareWar { player, .. } => Some(Order {
            kind: "war",
            subject: state
                .rivals
                .get(player.saturating_sub(1))
                .map(|r| r.player as i64),
            verb: Some("DECLARE".to_string()),
            pos: None,
        }),
        Action::Research { tech, .. } => Some(Order {
            kind: "research",
            subject: None,
            verb: Some(civ6_tech_name(tech.as_str())),
            pos: None,
        }),
        Action::Civic { civic, .. } => Some(Order {
            kind: "civic",
            subject: None,
            verb: Some(civ6_civic_name(civic.as_str())),
            pos: None,
        }),
        Action::Produce { city, item } => {
            mirror_state.cid_of.iter().find(|(_, cid)| **cid == *city).and_then(
                |(civ6, _)| {
                    civ6_build_name(item).map(|name| Order {
                        kind: "produce",
                        subject: Some(*civ6),
                        verb: Some(name),
                        pos: None,
                    })
                },
            )
        }
        other => {
            *skipped.entry(action_label(other)).or_insert(0) += 1;
            None
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(dir) = arg_text(&args, "--mirror") else {
        eprintln!("usage: civvis-orders --mirror <run-dir> [--turn N] [--serve]");
        std::process::exit(2);
    };
    let players: usize = arg_text(&args, "--players")
        .and_then(|v| v.parse().ok())
        .unwrap_or(4);
    let max_turns: u32 = arg_text(&args, "--max-turns")
        .and_then(|v| v.parse().ok())
        .unwrap_or(250);
    let frontier: u32 = arg_text(&args, "--frontier")
        .and_then(|v| v.parse().ok())
        .unwrap_or(6);
    let victory = arg_text(&args, "--victory").unwrap_or_else(|| "domination".to_string());
    let mut ai = match victory.as_str() {
        // ★ NAMING THE OBJECTIVE IS NOT MAKING THE DECISIONS. `targeting` pins which
        // victory CIVVIS plays for and leaves every choice about how to reach it —
        // war target, army size, what each city builds, where each unit goes — to
        // CIVVIS. Left to itself on a reconstruction carrying no wonders or tech
        // history it picked `religion` with `victory=None`, unreachable in 250 turns.
        // `--victory civvis` restores letting it choose, so the two are comparable.
        "civvis" => civvis::ai::AdvancedAi::new(),
        "domination" => civvis::ai::AdvancedAi::targeting(civvis::ai::VictoryTarget::Domination),
        "science" => civvis::ai::AdvancedAi::targeting(civvis::ai::VictoryTarget::Science),
        "score" => civvis::ai::AdvancedAi::targeting(civvis::ai::VictoryTarget::Score),
        other => {
            eprintln!("unknown --victory {other}; use domination|science|score|civvis");
            std::process::exit(2);
        }
    };

    let events = Path::new(&dir).join("events.jsonl");
    let serve = args.iter().any(|a| a == "--serve");
    // ★ CIVVIS's own account of WHY. `--explain` attaches a recording journal — the
    // same one the spectator HUD reads — and dumps it to stderr. When the agent
    // returns no orders, "it chose nothing" and "it never reached the question" are
    // indistinguishable from the outside, and this is the difference.
    let explain = args.iter().any(|a| a == "--explain");
    let journal = if explain {
        let j = civvis::reasoning::Journal::recording();
        ai.attach_journal(j.handle());
        Some(j)
    } else {
        None
    };
    let fresh_ai = args.iter().any(|a| a == "--fresh-ai");
    let war_from_plan = args.iter().any(|a| a == "--war-from-plan");
    let fresh_board = args.iter().any(|a| a == "--fresh-board");

    // Read the board fresh each time: the mod appends to this file every turn.
    let load = |want: Option<u32>| -> Option<(civvis::mirror::Snapshot, civvis::mirror::StateSnapshot)> {
        let snapshot = mirror::snapshot_from_events(&events).ok()?;
        let state = mirror::state_from_events(&events, want)?;
        if snapshot.revealed_count() == 0 {
            return None;
        }
        Some((snapshot, state))
    };

    if !serve {
        let want_turn: Option<u32> = arg_text(&args, "--turn").and_then(|v| v.parse().ok());
        let Some((snapshot, state)) = load(want_turn) else {
            println!("{{\"turn\":0,\"orders\":[],\"note\":\"no revealed terrain or no state yet\"}}");
            return;
        };
        let mut live = civvis::mirror::LiveMirror::new(
            &snapshot, &state, players, 1, max_turns, frontier,
        );
        println!("{}", decide(&mut live, &mut ai, &snapshot, &state, war_from_plan));
        return;
    }

    // ---- persistent mode -------------------------------------------------------
    //
    // One line of input per turn (the turn number, or blank for "newest"), one line of
    // orders JSON out. The mirror and the agent live for the whole game, which is what
    // gives CIVVIS a plan that spans turns.
    //
    // ⚠ Errors answer with an EMPTY order list rather than dying, so a bad turn costs
    // one turn's decisions. The mod then records `fallback` and the game keeps moving —
    // a brain that takes the run down with it is worse than one that misses a turn.
    use std::io::{BufRead, Write};
    let stdin = std::io::stdin();
    let mut out = std::io::stdout();
    let mut live: Option<civvis::mirror::LiveMirror> = None;
    let mut explain_cursor: u64 = 0;
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let want: Option<u32> = line.trim().parse().ok();
        let reply = match load(want) {
            None => format!(
                "{{\"turn\":{},\"orders\":[],\"note\":\"no revealed terrain or no state yet\"}}",
                want.unwrap_or(0)
            ),
            Some((snapshot, state)) => {
                // ★★★★★ FRESH BOARD, PERSISTENT AGENT — the one combination that works.
                //
                // Three of the four quadrants were measured: fresh board + fresh agent
                // gives 17-30 real orders; persistent board + persistent agent gives 0;
                // persistent board + fresh agent gives 0. The board is what cannot be
                // reused, because `Ai::take_turn` needs a turn that has advanced through
                // the engine's own `begin_turn`, which is private and would simulate a
                // second game.
                //
                // The AGENT can be reused, and it is the half that carries the plan:
                // `StrategicPlan` holds the grand strategy, the war target and the city
                // target, and none of those are keyed to a unit id, so rebuilding the
                // board does not invalidate them. That is the continuity a domination
                // win needs — a target chosen once and built toward, instead of
                // re-derived every turn by an agent that has never seen this world
                // before.
                //
                // ⚠ Unit-keyed memory (`settler_targets`) CAN attach to the wrong unit,
                // because ids are reassigned when the board is rebuilt. Bounded: it
                // misdirects one settler, and CIVVIS re-targets next turn. Worth
                // watching if settlers start wandering.
                if fresh_board {
                    // ⚠ The plan survives; the unit memory must not. Rebuilding the
                    // board reassigns unit ids, and the livelock detector keyed to
                    // them then stands the whole army down.
                    ai.forget_unit_memory();
                    let mut board = civvis::mirror::LiveMirror::new(
                        &snapshot, &state, players, 1, max_turns, frontier,
                    );
                    let reply = decide(&mut board, &mut ai, &snapshot, &state, war_from_plan);
                    live = Some(board);
                    reply
                } else {
                match live.as_mut() {
                    None => {
                        let mut fresh = civvis::mirror::LiveMirror::new(
                            &snapshot, &state, players, 1, max_turns, frontier,
                        );
                        let reply = decide(&mut fresh, &mut ai, &snapshot, &state, war_from_plan);
                        live = Some(fresh);
                        reply
                    }
                    Some(existing) => {
                        existing.sync(&snapshot, &state, frontier);
                        // `--fresh-ai` isolates the two halves of persistence: keep the
                        // mirror, throw away the agent. If orders come back, the empty
                        // turns are the AGENT's carried state; if they stay empty, they
                        // are the MIRROR's. Guessing between the two cost several
                        // rebuilds, so it is worth a flag.
                        if fresh_ai {
                            let mut throwaway = match victory.as_str() {
                                "civvis" => civvis::ai::AdvancedAi::new(),
                                "science" => civvis::ai::AdvancedAi::targeting(
                                    civvis::ai::VictoryTarget::Science),
                                "score" => civvis::ai::AdvancedAi::targeting(
                                    civvis::ai::VictoryTarget::Score),
                                _ => civvis::ai::AdvancedAi::targeting(
                                    civvis::ai::VictoryTarget::Domination),
                            };
                            decide(existing, &mut throwaway, &snapshot, &state, war_from_plan)
                        } else {
                            decide(existing, &mut ai, &snapshot, &state, war_from_plan)
                        }
                    }
                }
                }
            }
        };
        if let Some(j) = &journal {
            let delta = j.since(explain_cursor);
            explain_cursor = delta.cursor;
            for thought in &delta.thoughts {
                eprintln!(
                    "[why] t{} {:?}/{:?} {} | {}",
                    thought.turn, thought.topic, thought.level,
                    thought.headline, thought.detail
                );
            }
        }
        if writeln!(out, "{reply}").is_err() {
            break;
        }
        if out.flush().is_err() {
            break;
        }
    }
}

/// A short, stable label per action kind, for the skipped tally.
fn action_label(action: &Action) -> &'static str {
    match action {
        Action::Buy { .. } | Action::BuyBuilding { .. } | Action::BuyDistrict { .. } => "buy",
        Action::BuyPlot { .. } => "buy_plot",
        Action::Improve { .. } => "improve",
        Action::Promote { .. } => "promote",
        Action::Government { .. } => "government",
        Action::ChooseDedication { .. } => "dedication",
        Action::MakePeace { .. } => "peace",
        Action::Produce { .. } => "produce_nonunit",
        _ => "other",
    }
}
