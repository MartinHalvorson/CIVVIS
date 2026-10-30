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
