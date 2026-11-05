use super::*;

/// A founded capital, its opening units removed, and `n` Warriors in it.
fn warriors(n: usize) -> (Game, u32) {
    let mut game = Game::new_full(1, 24, 16, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let capital = game.player_city_ids(0)[0];
    let pos = game.cities[&capital].pos;
    for _ in 0..n {
        game.spawn_unit("warrior", 0, pos);
    }
    (game, capital)
}

/// Warriors in the Ancient era are current; two eras later they are an
/// obsolete army, but only with the gene on does the peacetime floor drop.
#[test]
fn an_obsolete_army_spends_down_to_the_wartime_floor_under_the_gene() {
    let (mut game, _) = warriors(4);
    let mut ai = AdvancedAi::new();
    ai.enable_obsolete_army_spends_the_floor();
    game.world_era = 0;
    assert!(
        !AdvancedAi::army_is_obsolete(&game, 0),
        "Ancient Warriors in the Ancient era"
    );
    assert_eq!(ai.obsolete_army_upgrade_floor(&game, 0, 120.0), 120.0);

    game.world_era = OBSOLETE_ERA_GAP;
    assert!(AdvancedAi::army_is_obsolete(&game, 0));
    assert_eq!(
        ai.obsolete_army_upgrade_floor(&game, 0, 120.0),
        OBSOLETE_ARMY_FLOOR
    );
    let off = AdvancedAi::new();
    assert_eq!(
        off.obsolete_army_upgrade_floor(&game, 0, 120.0),
        120.0,
        "off"
    );
}

/// Two Warriors are too few to call an army obsolete.
#[test]
fn a_token_force_is_not_an_obsolete_army() {
    let (mut game, _) = warriors(OBSOLETE_ARMY_MIN_UNITS - 1);
    game.world_era = OBSOLETE_ERA_GAP + 2;
    assert!(!AdvancedAi::army_is_obsolete(&game, 0));
}
