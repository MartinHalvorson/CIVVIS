use super::*;
use crate::ai::{run_game, Ai};

#[test]
fn a_finishing_batch_can_fire_more_shots_than_the_replan_budget() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    game.at_war.insert((0, 1));
    let pos = (8, 6);
    let target = game.spawn_unit("warrior", 1, pos);
    game.units.get_mut(&target).unwrap().hp = 1000;
    let attackers: Vec<_> = crate::hex::neighbors(pos)
        .into_iter()
        .take(REPLAN_FRAMES + 2)
        .map(|pos| game.spawn_unit("archer", 0, pos))
        .collect();
    let finishing = super::super::finishing::WarFinishingVolley {
        execution: attackers
            .iter()
            .map(|unit| {
                (
                    target,
                    vec![Action::Ranged {
                        unit: *unit,
                        target: pos,
                    }],
                )
            })
            .collect(),
        ..Default::default()
    };
    let research = game
        .legal_actions(0)
        .into_iter()
        .find(|a| matches!(a, Action::Research { .. }))
        .unwrap();
    let prior_research = game.players[0].research.clone();
    assert!(execute_frame(
        &mut game,
        0,
        &finishing,
        [(0, research)].iter()
    ));
    for unit in attackers {
        assert_eq!(game.units[&unit].attacks_left, 0);
    }
    assert_eq!(
        game.players[0].research, prior_research,
        "observe the full volley before ordinary planning"
    );
    assert!(!game.players[0].counters.contains_key("player:refused"));
}

#[test]
fn ordinary_batches_keep_their_throughput_before_refreshing() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    let founding = game
        .legal_actions(0)
        .into_iter()
        .find(|a| matches!(a, Action::FoundCity { .. }))
        .unwrap();
    let research = game
        .legal_actions(0)
        .into_iter()
        .find(|a| matches!(a, Action::Research { .. }))
        .unwrap();
    let prior_research = game.players[0].research.clone();
    assert!(execute_frame(
        &mut game,
        0,
        &Default::default(),
        [(0, founding), (0, research)].iter()
    ));
    assert_eq!(game.player_city_ids(0).len(), 1);
    assert_ne!(game.players[0].research, prior_research);
}

#[test]
fn finishing_refusal_stops_other_lines_and_ordinary_orders() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    let target = game.player_unit_ids(1)[0];
    let research = game
        .legal_actions(0)
        .into_iter()
        .find(|a| matches!(a, Action::Research { .. }))
        .unwrap();
    let prior_research = game.players[0].research.clone();
    let finishing = super::super::finishing::WarFinishingVolley {
        execution: vec![
            (target, vec![Action::FoundCity { unit: u32::MAX }]),
            (target, vec![research.clone()]),
        ],
        ..Default::default()
    };
    assert!(execute_frame(
        &mut game,
        0,
        &finishing,
        [(0, research)].iter()
    ));
    assert_eq!(game.players[0].research, prior_research);
    assert_eq!(game.players[0].counters["player:refused"], 1);
}

#[test]
fn refused_action_invalidates_batch_without_mutating_authoritative_assets() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    let units = game.units.clone();
    let gold = game.players[0].gold;
    assert!(execute_observed_action(&mut game, 0, &Action::FoundCity { unit: u32::MAX }).is_none());
    assert_eq!(game.players[0].counters["player:refused"], 1);
    assert_eq!(game.players[0].gold, gold);
    assert_eq!(
        serde_json::to_value(game.units.values().collect::<Vec<_>>()).unwrap(),
        serde_json::to_value(units.values().collect::<Vec<_>>()).unwrap()
    );
}

#[test]
fn successful_allocation_requires_new_observation_but_research_does_not() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    let research = game
        .legal_actions(0)
        .into_iter()
        .find(|a| matches!(a, Action::Research { .. }))
        .unwrap();
    assert_eq!(
        execute_observed_action(&mut game, 0, &research),
        Some(false)
    );
    let founding = game
        .legal_actions(0)
        .into_iter()
        .find(|a| matches!(a, Action::FoundCity { .. }))
        .unwrap();
    assert_eq!(execute_observed_action(&mut game, 0, &founding), Some(true));
    assert_eq!(game.player_city_ids(0).len(), 1);
}

#[test]
fn an_executor_never_applies_another_seats_plan_after_turn_changes() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    game.current = 1;
    let before = serde_json::to_value(&game).unwrap();
    assert!(execute_observed_action(&mut game, 0, &Action::EndTurn).is_none());
    assert_eq!(serde_json::to_value(&game).unwrap(), before);
}

#[test]
fn targets_are_seeded_per_seat_without_consuming_genome_randomness() {
    let targets = parse_targets(TRAINING_TARGETS).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..100 {
        for seat in 0..6 {
            let target = target_for(seed, seat, &targets);
            assert_eq!(target, target_for(seed, seat, &targets));
            seen.insert(target.map_or("civvis", |t| t.as_str()));
        }
    }
    assert_eq!(seen.len(), targets.len());
    assert!(parse_targets("").is_err());
    assert!(parse_targets("science,typo").is_err());
}

#[test]
fn production_adapters_plan_the_same_actions_from_the_same_observation() {
    for seed in [41, 73, 191] {
        let game = Game::new_full(2, 24, 16, seed, 20, 0, false);
        let mut native = AdvancedAi::new();
        native.enable_engine_repairs();
        let mut external = AdvancedAi::new();
        external.enable_live_bridge();
        assert!(native.uses_player_observation());
        assert!(external.uses_player_observation());
        let mut first = game.player_decision_view(0);
        let mut second = first.clone();
        let mapped = first.units.keys().map(|id| (*id, i64::from(*id))).collect();
        let (a, a_begin) = plan_frame(&mut native, &mut first, 0, &mapped);
        let (b, b_begin) = plan_frame(&mut external, &mut second, 0, &mapped);
        assert_eq!(a_begin, b_begin);
        assert_eq!(
            serde_json::to_value(a.actions).unwrap(),
            serde_json::to_value(b.actions).unwrap()
        );
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap()
        );
    }
}

#[test]
fn all_six_observed_players_finish_a_native_game() {
    let mut game = Game::new_full(6, 40, 28, 392, 12, 0, false);
    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
        .map(|_| {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            ai
        })
        .collect();
    run_game(&mut game, &mut ais);
    assert!(game.winner.is_some());
    for pid in 0..6 {
        assert!(!game.players[pid].remembered_tiles.is_empty());
        assert!(
            !game.player_city_ids(pid).is_empty(),
            "seat {pid} never founded"
        );
    }
}

/// What a fog-honest seat leaves on an empty queue at the end of its own
/// turn is lost: the engine processes a seat's cities in its next
/// `begin_turn`, and `process_city_with_upkeep` adds Production only to a
/// queue head. So the budget is read at exactly that moment, never at the
/// start of a turn (where every just-completed queue reads as idle).
///
/// The failure this guards is a planning view that offers what the
/// authoritative board refuses. The executor re-plans up to `REPLAN_FRAMES`
/// times, the same refused order comes back every frame, and the city ends
/// its turn empty. Measured before #3748 at Emperor over 100 turns, wonders
/// already finished in unseen cities cost those seats 8.71% of their
/// Production (worst seat 17.9%); after it, 0.17%.
#[test]
fn fog_honest_seats_lose_almost_no_production_to_refused_orders() {
    use crate::game::GameOptions;
    let mut game = Game::new_with(GameOptions {
        speed: "online".to_string(),
        ..GameOptions::new(4, 44, 28, 3_750_002, 110, 3)
    });
    let majors = 4;
    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
        .map(|_| {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            ai
        })
        .collect();
    assert!(ais[0].uses_player_observation());
    game.set_fog_memory(true);
    game.set_war_ledger(false);
    let (mut lost, mut total) = (0.0_f64, 0.0_f64);
    let mut idle_turns = Vec::new();
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        let turn = game.turn;
        let owned_before = game.player_city_ids(pid);
        let refused_before = game.players[pid]
            .counters
            .get("player:refused")
            .copied()
            .unwrap_or(0);
        let log_begin = game.log.len();
        ais[pid].take_turn(&mut game, pid);
        if pid < majors {
            for city in game.cities.values().filter(|city| city.owner == pid) {
                let production = game.city_yields(city.id).production;
                total += production;
                if city.queue.is_empty() {
                    lost += production;
                    let refused = game.players[pid]
                        .counters
                        .get("player:refused")
                        .copied()
                        .unwrap_or(0)
                        - refused_before;
                    let production_orders: Vec<_> = game.log.since(log_begin).filter(|(actor, action)| {
                        *actor == pid && matches!(action, Action::Produce { city: cid, .. } if *cid == city.id)
                    }).collect();
                    idle_turns.push(format!("turn {turn}, seat {pid}, city {}, Production {production:.2}, newly owned {}, refusals {refused}, production orders {production_orders:?}", city.id, !owned_before.contains(&city.id)));
                }
            }
        }
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    assert!(total > 0.0, "the seats produced something");
    let share = lost / total;
    assert!(
        share < 0.005,
        "fog-honest seats ended {:.2}% of their city-turns' Production on an empty queue \
         ({lost:.0} of {total:.0}); an order the view offers is being refused by the board. \
         Idle city-turns: {idle_turns:?}",
        100.0 * share
    );
}

// ---- TEMPORARY PROBE (draft PR #5234): removed before merge. ----
thread_local! {
    static REFUSALS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

pub(super) fn note_refusal(turn: u32, pid: usize, action: &Action, error: &str) {
    REFUSALS.with(|rows| {
        rows.borrow_mut()
            .push(format!("t{turn} p{pid} {action:?} => {error}"))
    });
}

const PROBE_REMOVED: &[&str] = &[
    "army-upgrade-materials",
    "blind-objective-strength",
    "boosted-bargain-first-2",
    "breaker-research-first",
    "builder-barbarian-safety",
    "builder-before-the-army-2",
    "builders-cover-the-worked-backlog",
    "builders-improve-the-worked-for-production",
    "buildings-before-projects",
    "camp-tile-buyout",
    "campus-adjacency-threshold-2",
    "capital-settler-after-completion",
    "capital-walls-before-the-surprise",
    "capture-go-or-stand-down",
    "capture-hold-chain",
    "capture-waits-on-the-march",
    "close-as-a-body",
    "counter-war-needs-the-emperor-edge",
    "culture-defense-theater",
    "declaration-needs-the-edge",
    "defend-where-you-stand",
    "district-planning-3",
    "early-contact-window",
    "early-settler-floor",
    "elective-war-in-reach",
    "elective-war-yields-to-a-lane",
    "encampment-seals-the-pass",
    "escort-unstick",
    "expansion-best-idle-city",
    "first-district-first",
    "first-granary-reserve",
    "formations-heed-refusals",
    "guns-enter-together",
    "lane-space-race",
    "missionary-last-charge-explores",
    "one-war-at-a-time",
    "opening-warrior-recon-2",
    "opening-yields-to-walls",
    "paused-district-finishes",
    "peace-when-the-war-does-not-pay",
    "peacetime-deterrence",
    "phantom-capture-keeps-the-siege",
    "pingala-follows-research",
    "policy-deck-hysteresis",
    "price-the-suzerainty",
    "ranged-before-melee",
    "recorded-tactical-step",
    "religion-sues-peace",
    "research-building-catchup-3",
    "research-tier-premium",
    "runaway-expander-counter",
    "science-denial-every-pad",
    "science-denounce-waits-for-the-race",
    "scout-first-opening",
    "second-front-keeps-its-war",
    "settler-backlog-brake",
    "settler-guard-holds",
    "siege-budget-projects-the-seal",
    "siege-is-progress-3",
    "slot-kind-tiebreak",
    "staging-reaches-the-border",
    "stall-waits-for-the-breach",
    "strategic-wonders",
    "strike-reach",
    "surprise-war-mobilization",
    "trade-route-network",
    "unwalled-target-declares-into-the-strike",
    "war-policy-via-board",
];

/// (lost share, total refusals, the refusal rows) for the guard's own game
/// with `extra` genes enabled on top of the ledger on every seat.
fn probe_stall(extra: &[&str]) -> (f64, usize, Vec<String>) {
    use crate::game::GameOptions;
    REFUSALS.with(|rows| rows.borrow_mut().clear());
    let mut game = Game::new_with(GameOptions {
        speed: "online".to_string(),
        ..GameOptions::new(4, 44, 28, 3_750_002, 110, 3)
    });
    let majors = 4;
    let mut ais: Vec<AdvancedAi> = (0..game.players.len())
        .map(|_| {
            let mut ai = AdvancedAi::new();
            ai.enable_engine_repairs();
            for tag in extra {
                (crate::ai::gene(tag).expect("a registered gene").enable)(&mut ai);
            }
            ai
        })
        .collect();
    game.set_fog_memory(true);
    game.set_war_ledger(false);
    let (mut lost, mut total) = (0.0_f64, 0.0_f64);
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        ais[pid].take_turn(&mut game, pid);
        if pid < majors {
            for city in game.cities.values().filter(|city| city.owner == pid) {
                let production = game.city_yields(city.id).production;
                total += production;
                if city.queue.is_empty() {
                    lost += production;
                }
            }
        }
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    let rows = REFUSALS.with(|rows| rows.borrow().clone());
    (lost / total.max(1.0), rows.len(), rows)
}

#[test]
fn zz_probe_which_removed_gene_masks_the_stall() {
    let mut variants: Vec<(String, Vec<&'static str>)> = vec![
        ("ledger".to_string(), Vec::new()),
        ("ledger+all68".to_string(), PROBE_REMOVED.to_vec()),
    ];
    variants.extend(
        PROBE_REMOVED
            .iter()
            .map(|tag| (format!("+{tag}"), vec![*tag])),
    );
    let workers = 4;
    let mut rows: Vec<(usize, String, f64, usize, Vec<String>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers)
            .map(|w| {
                let variants = &variants;
                scope.spawn(move || {
                    variants
                        .iter()
                        .enumerate()
                        .skip(w)
                        .step_by(workers)
                        .map(|(index, (name, extra))| {
                            let (share, refusals, rows) = probe_stall(extra);
                            (index, name.clone(), share, refusals, rows)
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("a probe game panicked"))
            .collect()
    });
    rows.sort_by_key(|row| row.0);
    let mut report = String::from("PROBE variant | lost share | refusals\n");
    for (_, name, share, refusals, _) in &rows {
        report.push_str(&format!(
            "PROBE {name} | {:.2}% | {refusals}\n",
            100.0 * share
        ));
    }
    for (_, name, _, _, refusal_rows) in rows.iter().take(2) {
        report.push_str(&format!("PROBE-ROWS {name}: last 60 refusals\n"));
        for row in refusal_rows.iter().rev().take(60).rev() {
            let row: String = row.chars().take(400).collect();
            report.push_str(&format!("PROBE-ROW {row}\n"));
        }
    }
    panic!("{report}");
}
