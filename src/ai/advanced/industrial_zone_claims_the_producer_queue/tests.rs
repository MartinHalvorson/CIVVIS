use super::*;
use crate::ai::industrial_zones::tests::producer_empire;

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_industrial_zone_claims_the_producer_queue();
    ai
}

fn head(game: &Game, cid: u32) -> Option<Item> {
    game.cities[&cid].queue.first().cloned()
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

fn is_zone(game: &Game, item: &Option<Item>) -> bool {
    matches!(item, Some(Item::District { district, .. })
        if game.district_family(*district) == "industrial_zone")
}

/// Every city's queue set to `item`.
fn all_queue(game: &mut Game, cities: &[u32], item: &Item) {
    for cid in cities {
        game.cities.get_mut(cid).unwrap().queue = vec![item.clone()];
    }
}

/// The blocked case on the six-city Emperor shape (production 40 down to
/// 10, Apprenticeship, a Campus and Library each): every city training a
/// routine Knight, the most productive one puts its Industrial Zone first,
/// one claim a turn, and the claim holds against the rescoring; off, nothing
/// moves and nothing holds.
#[test]
fn the_most_productive_city_claims_its_industrial_zone_over_a_routine_unit() {
    let (game, ranked) = producer_empire("ZONECLAIM", 93_101, 1);
    let mut busy = game.clone();
    // A Knight: a routine unit at peace that no city finishes this turn (a
    // Warrior finishes in the 50-Production city and is never deferred).
    all_queue(&mut busy, &ranked, &unit("knight"));

    let stock = AdvancedAi::new();
    let mut off = busy.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_producer_zone(&mut off, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&off, *cid) == Some(unit("knight"))),
        "off: no claim"
    );

    let ai = armed();
    let mut on = busy.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_producer_zone(&mut on, 0, &plan);
    let claimed: Vec<u32> = ranked
        .iter()
        .copied()
        .filter(|cid| is_zone(&on, &head(&on, *cid)))
        .collect();
    assert_eq!(
        claimed,
        vec![ranked[0]],
        "the most productive city, one claim a turn"
    );
    let zone = head(&on, ranked[0]).unwrap();
    assert!(
        ai.producer_zone_claim_holds(&on, 0, ranked[0], &plan, &zone),
        "the claimed zone holds"
    );
    assert!(
        !stock.producer_zone_claim_holds(&on, 0, ranked[0], &plan, &zone),
        "off: no hold"
    );
}

/// A Settler at the head keeps its queue: it is never deferred.
#[test]
fn a_settler_keeps_its_queue() {
    let (mut game, ranked) = producer_empire("ZONECLAIM", 93_102, 1);
    all_queue(&mut game, &ranked, &unit("settler"));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_producer_zone(&mut game, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&game, *cid) == Some(unit("settler"))),
        "a Settler is never displaced"
    );
}

/// The science items -b6's lane owns keep their queue: a University is never
/// deferred for the zone.
#[test]
fn a_university_keeps_its_queue() {
    let (mut game, ranked) = producer_empire("ZONECLAIM", 93_103, 1);
    all_queue(&mut game, &ranked, &building("university"));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_producer_zone(&mut game, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&game, *cid) == Some(building("university"))),
        "the University keeps its queue"
    );
}

/// Without Apprenticeship the producer step names no zone, so nothing moves.
#[test]
fn no_claim_before_apprenticeship() {
    let (mut game, ranked) = producer_empire("ZONECLAIM", 93_104, 1);
    game.players[0].techs.remove(&Name::new("apprenticeship"));
    all_queue(&mut game, &ranked, &unit("warrior"));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_producer_zone(&mut game, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&game, *cid) == Some(unit("warrior"))),
        "no zone before the technology"
    );
}

/// A wall breaker the siege reservation queued keeps its queue: a reserved
/// Trebuchet at the head is never deferred for the zone, though a routine
/// Knight in the same city is (see the first test).
#[test]
fn a_reserved_trebuchet_keeps_its_queue() {
    let (mut game, ranked) = producer_empire("ZONECLAIM", 93_105, 1);
    all_queue(&mut game, &ranked, &unit("trebuchet"));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_producer_zone(&mut game, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&game, *cid) == Some(unit("trebuchet"))),
        "the reserved Trebuchet is never displaced"
    );
}

#[test]
fn depleted_deficit_queues_wait_for_recovery_before_claiming_industry() {
    let mut unexpected_claims = Vec::new();
    for (workshop, gold, income) in [
        (false, 81.0, -7.84375),
        (true, 21.0, -10.8438),
        (true, 0.0, -11.1914),
    ] {
        let (mut game, ranked) = producer_empire("ZONECLAIM", 93_101, 1);
        if workshop {
            crate::game::install_test_district(&mut game, ranked[0], "industrial_zone");
        }
        let ai = armed();
        let funded_plan = ai.assess(&game, 0);
        let (item, _) = ai
            .producer_zone_claim_item(&game, 0, ranked[0], &funded_plan)
            .expect("the same funded city has an industrial investment");
        assert_eq!(
            matches!(&item, Item::Building { building } if building == "workshop"),
            workshop
        );
        game.players[0].gold = gold;
        game.players[0].gold_per_turn = income;
        assert!(game.can_produce(0, ranked[0], &item));
        assert!(ranked.iter().all(|cid| head(&game, *cid).is_none()));
        let plan = ai.assess(&game, 0);
        ai.claim_producer_zone(&mut game, 0, &plan);
        let claimed: Vec<_> = ranked
            .iter()
            .filter_map(|cid| head(&game, *cid).map(|item| (*cid, item)))
            .collect();
        if !claimed.is_empty() {
            unexpected_claims.push((workshop, gold, income, claimed));
        }
    }
    assert!(
        unexpected_claims.is_empty(),
        "recovery keeps every depleted queue: {unexpected_claims:?}"
    );
}

#[test]
fn an_industrial_claim_releases_its_hold_when_treasury_recovery_is_due() {
    let (mut game, ranked) = producer_empire("ZONECLAIM", 93_101, 1);
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_producer_zone(&mut game, 0, &plan);
    let item = head(&game, ranked[0]).expect("a funded city claims its zone");
    assert!(ai.producer_zone_claim_holds(&game, 0, ranked[0], &plan, &item));
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -11.1914;
    let recovery_plan = ai.assess(&game, 0);
    assert!(game.can_produce(0, ranked[0], &item));
    assert!(
        !ai.producer_zone_claim_holds(&game, 0, ranked[0], &recovery_plan, &item),
        "an insolvent industrial queue must lose the claim's protection"
    );
}

#[test]
fn funded_deficits_and_the_break_even_boundary_keep_the_industrial_claim() {
    let (original, ranked) = producer_empire("ZONECLAIM", 93_101, 1);
    let reserve = 100.0 + 25.0 * ranked.len() as f64;
    for (gold, income) in [(reserve, -11.0), (0.0, -0.5), (0.0, 0.0)] {
        let mut game = original.clone();
        game.players[0].gold = gold;
        game.players[0].gold_per_turn = income;
        let ai = armed();
        let plan = ai.assess(&game, 0);
        ai.claim_producer_zone(&mut game, 0, &plan);
        let item = head(&game, ranked[0]).expect("the existing recovery boundary still invests");
        assert!(is_zone(&game, &Some(item.clone())));
        assert!(ai.producer_zone_claim_holds(&game, 0, ranked[0], &plan, &item));
    }
}
