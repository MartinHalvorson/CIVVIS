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
