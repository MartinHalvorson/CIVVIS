use super::*;
use crate::ai::advanced::tests::{strategic_wonder_fixture, wonder_plan};

/// The Mahabodhi Temple (two points) a Diplomatic seat can finish, with
/// most of its cost already in so the 30-turn bar is about the claim, not
/// the fixture's Production.
fn mahabodhi_board() -> (Game, u32, Item) {
    let (mut game, city) = strategic_wonder_fixture(6_401, "mahabodhi_temple");
    let item = game
        .producible_items(0, city)
        .into_iter()
        .find(|item| matches!(item, Item::Wonder { wonder, .. } if *wonder == "mahabodhi_temple"))
        .expect("fixture: the Mahabodhi Temple is producible");
    let cost = game.item_cost_for_city(0, city, &item);
    let key = Game::item_progress_key(&item);
    game.cities
        .get_mut(&city)
        .unwrap()
        .production_progress
        .insert(key, cost * 0.95);
    (game, city, item)
}

#[test]
fn a_diplomatic_seat_claims_a_diplomatic_point_wonder() {
    let (game, city, mahabodhi) = mahabodhi_board();
    let plan = wonder_plan(GrandStrategy::Diplomacy, game.turn);
    let mut seat = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    assert!(
        seat.diplo_wonder_item(&game, 0, city, &plan).is_none(),
        "off: nothing"
    );
    seat.enable_diplo_seat_wonder_claim();
    let (item, points, turns) = seat
        .diplo_wonder_item(&game, 0, city, &plan)
        .expect("the Diplomatic seat wants the Mahabodhi Temple");
    assert_eq!(item, mahabodhi);
    assert_eq!(points, 2.0);
    assert!(turns <= DIPLO_WONDER_MAX_TURNS);

    let mut claimed = game.clone();
    seat.claim_diplo_wonder(&mut claimed, 0, &plan);
    assert_eq!(claimed.cities[&city].queue.first(), Some(&mahabodhi));
    assert!(seat.diplo_wonder_holds(&claimed, 0, city, &plan, &mahabodhi));
    // One lane: with a Diplomatic-point wonder at a head, nothing more.
    let before = claimed.clone();
    seat.claim_diplo_wonder(&mut claimed, 0, &plan);
    for cid in claimed.player_city_ids(0) {
        assert_eq!(
            claimed.cities[&cid].queue.first(),
            before.cities[&cid].queue.first()
        );
    }

    // Another lane: nothing, and nothing held.
    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_diplo_seat_wonder_claim();
    assert!(culture.diplo_wonder_item(&game, 0, city, &plan).is_none());
    assert!(!culture.diplo_wonder_holds(&claimed, 0, city, &plan, &mahabodhi));

    // A threatened city does not claim.
    let threatened = StrategicPlan {
        threatened_city: Some(city),
        ..plan.clone()
    };
    assert!(seat
        .diplo_wonder_item(&game, 0, city, &threatened)
        .is_none());

    // A lost lane releases it.
    let mut lost = seat.clone();
    lost.lane_lost = true;
    assert!(lost.diplo_wonder_item(&game, 0, city, &plan).is_none());
}

/// A wonder that pays no Diplomatic points is not this gene's.
/// `science-seat-builds-the-statue`: a Science seat under the tag passes over
/// a lesser wonder, and its claim survives a lost Science race. The Statue's
/// four points clear the bar; two do not.
#[test]
fn a_science_seat_claims_only_the_four_point_wonder() {
    let (game, city, _) = mahabodhi_board();
    let plan = wonder_plan(GrandStrategy::Science, game.turn);
    let mut seat = AdvancedAi::targeting(VictoryTarget::Science);
    seat.enable_diplo_seat_wonder_claim();
    assert!(!seat.diplo_wonder_claim_on(&game), "off without the tag");
    seat.enable_science_seat_builds_the_statue();
    seat.lane_lost = true;
    assert!(
        seat.diplo_wonder_claim_on(&game),
        "a lost race keeps the claim"
    );
    assert!(
        seat.diplo_wonder_item(&game, 0, city, &plan).is_none(),
        "the Mahabodhi Temple's two points stay with the science race"
    );
    assert!(seat.diplo_wonder_worth_it(&game, SCIENCE_SEAT_WONDER_POINTS));
    assert!(!seat.diplo_wonder_worth_it(&game, SCIENCE_SEAT_WONDER_POINTS - 1.0));

    let mut diplomat = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    diplomat.enable_diplo_seat_wonder_claim();
    assert!(
        diplomat.diplo_wonder_worth_it(&game, 1.0),
        "a Diplomacy seat still takes the Potala"
    );
    diplomat.lane_lost = true;
    assert!(
        !diplomat.diplo_wonder_claim_on(&game),
        "a Diplomacy seat's lost lane still stops it"
    );
}

#[test]
fn a_wonder_without_points_is_not_claimed() {
    let (game, city) = strategic_wonder_fixture(6_402, "pyramids");
    let plan = wonder_plan(GrandStrategy::Diplomacy, game.turn);
    let mut seat = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    seat.enable_diplo_seat_wonder_claim();
    if let Some((item, points, _)) = seat.diplo_wonder_item(&game, 0, city, &plan) {
        assert!(points > 0.0, "{item:?}");
        assert!(!matches!(&item, Item::Wonder { wonder, .. } if *wonder == "pyramids"));
    }
}

/// The soonest finish ranks ahead of the most Production: a city with the
/// wonder half built finishes first even with less Production.
#[test]
fn the_soonest_finish_claims_ahead_of_the_strongest_city() {
    // (points, turns, production, city)
    let strong_fresh = (4.0, 25.0, 50.0, 7);
    let weaker_sooner = (4.0, 14.0, 30.0, 9);
    assert!(diplo_wonder_ranks_before(weaker_sooner, strong_fresh));
    assert!(!diplo_wonder_ranks_before(strong_fresh, weaker_sooner));
    // Points still come first: a two-point wonder sooner loses to four later.
    let mahabodhi_sooner = (2.0, 5.0, 60.0, 3);
    assert!(diplo_wonder_ranks_before(strong_fresh, mahabodhi_sooner));
    // Equal finish: the stronger city, then the lower id.
    assert!(diplo_wonder_ranks_before(
        (4.0, 14.0, 40.0, 9),
        (4.0, 14.0, 30.0, 2)
    ));
    assert!(diplo_wonder_ranks_before(
        (4.0, 14.0, 30.0, 2),
        (4.0, 14.0, 30.0, 9)
    ));
}

/// The Mahabodhi board with the wonder raised to the Statue of Liberty's four
/// points and all but a sliver of its cost in: a finisher at its last turn.
fn finisher_board() -> (Game, u32, Item) {
    let (mut game, city, item) = mahabodhi_board();
    std::sync::Arc::make_mut(&mut game.rules)
        .wonders
        .get_mut("mahabodhi_temple")
        .unwrap()
        .effects
        .insert("diplomatic_victory_points".to_string(), 4.0);
    let cost = game.item_cost_for_city(0, city, &item);
    let key = Game::item_progress_key(&item);
    game.cities
        .get_mut(&city)
        .unwrap()
        .production_progress
        .insert(key, cost - 0.25);
    (game, city, item)
}

fn finisher_seat() -> AdvancedAi {
    let mut seat = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    seat.enable_diplo_seat_wonder_claim();
    seat.enable_diplo_seat_statue_is_the_finisher();
    seat
}

#[test]
fn diplo_seat_statue_is_the_finisher_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "diplo-seat-statue-is-the-finisher",
        |ai| ai.diplo_seat_statue_is_the_finisher,
    );
}

/// `diplo-seat-statue-is-the-finisher`: at twelve points a four-point wonder
/// one turn from done is held: not offered by the claim, not kept at a head, and
/// replaced there by the last writer. At sixteen it is the win: nothing is
/// held, and the last writer puts it back at the head. Without the tag the
/// claim takes it and keeps it, as before.
#[test]
fn a_finisher_at_its_last_turn_waits_for_the_points_that_win() {
    let (mut game, city, statue) = finisher_board();
    let plan = wonder_plan(GrandStrategy::Diplomacy, game.turn);
    game.players[0].dvp = 12;

    // Without the tag: claimed and kept.
    let mut plain = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    plain.enable_diplo_seat_wonder_claim();
    assert_eq!(plain.diplo_finisher_held(&game, 0, city), None);
    let mut claimed = game.clone();
    plain.claim_diplo_wonder(&mut claimed, 0, &plan);
    assert_eq!(claimed.cities[&city].queue.first(), Some(&statue));
    assert!(plain.diplo_wonder_holds(&claimed, 0, city, &plan, &statue));
    plain.hold_the_diplo_finisher(&mut claimed, 0);
    assert_eq!(claimed.cities[&city].queue.first(), Some(&statue));

    // With it, at twelve points: held.
    let mut seat = finisher_seat();
    assert_eq!(
        seat.diplo_finisher_held(&game, 0, city),
        Some(statue.clone())
    );
    assert!(
        !seat.diplo_wonder_holds(&claimed, 0, city, &plan, &statue),
        "a held finisher at a head is not kept there"
    );
    let mut held = game.clone();
    assert!(
        !matches!(seat.diplo_wonder_item(&held, 0, city, &plan), Some((item, _, _)) if item == statue),
        "the claim does not offer a held finisher"
    );
    seat.claim_diplo_wonder(&mut held, 0, &plan);
    assert_ne!(held.cities[&city].queue.first(), Some(&statue));
    // Already at the head (the host was building it): the last writer of the
    // turn builds something else there, and the wonder keeps its progress.
    let mut at_head = claimed.clone();
    seat.hold_the_diplo_finisher(&mut at_head, 0);
    let head = at_head.cities[&city].queue.first().cloned();
    assert!(head.is_some(), "the city is not left idle");
    assert_ne!(head.as_ref(), Some(&statue));
    assert!(!matches!(head, Some(Item::Wonder { .. })));
    assert_eq!(
        seat.diplo_finisher_held(&at_head, 0, city),
        Some(statue.clone()),
        "still one turn from done"
    );

    // Sixteen points: the four make twenty. Nothing is held, and the last
    // writer puts the wonder at the head over whatever stands there.
    at_head.players[0].dvp = 16;
    assert_eq!(seat.diplo_finisher_held(&at_head, 0, city), None);
    seat.hold_the_diplo_finisher(&mut at_head, 0);
    assert_eq!(at_head.cities[&city].queue.first(), Some(&statue));
    assert!(seat.diplo_wonder_holds(&at_head, 0, city, &plan, &statue));
}

/// A finisher many turns from done is built as before, and only a seat whose
/// ballot races waits: a Science seat that builds the Statue without
/// `science-seat-races-the-congress` lands it as soon as it can.
#[test]
fn only_a_racing_seat_holds_and_only_at_the_last_turn() {
    let (mut near, city, statue) = finisher_board();
    near.players[0].dvp = 12;
    let key = Game::item_progress_key(&statue);
    let mut far = near.clone();
    far.cities
        .get_mut(&city)
        .unwrap()
        .production_progress
        .remove(&key);
    let seat = finisher_seat();
    assert!(seat.diplo_finisher_held(&near, 0, city).is_some());
    assert_eq!(
        seat.diplo_finisher_held(&far, 0, city),
        None,
        "nothing in, many turns from done: not held"
    );

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_diplo_seat_wonder_claim();
    science.enable_science_seat_builds_the_statue();
    science.enable_diplo_seat_statue_is_the_finisher();
    assert_eq!(science.diplo_finisher_held(&near, 0, city), None);
    science.enable_science_seat_races_the_congress();
    assert_eq!(science.diplo_finisher_held(&near, 0, city), Some(statue));

    let mut lost = finisher_seat();
    lost.lane_lost = true;
    assert_eq!(lost.diplo_finisher_held(&near, 0, city), None);
}

/// `diplo-seat-statue-is-the-finisher`: a finisher is claimed whatever its
/// build time. Live Deity 2026-11-18 the host offered the Statue at 57 turns
/// (Gaul) and 33 (Hungary) and the 30-turn bar left it unstarted for 23 and
/// 12 turns. A lesser wonder still waits for the bar, and nothing is started
/// that the game's own clock leaves no room for.
#[test]
fn a_finisher_is_started_past_the_thirty_turn_bar() {
    let (mut far, city, statue) = finisher_board();
    let plan = wonder_plan(GrandStrategy::Diplomacy, far.turn);
    far.players[0].dvp = 8;
    let key = Game::item_progress_key(&statue);
    far.cities
        .get_mut(&city)
        .unwrap()
        .production_progress
        .remove(&key);

    let mut plain = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    plain.enable_diplo_seat_wonder_claim();
    assert!(
        plain.diplo_wonder_item(&far, 0, city, &plan).is_none(),
        "fixture: with nothing in, the wonder is over the 30-turn bar"
    );

    let seat = finisher_seat();
    let (item, points, turns) = seat
        .diplo_wonder_item(&far, 0, city, &plan)
        .expect("a finisher is claimed whatever its build time");
    assert_eq!(item, statue);
    assert_eq!(points, 4.0);
    assert!(turns > DIPLO_WONDER_MAX_TURNS);
    let mut claimed = far.clone();
    seat.claim_diplo_wonder(&mut claimed, 0, &plan);
    assert!(
        claimed.player_city_ids(0).into_iter().any(|cid| matches!(
            claimed.cities[&cid].queue.first(),
            Some(Item::Wonder { wonder, .. }) if *wonder == "mahabodhi_temple"
        )),
        "the finisher is started past the bar"
    );
    let mut unclaimed = far.clone();
    plain.claim_diplo_wonder(&mut unclaimed, 0, &plan);
    assert!(
        !unclaimed.player_city_ids(0).into_iter().any(|cid| matches!(
            unclaimed.cities[&cid].queue.first(),
            Some(Item::Wonder { wonder, .. }) if *wonder == "mahabodhi_temple"
        )),
        "without the tag the bar leaves it unstarted"
    );

    // Past the bar it is started only where it finishes soonest, or nearly:
    // live Gaul, replayed at t181, placed the Statue in a 50-turn city on the
    // turn its 32-turn city was finishing a Shipyard.
    assert!(diplo_finisher_starts_here(46.0, Some(46.0)));
    assert!(diplo_finisher_starts_here(40.0, Some(32.0)));
    assert!(!diplo_finisher_starts_here(50.0, Some(32.0)));
    assert!(
        diplo_finisher_starts_here(24.0, Some(12.0)),
        "under the bar"
    );
    assert!(diplo_finisher_starts_here(57.0, None));

    // Two points are not a finisher: the bar stands.
    let mut lesser = far.clone();
    std::sync::Arc::make_mut(&mut lesser.rules)
        .wonders
        .get_mut("mahabodhi_temple")
        .unwrap()
        .effects
        .insert("diplomatic_victory_points".to_string(), 2.0);
    assert!(seat.diplo_wonder_item(&lesser, 0, city, &plan).is_none());

    // Five turns to the limit: nothing that slow is started.
    let mut late = far.clone();
    late.max_turns = late.turn + 5;
    assert!(seat.diplo_wonder_item(&late, 0, city, &plan).is_none());
}

/// `diplo-seat-statue-is-the-finisher`: a held finisher is completed when
/// the game is about to end anyway, on the turn limit or on a rival's
/// expedition by its own trip, and not before: an expedition twenty turns
/// out leaves a session in which the points on the board are voted away.
#[test]
fn a_held_finisher_is_finished_when_the_game_is_ending() {
    let (mut game, city, statue) = finisher_board();
    game.players[0].dvp = 12;
    let mut seat = finisher_seat();
    assert_eq!(seat.diplo_finisher_release(&game, 0, 4.0), None);
    assert_eq!(
        seat.diplo_finisher_held(&game, 0, city),
        Some(statue.clone())
    );
    assert!(seat.diplo_finisher_release(&game, 0, 8.0).is_some());

    // The turn limit.
    let mut clock = game.clone();
    clock.max_turns = clock.turn + DIPLO_FINISHER_LAST_TURNS as u32;
    assert!(seat.diplo_finisher_release(&clock, 0, 4.0).is_some());
    assert_eq!(seat.diplo_finisher_held(&clock, 0, city), None);
    clock.max_turns += 1;
    assert_eq!(seat.diplo_finisher_release(&clock, 0, 4.0), None);

    // A rival's expedition, by its trip.
    let mut race = game.clone();
    race.players[1]
        .science_projects
        .insert("exoplanet_expedition".to_string());
    race.players[1].exoplanet_distance = race.exoplanet_destination() - 20.0;
    assert_eq!(
        seat.diplo_finisher_release(&race, 0, 4.0),
        None,
        "twenty turns out: still held"
    );
    assert_eq!(
        seat.diplo_finisher_held(&race, 0, city),
        Some(statue.clone())
    );
    race.players[1].exoplanet_distance = race.exoplanet_destination() - DIPLO_FINISHER_LAST_TURNS;
    assert!(seat.diplo_finisher_release(&race, 0, 4.0).is_some());
    assert_eq!(seat.diplo_finisher_held(&race, 0, city), None);
    // The last writer puts it at the head over whatever stands there.
    let other = race
        .producible_items(0, city)
        .into_iter()
        .find(|item| matches!(item, Item::Unit { .. }))
        .expect("fixture: the city can train a unit");
    race.apply(
        0,
        &Action::Produce {
            city,
            item: other.clone(),
        },
    )
    .expect("fixture: the unit is queued");
    assert_eq!(race.cities[&city].queue.first(), Some(&other));
    seat.hold_the_diplo_finisher(&mut race, 0);
    assert_eq!(race.cities[&city].queue.first(), Some(&statue));
}

/// A finisher held in one city is not offered to another city of ours: the
/// claim would start a second copy of the wonder that is waiting.
#[test]
fn a_held_finisher_is_not_offered_anywhere() {
    let (mut game, city, statue) = finisher_board();
    let plan = wonder_plan(GrandStrategy::Diplomacy, game.turn);
    game.players[0].dvp = 12;
    let seat = finisher_seat();
    assert_eq!(seat.diplo_finisher_held(&game, 0, city), Some(statue));
    for cid in game.player_city_ids(0) {
        assert!(
            !matches!(
                seat.diplo_wonder_item(&game, 0, cid, &plan),
                Some((Item::Wonder { wonder, .. }, _, _)) if wonder == "mahabodhi_temple"
            ),
            "city {cid} is offered the held finisher"
        );
    }
}
