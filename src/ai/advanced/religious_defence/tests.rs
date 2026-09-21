use super::*;
use std::sync::Arc;

#[test]
fn observed_majorities_count_unseen_rivals_in_religious_defense() {
    let mut game = Game::new_full(3, 42, 24, 7_626, 300, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.victory_conditions.religious = true;
    game.players[1].religion = Some("Runaway Faith".to_string());
    let mut ai = AdvancedAi::new();
    ai.enable_religious_veto_defence();

    // A mirror can know the host's majority without seeing any rival cities.
    assert!(game.player_city_ids(2).is_empty());
    assert!(ai.religious_veto_engaged(&game, 0).is_none());
    Arc::make_mut(&mut game.observed_majority_religion).insert(2, "Runaway Faith".to_string());
    let stakes = ai.religious_veto_engaged(&game, 0).unwrap();
    assert_eq!((stakes.founder, stakes.dominated, stakes.others), (1, 1, 1));
    assert_eq!(stakes.stake, 0.5);
    assert_eq!(
        ai.religious_veto_threat(&game, 0, None).as_deref(),
        Some("Runaway Faith")
    );
    assert!(!AdvancedAi::religious_veto_spends(Some(&stakes)));

    // Our actual city count still determines whether the threat warrants spending.
    let ours = game.player_city_ids(0)[0];
    game.cities
        .get_mut(&ours)
        .unwrap()
        .pressure
        .insert("Runaway Faith".to_string(), 800.0);
    let stakes = ai.religious_veto_engaged(&game, 0).unwrap();
    assert_eq!(stakes.stake, 1.0);
    assert!(AdvancedAi::religious_veto_spends(Some(&stakes)));

    Arc::make_mut(&mut game.observed_majority_religion).insert(2, "Other Faith".to_string());
    let stakes = ai.religious_veto_stakes(&game, 0).unwrap();
    assert_eq!(stakes.dominated, 0);
    assert!(!AdvancedAi::religious_veto_spends(Some(&stakes)));

    ai.disable_religious_veto_defence();
    assert!(ai.religious_veto_stakes(&game, 0).is_none());
}
