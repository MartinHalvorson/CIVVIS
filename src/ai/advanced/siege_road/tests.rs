use super::super::commitments::{Kind, Owner, Target};
use super::super::siege_train::StageMarch;
use super::super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use super::ROAD_HOLD_TURNS;
use crate::game::Game;

/// A land strip: us at the west end, a civilization with closed borders
/// across the middle, the target at the east end (at war with us). The
/// strip is closed at both ends so the wrap cannot go round the middle.
fn strip() -> (Game, u32, u32) {
    let mut g = Game::new_full(3, 40, 12, 931_036, 300, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if (4..=8).contains(&tile.pos.1) && (2..=36).contains(&tile.pos.0) {
            crate::name!("grassland")
        } else {
            crate::name!("ocean")
        };
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (4, 6), None);
    let screen = g.found_city_for(1, (18, 6), None);
    let target = g.found_city_for(2, (32, 6), None);
    for tile in g.map.tiles.values_mut() {
        if (14..=22).contains(&tile.pos.0) && (4..=8).contains(&tile.pos.1) {
            tile.owner_city = Some(screen);
        }
    }
    g.players[1].borders_enforced = Some(true);
    g.at_war.insert((0, 2));
    g.at_war.insert((2, 0));
    let soldier = g.spawn_test_unit("warrior", 0, (8, 6));
    (g, target, soldier)
}

/// `strip` with a land detour north of it: up column 2 from the west
/// end to row 0, along row 0, and down column 36 into the east end, so a
/// dry road round the screen exists, more than twice the one through it.
fn strip_with_a_detour() -> (Game, u32, u32) {
    let (mut g, target, soldier) = strip();
    for tile in g.map.tiles.values_mut() {
        let (x, y) = tile.pos;
        if (y <= 3 && (x == 2 || x == 36)) || (y == 0 && (2..=36).contains(&x)) {
            tile.terrain = crate::name!("grassland");
        }
    }
    (g, target, soldier)
}

fn ai(gene: bool, g: &Game, target: u32) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_capture_go_or_stand_down_2();
    if gene {
        ai.enable_siege_target_needs_a_road();
    }
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(2),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    });
    ai
}

/// Run `turns` turns in which the Stage step reads `march` for the
/// soldier, reconciling the ledger after each.
fn run(
    ai: &mut AdvancedAi,
    g: &mut Game,
    target: u32,
    soldier: u32,
    march: StageMarch,
    turns: u32,
) {
    for _ in 0..turns {
        let _ = ai.note_stage_march(g, soldier, target, &march);
        ai.reconcile_commitments(g, 0);
        g.turn += 1;
    }
}

/// `long-road-names-the-blocker`: the dry road round a peaceful major's
/// closed borders, over twice the road through them, holds short of
/// them and names the major as the passage to buy; the gene off, or at
/// war with the screen's owner, the march keeps its dry road.
#[test]
fn a_long_road_round_closed_borders_holds_and_names_the_blocker_under_the_gene() {
    use super::super::siege_train::{STAGE_DRY_LIMIT, STAGING_FAR};
    let (g, target, soldier) = strip_with_a_detour();
    let pos = g.cities[&target].pos;
    let (step, dry) = g
        .route_step_dry(soldier, pos, STAGING_FAR, STAGE_DRY_LIMIT)
        .expect("the way round");
    let free = super::border_free_road(&g, soldier, pos).expect("a road through");
    assert_eq!(free.1, Some(1), "through the screen");
    assert!(
        dry >= 2 * free.0 && dry >= free.0 + 10,
        "fixture: {dry} against {free:?}"
    );
    let march = StageMarch::Dry { step, dry, wet: 12 };
    let mut off = ai(true, &g, target);
    off.enable_siege_buys_the_passage();
    assert!(
        matches!(
            off.long_road_holds(&g, soldier, pos, march),
            StageMarch::Dry { .. }
        ),
        "off"
    );
    let mut on = ai(true, &g, target);
    on.enable_siege_buys_the_passage();
    on.enable_long_road_names_the_blocker();
    let held = on.long_road_holds(&g, soldier, pos, march);
    assert!(matches!(held, StageMarch::Hold { wet: 12 }), "{held:?}");
    let _ = on.note_stage_march(&g, soldier, target, &held);
    assert_eq!(
        on.siege_passage_blockers(&g, 0),
        [1].into(),
        "the screen's owner"
    );
    let mut war = g.clone();
    war.at_war.insert((0, 1));
    war.at_war.insert((1, 0));
    assert!(
        matches!(
            on.long_road_holds(&war, soldier, pos, march),
            StageMarch::Dry { .. }
        ),
        "at war the road through is open"
    );
}

/// `siege-buys-the-passage`: a march held short of a major's closed
/// borders names that major as the passage to buy, while we are at peace
/// with it and for a turn after the hold; the gene off, nobody.
#[test]
fn a_held_march_names_the_passage_to_buy_under_the_gene() {
    let (g, target, soldier) = strip();
    let hold = StageMarch::Hold { wet: 30 };
    let mut off = ai(true, &g, target);
    let _ = off.note_stage_march(&g, soldier, target, &hold);
    assert!(off.siege_passage_blockers(&g, 0).is_empty(), "off");
    let mut on = ai(true, &g, target);
    on.enable_siege_buys_the_passage();
    let _ = on.note_stage_march(&g, soldier, target, &hold);
    assert_eq!(
        on.siege_passage_blockers(&g, 0),
        [1].into(),
        "the screen's owner"
    );
    let mut later = g.clone();
    later.turn += 1;
    assert_eq!(
        on.siege_passage_blockers(&later, 0),
        [1].into(),
        "a turn on"
    );
    later.turn += 1;
    assert!(on.siege_passage_blockers(&later, 0).is_empty(), "stale");
    let mut war = g.clone();
    war.at_war.insert((0, 1));
    war.at_war.insert((1, 0));
    assert!(
        on.siege_passage_blockers(&war, 0).is_empty(),
        "no purchase at war"
    );
}

/// `siege-buys-the-passage`: a siege stood down for want of the road keeps
/// naming the passage to buy, since the grant would lift the stand-down.
#[test]
fn a_stood_down_siege_still_names_the_passage_under_the_gene() {
    let (game, target, soldier) = strip();
    let hold = StageMarch::Hold { wet: 30 };
    let mut on = ai(true, &game, target);
    on.enable_siege_buys_the_passage();
    let mut g = game.clone();
    run(&mut on, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
    assert!(
        on.capture_stood_down_holds(&g, target),
        "fixture: stood down"
    );
    g.turn += 5;
    assert_eq!(
        on.siege_passage_blockers(&g, 0),
        [1].into(),
        "five turns after the last hold, the stood-down siege still asks"
    );
}

/// A target whose only land road runs through closed borders is stood
/// down within `ROAD_HOLD_TURNS` under the gene, names the border, and
/// drops its owner from the campaign; the gene off, the same holds
/// stand nothing down.
#[test]
fn a_border_blocked_siege_is_stood_down_within_three_turns() {
    let (game, target, soldier) = strip();
    assert!(
        game.route_step_dry(soldier, game.cities[&target].pos, 5, 128)
            .is_none(),
        "fixture: no open land road"
    );
    let hold = StageMarch::Hold { wet: 30 };

    let mut on = ai(true, &game, target);
    let mut g = game.clone();
    assert_eq!(
        on.note_stage_march(&g, soldier, target, &hold).as_deref(),
        Some(g.players[1].civ.as_str()),
        "the hold names the closed border"
    );
    assert!(
        on.campaign_target_legal(&g, 0, 2),
        "at war: a legal campaign target"
    );
    run(&mut on, &mut g, target, soldier, hold, ROAD_HOLD_TURNS - 1);
    assert!(
        !on.capture_stood_down.contains_key(&target),
        "two holds are not yet the rule"
    );
    run(&mut on, &mut g, target, soldier, hold, 1);
    assert!(
        on.capture_stood_down.contains_key(&target),
        "gene on: stood down"
    );
    assert!(on.capture_stood_down_holds(&g, target));
    assert!(
        on.siege_road_cuts_off(&g, 0, 2),
        "the campaign drops the rival"
    );
    assert!(!on.campaign_target_legal(&g, 0, 2));
    assert_eq!(
        g.players[0].counters.get("commit:capture:road_stand_downs"),
        Some(&1)
    );
    // A war on the screen opens the road: the stand-down lifts.
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    assert!(on.siege_road_reopened(&g, target));
    assert!(!on.capture_stood_down_holds(&g, target));
    assert!(!on.siege_road_cuts_off(&g, 0, 2));

    let mut off = ai(false, &game, target);
    let mut g = game.clone();
    assert_eq!(off.note_stage_march(&g, soldier, target, &hold), None);
    run(&mut off, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
    assert!(
        !off.capture_stood_down.contains_key(&target),
        "gene off: three holds stand nothing down"
    );
    assert!(off.siege_road_closed.is_empty());
}

/// A reachable target is unaffected: a train that marches, or one held
/// only a minority of the time, keeps its capture.
#[test]
fn a_reachable_siege_is_not_stood_down() {
    let (mut game, target, soldier) = strip();
    game.players[1].borders_enforced = Some(false);
    assert!(game
        .route_step_dry(soldier, game.cities[&target].pos, 5, 128)
        .is_some());
    let mut on = ai(true, &game, target);
    let mut g = game.clone();
    run(
        &mut on,
        &mut g,
        target,
        soldier,
        StageMarch::Ordinary,
        ROAD_HOLD_TURNS + 2,
    );
    assert!(!on.capture_stood_down.contains_key(&target));
    assert_eq!(
        on.commitments()
            .open_for(Kind::Capture, Owner::Empire)
            .map(|c| c.target),
        Some(Target::City(target))
    );

    // Held, but by a minority of the train: two marchers stand beside it.
    let (game, target, soldier) = strip();
    let mut g = game.clone();
    let mates: Vec<u32> = [(9, 6), (9, 5)]
        .into_iter()
        .map(|pos| g.spawn_test_unit("warrior", 0, pos))
        .collect();
    let mut on = ai(true, &g, target);
    for _ in 0..ROAD_HOLD_TURNS + 2 {
        let _ = on.note_stage_march(&g, soldier, target, &StageMarch::Hold { wet: 30 });
        for mate in &mates {
            let _ = on.note_stage_march(&g, *mate, target, &StageMarch::Ordinary);
        }
        on.reconcile_commitments(&mut g, 0);
        g.turn += 1;
    }
    assert!(
        !on.capture_stood_down.contains_key(&target),
        "one of three held is not the train"
    );
}

fn front(target: usize, turn: u32) -> crate::ai::advanced::one_war::OneWarFront {
    crate::ai::advanced::one_war::OneWarFront {
        target,
        since: turn,
        ledger: (0, 0, 0, 0),
        window: std::collections::VecDeque::new(),
        tide_against_since: None,
        city_health: std::collections::BTreeMap::new(),
        sieges_advancing: 0,
        closure_wanted_since: None,
    }
}

/// `blocker-becomes-the-target`: the screen whose closed borders shut the
/// stood-down siege's road -- weak, met, at peace and holding its own
/// original capital -- is named as the war that opens the road, and the
/// one-war gate admits it as the second front beside the cut-off war.
/// With the gene off nothing is named, and a war on the screen (the road
/// open) or a screen at strength names nothing either.
#[test]
fn a_weak_blocker_holding_a_capital_becomes_the_second_front() {
    let (mut game, target, soldier) = strip();
    game.record_contact(0, 1);
    game.record_contact(0, 2);
    let screen_capital = game.player_city_ids(1)[0];
    assert!(
        game.cities[&screen_capital].is_capital,
        "fixture: the screen's capital"
    );
    for pos in [(6, 5), (6, 7)] {
        game.spawn_test_unit("warrior", 0, pos);
    }
    let hold = StageMarch::Hold { wet: 30 };
    for gene in [false, true] {
        let mut ai = ai(true, &game, target);
        if gene {
            ai.enable_blocker_becomes_the_target();
        }
        ai.disable_capital_prey_opens_a_front();
        ai.disable_capital_prey_opens_a_front_2();
        ai.enable_one_war_at_a_time();
        ai.one_war = Some(front(2, game.turn));
        let mut g = game.clone();
        run(&mut ai, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
        assert!(
            ai.capture_stood_down_holds(&g, target),
            "the road stand-down holds"
        );
        assert!(
            ai.declaration_edge_2(&g, 0, 1).passes(),
            "fixture: the screen is weak"
        );
        if gene {
            assert_eq!(
                ai.road_blocker_front(&g, 0),
                Some(1),
                "gene on: the screen is named"
            );
            assert_eq!(
                ai.one_war_second_front(&g, 0),
                Some(1),
                "and opens the second front"
            );
            // A war on the screen opens the road: nothing is named any more.
            let mut opened = g.clone();
            opened.at_war.insert((0, 1));
            opened.at_war.insert((1, 0));
            assert_eq!(
                ai.road_blocker_front(&opened, 0),
                None,
                "at war: the road is open"
            );
        } else {
            assert_eq!(
                ai.road_blocker_front(&g, 0),
                None,
                "gene off: nothing is named"
            );
            assert_ne!(
                ai.one_war_second_front(&g, 0),
                Some(1),
                "gene off: no second front on the screen"
            );
        }
    }
}

/// `blocker-becomes-the-target`: a screen at strength -- out-producing
/// nothing and short of the version-2 edge -- is not named, nor is a
/// screen without an original capital.
#[test]
fn a_blocker_short_of_the_edge_or_without_a_capital_is_not_named() {
    let (mut game, target, soldier) = strip();
    game.record_contact(0, 1);
    game.record_contact(0, 2);
    let hold = StageMarch::Hold { wet: 30 };

    // At strength: the screen fields more than half our power.
    let mut strong = game.clone();
    for pos in [(19, 5), (19, 7), (17, 5), (17, 7)] {
        strong.spawn_test_unit("warrior", 1, pos);
    }
    let mut ai_strong = ai(true, &strong, target);
    ai_strong.enable_blocker_becomes_the_target();
    run(
        &mut ai_strong,
        &mut strong,
        target,
        soldier,
        hold,
        ROAD_HOLD_TURNS,
    );
    assert!(ai_strong.capture_stood_down_holds(&strong, target));
    assert!(
        !ai_strong.declaration_edge_2(&strong, 0, 1).passes(),
        "fixture: short of the edge"
    );
    assert_eq!(ai_strong.road_blocker_front(&strong, 0), None);

    // Weak, but its capital is not an original capital of a rival we need:
    // the screen's city is ours by origin.
    let screen = game.player_city_ids(1)[0];
    game.cities.get_mut(&screen).unwrap().original_owner = 0;
    let mut ai_nocap = ai(true, &game, target);
    ai_nocap.enable_blocker_becomes_the_target();
    let mut g = game.clone();
    run(
        &mut ai_nocap,
        &mut g,
        target,
        soldier,
        hold,
        ROAD_HOLD_TURNS,
    );
    assert!(ai_nocap.capture_stood_down_holds(&g, target));
    assert_eq!(ai_nocap.road_blocker_front(&g, 0), None);
}

/// Live King civvis-20261005T215912Z (game 167)'s shape: at war with the
/// target (player 2), a Diplomatic Victory contender the elimination
/// front keeps the plan on; its siege's road shut by the screen's
/// (player 1's) closed borders and stood down; the screen weak, holding
/// its original capital, and at 90% of a Science Victory. The road
/// blocker is named and the plan stays on the elimination front either
/// way; under the gene the diplomacy desk declares on the screen, off it
/// nothing does.
#[test]
fn a_blocker_the_elimination_front_displaces_is_still_declared_on() {
    let (mut game, target, soldier) = strip();
    game.turn = 200;
    game.record_contact(0, 1);
    game.record_contact(0, 2);
    game.found_city_for(0, (8, 4), None);
    game.players[0].gold = 3000.0;
    game.players[2].dvp = 18;
    game.players[1].science_projects.extend([
        "launch_earth_satellite".to_string(),
        "launch_moon_landing".to_string(),
        "launch_mars_colony".to_string(),
        "exoplanet_expedition".to_string(),
    ]);
    let observed = std::sync::Arc::make_mut(&mut game.observed_public_empire_stats)
        .entry(1)
        .or_default();
    observed.science_victory_points = Some(14.0);
    observed.science_victory_points_needed = Some(25.0);
    observed.science_victory_points_per_turn = Some(6.0);
    for (seat, power) in [(0, 4000.0), (1, 600.0), (2, 500.0)] {
        std::sync::Arc::make_mut(&mut game.observed_military_power).insert(seat, power);
    }
    let hold = StageMarch::Hold { wet: 30 };
    for gene in [false, true] {
        let mut ai = ai(true, &game, target);
        if gene {
            ai.enable_blocker_becomes_the_target();
        }
        ai.enable_diplomatic_contender_eliminated();
        ai.enable_one_war_at_a_time();
        ai.disable_capital_prey_opens_a_front();
        ai.disable_capital_prey_opens_a_front_2();
        ai.coalition_before_war = false;
        ai.coalition_before_war_2 = false;
        ai.coalition_before_war_3 = false;
        ai.one_war = Some(front(2, game.turn));
        let mut g = game.clone();
        run(&mut ai, &mut g, target, soldier, hold, ROAD_HOLD_TURNS);
        assert!(
            ai.capture_stood_down_holds(&g, target),
            "the road stand-down holds"
        );
        assert_eq!(
            ai.diplomatic_contender_to_eliminate(&g, 0),
            Some(2),
            "fixture: eliminating the target"
        );
        assert_eq!(
            ai.rival_pressure(&g, 1).0,
            GrandStrategy::Science,
            "fixture: the screen's race"
        );
        let mut plan = ai.plan.clone().expect("the fixture's plan");
        plan.assessed_turn = g.turn;
        for _ in 0..12 {
            if g.is_at_war(0, 1) {
                break;
            }
            assert_eq!(
                plan.target_player,
                Some(2),
                "the plan stays on the elimination front"
            );
            ai.advanced_diplomacy(&mut g, 0, &plan);
            g.turn += 1;
        }
        if gene {
            assert_eq!(
                ai.road_blocker_front(&g, 0),
                None,
                "at war: the road is open"
            );
            assert!(
                g.is_at_war(0, 1),
                "gene on: the displaced blocker is declared on"
            );
        } else {
            assert!(
                !g.is_at_war(0, 1),
                "gene off: nothing declares on the screen"
            );
        }
    }
}

/// `blocker-becomes-the-target` is a registered, reversible opt-in.
#[test]
fn blocker_becomes_the_target_is_a_reversible_opt_in() {
    let mut ai = AdvancedAi::new();
    assert!(!ai.blocker_becomes_the_target);
    ai.enable_blocker_becomes_the_target();
    assert!(ai.blocker_becomes_the_target);
    ai.disable_blocker_becomes_the_target();
    assert!(!ai.blocker_becomes_the_target);
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.tag == "blocker-becomes-the-target" && gene.opt_in()));
}

/// `closed-border-blocker-declares`: `strip` with powers read from the
/// host -- ours 4,000, the screen's `screen`, the target's 500 -- and
/// two marchers beside the held soldier, so the train is never half held
/// and no road stand-down fires: the shape of a siege that holds short
/// of closed borders instead.
fn held_strip(screen: f64) -> (Game, u32, u32, Vec<u32>) {
    let (mut game, target, soldier) = strip();
    game.record_contact(0, 1);
    game.record_contact(0, 2);
    for (seat, power) in [(0, 4000.0), (1, screen), (2, 500.0)] {
        std::sync::Arc::make_mut(&mut game.observed_military_power).insert(seat, power);
    }
    let mates = [(9, 6), (9, 5)]
        .into_iter()
        .map(|pos| game.spawn_test_unit("warrior", 0, pos))
        .collect();
    (game, target, soldier, mates)
}

/// `turns` turns in which the soldier holds short of the screen and the
/// mates march, reconciling the ledger after each.
fn hold_beside_marchers(
    ai: &mut AdvancedAi,
    g: &mut Game,
    target: u32,
    soldier: u32,
    mates: &[u32],
    turns: u32,
) {
    for _ in 0..turns {
        let _ = ai.note_stage_march(g, soldier, target, &StageMarch::Hold { wet: 30 });
        for mate in mates {
            let _ = ai.note_stage_march(g, *mate, target, &StageMarch::Ordinary);
        }
        ai.reconcile_commitments(g, 0);
        g.turn += 1;
    }
}

fn blocker_ai(gene: bool, g: &Game, target: u32) -> AdvancedAi {
    let mut ai = ai(true, g, target);
    // As armed live: version 1 on beside the gene, or alone.
    ai.enable_blocker_becomes_the_target();
    if gene {
        ai.enable_closed_border_blocker_declares();
    }
    ai.disable_capital_prey_opens_a_front();
    ai.disable_capital_prey_opens_a_front_2();
    ai.enable_one_war_at_a_time();
    ai.one_war = Some(front(2, g.turn));
    ai
}

/// `closed-border-blocker-declares`: three turns of holds short of a
/// screen at half our power or under name it as the road-blocker front
/// and the second front, with no stand-down; two turns do not, nor does
/// a run that stopped, nor the gene off (version 1 waits on a stand-down
/// that never comes).
#[test]
fn a_weak_blocker_of_a_held_siege_becomes_the_second_front() {
    let (game, target, soldier, mates) = held_strip(600.0);
    for gene in [false, true] {
        let mut ai = blocker_ai(gene, &game, target);
        let mut g = game.clone();
        hold_beside_marchers(
            &mut ai,
            &mut g,
            target,
            soldier,
            &mates,
            ROAD_HOLD_TURNS - 1,
        );
        assert_eq!(
            ai.road_blocker_front(&g, 0),
            None,
            "two turns of holds name nothing"
        );
        hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, 1);
        assert!(
            !ai.capture_stood_down.contains_key(&target),
            "fixture: the siege holds, never stood down"
        );
        if gene {
            assert_eq!(
                ai.closed_border_front(&g, 0),
                Some(1),
                "gene on: the screen is named"
            );
            assert_eq!(ai.road_blocker_front(&g, 0), Some(1));
            assert_eq!(
                ai.one_war_second_front(&g, 0),
                Some(1),
                "and opens the second front"
            );
            let plan = ai.plan.clone().expect("the fixture's plan");
            assert!(ai.second_front_displaced(&g, 0, 1, &plan));
            // At war with the screen the road is open: nothing is named.
            let mut opened = g.clone();
            opened.at_war.insert((0, 1));
            opened.at_war.insert((1, 0));
            assert_eq!(
                ai.road_blocker_front(&opened, 0),
                None,
                "at war: the road is open"
            );
            // A run that stopped two turns ago is no hold.
            let mut later = g.clone();
            later.turn += 1;
            assert_eq!(
                ai.closed_border_front(&later, 0),
                None,
                "a stale run names nothing"
            );
        } else {
            assert_eq!(
                ai.road_blocker_front(&g, 0),
                None,
                "gene off: version 1 waits on a stand-down"
            );
            assert_ne!(ai.one_war_second_front(&g, 0), Some(1));
        }
    }
}

/// `closed-border-blocker-declares`: a break of a turn restarts the run.
#[test]
fn a_broken_hold_restarts_the_run() {
    let (game, target, soldier, mates) = held_strip(600.0);
    let mut ai = blocker_ai(true, &game, target);
    let mut g = game.clone();
    hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, 2);
    g.turn += 1; // a turn without a hold
    hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, 2);
    assert_eq!(
        ai.closed_border_front(&g, 0),
        None,
        "two and two are not three running"
    );
    hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, 1);
    assert_eq!(ai.closed_border_front(&g, 0), Some(1));
}

/// `closed-border-blocker-declares`: a screen over half our power, or one
/// we are friends with, is not named however long it holds the road.
#[test]
fn a_strong_or_friendly_blocker_is_not_named() {
    let (game, target, soldier, mates) = held_strip(2_100.0);
    let mut ai = blocker_ai(true, &game, target);
    let mut g = game.clone();
    hold_beside_marchers(
        &mut ai,
        &mut g,
        target,
        soldier,
        &mates,
        ROAD_HOLD_TURNS + 2,
    );
    assert_eq!(ai.closed_border_front(&g, 0), None, "over half our power");

    // Weak now, but a 30-turn peak over two-thirds of ours.
    let (game, target, soldier, mates) = held_strip(600.0);
    let mut ai = blocker_ai(true, &game, target);
    ai.rival_power_peak_seen
        .entry(1)
        .or_default()
        .push((game.turn, 3_000.0));
    let mut g = game.clone();
    hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, ROAD_HOLD_TURNS);
    assert_eq!(
        ai.closed_border_front(&g, 0),
        None,
        "a recent peak at strength"
    );

    let (mut game, target, soldier, mates) = held_strip(600.0);
    game.players[0].friends_until.insert(1, game.turn + 30);
    let mut ai = blocker_ai(true, &game, target);
    let mut g = game.clone();
    hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, ROAD_HOLD_TURNS);
    assert_eq!(
        ai.closed_border_front(&g, 0),
        None,
        "a friend is no campaign target"
    );
}

/// `closed-border-blocker-declares`: a city-state screen is named alone
/// or under a suzerain we outgun; a strong suzerain at peace with us
/// keeps it off the list, and so does our own suzerainty.
#[test]
fn a_city_state_blocker_needs_a_weak_suzerain() {
    let (mut game, target, _, _) = held_strip(300.0);
    game.players[1].is_minor = true;
    let ai = blocker_ai(true, &game, target);
    assert!(
        ai.closed_border_blocker_is_weak(&game, 0, 1),
        "no suzerain: named"
    );

    // The target, at war with us, is its suzerain: the city-state is in
    // that war already, its borders open, and there is nothing to declare.
    let mut fought = game.clone();
    fought.players[2].envoys.push((1, 5));
    assert_eq!(
        fought.suzerain_of(1),
        Some(2),
        "fixture: the target holds it"
    );
    assert!(
        fought.is_at_war(0, 1),
        "fixture: dragged into its suzerain's war"
    );
    assert!(
        !ai.closed_border_blocker_is_weak(&fought, 0, 1),
        "already at war: not named"
    );

    // At peace with that suzerain, and it at strength.
    let mut strong = fought.clone();
    strong.at_war.remove(&(0, 2));
    strong.at_war.remove(&(2, 0));
    std::sync::Arc::make_mut(&mut strong.observed_military_power).insert(2, 2_500.0);
    assert!(
        !ai.closed_border_blocker_is_weak(&strong, 0, 1),
        "a strong suzerain at peace"
    );
    // At peace, but weak.
    std::sync::Arc::make_mut(&mut strong.observed_military_power).insert(2, 900.0);
    assert!(
        ai.closed_border_blocker_is_weak(&strong, 0, 1),
        "a weak suzerain at peace"
    );

    // Ours.
    let mut ours = game.clone();
    ours.players[0].envoys.push((1, 5));
    assert!(
        !ai.closed_border_blocker_is_weak(&ours, 0, 1),
        "our own city-state"
    );
}

/// `closed-border-blocker-declares` at the declaration desk, with version
/// 1 armed in both runs as it is live: the plan stays on the target's
/// war, the siege holds short of the weak screen and is never stood down
/// for the road, and under the gene the desk declares on the screen; off
/// it nothing does.
#[test]
fn the_desk_declares_on_a_weak_blocker_of_a_held_siege() {
    let (mut game, target, soldier, mates) = held_strip(600.0);
    game.turn = 150;
    game.found_city_for(0, (8, 4), None);
    game.players[0].gold = 3000.0;
    for gene in [false, true] {
        let mut ai = blocker_ai(gene, &game, target);
        ai.coalition_before_war = false;
        ai.coalition_before_war_2 = false;
        ai.coalition_before_war_3 = false;
        let mut g = game.clone();
        hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, ROAD_HOLD_TURNS);
        let mut plan = ai.plan.clone().expect("the fixture's plan");
        for _ in 0..12 {
            if g.is_at_war(0, 1) {
                break;
            }
            plan.assessed_turn = g.turn;
            assert_eq!(
                plan.target_player,
                Some(2),
                "the plan stays on the target's war"
            );
            ai.advanced_diplomacy(&mut g, 0, &plan);
            hold_beside_marchers(&mut ai, &mut g, target, soldier, &mates, 1);
        }
        assert!(
            ai.siege_road_closed.is_empty(),
            "no road stand-down for version 1"
        );
        if gene {
            assert!(
                g.is_at_war(0, 1),
                "gene on: the weak blocker is declared on"
            );
        } else {
            assert!(
                !g.is_at_war(0, 1),
                "gene off: nothing declares on the screen"
            );
        }
    }
}

/// `closed-border-blocker-declares` is a registered, reversible opt-in.
#[test]
fn closed_border_blocker_declares_is_a_reversible_opt_in() {
    let mut ai = AdvancedAi::new();
    assert!(!ai.closed_border_blocker_declares);
    ai.enable_closed_border_blocker_declares();
    assert!(ai.closed_border_blocker_declares);
    ai.disable_closed_border_blocker_declares();
    assert!(!ai.closed_border_blocker_declares);
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.tag == "closed-border-blocker-declares" && gene.opt_in()));
}
