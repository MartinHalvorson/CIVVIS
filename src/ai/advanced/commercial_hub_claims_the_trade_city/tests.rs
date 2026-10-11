use super::*;
use crate::ai::commercial_hub::tests::emperor_empire;

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_commercial_hub_claims_the_trade_city();
    ai
}

/// The stock controller with only the Commercial Hub step's base armed (the
/// live pin's base), the claim off.
fn stock() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_commercial_hub_and_traders();
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

fn is_hub(game: &Game, item: &Option<Item>) -> bool {
    matches!(item, Some(Item::District { district, .. })
        if game.district_family(*district) == "commercial_hub")
}

fn all_queue(game: &mut Game, cities: &[u32], item: &Item) {
    for cid in cities {
        game.cities.get_mut(cid).unwrap().queue = vec![item.clone()];
    }
}

/// The gene asks the Commercial Hub step, so it arms the step's gene in the
/// controller and its delegated governor; turning it off leaves that base.
#[test]
fn the_gene_requires_and_arms_commercial_hub_and_traders() {
    let mut ai = AdvancedAi::new();
    ai.enable_commercial_hub_claims_the_trade_city();
    assert!(ai.commercial_hub_and_traders && ai.base.commercial_hub_and_traders);
    ai.disable_commercial_hub_claims_the_trade_city();
    assert!(!ai.commercial_hub_claims_the_trade_city);
    assert!(ai.commercial_hub_and_traders, "the base stays as it was");
}

/// The blocked case on the six-city Emperor shape: every city training a
/// routine Knight, the most productive city the step names puts its
/// Commercial Hub first, one claim a turn, and the claim holds; off, nothing
/// moves and nothing holds.
#[test]
fn the_most_productive_city_claims_its_commercial_hub_over_a_routine_unit() {
    let (game, ranked) = emperor_empire("HUBCLAIM", 95_101);
    let mut busy = game.clone();
    all_queue(&mut busy, &ranked, &unit("knight"));

    let off_ai = stock();
    let mut off = busy.clone();
    let plan = off_ai.assess(&off, 0);
    off_ai.claim_commercial_hub(&mut off, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&off, *cid) == Some(unit("knight"))),
        "off: no claim"
    );

    let ai = armed();
    let mut on = busy.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_commercial_hub(&mut on, 0, &plan);
    let claimed: Vec<u32> = ranked
        .iter()
        .copied()
        .filter(|cid| is_hub(&on, &head(&on, *cid)))
        .collect();
    assert_eq!(
        claimed,
        vec![ranked[0]],
        "the most productive city, one claim a turn"
    );
    let hub = head(&on, ranked[0]).unwrap();
    assert!(
        ai.commercial_hub_claim_holds(&on, 0, ranked[0], &plan, &hub),
        "the claimed hub holds"
    );
    assert!(
        !off_ai.commercial_hub_claim_holds(&on, 0, ranked[0], &plan, &hub),
        "off: no hold"
    );
}

/// A Settler at the head keeps its queue: it is never deferred.
#[test]
fn a_settler_keeps_its_queue() {
    let (mut game, ranked) = emperor_empire("HUBCLAIM", 95_102);
    all_queue(&mut game, &ranked, &unit("settler"));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_commercial_hub(&mut game, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&game, *cid) == Some(unit("settler"))),
        "a Settler is never displaced"
    );
}

/// A wall breaker the siege reservation queued keeps its queue: a reserved
/// Trebuchet at the head is never deferred for the hub, though a routine
/// Knight in the same city is (see the claim test).
#[test]
fn a_reserved_trebuchet_keeps_its_queue() {
    let (mut game, ranked) = emperor_empire("HUBCLAIM", 95_103);
    all_queue(&mut game, &ranked, &unit("trebuchet"));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_commercial_hub(&mut game, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&game, *cid) == Some(unit("trebuchet"))),
        "a reserved wall breaker is never displaced"
    );
}

/// Before the step's window (turn 60 live at Online speed) nothing moves.
#[test]
fn no_claim_before_the_window() {
    let (mut game, ranked) = emperor_empire("HUBCLAIM", 95_104);
    game.turn = 20;
    all_queue(&mut game, &ranked, &unit("knight"));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_commercial_hub(&mut game, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&game, *cid) == Some(unit("knight"))),
        "no hub before the window opens"
    );
}
