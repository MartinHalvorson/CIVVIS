use super::*;
use crate::ai::industrial_zones::tests::producer_empire;

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_campus_building_claims_the_queue();
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

/// Every city's queue set to `item`.
fn all_queue(game: &mut Game, cities: &[u32], item: &Item) {
    for cid in cities {
        game.cities.get_mut(cid).unwrap().queue = vec![item.clone()];
    }
}

/// The six-city Emperor shape (production 40 down to 10, a Campus and Library
/// each) with Education in hand: every city can build its University.
fn with_education(tag: &str, seed: u64, players: usize) -> (Game, Vec<u32>) {
    let (mut game, ranked) = producer_empire(tag, seed, players);
    for tech in ["education", "mathematics"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    for cid in &ranked {
        assert_eq!(
            BasicAi::campus_building_item(&game, 0, *cid),
            Some(building("university")),
            "fixture: the University is buildable"
        );
    }
    (game, ranked)
}

/// The blocked case: every city training a routine Knight, the quickest city
/// puts its University first, one claim a turn, and the claim holds against
/// the rescoring; off, nothing moves and nothing holds.
#[test]
fn the_quickest_city_claims_its_university_over_a_routine_unit() {
    let (game, ranked) = with_education("CAMPUSCLAIM", 94_101, 1);
    let mut busy = game.clone();
    all_queue(&mut busy, &ranked, &unit("knight"));

    let stock = AdvancedAi::new();
    let mut off = busy.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_campus_building(&mut off, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&off, *cid) == Some(unit("knight"))),
        "off: no claim"
    );

    let ai = armed();
    let mut on = busy.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_campus_building(&mut on, 0, &plan);
    let claimed: Vec<u32> = ranked
        .iter()
        .copied()
        .filter(|cid| head(&on, *cid) == Some(building("university")))
        .collect();
    assert_eq!(
        claimed,
        vec![ranked[0]],
        "the most productive (quickest) city, one claim a turn"
    );
    let university = building("university");
    assert!(
        ai.campus_building_claim_holds(&on, 0, ranked[0], &plan, &university),
        "the claimed University holds"
    );
    assert!(
        !stock.campus_building_claim_holds(&on, 0, ranked[0], &plan, &university),
        "off: no hold"
    );
}

/// A Campus without its Library claims the Library first, ahead of a quicker
/// city's University.
#[test]
fn a_missing_library_comes_before_a_university() {
    let (mut game, ranked) = with_education("CAMPUSCLAIM", 94_102, 1);
    let slowest = *ranked.last().unwrap();
    game.cities
        .get_mut(&slowest)
        .unwrap()
        .buildings
        .retain(|held| *held != "library");
    all_queue(&mut game, &ranked, &unit("knight"));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_campus_building(&mut game, 0, &plan);
    assert_eq!(head(&game, slowest), Some(building("library")));
    assert!(
        ranked
            .iter()
            .filter(|cid| **cid != slowest)
            .all(|cid| head(&game, *cid) == Some(unit("knight"))),
        "one claim a turn, the Library first"
    );
}

/// A Settler and Walls at the head keep their queue: they are never deferred.
#[test]
fn a_settler_and_walls_keep_their_queue() {
    for item in [unit("settler"), building("walls")] {
        let (mut game, ranked) = with_education("CAMPUSCLAIM", 94_103, 1);
        all_queue(&mut game, &ranked, &item);
        let ai = armed();
        let plan = ai.assess(&game, 0);
        ai.claim_campus_building(&mut game, 0, &plan);
        assert!(
            ranked
                .iter()
                .all(|cid| head(&game, *cid) == Some(item.clone())),
            "{item:?} is never displaced"
        );
    }
}

/// At war with a major the military unit keeps its queue.
#[test]
fn at_war_a_unit_keeps_its_queue() {
    let (mut game, ranked) = with_education("CAMPUSCLAIM", 94_104, 2);
    game.at_war.insert((0, 1));
    all_queue(&mut game, &ranked, &unit("knight"));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_campus_building(&mut game, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&game, *cid) == Some(unit("knight"))),
        "a war unit stays"
    );
}

/// Without Education and with every Library standing, no Campus building is
/// due, so nothing moves.
#[test]
fn no_claim_without_a_due_campus_building() {
    let (mut game, ranked) = producer_empire("CAMPUSCLAIM", 94_105, 1);
    all_queue(&mut game, &ranked, &unit("knight"));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_campus_building(&mut game, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&game, *cid) == Some(unit("knight"))),
        "nothing due"
    );
}
