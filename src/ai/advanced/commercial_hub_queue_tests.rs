//! `commercial-hub-in-the-strategic-queue` through the strategic controller:
//! the native opt-in, the strategic governor's idle queue and review, and the
//! plan's threatened city. See `BasicAi::commercial_hub_step`.
use super::*;
use crate::ai::commercial_hub::tests::emperor_empire;
use crate::game::install_test_district;

#[test]
fn commercial_hub_in_the_strategic_queue_is_a_native_opt_in_off_in_both_controllers() {
    super::test_support::opt_in_off_in_both_controllers(
        "commercial-hub-in-the-strategic-queue",
        |ai| ai.commercial_hub_in_the_strategic_queue,
    );
}

/// The gene asks the Commercial Hub step, so it arms the step's gene in the
/// controller and its delegated governor; turning it off leaves that base.
#[test]
fn the_gene_requires_and_arms_commercial_hub_and_traders() {
    let mut ai = AdvancedAi::new();
    ai.enable_commercial_hub_in_the_strategic_queue();
    assert!(ai.commercial_hub_and_traders && ai.base.commercial_hub_and_traders);
    ai.disable_commercial_hub_in_the_strategic_queue();
    assert!(!ai.commercial_hub_in_the_strategic_queue);
    assert!(ai.commercial_hub_and_traders, "the base stays as it was");
}

fn plan(threatened: Option<u32>) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: threatened,
        desired_cities: 6,
        assessed_turn: 70,
        rush: false,
    }
}

/// The controller with `commercial-hub-and-traders` armed (the live pin's
/// base), and this gene on or off.
fn controller(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.book_pos = 4;
    ai.base.w.city_target = 6.0;
    ai.enable_commercial_hub_and_traders();
    if gene {
        ai.enable_commercial_hub_in_the_strategic_queue();
    }
    ai
}

fn queues(
    ai: &mut AdvancedAi,
    game: &Game,
    ranked: &[u32],
    threatened: Option<u32>,
) -> Vec<Option<Item>> {
    let mut game = game.clone();
    ai.advanced_production(&mut game, 0, &plan(threatened), false);
    ranked
        .iter()
        .map(|cid| game.cities[cid].queue.first().cloned())
        .collect()
}

fn is_hub(game: &Game, item: &Option<Item>) -> bool {
    matches!(item, Some(Item::District { district, .. })
        if game.district_family(*district) == "commercial_hub")
}

/// On the six-city Emperor shape the strategic governor's scorer fills every
/// idle queue with the Trader reservation and soldiers, even with the
/// delegated governor's hub step armed; under the gene the most productive
/// idle cities open their Commercial Hubs before the scorer runs (the most
/// productive city keeps the Trader reservation that ranks above the step,
/// so the hubs go to the next two), and every other city keeps its queue.
#[test]
fn the_strategic_governor_opens_the_hubs_the_scorer_never_picks() {
    let (game, ranked) = emperor_empire("HUBSTRATEGIC", 94_101);
    let stock = queues(&mut controller(false), &game, &ranked, None);
    let gene = queues(&mut controller(true), &game, &ranked, None);
    assert!(
        stock
            .iter()
            .all(|item| item.is_some() && !is_hub(&game, item)),
        "fixture: the scorer opens no hub: {stock:?}"
    );
    let hubs: Vec<usize> = (0..ranked.len())
        .filter(|rank| is_hub(&game, &gene[*rank]))
        .collect();
    assert_eq!(
        hubs,
        vec![1, 2],
        "the next two producers open hubs: {gene:?}"
    );
    for rank in [0, 3, 4, 5] {
        assert_eq!(gene[rank], stock[rank], "rank {rank} keeps its queue");
    }
}

/// A hub queued with nothing invested survives the scorer's review under the
/// gene; the plan's threatened city opens no hub and the hub moves on.
#[test]
fn the_review_keeps_a_queued_hub_and_the_threatened_city_opens_none() {
    let (game, ranked) = emperor_empire("HUBSTRATEGICKEEP", 94_103);
    let gene = queues(&mut controller(true), &game, &ranked, None);
    let hub = gene[1].clone();
    assert!(is_hub(&game, &hub), "fixture: rank 1 opens a hub: {gene:?}");
    let mut queued = game.clone();
    queued.cities.get_mut(&ranked[1]).unwrap().queue = vec![hub.clone().unwrap()];
    assert_eq!(
        queues(&mut controller(true), &queued, &ranked, None)[1],
        hub,
        "the review keeps the queued hub"
    );
    let threatened = queues(&mut controller(true), &game, &ranked, Some(ranked[1]));
    assert!(
        !is_hub(&game, &threatened[1]),
        "threatened: no hub: {threatened:?}"
    );
    assert!(
        threatened.iter().any(|item| is_hub(&game, item)),
        "another producer still opens one: {threatened:?}"
    );
}

/// What the review holds: a Commercial Hub, or the Market of a standing hub;
/// never another item, a threatened city, a city attacked within four turns
/// or a closed window.
#[test]
fn the_hold_covers_the_hub_and_its_market_only() {
    let (mut game, ranked) = emperor_empire("HUBSTRATEGICHOLD", 94_105);
    let ai = controller(true);
    let top = ranked[0];
    let hub_district = BasicAi::civ_district(&game, 0, "commercial_hub");
    let pos = game.district_sites(top, hub_district)[0];
    let hub = Item::District {
        district: hub_district,
        pos,
    };
    let market = Item::Building {
        building: crate::name!("market"),
    };
    let library = Item::Building {
        building: crate::name!("library"),
    };
    assert!(ai.base.commercial_hub_build_holds(&game, top, &hub, None));
    assert!(!ai
        .base
        .commercial_hub_build_holds(&game, top, &hub, Some(top)));
    assert!(
        !ai.base
            .commercial_hub_build_holds(&game, top, &market, None),
        "no hub stands: no Market hold"
    );
    install_test_district(&mut game, top, "commercial_hub");
    assert!(ai
        .base
        .commercial_hub_build_holds(&game, top, &market, None));
    assert!(!ai
        .base
        .commercial_hub_build_holds(&game, top, &library, None));
    let mut attacked = game.clone();
    attacked.cities.get_mut(&top).unwrap().last_attacked = 69;
    assert!(!ai
        .base
        .commercial_hub_build_holds(&attacked, top, &market, None));
    let mut early = game.clone();
    early.turn = 55;
    assert!(!ai
        .base
        .commercial_hub_build_holds(&early, top, &market, None));
}
