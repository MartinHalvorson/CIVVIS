use super::*;

fn ledger_with(kind: Kind, uid: u32, site: Pos, turn: u32, hexes: i32) -> CommitmentLedger {
    let mut ledger = CommitmentLedger::default();
    ledger.open_new(Commitment {
        kind,
        owner: Owner::Unit(uid),
        target: Target::Tile(site),
        made: turn,
        eta: CommitmentLedger::walk_eta(turn, hexes),
        initial: hexes,
        best: hexes,
        best_turn: turn,
        observed_turn: turn,
        last_pos: None,
        improvement_then: None,
        retargets: 0,
        turns_open: 0,
        forgotten_turns: 0,
        forgotten_streak: 0,
        stalled_streak: 0,
        engaged: false,
        stalled_turns: 0,
    });
    ledger
}

#[test]
fn a_walk_is_priced_at_two_hexes_a_turn_and_never_zero() {
    assert_eq!(CommitmentLedger::walk_eta(10, 0), 10);
    assert_eq!(CommitmentLedger::walk_eta(10, 1), 11);
    assert_eq!(CommitmentLedger::walk_eta(10, 4), 12);
    assert_eq!(CommitmentLedger::walk_eta(10, 5), 13);
}

#[test]
fn an_unacted_turn_is_forgotten_and_a_fruitless_one_is_stalled_only_after_the_limit() {
    let mut ledger = ledger_with(Kind::Settle, 7, (5, 5), 10, 6);
    let key = (Kind::Settle, Owner::Unit(7));
    ledger.observe_open(key, 11, 6, Some(false), Some((1, 1)), "test");
    assert_eq!(ledger.census.settle.forgotten_turns, 1);
    assert_eq!(ledger.census.settle.stalled_turns, 0);
    // Acting without getting closer: stalled once STALL_TURNS have passed
    // since the best reading, not before.
    ledger.observe_open(key, 12, 6, Some(true), Some((1, 1)), "test");
    assert_eq!(ledger.census.settle.stalled_turns, 0);
    ledger.observe_open(key, 13, 6, Some(true), Some((1, 1)), "test");
    assert_eq!(ledger.census.settle.stalled_turns, 1);
    // Getting closer resets the clock.
    ledger.observe_open(key, 14, 4, Some(true), Some((2, 2)), "test");
    assert_eq!(ledger.census.settle.stalled_turns, 1);
    ledger.observe_open(key, 15, 4, Some(true), Some((2, 2)), "test");
    assert_eq!(ledger.census.settle.stalled_turns, 1);
    // ETA was turn 13; turns 14 and 15 are late.
    assert_eq!(ledger.census.settle.late_turns, 2);
    assert_eq!(ledger.census.settle.open_turns, 5);
}

#[test]
fn a_completion_records_its_turns_against_the_eta() {
    let mut ledger = ledger_with(Kind::Improve, 3, (2, 2), 20, 3);
    let c = ledger
        .open
        .remove(&(Kind::Improve, Owner::Unit(3)))
        .unwrap();
    ledger.end(c, "completed", 26);
    let census = ledger.census.improve;
    assert_eq!(
        (census.completed, census.completion_turns, census.eta_turns),
        (1, 6, 2)
    );
    assert_eq!(ledger.endings[&(Kind::Improve, "completed")], 1);
    assert!(
        census.line().contains("done in 6.0 turns v eta 2.0"),
        "{}",
        census.line()
    );
}

#[test]
fn a_capture_before_the_declaration_cannot_be_forgotten() {
    let mut ledger = CommitmentLedger::default();
    ledger.open_new(Commitment {
        kind: Kind::Capture,
        owner: Owner::Empire,
        target: Target::City(9),
        made: 50,
        eta: 80,
        initial: 4200,
        best: 4200,
        best_turn: 50,
        observed_turn: 50,
        last_pos: None,
        improvement_then: None,
        retargets: 0,
        turns_open: 0,
        forgotten_turns: 0,
        forgotten_streak: 0,
        stalled_streak: 0,
        engaged: false,
        stalled_turns: 0,
    });
    let key = (Kind::Capture, Owner::Empire);
    for turn in 51..60 {
        ledger.observe_open(key, turn, 4200, None, None, "test");
    }
    assert_eq!(ledger.census.capture.forgotten_turns, 0);
    assert_eq!(ledger.census.capture.stalled_turns, 0);
    ledger.observe_open(key, 60, 1200, Some(false), None, "test");
    assert_eq!(ledger.census.capture.forgotten_turns, 1);
}

#[test]
fn the_total_sums_every_kind_and_lines_name_them() {
    let mut census = CommitmentCensus::default();
    census.settle.made = 2;
    census.improve.made = 3;
    census.capture.made = 1;
    assert_eq!(census.total().made, 6);
    let lines = census.lines();
    assert_eq!(lines.len(), 3);
    assert!(lines[0].starts_with("settle  made 2"));
    assert!(lines[2].starts_with("capture made 1"));
}

#[test]
fn a_forgotten_streak_counts_consecutive_turns_and_an_acted_turn_resets_it() {
    let mut ledger = ledger_with(Kind::Settle, 7, (5, 5), 10, 6);
    let key = (Kind::Settle, Owner::Unit(7));
    ledger.observe_open(key, 11, 6, Some(false), None, "test");
    ledger.observe_open(key, 12, 6, Some(false), None, "test");
    assert_eq!(ledger.open_for(key.0, key.1).unwrap().forgotten_streak, 2);
    ledger.observe_open(key, 13, 6, None, None, "test");
    assert_eq!(
        ledger.open_for(key.0, key.1).unwrap().forgotten_streak,
        2,
        "no reading leaves it"
    );
    ledger.observe_open(key, 14, 5, Some(true), None, "test");
    assert_eq!(ledger.open_for(key.0, key.1).unwrap().forgotten_streak, 0);
    assert_eq!(ledger.open_for(key.0, key.1).unwrap().forgotten_turns, 2);
}

/// Two rival cities, a war, and a Conquest plan aimed at the farther one
/// with no unit of ours anywhere near it. With the gene on, six
/// unprosecuted turns stand the city down: it leaves the ranking, the
/// plan is stale at once, and the re-assessment names something else.
/// With the gene off the ledger only counts, and nothing is written.
/// Two rival cities, a war, and the rival city farthest from our capital
/// as the objective — out of our starting units' reach.
pub(super) fn conquest_fixture() -> (Game, u32) {
    use crate::game::Action;

    let fixture = || {
        let mut game = Game::new_full(2, 24, 16, 5_150, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|id| game.units[id].kind == "settler")
            .expect("a starting settler");
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let rival_settler = game
            .player_unit_ids(1)
            .into_iter()
            .find(|id| game.units[id].kind == "settler")
            .expect("a rival settler");
        let first = game.units[&rival_settler].pos;
        game.found_city_for(1, first, None);
        let second = game
            .wdisk(first, 7)
            .into_iter()
            .find(|pos| {
                game.wdist(*pos, first) >= 5
                    && !game.rules.is_water(&game.map.tiles[pos])
                    && game.units_at(*pos).is_empty()
                    && game.city_at(*pos).is_none()
            })
            .expect("land for a second rival city");
        game.found_city_for(1, second, None);
        game.at_war.insert((0, 1));
        game.at_war.insert((1, 0));
        let ours = game.cities[&game.player_city_ids(0)[0]].pos;
        // Aim at the rival city farthest from us, so our starting warrior
        // is not "present" by accident.
        let target = game
            .player_city_ids(1)
            .into_iter()
            .max_by_key(|cid| game.wdist(game.cities[cid].pos, ours))
            .unwrap();
        assert!(
            game.wdist(game.cities[&target].pos, ours) > CAPTURE_PRESENCE_RADIUS + 1,
            "the fixture needs the objective out of our starting units' reach"
        );
        (game, target)
    };
    fixture()
}

/// Aim a Conquest plan at `target`.
pub(super) fn aim(ai: &mut AdvancedAi, game: &Game, target: u32) {
    use super::super::StrategicPlan;
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    });
}

#[test]
fn a_declared_objective_nobody_goes_to_is_stood_down_and_the_ranking_moves_on() {
    let fixture = conquest_fixture;

    let (mut game, target) = fixture();
    let mut ai = AdvancedAi::new();
    ai.enable_capture_go_or_stand_down();
    aim(&mut ai, &game, target);
    // The first boundary registers the decision and takes no reading, so
    // six readings need seven boundaries.
    for _ in 0..=CAPTURE_GO_TURNS {
        ai.reconcile_commitments(&mut game, 0);
        game.turn += 1;
    }
    let open = ai
        .commitments()
        .open_for(Kind::Capture, Owner::Empire)
        .expect("the capture is open");
    assert_eq!(open.forgotten_streak, CAPTURE_GO_TURNS);
    assert!(ai.capture_stood_down.contains_key(&target), "stood down");
    assert!(ai.plan_stale(&game, 0), "re-assess now, not on the cadence");
    assert_eq!(
        game.players[0]
            .counters
            .get("commit:capture:gene_stand_downs"),
        Some(&1)
    );
    let next = ai.assess(&game, 0);
    assert_ne!(
        next.target_city,
        Some(target),
        "the stood-down city is out of the ranking"
    );
    // The stand-down expires.
    game.turn += CAPTURE_STAND_DOWN_TURNS;
    assert!(!ai.capture_stood_down_holds(&game, target));

    let (mut game, target) = fixture();
    let mut off = AdvancedAi::new();
    aim(&mut off, &game, target);
    for _ in 0..=CAPTURE_GO_TURNS {
        off.reconcile_commitments(&mut game, 0);
        game.turn += 1;
    }
    assert!(off.capture_stood_down.is_empty(), "off: nothing written");
    assert_eq!(
        game.players[0]
            .counters
            .get("commit:capture:gene_stand_downs"),
        None
    );
    assert_eq!(
        off.commitments()
            .open_for(Kind::Capture, Owner::Empire)
            .unwrap()
            .forgotten_streak,
        CAPTURE_GO_TURNS,
        "off: the ledger still counts"
    );
}

#[test]
fn a_stalled_streak_counts_present_readings_without_a_new_low_and_engaged_marks_a_dent() {
    let mut ledger = CommitmentLedger::default();
    ledger.open_new(Commitment {
        kind: Kind::Capture,
        owner: Owner::Empire,
        target: Target::City(9),
        made: 50,
        eta: 70,
        initial: 300,
        best: 300,
        best_turn: 50,
        observed_turn: 50,
        last_pos: None,
        improvement_then: None,
        retargets: 0,
        turns_open: 0,
        forgotten_turns: 0,
        forgotten_streak: 0,
        stalled_turns: 0,
        stalled_streak: 0,
        engaged: false,
    });
    let key = (Kind::Capture, Owner::Empire);
    let open = |ledger: &CommitmentLedger| ledger.open_for(key.0, key.1).unwrap().clone();
    // Present, city static: stalled readings start after STALL_TURNS.
    for turn in 51..=53 {
        ledger.observe_open(key, turn, 300, Some(true), None, "x");
    }
    assert_eq!(
        open(&ledger).stalled_streak,
        1,
        "turn 53 is the first stalled reading"
    );
    ledger.observe_open(key, 54, 300, Some(true), None, "x");
    assert_eq!(open(&ledger).stalled_streak, 2);
    assert!(!open(&ledger).engaged);
    // A new low resets the streak and marks the siege as engaged.
    ledger.observe_open(key, 55, 250, Some(true), None, "x");
    assert_eq!(open(&ledger).stalled_streak, 0);
    assert!(open(&ledger).engaged);
    // The city heals back: no new low, the streak grows again from
    // STALL_TURNS after the low; a forgotten turn resets it.
    for turn in 56..=59 {
        ledger.observe_open(key, turn, 280, Some(true), None, "x");
    }
    assert_eq!(open(&ledger).stalled_streak, 2);
    ledger.observe_open(key, 60, 280, Some(false), None, "x");
    assert_eq!(open(&ledger).stalled_streak, 0);
}

/// `capture-go-or-stand-down-2`: our warrior stands beside the objective
/// every turn and the city is never pushed lower. Version 1 never fires —
/// somebody went — while version 2 stands the siege down once the stalled
/// streak reaches CAPTURE_STALL_TURNS, and only version 2 is on then.
#[test]
fn a_siege_that_is_present_but_not_winning_is_stood_down_by_version_two() {
    let (mut game, target) = conquest_fixture();
    let at = game.cities[&target].pos;
    let beside = game
        .wdisk(at, 1)
        .into_iter()
        .find(|pos| {
            *pos != at
                && !game.rules.is_water(&game.map.tiles[pos])
                && game.units_at(*pos).is_empty()
        })
        .expect("a land hex beside the objective");
    game.spawn_test_unit("warrior", 0, beside);
    let rounds = 1 + STALL_TURNS + CAPTURE_STALL_TURNS;

    let mut v2 = AdvancedAi::new();
    v2.enable_capture_go_or_stand_down_2();
    assert!(
        !v2.capture_go_or_stand_down,
        "one version of the family plays"
    );
    aim(&mut v2, &game, target);
    let mut g2 = game.clone();
    for _ in 0..rounds {
        v2.reconcile_commitments(&mut g2, 0);
        g2.turn += 1;
    }
    let open = v2
        .commitments()
        .open_for(Kind::Capture, Owner::Empire)
        .expect("open");
    assert_eq!(open.forgotten_streak, 0, "somebody was there every turn");
    assert!(
        open.stalled_streak >= CAPTURE_STALL_TURNS,
        "{}",
        open.stalled_streak
    );
    assert!(
        v2.capture_stood_down.contains_key(&target),
        "version 2 stands the siege down"
    );
    assert_eq!(
        g2.players[0]
            .counters
            .get("commit:capture:stall_stand_downs"),
        Some(&1)
    );
    assert!(v2.plan_stale(&g2, 0));

    let mut v1 = AdvancedAi::new();
    v1.enable_capture_go_or_stand_down();
    aim(&mut v1, &game, target);
    let mut g1 = game.clone();
    for _ in 0..rounds {
        v1.reconcile_commitments(&mut g1, 0);
        g1.turn += 1;
    }
    assert!(
        v1.capture_stood_down.is_empty(),
        "version 1 only acts when nobody went"
    );
    assert_eq!(
        g1.players[0]
            .counters
            .get("commit:capture:stall_stand_downs"),
        None
    );
}

/// `siege-needs-a-breaker`: a walled siege held outside the city's reach
/// while a gun comes counts as neither forgotten nor stalled, so the
/// capture stands; with no gun coming, or once the wait outruns
/// `BREAKER_WAIT_TURNS`, the same readings stand it down.
#[test]
fn a_capture_waiting_on_a_coming_breaker_is_not_stood_down() {
    let (mut game, target) = conquest_fixture();
    // Late enough that a wait can have started before the cap.
    game.turn += 100;
    let at = game.cities[&target].pos;
    let land_free = |game: &Game, pos: &Pos| {
        !game.rules.is_water(&game.map.tiles[pos])
            && game.units_at(*pos).is_empty()
            && game.city_at(*pos).is_none()
    };
    let beside = game
        .wdisk(at, 1)
        .into_iter()
        .find(|pos| *pos != at && land_free(&game, pos))
        .expect("a land hex beside the objective");
    game.spawn_test_unit("warrior", 0, beside);
    let road = game
        .wdisk(at, 10)
        .into_iter()
        .filter(|pos| game.wdist(*pos, at) >= 4 && land_free(&game, pos))
        .max_by_key(|pos| (game.wdist(*pos, at), *pos))
        .expect("land for the gun on its way");
    game.spawn_test_unit("catapult", 0, road);
    let far = game.wdist(road, at);
    use super::super::siege_train::{BreakerWait, BREAKER_STALL_TURNS, BREAKER_WAIT_TURNS};
    let stall = game.standard_duration(BREAKER_STALL_TURNS);
    let rounds = 2 + stall + STALL_TURNS + CAPTURE_STALL_TURNS;
    // What `assess_siege` writes each turn the train holds out: the
    // nearest breaker `nearest` tiles out, closing in every turn or not.
    let run = |game: &Game, started_ago: u32, nearest: i32, closing: bool| {
        let mut ai = AdvancedAi::new();
        ai.enable_capture_go_or_stand_down_2();
        ai.enable_siege_needs_a_breaker();
        aim(&mut ai, game, target);
        let mut g = game.clone();
        let since = g.turn.saturating_sub(started_ago);
        for _ in 0..rounds {
            ai.siege_breaker_waits.insert(
                at,
                BreakerWait {
                    since,
                    last: g.turn,
                    nearest,
                    nearest_turn: if closing { g.turn } else { since },
                },
            );
            ai.reconcile_commitments(&mut g, 0);
            g.turn += 1;
        }
        ai
    };

    let waiting = run(&game, 0, far, true);
    assert!(
        !waiting.capture_stood_down.contains_key(&target),
        "the gun is on its way"
    );
    let open = waiting
        .commitments()
        .open_for(Kind::Capture, Owner::Empire)
        .expect("open");
    assert_eq!((open.stalled_streak, open.forgotten_streak), (0, 0));

    assert!(
        run(&game, 0, far, false)
            .capture_stood_down
            .contains_key(&target),
        "a gun that comes no nearer is not coming"
    );
    assert!(
        run(&game, 0, i32::MAX, false)
            .capture_stood_down
            .contains_key(&target),
        "nothing is coming: the hold is a stall"
    );
    let cap = game.standard_duration(BREAKER_WAIT_TURNS);
    assert!(
        run(&game, cap + 1, far, true)
            .capture_stood_down
            .contains_key(&target),
        "the wait has a limit"
    );
}

/// `siege-needs-a-breaker`: a siege train in Stage holds three to five
/// tiles out until its staged strength meets the bill. With a soldier of
/// ours on that ring the turn is neither forgotten nor stalled, so the
/// capture stands; the gene off, nobody on the ring, or a muster past
/// `BREAKER_WAIT_TURNS` and "nobody went" stands it down as before.
#[test]
fn a_capture_whose_train_gathers_on_its_staging_ring_is_not_stood_down() {
    use super::super::siege_train::{Siege, SiegeStage, BREAKER_WAIT_TURNS, STAGING_FAR};
    let (mut game, target) = conquest_fixture();
    game.turn += 100;
    let at = game.cities[&target].pos;
    let ring = game
        .wdisk(at, STAGING_FAR)
        .into_iter()
        .filter(|pos| {
            game.wdist(*pos, at) > CAPTURE_PRESENCE_RADIUS
                && !game.rules.is_water(&game.map.tiles[pos])
                && game.units_at(*pos).is_empty()
                && game.city_at(*pos).is_none()
        })
        .min_by_key(|pos| (game.wdist(*pos, at), *pos))
        .expect("land on the staging ring");
    let soldier = game.spawn_test_unit("warrior", 0, ring);
    let rounds = 2 + CAPTURE_GO_TURNS;
    let run = |game: &Game, gene: bool, started_ago: u32| {
        let mut ai = AdvancedAi::new();
        ai.enable_capture_go_or_stand_down_2();
        if gene {
            ai.enable_siege_needs_a_breaker();
        }
        aim(&mut ai, game, target);
        let mut g = game.clone();
        let entered = g.turn.saturating_sub(started_ago);
        for _ in 0..rounds {
            // What `assess_siege` keeps while the train stages.
            ai.sieges.insert(
                target,
                Siege {
                    stage: SiegeStage::Stage,
                    taker: None,
                    entered,
                    assessed: g.turn,
                    posts: BTreeMap::new(),
                    short_since: None,
                },
            );
            ai.reconcile_commitments(&mut g, 0);
            g.turn += 1;
        }
        ai.capture_stood_down.contains_key(&target)
    };
    assert!(!run(&game, true, 0), "the train is gathering");
    assert!(run(&game, false, 0), "gene off: nobody within three hexes");
    let cap = game.standard_duration(BREAKER_WAIT_TURNS);
    assert!(run(&game, true, cap + 1), "the muster has a limit");
    let mut empty = game.clone();
    empty.remove_unit(soldier);
    assert!(run(&empty, true, 0), "nobody on the ring");
}

/// `stall-waits-for-the-breach`: a train in Stage past its muster window
/// waits for a breaker; the gun comes and the train keeps gathering
/// around it. The gene off, the turns after the gun came read as stalled
/// and stand the capture down; on, they wait for `BREAKER_WAIT_TURNS`
/// from the last breaker wait, and no longer, and not once the breaker
/// leaves the train. Live King civvis-20261005T174615Z (game 148).
#[test]
fn a_muster_around_the_breaker_that_came_is_not_stood_down() {
    use super::super::siege_train::{BreakerWait, Siege, SiegeStage, BREAKER_WAIT_TURNS};
    let (mut game, target) = conquest_fixture();
    game.turn += 100;
    let at = game.cities[&target].pos;
    let beside = game
        .wdisk(at, 1)
        .into_iter()
        .find(|pos| {
            *pos != at
                && !game.rules.is_water(&game.map.tiles[pos])
                && game.units_at(*pos).is_empty()
                && game.city_at(*pos).is_none()
        })
        .expect("a land hex beside the objective");
    game.spawn_test_unit("warrior", 0, beside);
    let cap = game.standard_duration(BREAKER_WAIT_TURNS);
    let waits = 4;
    // `after` turns once the gun came; the breaker still in the train
    // unless `left`, when the reading again finds none and none comes.
    let run = |game: &Game, gene: bool, after: u32, left: bool| {
        let mut ai = AdvancedAi::new();
        ai.enable_capture_go_or_stand_down_2();
        ai.enable_siege_needs_a_breaker();
        if gene {
            ai.enable_stall_waits_for_the_breach();
        }
        aim(&mut ai, game, target);
        let mut g = game.clone();
        // Entered Stage long enough ago that `siege_mustering` is spent.
        let entered = g.turn.saturating_sub(cap + 1);
        let since = g.turn;
        for round in 0..(waits + after) {
            ai.sieges.insert(
                target,
                Siege {
                    stage: SiegeStage::Stage,
                    taker: None,
                    entered,
                    assessed: g.turn,
                    posts: BTreeMap::new(),
                    short_since: None,
                },
            );
            if round < waits {
                // A gun on its way, closing in.
                ai.siege_breaker_waits.insert(
                    at,
                    BreakerWait {
                        since,
                        last: g.turn,
                        nearest: 6,
                        nearest_turn: g.turn,
                    },
                );
            } else if left {
                // The gun is gone and nothing comes nearer.
                ai.siege_breaker_waits.insert(
                    at,
                    BreakerWait {
                        since,
                        last: g.turn,
                        nearest: i32::MAX,
                        nearest_turn: since,
                    },
                );
            } else {
                // The gun stands in the train: no wait.
                ai.siege_breaker_waits.remove(&at);
            }
            ai.reconcile_commitments(&mut g, 0);
            g.turn += 1;
        }
        ai
    };
    let readings = 2 + STALL_TURNS + CAPTURE_STALL_TURNS;
    assert!(
        run(&game, false, readings, false)
            .capture_stood_down
            .contains_key(&target),
        "gene off: the turns after the gun came read as stalled"
    );
    let held = run(&game, true, readings, false);
    assert!(
        !held.capture_stood_down.contains_key(&target),
        "the train gathers around the breaker that came"
    );
    let open = held
        .commitments()
        .open_for(Kind::Capture, Owner::Empire)
        .expect("open");
    assert_eq!((open.stalled_streak, open.forgotten_streak), (0, 0));
    assert!(
        run(&game, true, cap + readings, false)
            .capture_stood_down
            .contains_key(&target),
        "the muster after the breaker has a limit"
    );
    assert!(
        run(&game, true, readings, true)
            .capture_stood_down
            .contains_key(&target),
        "a breaker that left the train is not gathered around"
    );
}

/// `stall-waits-for-the-breach`: a siege one reading short of the
/// stand-down steps into Invest. The gene off, it is stood down; on, its
/// stall reading starts again from the step, the restarted reading still
/// stands down a siege that never dents the city, and a step back and in
/// again inside the stall window does not restart it twice. Live King
/// civvis-20261005T181436Z (game 150), Karkar at turn 92.
#[test]
fn a_siege_that_steps_forward_starts_its_stall_reading_again() {
    use super::super::siege_train::{Siege, SiegeStage};
    let (mut game, target) = conquest_fixture();
    game.turn += 100;
    let at = game.cities[&target].pos;
    let beside = game
        .wdisk(at, 1)
        .into_iter()
        .find(|pos| {
            *pos != at
                && !game.rules.is_water(&game.map.tiles[pos])
                && game.units_at(*pos).is_empty()
                && game.city_at(*pos).is_none()
        })
        .expect("a land hex beside the objective");
    game.spawn_test_unit("warrior", 0, beside);
    // The siege stage on each round, as `assess_siege` writes it.
    let run = |game: &Game, gene: bool, stages: &[SiegeStage]| {
        let mut ai = AdvancedAi::new();
        ai.enable_capture_go_or_stand_down_2();
        if gene {
            ai.enable_stall_waits_for_the_breach();
        }
        aim(&mut ai, game, target);
        let mut g = game.clone();
        for stage in stages {
            ai.sieges.insert(
                target,
                Siege {
                    stage: *stage,
                    taker: None,
                    entered: g.turn,
                    assessed: g.turn,
                    posts: BTreeMap::new(),
                    short_since: None,
                },
            );
            ai.reconcile_commitments(&mut g, 0);
            g.turn += 1;
        }
        ai.capture_stood_down.contains_key(&target)
    };
    use SiegeStage::{Invest, Stage};
    let window = (STALL_TURNS + CAPTURE_STALL_TURNS) as usize;
    // The first round registers the decision; the step comes on the round
    // the sixth stalled reading would have stood it down.
    let short = window - 1;
    let mut stepped = vec![Stage; short];
    stepped.extend(vec![Invest; window - 1]);
    assert!(
        run(&game, false, &stepped),
        "gene off: stood down before the step"
    );
    assert!(
        !run(&game, true, &stepped),
        "the step restarts the reading: the assault has its window"
    );
    let mut never = stepped.clone();
    never.push(Invest);
    assert!(
        run(&game, true, &never),
        "a restarted reading that never dents the city"
    );
    let mut wobble = vec![Stage; short];
    wobble.extend([Invest, Invest, Stage]);
    wobble.extend(vec![Invest; window - 2]);
    assert!(
        run(&game, true, &wobble),
        "in and out of Invest inside the window restarts it once"
    );
}

/// `commitment-patience`: a settler and a Builder that never act on their
/// targets. Three forgotten readings retire both — the targets leave the
/// maps, the sites are parked, the endings say `retired` — and the gene
/// off leaves both targets standing however long the unit idles.
#[test]
fn a_target_nobody_acts_on_is_retired_after_patience_and_parked() {
    use crate::game::Action;

    let fixture = || {
        let mut game = Game::new_full(2, 24, 16, 5_150, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|id| game.units[id].kind == "settler")
            .expect("a starting settler");
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        let settler = game.spawn_test_unit("settler", 0, home);
        let builder = game.spawn_test_unit("builder", 0, home);
        let land_at = |game: &Game, ring: i32| {
            game.wdisk(home, ring + 1)
                .into_iter()
                .filter(|pos| {
                    game.wdist(*pos, home) == ring && !game.rules.is_water(&game.map.tiles[pos])
                })
                .min()
                .expect("a land tile on the ring")
        };
        let far = land_at(&game, 5);
        let near = land_at(&game, 2);
        (game, settler, builder, far, near)
    };
    let idle = |ai: &mut AdvancedAi, game: &mut Game, rounds: u32| {
        for _ in 0..rounds {
            ai.reconcile_commitments(game, 0);
            game.turn += 1;
        }
    };

    let (mut game, settler, builder, far, near) = fixture();
    let mut ai = AdvancedAi::new();
    ai.enable_commitment_patience();
    ai.settler_targets.insert(settler, far);
    ai.builder_targets.insert(builder, near);
    // Registration, then COMMITMENT_PATIENCE forgotten readings.
    idle(&mut ai, &mut game, 1 + COMMITMENT_PATIENCE);
    assert!(
        !ai.settler_targets.contains_key(&settler),
        "the settle target is retired"
    );
    assert!(
        !ai.builder_targets.contains_key(&builder),
        "the improve target is retired"
    );
    assert!(
        ai.settler_site_is_dead(settler, far),
        "the site is parked for this settler"
    );
    assert_eq!(
        ai.builder_avoid.get(&builder).map(|(tile, _)| *tile),
        Some(near)
    );
    assert_eq!(
        ai.commitments().endings.get(&(Kind::Settle, "retired")),
        Some(&1)
    );
    assert_eq!(
        ai.commitments().endings.get(&(Kind::Improve, "retired")),
        Some(&1)
    );
    assert_eq!(ai.commitment_census().settle.abandoned, 1);
    assert_eq!(
        game.players[0].counters.get("commit:settle:retired"),
        Some(&1)
    );

    // A reading short of the patience retires nothing.
    let (mut game, settler, builder, far, near) = fixture();
    let mut early = AdvancedAi::new();
    early.enable_commitment_patience();
    early.settler_targets.insert(settler, far);
    early.builder_targets.insert(builder, near);
    idle(&mut early, &mut game, COMMITMENT_PATIENCE);
    assert!(early.settler_targets.contains_key(&settler));
    assert!(early.builder_targets.contains_key(&builder));

    let (mut game, settler, builder, far, near) = fixture();
    let mut off = AdvancedAi::new();
    off.settler_targets.insert(settler, far);
    off.builder_targets.insert(builder, near);
    idle(&mut off, &mut game, 2 + COMMITMENT_PATIENCE);
    assert!(
        off.settler_targets.contains_key(&settler),
        "off: the target stands"
    );
    assert!(
        off.builder_targets.contains_key(&builder),
        "off: the pin stands"
    );
    assert!(off.builder_avoid.is_empty());
    assert_eq!(
        off.commitments().endings.get(&(Kind::Settle, "retired")),
        None
    );
}

/// `route-block-is-a-wait`: a turn with no legal route step is a WAIT.
/// The owner keeps its site and tries again, and only a block that
/// outlasts `COMMITMENT_PATIENCE` consecutive forgotten turns releases
/// it. Off, the first blocked turn drops the target and parks the site —
/// which over 53 live Civ VI runs is 502 of 547 releases, 18% of the
/// settle drops landing within two tiles of the site.
///
/// The bound must not need `commitment-patience`: that gene is not in
/// `DEPLOYMENT_GENOME`, so this test leaves it off and still expects the
/// release to arrive.
#[test]
fn a_blocked_route_step_waits_before_it_gives_the_site_up() {
    use crate::game::Action;

    // A settler with a target no land route reaches: `route_step` returns
    // `None` every turn, the same reading a zone of control produces.
    let fixture = || {
        let mut game = Game::new_full(2, 24, 16, 5_150, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|id| game.units[id].kind == "settler")
            .expect("a starting settler");
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        let settler = game.spawn_test_unit("settler", 0, home);
        let unreachable = game
            .wdisk(home, 6)
            .into_iter()
            .filter(|pos| game.rules.is_water(&game.map.tiles[pos]))
            .min()
            .expect("a water tile a land unit cannot route to");
        (game, settler, unreachable)
    };
    let round = |ai: &mut AdvancedAi, game: &mut Game, rounds: u32| {
        for _ in 0..rounds {
            // Production order: `commitment_owners_act` runs before
            // `reconcile_commitments` within one turn.
            ai.commitment_owners_act(game, 0);
            ai.reconcile_commitments(game, 0);
            game.turn += 1;
        }
    };

    // Off: the very first blocked turn drops the target and parks it.
    let (mut game, settler, unreachable) = fixture();
    let mut off = AdvancedAi::new();
    off.enable_commitment_owner_acts();
    off.settler_targets.insert(settler, unreachable);
    round(&mut off, &mut game, 1);
    assert!(
        !off.settler_targets.contains_key(&settler),
        "off: one blocked turn is enough to drop the site"
    );
    assert!(
        off.settler_site_is_dead(settler, unreachable),
        "off: and to park it for the hysteresis window"
    );

    // On: the same turn holds, and the site is still the settler's.
    let (mut game, settler, unreachable) = fixture();
    let mut ai = AdvancedAi::new();
    ai.enable_commitment_owner_acts();
    ai.enable_route_block_is_a_wait();
    ai.settler_targets.insert(settler, unreachable);
    round(&mut ai, &mut game, 1);
    assert_eq!(
        ai.settler_targets.get(&settler),
        Some(&unreachable),
        "on: a blocked turn holds the decision"
    );
    assert!(
        !ai.settler_site_is_dead(settler, unreachable),
        "on: and does not park the site"
    );
    assert_eq!(
        game.players[0].counters.get("commit:settle:owner_held"),
        Some(&1)
    );

    // The wait is BOUNDED: a block that outlasts the patience releases.
    let (mut game, settler, unreachable) = fixture();
    let mut bounded = AdvancedAi::new();
    bounded.enable_commitment_owner_acts();
    bounded.enable_route_block_is_a_wait();
    bounded.settler_targets.insert(settler, unreachable);
    // One round registers the commitment; COMMITMENT_PATIENCE forgotten
    // readings then accumulate on it before the release is due.
    round(&mut bounded, &mut game, 1 + COMMITMENT_PATIENCE);
    assert_eq!(
        bounded.settler_targets.get(&settler),
        Some(&unreachable),
        "short of the patience the decision still stands"
    );
    round(&mut bounded, &mut game, 1);
    assert!(
        !bounded.settler_targets.contains_key(&settler),
        "a block that outlasts the patience releases the site"
    );
    assert!(
        bounded.settler_site_is_dead(settler, unreachable),
        "and parks it, exactly as the ungated path did"
    );
}

/// The ETA is the terrain walk at the unit's allowance, not two hexes a
/// turn: a five-hex walk for a two-move settler is at least three turns,
/// and never less than the flat-ground price.
#[test]
fn a_new_decision_is_priced_on_the_terrain_walk() {
    use crate::game::Action;

    let mut game = Game::new_full(2, 24, 16, 5_150, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .expect("a starting settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let settler = game.spawn_test_unit("settler", 0, home);
    let far = game
        .wdisk(home, 6)
        .into_iter()
        .filter(|pos| game.wdist(*pos, home) == 5 && !game.rules.is_water(&game.map.tiles[pos]))
        .min()
        .expect("a land tile five hexes out");
    let mut ai = AdvancedAi::new();
    ai.settler_targets.insert(settler, far);
    ai.reconcile_commitments(&mut game, 0);
    let c = ai
        .commitments()
        .open_for(Kind::Settle, Owner::Unit(settler))
        .expect("registered");
    let priced = c.eta - c.made;
    assert!(
        priced >= 3,
        "five hexes at two moves is three turns or more, got {priced}"
    );
    assert!(
        priced >= CommitmentLedger::walk_eta(0, 5),
        "never below the flat-ground price"
    );
}

/// The reading `docs/COMMITMENTS.md` is built on: eight 6-player 60x38
/// Online maps at the deployment genome, every major's ledger pooled.
/// `CIVVIS_CENSUS_OPT_INS=tag,tag` switches the named opt-in genes on for
/// every major, so a gene can be read against the same maps;
/// `CIVVIS_CENSUS_MAPS` sets the map count (default 8).
///
/// Run with `cargo test --profile ci commitment_census -- --ignored --nocapture`.
#[test]
#[ignore = "census, not an assertion; run explicitly with --nocapture"]
fn commitment_census() {
    use crate::ai::{Ai, GENES};
    use crate::game::{Action, GameOptions};

    let opt_ins: Vec<String> = std::env::var("CIVVIS_CENSUS_OPT_INS")
        .ok()
        .map(|text| {
            text.split(',')
                .filter(|t| !t.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let maps = std::env::var("CIVVIS_CENSUS_MAPS")
        .ok()
        .and_then(|text| text.parse::<u64>().ok())
        .unwrap_or(8);
    let mut totals = CommitmentCensus::default();
    let mut endings: BTreeMap<(Kind, &'static str), u32> = BTreeMap::new();
    let mut still_open: BTreeMap<Kind, u32> = BTreeMap::new();
    let mut forgotten_why: BTreeMap<(Kind, &'static str), u32> = BTreeMap::new();
    // What the ledger-acting genes did: every `commit:` counter that is
    // not one of the census's own exported fields.
    let mut gene_counters: BTreeMap<String, i64> = BTreeMap::new();
    for map in 0..maps {
        let seed = 98_500_000 + map;
        let mut game = Game::new_with(GameOptions {
            speed: "online".to_string(),
            randomize_civs: true,
            ..GameOptions::new(6, 60, 38, seed, 250, 6)
        });
        game.set_fog_memory(false);
        game.set_war_ledger(false);
        let major = |game: &Game, pid: usize| {
            !game.players[pid].is_minor && !game.players[pid].is_barbarian
        };
        let mut ais: Vec<AdvancedAi> = (0..game.players.len())
            .map(|pid| {
                let mut ai = AdvancedAi::new();
                if major(&game, pid) {
                    ai.enable_engine_repairs();
                    for tag in &opt_ins {
                        let enable = GENES
                            .iter()
                            .find(|gene| gene.tag == *tag && gene.opt_in())
                            .unwrap_or_else(|| panic!("{tag} is not an opt-in gene"))
                            .enable;
                        enable(&mut ai);
                    }
                }
                ai
            })
            .collect();
        while game.winner.is_none() && game.turn <= game.max_turns {
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }
        let mut census = CommitmentCensus::default();
        for (pid, ai) in ais.iter().enumerate() {
            if !major(&game, pid) {
                continue;
            }
            census.absorb(&ai.commitment_census());
            for (key, n) in &ai.commitments().endings {
                *endings.entry(*key).or_default() += n;
            }
            for c in ai.commitments().open() {
                *still_open.entry(c.kind).or_default() += 1;
            }
            for (key, n) in &ai.commitments().forgotten_why {
                *forgotten_why.entry(*key).or_default() += n;
            }
            for (key, n) in &game.players[pid].counters {
                if key.starts_with("commit:") && !ai.commitments().exports(key) {
                    *gene_counters.entry(key.clone()).or_default() += n;
                }
            }
        }
        println!("map {seed} t{}:", game.turn);
        for line in census.lines() {
            println!("  {line}");
        }
        totals.absorb(&census);
    }
    println!(
        "=== {maps} maps, deployment genome{} ===",
        if opt_ins.is_empty() {
            String::new()
        } else {
            format!(" + {}", opt_ins.join(","))
        }
    );
    for line in totals.lines() {
        println!("{line}");
    }
    println!("endings:");
    for ((kind, how), n) in &endings {
        println!("  {:<8}{how:<18}{n}", kind.as_str());
    }
    println!("open at game end:");
    for (kind, n) in &still_open {
        println!("  {:<8}{n}", kind.as_str());
    }
    println!("forgotten, by hold:");
    for ((kind, why), n) in &forgotten_why {
        println!("  {:<8}{why:<32}{n}", kind.as_str());
    }
    println!("gene counters:");
    for (key, n) in &gene_counters {
        println!("  {key:<40}{n}");
    }
}

/// See `advanced/capture_march.rs`: a declared objective whose nearest
/// land soldier keeps closing in is not stood down as "nobody went"
/// under `capture-waits-on-the-march`.
#[test]
fn a_marching_army_keeps_its_objective() {
    for gene in [false, true] {
        let (mut game, target) = conquest_fixture();
        let goal = game.cities[&target].pos;
        let mut ai = AdvancedAi::new();
        ai.enable_capture_go_or_stand_down();
        if gene {
            ai.enable_capture_waits_on_the_march();
        }
        aim(&mut ai, &game, target);
        // A soldier ten tiles out, stepped one tile closer each turn.
        let start = game
            .wdisk(goal, 10)
            .into_iter()
            .find(|pos| {
                game.wdist(*pos, goal) == 10
                    && !game.rules.is_water(&game.map.tiles[pos])
                    && game.units_at(*pos).is_empty()
            })
            .expect("land ten tiles out");
        let walker = game.spawn_test_unit("warrior", 0, start);
        for _ in 0..=CAPTURE_GO_TURNS {
            ai.reconcile_commitments(&mut game, 0);
            let here = game.units[&walker].pos;
            let step = game.nbrs(here).into_iter().find(|pos| {
                game.wdist(*pos, goal) < game.wdist(here, goal)
                    && game.wdist(*pos, goal) > CAPTURE_PRESENCE_RADIUS
                    && game
                        .map
                        .tiles
                        .get(pos)
                        .is_some_and(|tile| !game.rules.is_water(tile))
                    && game.units_at(*pos).is_empty()
                    && game.city_at(*pos).is_none()
            });
            if let Some(step) = step {
                game.relocate(walker, step);
            }
            game.turn += 1;
        }
        assert_eq!(
            ai.capture_stood_down.contains_key(&target),
            !gene,
            "gene {gene}: a marching army is not 'nobody went'"
        );
    }
}

/// See `rebase_capture_on_new_walls`: under the gene, walls built
/// mid-siege and then knocked down count as new lows, so the siege that
/// is breaking them is not stood down as stalled.
#[test]
fn new_walls_knocked_down_are_progress_under_the_gene() {
    for gene in [false, true] {
        let (mut game, target) = conquest_fixture();
        let goal = game.cities[&target].pos;
        let beside = game
            .nbrs(goal)
            .into_iter()
            .find(|pos| {
                !game.rules.is_water(&game.map.tiles[pos]) && game.units_at(*pos).is_empty()
            })
            .expect("a land hex beside the objective");
        game.spawn_test_unit("warrior", 0, beside);
        let mut ai = AdvancedAi::new();
        ai.enable_capture_go_or_stand_down_2();
        if gene {
            ai.enable_stall_rebases_on_new_walls();
        }
        aim(&mut ai, &game, target);
        // Two unwalled readings, then walls that fall ten a turn.
        for round in 0..(4 + STALL_TURNS + CAPTURE_STALL_TURNS) {
            if round == 2 {
                std::sync::Arc::make_mut(&mut game.observed_city_max_wall_hp).insert(target, 100);
            }
            if round >= 2 {
                let walls = 100 - 10 * (round as i32 - 2);
                game.cities.get_mut(&target).unwrap().wall_hp = walls.max(0);
            }
            ai.reconcile_commitments(&mut game, 0);
            game.turn += 1;
        }
        assert_eq!(
            ai.capture_stood_down.contains_key(&target),
            !gene,
            "gene {gene}: falling new walls are not a stalled siege"
        );
    }
}
