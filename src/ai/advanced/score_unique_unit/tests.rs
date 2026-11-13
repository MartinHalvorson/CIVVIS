use super::*;

#[test]
fn score_seat_fields_its_unique_unit_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "score-seat-fields-its-unique-unit",
        |ai| ai.score_seat_fields_its_unique_unit,
    );
}

/// One major playing `civ`, with a founded capital and a second city, on
/// turn 15.
fn two_cities(civ: &str) -> Game {
    let mut game = Game::new_full(1, 32, 20, 936_221, 200, 0, false);
    game.players[0].civ = civ.to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let sites: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for site in sites {
        if game.wdist(site, home) < 4 || game.wdist(site, home) > 6 {
            continue;
        }
        let settler = game.spawn_unit("settler", 0, site);
        if game.apply(0, &Action::FoundCity { unit: settler }).is_ok() {
            break;
        }
        game.remove_unit(settler);
    }
    assert_eq!(game.player_city_ids(0).len(), 2, "fixture: a second city");
    game.turn = 15;
    game
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_score_seat_fields_its_unique_unit();
    }
    ai
}

/// Rome's early unique soldier is the Legion, behind Iron Working.
#[test]
fn romes_early_unique_soldier_is_the_legion() {
    let game = two_cities("Rome");
    assert_eq!(
        AdvancedAi::early_unique_soldier(&game, 0),
        Some((crate::name!("legion"), crate::name!("iron_working")))
    );
}

/// The goal is Iron Working on a Score seat with the gene on; off, on
/// another target, and with one city it is nothing.
#[test]
fn iron_working_is_the_goal_from_the_second_city() {
    let game = two_cities("Rome");
    let goal = Some(crate::name!("iron_working"));
    assert_eq!(
        seat(VictoryTarget::Score, true).score_unique_unit_research_goal(&game, 0),
        goal
    );
    assert_eq!(
        seat(VictoryTarget::Score, false).score_unique_unit_research_goal(&game, 0),
        None,
        "off"
    );
    assert_eq!(
        seat(VictoryTarget::Science, true).score_unique_unit_research_goal(&game, 0),
        None,
        "another target"
    );
    let mut one = game.clone();
    let second = one.player_city_ids(0)[1];
    one.cities.remove(&second);
    if one.player_city_ids(0).len() == 1 {
        assert_eq!(
            seat(VictoryTarget::Score, true).score_unique_unit_research_goal(&one, 0),
            None,
            "one city: the opening keeps its research"
        );
    }
}

/// Known, the goal ends; and it ends unmet once the Man-at-Arms can be
/// trained, because the host then hides the Legion.
#[test]
fn the_goal_ends_when_known_or_when_the_window_shuts() {
    let ai = seat(VictoryTarget::Score, true);
    let mut known = two_cities("Rome");
    known.players[0].techs.insert(crate::name!("iron_working"));
    assert_eq!(ai.score_unique_unit_research_goal(&known, 0), None);
    let mut shut = two_cities("Rome");
    let upgrade = shut.rules.units[&crate::name!("legion")]
        .upgrade_to
        .and_then(|unit| shut.rules.units[&unit].tech)
        .expect("fixture: the Legion's upgrade has a technology");
    shut.players[0].techs.insert(upgrade);
    assert_eq!(ai.score_unique_unit_research_goal(&shut, 0), None);
}
