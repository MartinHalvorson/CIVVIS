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
    // ⚠ MEASURE LEGALITY BEFORE THE TURN IS TAKEN. Asking afterwards reported
    // `all_legal = 0` — the enumeration short-circuits once the seat has acted — which
    // would have been read as "CIVVIS cannot declare war" when it only meant "I asked
    // at the wrong moment".
    let (pre_all_legal, pre_war_legal) = {
        let legal = mirror_state.game.legal_actions(0);
        let wars = legal
            .iter()
            .filter(|a| {
                matches!(
                    a,
                    Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }
                )
            })
            .count();
        (legal.len(), wars)
    };
    // ⚠ MEASURE MOVEMENT BEFORE THE TURN IS TAKEN, for the same reason as legality
    // above. Counted afterwards, `movable` reports what is left AFTER CIVVIS has
    // moved everything -- so a perfectly healthy turn reads `movable=0/8`, which
    // looks exactly like an army that cannot move. It nearly cost a wrong conclusion
    // about units parked for 171 turns.
    let pre_movable = mirror_state
        .game
        .units
        .values()
        .filter(|u| u.owner == 0 && u.moves_left > 0.0)
        .count();
    ai.take_turn(&mut mirror_state.game, 0);

    let mut orders: Vec<Order> = Vec::new();
    let mut skipped: std::collections::BTreeMap<String, usize> =
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
                // Which half failed: the action had no counterpart, or it named a unit
                // or city this bridge could not map back to Civilization VI. Those are
                // completely different repairs.
                let why = match action {
                    Action::MoveTo { unit, .. }
                    | Action::Move { unit, .. }
                    | Action::Attack { unit, .. }
                    | Action::Ranged { unit, .. }
                    | Action::FoundCity { unit }
                    | Action::Fortify { unit }
                    | Action::Improve { unit, .. }
                    | Action::UpgradeUnit { unit } => {
                        if rebuilt_unit_missing(mirror_state, *unit) {
                            "unit_not_mapped"
                        } else {
                            "unit_action_untranslated"
                        }
                    }
                    Action::Produce { .. } => "produce_not_mapped",
                    _ => "",
                };
                let why = if why.is_empty() {
                    action_variant(action)
                } else {
                    why.to_string()
                };
                *skipped.entry(why).or_insert(0) += 1;
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
    // ★★★★★ UNITS THE EXPORT NAMED THAT NEVER REACHED THE BOARD. A unit CIVVIS cannot
    // see gets no order and stands where it was built for the rest of the game — the
    // "units stacking up in the capital" the operator reported, arriving by a route
    // nobody had looked at. `unmapped` cannot show these: they are not translation
    // failures, they are units the reconstruction refused for a REASON, and the reason
    // is what says whether it is fog, water, an untranslatable type, or a tile CIVVIS
    // will not stack the way Civilization VI does.
    if !mirror_state.dropped_units.is_empty() {
        note_bits.push(format!(
            "dropped_units={} [{}]",
            mirror_state.dropped_units.len(),
            mirror_state.dropped_units.join(" ")
        ));
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
    // What is left after the turn. Reported beside the pre-turn count, because the
    // pair is what distinguishes "could not move" from "already moved".
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
    // ⚠ Is war even LEGAL in CIVVIS's model? Its journal says "campaign aimed at Egypt,
    // not yet at war" while our power is 156 against 20 — so either it is choosing not
    // to, or the action is not on the table. Those are opposite problems.
    {
        let has_met = mirror_state.game.has_met(0, 1);
        // ⚠ Which units does CIVVIS think it has that this bridge cannot name? A unit
        // with no Civ 6 counterpart takes orders that vanish, so CIVVIS believes a
        // settler is marching to a site while nothing moves in the real game.
        let phantom: Vec<String> = mirror_state
            .game
            .units
            .values()
            .filter(|u| u.owner == 0 && !mirror_state.civ6_of.contains_key(&u.id))
            .map(|u| format!("{}:{}", u.id, u.kind.as_str()))
            .collect();
        note_bits.push(format!(
            "pre_all_legal={pre_all_legal} pre_war_legal={pre_war_legal} has_met01={has_met} \
             phantom=[{}]",
            phantom.join(",")
        ));
        let legal = mirror_state
            .game
            .legal_actions(0)
            .into_iter()
            .filter(|a| {
                matches!(
                    a,
                    Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }
                )
            })
            .count();
        let minors: Vec<String> = mirror_state
            .game
            .players
            .iter()
            .map(|p| format!("{}:{}", p.id, if p.is_minor { "minor" } else { "major" }))
            .collect();
        let g = &mirror_state.game;
        note_bits.push(format!(
            "p1 alive={} at_war={} friends={} allied={} treaty={:?} denounced={:?}",
            g.players.get(1).map(|p| p.alive).unwrap_or(false),
            g.is_at_war(0, 1),
            g.are_friends(0, 1),
            g.are_allied(0, 1),
            g.peace_treaty_until(0, 1),
            g.players[0].denounced_until.get(&1),
        ));
        note_bits.push(format!(
            "war_legal={} met={:?} players=[{}]",
            legal,
            mirror_state.game.players[0].met,
            minors.join(",")
        ));
    }
    note_bits.push(format!(
        "roster={} ",
        mirror_state.game.player_unit_ids(0).len()
    ));
    note_bits.push(format!(
        "movable_before={}/{} movable_after={} winner={:?} logged={}",
        pre_movable,
        ours,
        movable,
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

/// Whether this CIVVIS unit has no Civilization VI counterpart in the id map.
fn rebuilt_unit_missing(mirror_state: &civvis::mirror::LiveMirror, uid: u32) -> bool {
    !mirror_state.civ6_of.contains_key(&uid)
}

/// One CIVVIS action -> one Civilization VI order, or None with a counted reason.
fn translate(
    action: &Action,
    mirror_state: &civvis::mirror::LiveMirror,
    state: &civvis::mirror::StateSnapshot,
    skipped: &mut std::collections::BTreeMap<String, usize>,
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
        // ★★★ A TRADER THAT CANNOT BE GIVEN A ROUTE IS PRODUCTION AND GOLD SPENT ON A
        // UNIT THAT WILL NEVER ACT. `Action::TradeRoute` was on the untranslatable
        // list, so every trader CIVVIS built stood where it was made for the rest of
        // the game — `civ6_watchdogs.py` names one in every run, motionless for 114
        // turns in the longest case.
        //
        // Civilization VI takes the DESTINATION CITY's plot, so the order carries the
        // target city's position rather than a unit id the bridge would have to map
        // twice. The mod resolves the operation and reports a refusal if the engine
        // will not take it, which is the honest failure for something untested against
        // a live game.
        Action::TradeRoute { unit, city } => civ6_of.get(unit).and_then(|civ6| {
            mirror_state
                .game
                .cities
                .get(city)
                .map(|destination| Order {
                    kind: "unit",
                    subject: Some(*civ6),
                    verb: Some("TRADE_ROUTE".to_string()),
                    pos: Some(civvis::hex::axial_to_offset(
                        destination.pos.0,
                        destination.pos.1,
                    )),
                })
        }),
        // A builder improving the tile it stands on. Dropping this is what kept the
        // mirror looking undeveloped and made CIVVIS order builder after builder.
        // Gold purchases. Dropping these both wastes the treasury and leaves CIVVIS
        // believing it owns something it does not — the phantom-settler failure again.
        Action::Buy { city, unit, currency, .. } if currency.as_str() == "gold" => mirror_state
            .cid_of
            .iter()
            .find(|(_, cid)| **cid == *city)
            .map(|(civ6, _)| Order {
                kind: "purchase",
                subject: Some(*civ6),
                verb: Some(format!("UNIT_{}", unit.as_str().to_ascii_uppercase())),
                pos: None,
            }),
        Action::BuyBuilding { city, building, currency } if currency.as_str() == "gold" => mirror_state
            .cid_of
            .iter()
            .find(|(_, cid)| **cid == *city)
            .map(|(civ6, _)| Order {
                kind: "purchase",
                subject: Some(*civ6),
                verb: Some(format!("BUILDING_{}", building.as_str().to_ascii_uppercase())),
                pos: None,
            }),
        Action::Improve { unit, improvement } => civ6_of.get(unit).map(|civ6| Order {
            kind: "unit",
            subject: Some(*civ6),
            verb: Some("IMPROVE".to_string()),
            pos: None,
        })
        .map(|mut order| {
            // The improvement name rides in `verb` alongside the operation, because the
            // order row has no spare column; the mod splits them.
            order.verb = Some(format!("IMPROVE:IMPROVEMENT_{}", improvement.as_str().to_ascii_uppercase()));
            order
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
        // ★★★★★ POLICY CARDS, WHICH WERE BEING DROPPED WHOLESALE.
        //
        // CIVVIS issued `SlotPolicy` on every turn from t80 to t233 of run 233331Z --
        // six a turn by the end -- and translate() had no arm, so every one was
        // counted as `skipped` and thrown away. Meanwhile the mod filled the slots
        // with its own crude "first unlocked card that fits" heuristic, which is
        // precisely the arrangement this project was told to remove: the harness
        // deciding while CIVVIS's decision is discarded.
        //
        // Policy cards are not a marginal lever here -- this project has already
        // measured them as mattering (p=0.0023).
        Action::SlotPolicy { policy } => Some(Order {
            kind: "policy",
            subject: None,
            verb: Some(format!("POLICY_{}", policy.as_str().to_ascii_uppercase())),
            pos: None,
        }),
        Action::Government { government } => Some(Order {
            kind: "government",
            subject: None,
            verb: Some(format!("GOVERNMENT_{}", government.as_str().to_ascii_uppercase())),
            pos: None,
        }),
        Action::ChoosePantheon { belief } => Some(Order {
            kind: "pantheon",
            subject: None,
            verb: Some(format!("BELIEF_{}", belief.as_str().to_ascii_uppercase())),
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
            *skipped.entry(action_variant(other)).or_insert(0) += 1;
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

    // ★★★★ HOLD THE SITE ACROSS A TURN THE SETTLER COULD NOT MOVE.
    //
    // Off by default in CIVVIS's own games; on here, and the reason is specific to
    // this bridge rather than a preference. Without it `advanced_settler_step` drops
    // the target on ANY turn the unit fails to move — a friendly unit in the way, a
    // zone of control, a barbarian standing on the route — and this bridge fails to
    // move settlers far more often than an ordinary game does, because it is also
    // refusing steps that would end inside a captor's reach. Dropping the target on
    // each of those turns would undo the unit memory that is now carried across the
    // rebuild, which is the whole point of carrying it.
    //
    // ⚠ Bounded, and the bound is what makes it safe: `SETTLER_STALL_LIMIT`
    // consecutive turns without moving releases the site, so an unreachable target
    // cannot hold a settler hostage — which is the livelock #492 was merged to fix.
    ai.settler_commit = true;
    // ⚠ NOT `parallel_settlers`. That widens the RATE at which settlers are produced
    // and it carries a measured null; this seat's constraint is settlers that never
    // arrive, not settlers that are never built. Turning both on at once would make
    // the next ledger unreadable.

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

    // ★★★★★ PRINT THE BOARD CIVVIS IS ACTUALLY ANSWERING, so it can be diffed against
    // the one Civilization VI exported.
    //
    // ⚠ THIS EXISTS BECAUSE "the mirror is 1:1" HAS ALREADY BEEN CLAIMED AND BEEN
    // FALSE. It was rendering the right terrain at the wrong hexes: Civ 6 speaks
    // OFFSET, CIVVIS stores AXIAL, both are pairs of small integers, and nothing
    // complains when they are mixed. A capital at offset (56,28) had NO TILE in the
    // reconstruction and the only symptom was CIVVIS reporting "no legal revealed
    // site" on a map with 323 revealed plots — it blamed the map.
    //
    // The dump is keyed back in OFFSET (`hex::axial_to_offset`) precisely so the
    // round trip is exercised: a plot the export named and this dump cannot produce
    // is the coordinate bug's signature, and it shows as an ABSENT tile rather than a
    // wrong value.
    //
    // Not everything here is an independent check — terrain names are written from
    // this same export through this same vocabulary, so they must agree. What IS
    // independent, and is where the defects have been:
    //
    //   w    Civ 6 answers `IsWater()`; CIVVIS derives water from its own ruleset via
    //        the translated terrain. This is the "unrevealed ground reads as OCEAN"
    //        family, which cost a seat its whole continent.
    //   h    the export encodes hills in the terrain NAME (`TERRAIN_*_HILLS`) and
    //        CIVVIS carries a separate flag. Disagreement here is the standing
    //        explanation for improvement orders refused and re-issued forever.
    //   res  whether the name resolved at all. An unresolved name does not error: the
    //        tile silently keeps whatever `Game::new` generated, which is a wrong
    //        terrain wearing a right one's clothes.
    if args.iter().any(|a| a == "--dump-mirror") {
        let want_turn: Option<u32> = arg_text(&args, "--turn").and_then(|v| v.parse().ok());
        let Some((snapshot, state)) = load(want_turn) else {
            println!("{{\"plots\":[],\"note\":\"no revealed terrain or no state yet\"}}");
            return;
        };
        let live = civvis::mirror::LiveMirror::new(
            &snapshot, &state, players, 1, max_turns, frontier,
        );
        let game = &live.game;
        let vocab = civvis::mirror::Vocabulary::embedded();
        let width = snapshot.width.max(1);
        let height = snapshot.height.max(1);
        let mut plots: Vec<String> = Vec::new();
        let mut unresolved: std::collections::BTreeMap<String, usize> = Default::default();
        for y in 0..height {
            for x in 0..width {
                let pos = civvis::hex::offset_to_axial(x, y);
                let Some(tile) = game.map.get(pos) else {
                    // Deliberately NOT skipped silently: a plot with no tile is the
                    // whole reason this dump exists. It is reported as absent by
                    // simply not appearing, and the diff counts it.
                    continue;
                };
                let exported = snapshot.plot((x, y));
                // Only dump ground either side has an opinion about. The far unknown
                // is ocean filler on both and would drown the diff in agreement.
                if exported.is_none() && !snapshot.is_revealed((x, y)) {
                    continue;
                }
                let mut resolved = true;
                if let Some(plot) = exported {
                    if let Some(name) = &plot.t {
                        match vocab.terrain(name) {
                            civvis::mirror::Resolved::Known(_) => {}
                            _ => {
                                resolved = false;
                                *unresolved.entry(name.clone()).or_default() += 1;
                            }
                        }
                    }
                }
                let field = |value: &Option<civvis::name::Name>| match value {
                    Some(name) => format!("\"{}\"", name.as_str()),
                    None => "null".to_string(),
                };
                // Whose ground the mirror thinks this is, as a CIVVIS seat index.
                // The export gives a Civ 6 player id and our seat is always its
                // local player 0, so "ours" is the part that compares cleanly; a
                // rival's id does not, because rivals are remapped on the way in.
                let owner = tile
                    .owner_city
                    .and_then(|cid| game.cities.get(&cid))
                    .map(|city| city.owner as i64);
                plots.push(format!(
                    "{{\"x\":{},\"y\":{},\"t\":\"{}\",\"h\":{},\"w\":{},\"f\":{},\"r\":{},\
                     \"im\":{},\"own\":{},\"res\":{}}}",
                    x,
                    y,
                    tile.terrain.as_str(),
                    tile.hills,
                    game.rules.is_water(tile),
                    field(&tile.feature),
                    field(&tile.resource),
                    field(&tile.improvement),
                    owner.map(|o| o == 0).unwrap_or(false),
                    resolved,
                ));
            }
        }
        let unresolved_json: Vec<String> = unresolved
            .iter()
            .map(|(name, count)| format!("\"{name}\":{count}"))
            .collect();
        println!(
            "{{\"turn\":{},\"width\":{},\"height\":{},\"revealed\":{},\
             \"unresolved_terrain\":{{{}}},\"plots\":[{}]}}",
            state.turn,
            width,
            height,
            snapshot.revealed_count(),
            unresolved_json.join(","),
            plots.join(",")
        );
        return;
    }

    if !serve {
        let want_turn: Option<u32> = arg_text(&args, "--turn").and_then(|v| v.parse().ok());
        let Some((snapshot, state)) = load(want_turn) else {
            println!("{{\"turn\":0,\"orders\":[],\"note\":\"no revealed terrain or no state yet\"}}");
            return;
        };
        let mut live = civvis::mirror::LiveMirror::new(
            &snapshot, &state, players, 1, max_turns, frontier,
        );
        let reply = decide(&mut live, &mut ai, &snapshot, &state, war_from_plan);
        // ⚠ `--explain` USED TO WORK ONLY UNDER `--serve`, which is the mode you cannot
        // debug in. Replaying one recorded turn is the fast loop — seconds, no game,
        // no lock — and it was the one path that could not say why it chose anything.
        if let Some(j) = &journal {
            for thought in &j.since(0).thoughts {
                eprintln!("{}", explain_line(thought));
            }
        }
        println!("{reply}");
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
                    // ★★★★★ CARRY THE UNIT MEMORY ACROSS THE REBUILD INSTEAD OF
                    // DROPPING IT. This used to call `forget_unit_memory`, on the
                    // sound reasoning that rebuilding the board reassigns unit ids so
                    // every unit-keyed map describes the wrong unit. The reasoning was
                    // right and the conclusion was too strong: the mirror knows each
                    // board's Civ 6 id for every unit, so old id -> Civ 6 id -> new id
                    // recovers the mapping exactly, and units that died just drop out.
                    //
                    // What forgetting cost, measured on run civvis-20260731T055749Z:
                    // the settler's DESTINATION was re-derived from scratch every turn
                    // and flipped — a site 23 tiles away on t14, t18 and t20, a
                    // different one 7 tiles away on t16 — so it never committed to
                    // anything and never arrived. The livelock detector is unit-keyed
                    // too, so the one mechanism that exists to catch a unit going in
                    // circles could never fire in this bridge at all.
                    let previous: Option<std::collections::BTreeMap<u32, i64>> =
                        live.as_ref().map(|board| board.civ6_of.clone());
                    let mut board = civvis::mirror::LiveMirror::new(
                        &snapshot, &state, players, 1, max_turns, frontier,
                    );
                    match previous {
                        Some(old) => {
                            let carried: std::collections::BTreeMap<u32, u32> = old
                                .iter()
                                .filter_map(|(old_uid, civ6)| {
                                    board.uid_of.get(civ6).map(|new| (*old_uid, *new))
                                })
                                .collect();
                            ai.remap_unit_memory(&carried);
                        }
                        None => ai.forget_unit_memory(),
                    }
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
                eprintln!("{}", explain_line(thought));
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


/// One line of CIVVIS's reasoning, with the coordinates the OPERATOR can check.
///
/// ★★★★★ EVERY POSITION IN THE JOURNAL IS AXIAL AND EVERY POSITION ON THE SCREEN IS
/// OFFSET, and the two are both pairs of small integers. Reading "Settler marching to
/// (10, 11)" against a game window showing the settler at (15, 11) reads as CIVVIS
/// ordering nonsense; they are the SAME TILE. I lost most of an hour to it tonight,
/// chasing a coordinate bug that did not exist, on the very run the operator asked me
/// to watch for exactly this.
///
/// The headline text is CIVVIS's own and stays in CIVVIS's own coordinates — rewriting
/// another module's prose would be worse — but the thought carries its focus position
/// separately, and that is appended here in OFFSET, tagged, so the number beside the
/// line is the number on the screen. See [[civvis-civ6-bridge]]: Civ 6 exports OFFSET,
/// CIVVIS stores AXIAL, and nothing complains when they are mixed.
fn explain_line(thought: &civvis::reasoning::Thought) -> String {
    let focus = match thought.focus {
        Some(pos) => {
            let (x, y) = civvis::hex::axial_to_offset(pos.0, pos.1);
            format!("  [civ6 ({x},{y}) = axial ({},{})]", pos.0, pos.1)
        }
        None => String::new(),
    };
    format!(
        "[why] t{} {:?}/{:?} {} | {}{}",
        thought.turn, thought.topic, thought.level, thought.headline, thought.detail, focus
    )
}

/// A short, stable label per action kind, for the skipped tally.
///
/// ⚠ NAME EVERY BUCKET. A tally whose biggest entry is `other` cannot be acted on —
/// over 81 replayed turns it read `untranslatable 849, other 466, buy 122,
/// government 81`, and the two largest said nothing at all about what was lost.
/// `Action::Improve` hid in `other` for the whole project and was the reason CIVVIS
/// ordered builder after builder.
/// The variant's own name, taken from its Debug form.
///
/// ⚠ A HAND-WRITTEN LIST OF LABELS GOES STALE AND LIES. The curated `action_label`
/// below still reported `other = 932` over 81 turns — 11 a turn of something nobody
/// could name, because the list did not cover every variant and there is no compiler
/// error for a missing arm behind `_`. Reading the name off Debug cannot drift.
fn action_variant(action: &Action) -> String {
    let text = format!("{action:?}");
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .next()
        .unwrap_or("other")
        .to_string()
}

#[allow(dead_code)]
fn action_label(action: &Action) -> &'static str {
    match action {
        Action::Buy { .. } => "buy_unit",
        Action::BuyBuilding { .. } => "buy_building",
        Action::BuyDistrict { .. } => "buy_district",
        Action::BuyPlot { .. } => "buy_plot",
        Action::Improve { .. } => "improve",
        Action::Promote { .. } => "promote",
        Action::Government { .. } => "government",
        Action::ChooseDedication { .. } => "dedication",
        Action::MakePeace { .. } => "peace",
        Action::Denounce { .. } => "denounce",
        Action::Produce { .. } => "produce_unmapped",
        Action::Pillage { .. } => "pillage",
        Action::RepairImprovement { .. } => "repair",
        Action::Upgrade { .. } => "upgrade_other",
        Action::CombineUnits { .. } => "combine",
        Action::LinkUnits { .. } | Action::UnlinkUnits { .. } => "link",
        Action::ContributeDistrict { .. } | Action::ContributeProject { .. } => "contribute",
        Action::AssignSpy { .. } | Action::SpyMission { .. } | Action::PromoteSpy { .. } => "spy",
        Action::ProposeDeal { .. } | Action::AcceptDeal { .. } | Action::RejectDeal { .. } => "deal",
        Action::Trade { .. } => "trade",
        Action::CongressVote { .. } => "congress",
        _ => "other",
    }
}
