use super::*;
use crate::ai::commercial_hub::tests::emperor_empire;
use crate::game::install_test_district;

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 70,
        rush: false,
    }
}

/// The live pin's base (`commercial-hub-in-the-strategic-queue`, which arms
/// `commercial-hub-and-traders`) on a Domination seat, with this gene on or
/// off.
fn controller(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.book_pos = 4;
    ai.base.w.city_target = 6.0;
    ai.enable_commercial_hub_in_the_strategic_queue();
    if gene {
        ai.enable_commercial_hub_claims_a_peace_queue();
    }
    ai
}

fn unit(kind: &str) -> Item {
    Item::Unit {
        unit: Name::new(kind),
    }
}

/// The six-city Emperor shape at turn 70 Online, at peace, every queue
/// holding `head` with nothing invested.
fn empire(tag: &str, head: &Item) -> (Game, Vec<u32>) {
    let (mut game, ranked) = emperor_empire(tag, 95_201);
    for cid in &ranked {
        game.cities.get_mut(cid).unwrap().queue = vec![head.clone()];
    }
    (game, ranked)
}

fn heads(game: &Game, ranked: &[u32]) -> Vec<Option<Item>> {
    ranked
        .iter()
        .map(|cid| game.cities[cid].queue.first().cloned())
        .collect()
}

fn is_hub(game: &Game, item: &Option<Item>) -> bool {
    matches!(item, Some(Item::District { district, .. })
        if game.district_family(*district) == "commercial_hub")
}

#[test]
fn commercial_hub_claims_a_peace_queue_is_an_opt_in_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "commercial-hub-claims-a-peace-queue",
        |ai| ai.commercial_hub_claims_a_peace_queue,
    );
}

/// Decision 2: with no Hub in the empire, one of the Hub step's best
/// producers starts its Hub ahead of a soldier in training, once; every
/// other city keeps its soldier, and a second turn opens no second Hub.
#[test]
fn the_first_hub_goes_ahead_of_a_soldier_at_peace_once() {
    let catapult = unit("catapult");
    let (mut game, ranked) = empire("PEACEHUBFIRST", &catapult);
    let mut ai = controller(true);
    ai.claim_peace_queue_for_the_commercial_hub(&mut game, 0, &plan());
    ai.claim_peace_queue_for_the_commercial_hub(&mut game, 0, &plan());
    let after = heads(&game, &ranked);
    let hubs: Vec<usize> = (0..ranked.len())
        .filter(|rank| is_hub(&game, &after[*rank]))
        .collect();
    assert_eq!(hubs.len(), 1, "one Hub by claim: {after:?}");
    assert!(hubs[0] < 3, "one of the step's best producers: {after:?}");
    for (rank, head) in after.iter().enumerate() {
        if rank != hubs[0] {
            assert_eq!(
                head.as_ref(),
                Some(&catapult),
                "rank {rank} keeps its soldier"
            );
        }
    }
}

/// Decision 1: a standing Hub without its Market starts the Market ahead of
/// a soldier, and with that Hub standing no further Hub is claimed.
#[test]
fn a_standing_hub_takes_its_market_and_no_second_hub_is_claimed() {
    let catapult = unit("catapult");
    let (mut game, ranked) = empire("PEACEHUBMARKET", &catapult);
    install_test_district(&mut game, ranked[4], "commercial_hub");
    let mut ai = controller(true);
    ai.claim_peace_queue_for_the_commercial_hub(&mut game, 0, &plan());
    ai.claim_peace_queue_for_the_commercial_hub(&mut game, 0, &plan());
    let after = heads(&game, &ranked);
    assert!(
        matches!(&after[4], Some(Item::Building { building }) if *building == "market"),
        "the Hub city builds its Market: {after:?}"
    );
    for rank in [0, 1, 2, 3, 5] {
        assert_eq!(
            after[rank].as_ref(),
            Some(&catapult),
            "rank {rank} keeps its soldier"
        );
    }
}

/// The kept queues and the gates: a Settler is never displaced; with the
/// gene off, without `commercial-hub-in-the-strategic-queue`, or on another
/// victory target nothing moves.
#[test]
fn settlers_and_the_gates_keep_every_queue() {
    let settler = unit("settler");
    let (mut game, ranked) = empire("PEACEHUBKEEP", &settler);
    let before = heads(&game, &ranked);
    controller(true).claim_peace_queue_for_the_commercial_hub(&mut game, 0, &plan());
    assert_eq!(heads(&game, &ranked), before, "a Settler keeps its queue");

    let catapult = unit("catapult");
    let (game, ranked) = empire("PEACEHUBGATES", &catapult);
    let before = heads(&game, &ranked);
    let mut off = controller(false);
    let mut riderless = AdvancedAi::targeting(VictoryTarget::Domination);
    riderless.enable_commercial_hub_and_traders();
    riderless.enable_commercial_hub_claims_a_peace_queue();
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_commercial_hub_in_the_strategic_queue();
    science.enable_commercial_hub_claims_a_peace_queue();
    for ai in [&mut off, &mut riderless, &mut science] {
        let mut board = game.clone();
        ai.claim_peace_queue_for_the_commercial_hub(&mut board, 0, &plan());
        assert_eq!(heads(&board, &ranked), before);
    }
}
