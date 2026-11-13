use super::*;
use crate::ai::advanced::tests::{strategic_wonder_fixture, wonder_plan};

fn project(name: &str) -> Item {
    Item::Project {
        project: Name::new(name),
    }
}

fn competition(game: &Game, kind: &str, left: u32) -> crate::game::HostCompetition {
    crate::game::HostCompetition {
        kind: kind.to_string(),
        ends: game.turn + left,
        ours: 0.0,
        leader: 60.0,
    }
}

/// The wonder claim's own empire with the seat a member of a World Games
/// that has `left` turns to run (a thousand where the fixture's one-citizen
/// cities must be able to finish a project).
fn games_board(left: u32) -> Game {
    let (mut game, _) = strategic_wonder_fixture(6_401, "mahabodhi_temple");
    let games = competition(&game, "EMERGENCY_WORLD_GAMES", left);
    game.replace_host_competitions(vec![games]);
    game
}

fn offers(seat: &AdvancedAi, game: &Game, plan: &StrategicPlan) -> Vec<(u32, Item, f64)> {
    game.player_city_ids(0)
        .into_iter()
        .filter_map(|cid| {
            seat.diplo_competition_item(game, 0, cid, plan)
                .map(|(item, points, _)| (cid, item, points))
        })
        .collect()
}

fn holders(game: &Game) -> Vec<u32> {
    game.player_city_ids(0)
        .into_iter()
        .filter(|cid| {
            game.cities[cid]
                .queue
                .first()
                .is_some_and(competition_project)
        })
        .collect()
}

/// One city is lent to the competition, the claim holds while the
/// competition runs, and it is made once.
#[test]
fn a_diplomatic_seat_claims_the_competitions_project_in_one_city() {
    let game = games_board(1_000);
    let plan = wonder_plan(GrandStrategy::Diplomacy, game.turn);
    let mut seat = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    assert!(offers(&seat, &game, &plan).is_empty(), "off: nothing");
    let mut untouched = game.clone();
    seat.claim_diplo_competition(&mut untouched, 0, &plan);
    assert!(holders(&untouched).is_empty(), "off: no claim");

    seat.enable_diplo_seat_claims_the_competition();
    let offered = offers(&seat, &game, &plan);
    assert!(
        !offered.is_empty(),
        "fixture: some city can train athletes in time"
    );
    for (_, item, points) in &offered {
        assert_eq!(item, &project("train_athletes"));
        assert_eq!(*points, 1.0, "the World Games pay one point");
    }

    let mut claimed = game.clone();
    seat.claim_diplo_competition(&mut claimed, 0, &plan);
    let lent = holders(&claimed);
    assert_eq!(lent.len(), 1, "one city is lent to the competition");
    let athletes = project("train_athletes");
    assert!(seat.diplo_competition_holds(&claimed, 0, lent[0], &plan, &athletes));

    // One lane: with the project at a head, nothing more is claimed.
    let before = claimed.clone();
    seat.claim_diplo_competition(&mut claimed, 0, &plan);
    for cid in claimed.player_city_ids(0) {
        assert_eq!(
            claimed.cities[&cid].queue.first(),
            before.cities[&cid].queue.first()
        );
    }

    // The competition over, the project is no longer held.
    claimed.replace_host_competitions(Vec::new());
    assert!(!seat.diplo_competition_holds(&claimed, 0, lent[0], &plan, &athletes));
}

/// Another target, a lost lane and a threatened city claim nothing; a
/// competition too close to its end offers nothing.
#[test]
fn only_an_unthreatened_diplomatic_seat_with_time_left_claims() {
    let game = games_board(1_000);
    let plan = wonder_plan(GrandStrategy::Diplomacy, game.turn);
    let mut seat = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    seat.enable_diplo_seat_claims_the_competition();
    let offered = offers(&seat, &game, &plan);
    assert!(!offered.is_empty(), "fixture");

    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_diplo_seat_claims_the_competition();
    assert!(offers(&culture, &game, &plan).is_empty(), "a Culture seat");
    let mut lost = seat.clone();
    lost.lane_lost = true;
    assert!(offers(&lost, &game, &plan).is_empty(), "a lost lane");

    let city = offered[0].0;
    let threatened = StrategicPlan {
        threatened_city: Some(city),
        ..plan.clone()
    };
    assert!(
        seat.diplo_competition_item(&game, 0, city, &threatened)
            .is_none(),
        "a threatened city is not lent"
    );
    assert!(
        !seat.diplo_competition_holds(&game, 0, city, &threatened, &project("train_athletes")),
        "nor held"
    );

    // No turns left to finish a project in: nothing is offered or claimed.
    let mut ending = games_board(0);
    assert!(offers(&seat, &ending, &plan).is_empty());
    seat.claim_diplo_competition(&mut ending, 0, &plan);
    assert!(holders(&ending).is_empty());
}

/// An Aid Request pays two points and the World Games one: with both
/// running the city is lent to Send Aid.
#[test]
fn the_aid_request_outranks_the_world_games() {
    let mut game = games_board(1_000);
    let aid = competition(&game, "EMERGENCY_SEND_AID", 1_000);
    let games = competition(&game, "EMERGENCY_WORLD_GAMES", 1_000);
    game.replace_host_competitions(vec![aid, games]);
    let plan = wonder_plan(GrandStrategy::Diplomacy, game.turn);
    let mut seat = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    seat.enable_diplo_seat_claims_the_competition();
    let offered = offers(&seat, &game, &plan);
    assert!(!offered.is_empty(), "fixture");
    for (_, item, points) in &offered {
        assert_eq!(item, &project("send_aid"));
        assert_eq!(*points, 2.0);
    }
    seat.claim_diplo_competition(&mut game, 0, &plan);
    let lent = holders(&game);
    assert_eq!(lent.len(), 1);
    assert_eq!(
        game.cities[&lent[0]].queue.first(),
        Some(&project("send_aid"))
    );
}

/// Only the three Production-for-score projects: a decommissioning (our own
/// power plant, for the Climate Accords) and an ordinary build are not the
/// claim's.
#[test]
fn only_the_three_score_projects_are_claimed() {
    for name in ["send_aid", "train_athletes", "train_astronauts"] {
        assert!(competition_project(&project(name)), "{name}");
    }
    for name in [
        "decommission_coal_power_plant",
        "decommission_oil_power_plant",
        "decommission_nuclear_power_plant",
        "carbon_recapture",
    ] {
        assert!(!competition_project(&project(name)), "{name}");
    }
    assert!(!competition_project(&Item::Building {
        building: crate::name!("granary"),
    }));
}
