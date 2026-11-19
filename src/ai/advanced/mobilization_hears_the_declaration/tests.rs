use super::*;

/// Two majors at war: our founded capital (population 4) building a
/// Monument, the rival's city 9-12 tiles off, every unit removed, Archery
/// known, turn 41.
fn board() -> (Game, u32) {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a Settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital");
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (9..=12).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(1, sites[0], None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    game.at_war.insert((0, 1));
    game.turn = 41;
    game.players[0]
        .techs
        .extend([crate::name!("animal_husbandry"), crate::name!("archery")]);
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 4;
    city.queue = vec![Item::Building {
        building: crate::name!("monument"),
    }];
    city.production = 0.0;
    (game, capital)
}

fn heard() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_surprise_war_mobilization();
    ai.enable_mobilization_hears_the_declaration();
    ai.observe_surprise_declarations(vec![(1, 40)]);
    ai
}

#[test]
fn a_heard_declaration_opens_the_window_without_a_war_record() {
    let (game, _) = board();
    assert!(game.wars.is_empty(), "the live mirror writes no war record");
    let window = heard()
        .surprise_defense_window(&game, 0)
        .expect("the heard declaration opens the window");
    assert_eq!((window.attacker, window.declared), (1, 40));
    assert_eq!(
        window.ends,
        40 + game.standard_duration(SURPRISE_MOBILIZATION_TURNS).max(1)
    );
}

#[test]
fn off_the_window_reads_only_the_war_ledger() {
    let (game, _) = board();
    let mut ai = AdvancedAi::new();
    ai.enable_surprise_war_mobilization();
    ai.observe_surprise_declarations(vec![(1, 40)]);
    assert_eq!(ai.surprise_defense_window(&game, 0), None);

    // The new tag alone does not arm the mobilization.
    let mut alone = AdvancedAi::new();
    alone.enable_mobilization_hears_the_declaration();
    alone.observe_surprise_declarations(vec![(1, 40)]);
    assert_eq!(alone.surprise_defense_window(&game, 0), None);
}

#[test]
fn peace_an_old_declaration_or_a_future_one_keeps_it_shut() {
    let (mut game, _) = board();
    let ai = heard();
    game.at_war.clear();
    assert_eq!(ai.surprise_defense_window(&game, 0), None);

    let (mut game, _) = board();
    game.turn = 40 + game.standard_duration(SURPRISE_MOBILIZATION_TURNS).max(1);
    assert_eq!(ai.surprise_defense_window(&game, 0), None);

    let (mut game, _) = board();
    game.turn = 39;
    assert_eq!(ai.surprise_defense_window(&game, 0), None);
}

#[test]
fn the_heard_window_redirects_a_peacetime_queue_to_a_defender() {
    let (mut game, capital) = board();
    assert_eq!(
        heard().mobilize_surprise_defense_production(&mut game, 0),
        1
    );
    let head = game.cities[&capital].queue.first().cloned();
    assert!(
        matches!(&head, Some(Item::Unit { .. })),
        "the Monument gives way to a land defender, got {head:?}"
    );

    let (mut game, capital) = board();
    let mut off = AdvancedAi::new();
    off.enable_surprise_war_mobilization();
    off.observe_surprise_declarations(vec![(1, 40)]);
    assert_eq!(off.mobilize_surprise_defense_production(&mut game, 0), 0);
    assert!(matches!(
        game.cities[&capital].queue.first(),
        Some(Item::Building { .. })
    ));
}
