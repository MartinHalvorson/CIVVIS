use super::*;

/// A fixture with THREE cities under one player: the capital at the starting
/// settler's site, plus two more well clear of it and of each other.
/// `legal_purchase_actions` walks every city of the asking player, so this is
/// the shape `legal_purchase_actions_for_city`'s per-unit, per-formation,
/// per-currency sweep actually runs across in a real game — a single-city
/// fixture would never exercise the memo being shared across cities.
fn several_cities(seed: u64) -> Game {
    let mut game = Game::new_full(1, 34, 22, seed, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("a starting settler");
    let capital = game.units[&settler].pos;
    game.found_city_for(0, capital, None);

    let mut candidates: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| {
            let tile = &game.map.tiles[pos];
            !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && !game.tile_is_natural_wonder(tile)
        })
        .collect();
    candidates.sort_unstable();
    for pos in candidates {
        if game.player_city_ids(0).len() >= 3 {
            break;
        }
        if game.cities.values().all(|c| game.wdist(c.pos, pos) >= 5) {
            game.found_city_for(0, pos, None);
        }
    }
    assert!(
        game.player_city_ids(0).len() >= 3,
        "the fixture map needs room for three cities at this spacing"
    );
    game.players[0].gold = 5_000.0;
    game.players[0].faith = 5_000.0;
    game
}

/// ★★★ THE MEMO MUST BE INVISIBLE TO THE CALLER.
///
/// `legal_purchase_actions_for_city` used to call
/// `unit_purchase_cost_for_formation` six times per unit kind per city (three
/// formations, two currencies) with nothing between those calls that could
/// change the answer — a read-only enumeration mutates nothing. Sharing one
/// answer per `(pid, cid, unit, formation, currency)` must not change which
/// purchases come back, their order, or their fields: a cold ask and a warm
/// ask of the same unchanged board must be byte-identical, and every
/// memoized answer must equal an uncached re-derivation.
#[test]
fn a_warm_purchase_price_memo_matches_a_cold_derivation_across_several_cities() {
    let game = several_cities(91_401);

    assert!(
        game.query_memo.purchase_price.borrow().is_empty(),
        "nothing has been asked yet"
    );
    let cold = game.legal_purchase_actions(0);
    assert!(
        !cold.is_empty(),
        "three rich cities must offer at least one purchase"
    );
    assert!(
        !game.query_memo.purchase_price.borrow().is_empty(),
        "the ask must have populated the price memo"
    );

    // Warm: read again with nothing between the two asks that could change
    // an answer.
    let warm = game.legal_purchase_actions(0);
    assert_eq!(
        warm, cold,
        "a warm price memo must not change which purchases are legal, or their order"
    );

    // Every memoized answer must equal an uncached re-derivation, for every
    // unit, formation and currency, in every city of the fixture.
    for cid in game.player_city_ids(0) {
        for unit in game.rules.units.keys() {
            for formation in 0..=2u8 {
                for currency in ["gold", "faith"] {
                    let memoized =
                        game.unit_purchase_cost_for_formation(0, cid, unit, formation, currency);
                    let direct = game.unit_purchase_cost_for_formation_uncached(
                        0, cid, unit, formation, currency,
                    );
                    assert_eq!(
                        memoized, direct,
                        "city {cid} unit {unit} formation {formation} currency {currency}"
                    );
                }
            }
        }
    }
}

/// The memo shares `producible`'s outlives-`QueryMemo` lifetime and its
/// invalidation boundary: a successful `Game::apply` must retire both, and a
/// re-ask right after must reflect the new board, never a cached answer from
/// before the mutation.
#[test]
fn a_successful_purchase_apply_retires_the_price_memo_it_used() {
    let mut game = several_cities(91_402);
    // Index [0] is the capital, sited on the starting settler's tile — its
    // starting Warrior spawns on that same tile (`Game::new_with`), so its
    // land-combat slot is already filled. Index [1] is one of the two extra
    // cities `several_cities` founds directly with no units on them.
    let cid = game.player_city_ids(0)[1];

    let before = game.unit_purchase_cost_for_formation(0, cid, "warrior", 0, "gold");
    assert!(before.is_some(), "an empty city center can price a warrior");
    assert!(!game.query_memo.purchase_price.borrow().is_empty());

    game.apply(
        0,
        &Action::Buy {
            city: cid,
            unit: crate::name!("warrior"),
            formation: 0,
            currency: "gold".to_string(),
        },
    )
    .expect("5,000 gold affords a warrior");

    assert!(
        game.query_memo.purchase_price.borrow().is_empty(),
        "a successful apply must retire the price memo, the same boundary that retires `producible`"
    );

    // The city center now holds a land-combat unit, so
    // `land_combat_purchase_slot_open` refuses a second one. A stale cached
    // price would wrongly still quote one.
    let after = game.unit_purchase_cost_for_formation(0, cid, "warrior", 0, "gold");
    assert_eq!(
        after, None,
        "the purchased warrior fills the land-combat slot; the price must reflect \
         that, not a memo answer from before the purchase"
    );
}

/// `replace_blocked_purchases` deliberately leaves `producible` untouched
/// (the production catalog never reads `blocked_purchases`), but the price
/// memo's host-priced branch reads it through `purchase_is_blocked` — so this
/// is the one clear site the price memo needs beyond every one `producible`
/// already has, and it is the easiest one to have missed.
#[test]
fn replace_blocked_purchases_retires_a_cached_host_priced_answer() {
    let mut game = several_cities(91_403);
    let cid = game.player_city_ids(0)[0];

    // A host purchase menu that prices this unit in gold:
    // `unit_purchase_cost_for_formation`'s formation-0 branch takes this
    // price unconditionally when the export carries it.
    game.replace_host_menus(
        std::collections::BTreeMap::new(),
        std::collections::BTreeMap::from([(
            cid,
            std::collections::BTreeMap::from([(
                "unit:warrior".to_string(),
                HostPurchaseEntry {
                    gold: Some(42.0),
                    faith: None,
                },
            )]),
        )]),
        std::collections::BTreeMap::new(),
    );

    let priced = game.unit_purchase_cost_for_formation(0, cid, "warrior", 0, "gold");
    assert_eq!(priced, Some(42.0));
    assert!(!game.query_memo.purchase_price.borrow().is_empty());

    // The mirror refuses the sale on a later tick, without a fresh menu
    // export to trip `replace_host_menus`'s own clear.
    game.replace_blocked_purchases(std::collections::BTreeMap::from([(
        cid,
        std::collections::BTreeSet::from(["unit:warrior".to_string()]),
    )]));

    assert!(
        game.query_memo.purchase_price.borrow().is_empty(),
        "replace_blocked_purchases must retire the price memo even though it \
         leaves `producible` untouched"
    );
    assert_eq!(
        game.unit_purchase_cost_for_formation(0, cid, "warrior", 0, "gold"),
        None,
        "a stale cached host price would ignore the refusal that just arrived"
    );
}
