use super::*;
use crate::ai::industrial_zones::tests::producer_empire;
use crate::game::install_test_district;

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_industrial_zone_claims_the_factory();
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

/// The six-city producer empire (production 40 down to 10) with
/// Industrialization and an Industrial Zone and Workshop in its three most
/// productive cities: every one of them can build a Factory.
fn industrial_empire(seed: u64) -> (Game, Vec<u32>) {
    let (mut game, ranked) = producer_empire("FACTORYCLAIM", seed, 1);
    game.players[0]
        .techs
        .insert(crate::name!("industrialization"));
    for cid in ranked.iter().take(3) {
        install_test_district(&mut game, *cid, "industrial_zone");
        game.cities
            .get_mut(cid)
            .unwrap()
            .buildings
            .push(crate::name!("workshop"));
    }
    for cid in ranked.iter().take(3) {
        assert!(
            game.can_produce(0, *cid, &building("factory")),
            "fixture: a Factory is legal in a zone city"
        );
    }
    (game, ranked)
}

/// Each city whose head is a Factory.
fn factory_heads(game: &Game, cities: &[u32]) -> Vec<u32> {
    cities
        .iter()
        .copied()
        .filter(|cid| head(game, *cid) == Some(building("factory")))
        .collect()
}

/// Every city training a routine Knight: the zone city whose Factory reaches
/// the most of our cities puts it first, one claim a turn, and the claim holds
/// against the rescoring; off, nothing moves and nothing holds.
#[test]
fn the_factory_that_reaches_the_most_cities_claims_over_a_routine_unit() {
    let (game, ranked) = industrial_empire(94_101);
    let mut busy = game.clone();
    all_queue(&mut busy, &ranked, &unit("knight"));

    let stock = AdvancedAi::new();
    let mut off = busy.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_factory(&mut off, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&off, *cid) == Some(unit("knight"))),
        "off: no claim"
    );

    let ai = armed();
    let mut on = busy.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_factory(&mut on, 0, &plan);
    let claimed = factory_heads(&on, &ranked);
    assert_eq!(claimed.len(), 1, "one claim a turn");
    let reach = |cid: u32| AdvancedAi::factory_new_reach(&busy, 0, cid);
    let best = ranked.iter().take(3).map(|cid| reach(*cid)).max().unwrap();
    assert!(
        best >= 1,
        "fixture: a Factory reaches its own city at least"
    );
    assert_eq!(
        reach(claimed[0]),
        best,
        "the claim goes to the Factory that reaches the most cities"
    );
    let factory = head(&on, claimed[0]).unwrap();
    assert!(
        ai.factory_claim_holds(&on, 0, claimed[0], &plan, &factory),
        "the claimed Factory holds"
    );
    assert!(
        !stock.factory_claim_holds(&on, 0, claimed[0], &plan, &factory),
        "off: no hold"
    );
}

/// A city a standing Factory already reaches gains nothing from a second one:
/// it no longer counts toward a candidate's reach.
#[test]
fn a_standing_factory_takes_its_cities_out_of_the_reach() {
    let (mut game, ranked) = industrial_empire(94_102);
    let before = AdvancedAi::factory_new_reach(&game, 0, ranked[1]);
    game.cities
        .get_mut(&ranked[0])
        .unwrap()
        .buildings
        .push(crate::name!("factory"));
    let after = AdvancedAi::factory_new_reach(&game, 0, ranked[1]);
    assert!(after <= before, "a standing Factory never adds reach");
    let range = game.rules.buildings["factory"].regional_range;
    let zone = |cid: u32| {
        game.city_district_family_position(&game.cities[&cid], crate::name!("industrial_zone"))
            .unwrap()
    };
    if game.wdist(zone(ranked[0]), game.cities[&ranked[1]].pos) <= range {
        assert!(
            after < before,
            "the second zone city is already reached, so its own centre drops out"
        );
    }
}

/// With every zone city's Factory standing, the plant waits for Coal; with a
/// stockpile that runs it, the Factory city whose plant powers the most
/// Factories puts it first.
#[test]
fn the_plant_waits_for_coal_and_then_powers_the_factories() {
    let (mut game, ranked) = industrial_empire(94_103);
    for cid in ranked.iter().take(3) {
        game.cities
            .get_mut(cid)
            .unwrap()
            .buildings
            .push(crate::name!("factory"));
    }
    all_queue(&mut game, &ranked, &unit("knight"));
    let ai = armed();

    let mut dry = game.clone();
    let plan = ai.assess(&dry, 0);
    assert_eq!(
        dry.strategic_resource_rate(0, "coal"),
        0.0,
        "fixture: no Coal income"
    );
    ai.claim_factory(&mut dry, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&dry, *cid) == Some(unit("knight"))),
        "no Coal, no plant"
    );

    let mut fuelled = game.clone();
    fuelled.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), 10.0);
    let plan = ai.assess(&fuelled, 0);
    ai.claim_factory(&mut fuelled, 0, &plan);
    let claimed: Vec<u32> = ranked
        .iter()
        .copied()
        .filter(|cid| head(&fuelled, *cid) == Some(building("coal_power_plant")))
        .collect();
    assert_eq!(claimed.len(), 1, "one plant claimed");
    let best = ranked
        .iter()
        .take(3)
        .map(|cid| AdvancedAi::plant_new_power(&fuelled, 0, *cid))
        .max()
        .unwrap();
    assert_eq!(
        AdvancedAi::plant_new_power(&fuelled, 0, claimed[0]),
        best,
        "the plant goes where it powers the most Factories"
    );
    let plant = head(&fuelled, claimed[0]).unwrap();
    assert!(
        ai.factory_claim_holds(&fuelled, 0, claimed[0], &plan, &plant),
        "the claimed plant holds"
    );
}

/// The Industrial Zone claim's never-displace rules: a Settler, a reserved
/// wall breaker and the science lane's University keep their queues.
#[test]
fn a_settler_a_wall_breaker_and_a_university_keep_their_queue() {
    for keeper in [unit("settler"), unit("trebuchet"), building("university")] {
        let (mut game, ranked) = industrial_empire(94_104);
        all_queue(&mut game, &ranked, &keeper);
        let ai = armed();
        let plan = ai.assess(&game, 0);
        ai.claim_factory(&mut game, 0, &plan);
        assert!(
            ranked
                .iter()
                .all(|cid| head(&game, *cid) == Some(keeper.clone())),
            "{keeper:?} keeps its queue"
        );
    }
}

/// Without Industrialization no Factory is legal, so nothing moves.
#[test]
fn no_claim_before_industrialization() {
    let (mut game, ranked) = industrial_empire(94_105);
    game.players[0]
        .techs
        .remove(&crate::name!("industrialization"));
    all_queue(&mut game, &ranked, &unit("knight"));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_factory(&mut game, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&game, *cid) == Some(unit("knight"))),
        "no Factory before the technology"
    );
}

#[test]
fn industrial_zone_claims_the_factory_is_off_by_default_and_reversible() {
    assert!(!AdvancedAi::new().industrial_zone_claims_the_factory);
    assert!(!AdvancedAi::legacy().industrial_zone_claims_the_factory);
    let (game, ranked) = industrial_empire(94_106);
    let mut ai = AdvancedAi::new();
    let plan = ai.assess(&game, 0);
    assert!(ai.factory_claim_item(&game, 0, ranked[0], &plan).is_none());
    ai.enable_industrial_zone_claims_the_factory();
    assert!(ai.factory_claim_item(&game, 0, ranked[0], &plan).is_some());
    ai.disable_industrial_zone_claims_the_factory();
    assert!(ai.factory_claim_item(&game, 0, ranked[0], &plan).is_none());
}
