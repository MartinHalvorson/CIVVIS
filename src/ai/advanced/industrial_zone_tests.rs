//! `industrial-zone-in-the-producers` through the strategic controller: the
//! native opt-in, the delegated governor's lent threat, the strategic
//! governor's idle queue and review, and the culture-defense Theater
//! reservation. See `BasicAi::industrial_zone_in_the_producers`.
use super::*;
use crate::ai::industrial_zones::tests::{is_zone, producer_empire};
use crate::ai::industrial_zones::PRODUCER_ZONE_MAX;

#[test]
fn industrial_zone_in_the_producers_is_a_native_opt_in_off_in_both_controllers() {
    super::test_support::opt_in_off_in_both_controllers("industrial-zone-in-the-producers", |ai| {
        assert_eq!(
            ai.industrial_zone_in_the_producers, ai.base.industrial_zone_in_the_producers,
            "the controller and its delegated governor agree"
        );
        ai.industrial_zone_in_the_producers
    });
}

fn plan(strategy: GrandStrategy, threatened: Option<u32>) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: threatened,
        desired_cities: 6,
        assessed_turn: 90,
        rush: false,
    }
}

fn controller(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.book_pos = 4;
    ai.base.w.city_target = 6.0;
    if gene {
        ai.enable_industrial_zone_in_the_producers();
    }
    ai
}

/// Through the delegated governor the most productive city opens its zone
/// under the gene, the plan's threatened city keeps the stock queue, and the
/// threat is lent for the call only.
#[test]
fn the_delegated_governor_opens_the_zone_but_not_in_the_threatened_city() {
    let (game, ranked) = producer_empire("PRODUCERDELEGATED", 93_101, 2);
    let top = ranked[0];
    let run = |gene: bool, threatened: Option<u32>| {
        let mut game = game.clone();
        let mut ai = controller(gene);
        ai.delegated_cities(&mut game, 0, &plan(GrandStrategy::Expansion, threatened));
        assert_eq!(ai.base.plan_threatened_city, None, "the lend ends with the call");
        game.cities[&top].queue.first().cloned()
    };
    let stock = run(false, None);
    assert!(stock.is_some(), "the stock governor builds something");
    assert!(!is_zone(&game, &stock), "fixture: the stock queue is no zone: {stock:?}");
    let gene = run(true, None);
    assert!(is_zone(&game, &gene), "the gene opens the zone: {gene:?}");
    assert_eq!(run(true, Some(top)), run(false, Some(top)), "threatened: the stock queue");
}

/// The strategic governor's idle queue: under the gene the most productive
/// idle city starts its zone before the scorer fills it, and a queued zone
/// with nothing invested yet survives the review.
#[test]
fn the_strategic_governor_starts_and_keeps_the_zone() {
    let (game, ranked) = producer_empire("PRODUCERSTRATEGIC", 93_103, 2);
    let top = ranked[0];
    let run = |gene: bool, game: &Game| {
        let mut game = game.clone();
        let mut ai = controller(gene);
        ai.advanced_production(&mut game, 0, &plan(GrandStrategy::Conquest, None), false);
        game.cities[&top].queue.first().cloned()
    };
    let gene = run(true, &game);
    assert!(is_zone(&game, &gene), "the gene starts the zone: {gene:?}");
    let stock = run(false, &game);
    assert_ne!(stock, gene, "fixture: the scorer picks something else");
    // A zone queued with nothing invested: the scorer's review keeps it under
    // the gene.
    let mut queued = game.clone();
    queued.cities.get_mut(&top).unwrap().queue = vec![gene.clone().unwrap()];
    assert_eq!(run(true, &queued), gene, "the review keeps the queued zone");
    // A city attacked a turn ago no longer holds it for the review.
    let mut threatened = queued.clone();
    threatened.cities.get_mut(&top).unwrap().last_attacked = 89;
    assert!(!controller(true).base.industrial_zone_build_holds(
        &threatened,
        top,
        gene.as_ref().unwrap(),
        None
    ));
}

/// The culture-defense Theater reservation claims the most productive idle
/// Campus city; under the gene it passes over the producers due their zone
/// and claims the next idle city.
#[test]
fn the_theater_reservation_leaves_the_producers_slot_to_the_zone() {
    let (mut game, ranked) = producer_empire("PRODUCERTHEATER", 93_105, 2);
    // Each city that can gets one more bare land plot within three tiles (no
    // feature, resource or improvement, so its housing and yields stand), so
    // more cities than the producers have a district site.
    for cid in &ranked {
        let center = game.cities[cid].pos;
        let Some(site) = game
            .wdisk(center, 3)
            .into_iter()
            .filter(|position| *position != center)
            .find(|position| {
                game.map.tiles.get(position).is_some_and(|tile| {
                    tile.owner_city.is_none()
                        && tile.district.is_none()
                        && tile.wonder.is_none()
                        && tile.feature.is_none()
                        && tile.resource.is_none()
                        && tile.improvement.is_none()
                        && game.rules.is_passable(tile)
                        && !game.rules.is_water(tile)
                })
            })
        else {
            continue;
        };
        game.map.tiles.get_mut(&site).unwrap().owner_city = Some(*cid);
        game.cities.get_mut(cid).unwrap().owned_tiles.push(site);
    }
    game.players[0].civics.insert(crate::name!("drama_poetry"));
    std::sync::Arc::make_mut(&mut game.observed_yield_adjustments).insert(
        1,
        crate::rules::Yields {
            culture: 100.0,
            ..Default::default()
        },
    );
    let producers: Vec<u32> = ranked
        .iter()
        .copied()
        .filter(|cid| {
            controller(true)
                .base
                .industrial_zone_producer_item(&game, 0, *cid, 6, None)
                .is_some()
        })
        .collect();
    assert_eq!(producers.len(), PRODUCER_ZONE_MAX, "fixture: three producers");
    let claimed = |gene: bool| {
        let mut game = game.clone();
        let mut ai = controller(gene);
        ai.enable_culture_defense_theater_2();
        ai.reserve_culture_defense_theater(&mut game, 0, &plan(GrandStrategy::Conquest, None));
        ranked
            .iter()
            .copied()
            .find(|cid| {
                matches!(game.cities[cid].queue.first(), Some(Item::District { district, .. })
                    if game.district_family(*district) == "theater_square")
            })
    };
    let stock = claimed(false);
    assert!(
        stock.is_some_and(|cid| producers.contains(&cid)),
        "fixture: the most productive idle Campus city is a producer: {stock:?}"
    );
    let gene = claimed(true);
    assert!(
        gene.is_some_and(|cid| !producers.contains(&cid)),
        "the Theater goes past the producers: {gene:?}"
    );
}
