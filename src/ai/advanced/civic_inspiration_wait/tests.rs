use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use crate::game::Action;
use crate::name;

#[test]
fn civic_awaits_its_inspiration_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("civic-awaits-its-inspiration", |ai| {
        ai.civic_awaits_its_inspiration
    });
}

/// One founded capital on fast Culture (State Workforce finishes in a turn)
/// and a Campus two or three turns from done at the head of its queue.
fn district_due(seed: u64) -> (Game, u32) {
    let mut game = Game::new_full(1, 20, 14, seed, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let cid = game.player_city_ids(0)[0];
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            culture: 100.0,
            production: 20.0,
            ..Default::default()
        },
    );
    let pos = game.cities[&cid].pos;
    let near = game
        .nbrs(pos)
        .into_iter()
        .next()
        .expect("the capital has a neighbour");
    game.cities.get_mut(&cid).unwrap().queue.push(Item::District {
        district: name!("campus"),
        pos: near,
    });
    for civic in ["code_of_laws", "craftsmanship", "foreign_trade"] {
        game.players[0].civics.insert(Name::new(civic));
    }
    (game, cid)
}

fn with_gene() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_civic_awaits_its_inspiration();
    ai
}

#[test]
fn a_district_due_after_state_workforce_would_finish_is_a_missed_inspiration() {
    let (game, _) = district_due(71_001);
    let (eta, trigger) = AdvancedAi::civic_misses_its_inspiration(&game, 0, "state_workforce")
        .expect("the Campus lands after State Workforce would finish");
    assert_eq!(trigger, "specialty_districts");
    assert!(eta > 1.0 && eta <= INSPIRATION_WAIT_TURNS, "eta {eta}");
    // A civic whose trigger nothing in hand advances is never held.
    assert!(AdvancedAi::civic_misses_its_inspiration(&game, 0, "early_empire").is_none());
}

#[test]
fn an_inspiration_in_hand_or_landing_mid_research_is_not_missed() {
    let (mut game, cid) = district_due(71_002);
    // Slow Culture: the Campus lands while State Workforce is still running,
    // and the host credits the inspiration mid-research.
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            culture: 1.0,
            production: 20.0,
            ..Default::default()
        },
    );
    assert!(AdvancedAi::civic_misses_its_inspiration(&game, 0, "state_workforce").is_none());
    let (mut game, _) = district_due(71_003);
    game.players[0].boosted_civics.insert(name!("state_workforce"));
    assert!(AdvancedAi::civic_misses_its_inspiration(&game, 0, "state_workforce").is_none());
}

#[test]
fn on_the_beeline_a_step_with_nothing_left_to_earn_goes_first() {
    let (mut game, _) = district_due(71_004);
    let available = [
        name!("state_workforce"),
        name!("early_empire"),
        name!("mysticism"),
    ];
    let pick = |game: &Game, ai: &AdvancedAi| {
        ai.civic_awaits_its_inspiration_pick(
            game,
            0,
            &available,
            &name!("state_workforce"),
            Some("political_philosophy"),
            GrandStrategy::Conquest,
        )
    };
    assert_eq!(pick(&game, &AdvancedAi::new()), None, "off, the beeline's pick stands");
    // Early Empire's own population trigger is still ours to earn: two
    // pending steps keep the beeline's order.
    assert_eq!(pick(&game, &with_gene()), None);
    game.players[0].boosted_civics.insert(name!("early_empire"));
    assert_eq!(
        pick(&game, &with_gene()),
        Some(name!("early_empire")),
        "the step in hand goes first; Mysticism is off the path"
    );
}

#[test]
fn craftsmanship_waits_for_its_builders_behind_foreign_trade() {
    let (mut game, cid) = district_due(71_008);
    game.cities.get_mut(&cid).unwrap().queue.clear();
    game.players[0].civics.remove(&name!("craftsmanship"));
    game.players[0].civics.remove(&name!("foreign_trade"));
    // No Builder anywhere: the improvements trigger has no arrival turn, but
    // it is still ours to earn; another continent is not.
    assert!(AdvancedAi::civic_inspiration_pending(&game, 0, "craftsmanship"));
    assert!(!AdvancedAi::civic_inspiration_pending(&game, 0, "foreign_trade"));
    let available = [name!("craftsmanship"), name!("foreign_trade")];
    let pick = with_gene().civic_awaits_its_inspiration_pick(
        &game,
        0,
        &available,
        &name!("craftsmanship"),
        Some("political_philosophy"),
        GrandStrategy::Conquest,
    );
    assert_eq!(pick, Some(name!("foreign_trade")));
    game.players[0].boosted_civics.insert(name!("craftsmanship"));
    let pick = with_gene().civic_awaits_its_inspiration_pick(
        &game,
        0,
        &available,
        &name!("craftsmanship"),
        Some("political_philosophy"),
        GrandStrategy::Conquest,
    );
    assert_eq!(pick, None, "an inspiration in hand is never waited for");
}

#[test]
fn with_no_other_step_toward_the_goal_the_pick_stands() {
    let (game, _) = district_due(71_005);
    let available = [name!("state_workforce"), name!("mysticism")];
    let pick = with_gene().civic_awaits_its_inspiration_pick(
        &game,
        0,
        &available,
        &name!("state_workforce"),
        Some("political_philosophy"),
        GrandStrategy::Conquest,
    );
    assert_eq!(pick, None);
}

#[test]
fn off_the_beeline_a_comparable_civic_goes_first() {
    let (game, _) = district_due(71_006);
    let available = [name!("state_workforce"), name!("early_empire")];
    let pick = with_gene().civic_awaits_its_inspiration_pick(
        &game,
        0,
        &available,
        &name!("state_workforce"),
        None,
        GrandStrategy::Conquest,
    );
    assert_eq!(pick, Some(name!("early_empire")));
}

#[test]
fn construction_in_research_holds_games_and_recreation() {
    let (mut game, cid) = district_due(71_007);
    game.cities.get_mut(&cid).unwrap().queue.clear();
    game.players[0].civics.insert(name!("state_workforce"));
    game.players[0].research = Some("construction".to_string());
    game.players[0].research_progress = game.tech_cost("construction") * 0.75;
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            culture: 100.0,
            science: 10.0,
            ..Default::default()
        },
    );
    let (eta, trigger) = AdvancedAi::civic_misses_its_inspiration(&game, 0, "games_recreation")
        .expect("Construction lands after Games and Recreation would finish");
    assert_eq!(trigger, "tech:construction");
    assert!(eta >= 2.0 && eta <= INSPIRATION_WAIT_TURNS, "eta {eta}");
    game.players[0].research = Some("mining".to_string());
    assert!(AdvancedAi::civic_misses_its_inspiration(&game, 0, "games_recreation").is_none());
}
