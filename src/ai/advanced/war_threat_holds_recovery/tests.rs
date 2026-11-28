use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

#[test]
fn war_threat_holds_recovery_is_opt_in() {
    opt_in_off_in_both_controllers("war-threat-holds-recovery", |ai| {
        ai.war_threat_holds_recovery
    });
}

#[test]
fn a_threat_holds_for_its_window_only() {
    assert!(!threat_still_held(None, 100, 5));
    assert!(threat_still_held(Some(100), 100, 5));
    assert!(threat_still_held(Some(96), 100, 5));
    assert!(!threat_still_held(Some(95), 100, 5));
    // A zero window still holds the turn the threat was seen.
    assert!(threat_still_held(Some(100), 100, 0));
}

/// Two empires at war, ours far weaker, the rival's army at home so no city
/// of ours reads threatened: only the power gap can put the plan in Recovery.
fn outgunned_at_war() -> Game {
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
    game.found_city_for(1, game.units[&rival_settler].pos, None);
    let rival_home = game.cities[&game.player_city_ids(1)[0]].pos;
    for position in game.wdisk(rival_home, 3) {
        if game.unit_ids_at(position).is_empty() && !game.rules.is_water(&game.map.tiles[&position])
        {
            game.spawn_test_unit("swordsman", 1, position);
        }
    }
    game.at_war.insert((0, 1));
    game.at_war.insert((1, 0));
    game
}

fn stale_war_ai(gene: bool, threat_seen: Option<u32>) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.bounded_recovery = true;
    ai.major_war_since = Some(0);
    ai.war_threat_holds_recovery = gene;
    ai.war_threat_seen = threat_seen;
    ai
}

/// The reviewed flip: a war past the stale bound, outgunned four to one, a
/// city threatened two turns ago and not this turn. Shipped, the plan leaves
/// Recovery for the lane; with the gene it holds until the window runs out.
#[test]
fn a_recent_threat_keeps_a_stale_losing_war_in_recovery() {
    let mut game = outgunned_at_war();
    game.turn = 100;
    assert!(
        game.military_power(0) * 2.0 < game.military_power(1),
        "the fixture must be a losing war"
    );
    let shipped = stale_war_ai(false, Some(98)).assess(&game, 0);
    assert!(
        shipped.threatened_city.is_none(),
        "no live threat this turn"
    );
    assert_ne!(
        shipped.strategy,
        GrandStrategy::Recovery,
        "shipped, a stale war with no live threat leaves Recovery (the flip)"
    );

    let held = stale_war_ai(true, Some(98)).assess(&game, 0);
    assert_eq!(held.strategy, GrandStrategy::Recovery);

    // Past the window the war releases the plan exactly as shipped.
    let window = game.standard_duration(WAR_THREAT_HOLD_TURNS);
    let quiet = stale_war_ai(true, Some(100 - window)).assess(&game, 0);
    assert_eq!(quiet.strategy, shipped.strategy);
    // No threat ever seen: shipped behaviour.
    let never = stale_war_ai(true, None).assess(&game, 0);
    assert_eq!(never.strategy, shipped.strategy);
}

/// The observer stamps a threatened turn only during a major war with the
/// gene on, and peace forgets it.
#[test]
fn the_observer_stamps_threats_and_peace_forgets_them() {
    let mut game = outgunned_at_war();
    game.turn = 50;
    let city = game.player_city_ids(0)[0];
    let home = game.cities[&city].pos;
    for position in game.wdisk(home, 1) {
        if position != home
            && game.unit_ids_at(position).is_empty()
            && !game.rules.is_water(&game.map.tiles[&position])
        {
            game.spawn_test_unit("swordsman", 1, position);
        }
    }
    let mut ai = stale_war_ai(true, None);
    assert!(
        ai.threatened_city(&game, 0).is_some(),
        "the fixture needs a threatened city"
    );
    ai.observe_war_threat(&game, 0, true);
    assert_eq!(ai.war_threat_seen, Some(50));
    ai.observe_war_threat(&game, 0, false);
    assert_eq!(ai.war_threat_seen, None, "peace forgets the threat");

    let mut off = stale_war_ai(false, None);
    off.observe_war_threat(&game, 0, true);
    assert_eq!(off.war_threat_seen, None, "off, nothing is recorded");
}
