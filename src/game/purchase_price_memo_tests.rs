use super::*;

/// A fixture with THREE cities under one player: the capital at the starting
/// settler's site, plus two more well clear of it and of each other.
/// `legal_purchase_actions` walks every city of the asking player under one
/// shared `QueryMemo` guard, so this is the shape
/// `legal_purchase_actions_for_city`'s per-unit, per-formation, per-currency
/// sweep actually runs across in a real game.
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

/// ★★★ THE MEMO MUST BE INVISIBLE TO THE CALLER, UNDER A SHARED GUARD.
///
/// `legal_purchase_actions_for_city` calls
/// `unit_purchase_cost_for_formation` once per unit kind, formation and
/// currency; `legal_actions_within`'s purchase family repeats the identical
/// sweep. `QueryCache::purchase_price` shares one answer per
/// `(pid, cid, unit, formation, currency)` for the life of one `QueryMemo`
/// guard — an explicit outer guard here stands in for the guard a real
/// decision holds open across several such helpers. Sharing that answer must
/// not change which purchases come back, their order, or their fields: a
/// cold ask and a warm ask under the same guard must be byte-identical, and
/// every memoized answer must equal an uncached re-derivation.
#[test]
fn a_warm_purchase_price_memo_matches_a_cold_derivation_under_one_guard() {
    let game = several_cities(91_401);
    let _memo = game.query_memo();

    let cold = game.legal_purchase_actions(0);
    assert!(
        !cold.is_empty(),
        "three rich cities must offer at least one purchase"
    );
    assert!(
        game.query_memo
            .purchase_price
            .borrow()
            .as_ref()
            .is_some_and(|quotes| !quotes.is_empty()),
        "an enclosing QueryMemo must still retain quotes for a second purchase-menu pass"
    );

    // Warm: read again under the same still-open guard, with nothing between
    // the two asks that could change an answer.
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

/// The standalone projection takes the cheap uncached price path for its
/// one-shot sweep. Its answer must nevertheless remain the literal purchase
/// portion of the full legal-action enumeration, including its stable order.
#[test]
fn standalone_purchase_menu_matches_the_stock_purchase_projection() {
    let game = several_cities(91_405);
    let stock = game
        .legal_actions_within(0, ActionFamilies::PURCHASES | ActionFamilies::EMPIRE)
        .into_iter()
        .filter(|action| {
            matches!(
                action,
                Action::Buy { .. }
                    | Action::BuyBuilding { .. }
                    | Action::BuyDistrict { .. }
                    | Action::BuyPlot { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(game.legal_purchase_actions(0), stock);
}

/// ★★★★ THE REGRESSION THIS DESIGN EXISTS TO AVOID.
///
/// An earlier version of this memo was scoped like `producible_items`:
/// outlives `QueryMemo`, cleared only at explicit sites (a successful
/// `Game::apply`, the mirror-input `replace_*` calls). It went stale here —
/// `land_combat_purchase_slot_open` reads live unit occupancy and the
/// production queue head, and `relocate` plus a direct queue edit are
/// `pub(crate)` and do not go through `Game::apply`.
/// `district_building_wonder_runtime_tests::
/// land_combat_purchase_requires_an_unreserved_city_center_combat_layer`
/// caught it directly (a queue edit and a `relocate` between two identical
/// asks, expecting the second to differ).
///
/// The fix scopes the memo like `Game::tile_appeal`'s `appeal` cache instead:
/// live only for one `QueryMemo` guard. This test pins that shape rather than
/// re-deriving the scenario: outside any guard, the field must never even
/// arm, so a bare call always re-derives.
#[test]
fn unit_purchase_cost_for_formation_never_caches_outside_a_query_memo_guard() {
    let mut game = several_cities(91_404);
    // Index [0] is the capital, sited on the starting settler's tile — its
    // starting Warrior spawns on that same tile (`Game::new_with`), so its
    // land-combat slot is already filled. Index [1] is one of the two extra
    // cities `several_cities` founds directly with no units on them.
    let cid = game.player_city_ids(0)[1];

    assert!(
        game.query_memo.purchase_price.borrow().is_none(),
        "no guard is open yet"
    );
    let before = game.unit_purchase_cost_for_formation(0, cid, "warrior", 0, "gold");
    assert!(before.is_some(), "an empty city center can price a warrior");
    assert!(
        game.query_memo.purchase_price.borrow().is_none(),
        "a bare call outside any QueryMemo guard must never arm the memo"
    );

    // Mutate the board directly — no `Game::apply` in between, the same
    // shape the caught regression used.
    game.spawn_unit("warrior", 0, game.cities[&cid].pos);

    let after = game.unit_purchase_cost_for_formation(0, cid, "warrior", 0, "gold");
    assert_eq!(
        after, None,
        "the new warrior fills the land-combat slot; a stale cached price from \
         before the direct mutation would wrongly still quote one"
    );
}

/// The host-priced branch reads `purchase_is_blocked` (`blocked_purchases`),
/// so a refusal that arrives after a host price was quoted must be reflected
/// immediately. Nothing outside a `QueryMemo` guard is ever cached, so there
/// is nothing that needs an explicit invalidation site for this to hold.
#[test]
fn a_host_priced_purchase_reflects_a_later_purchase_refusal_immediately() {
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
    assert_eq!(
        game.unit_purchase_cost_for_formation(0, cid, "warrior", 0, "gold"),
        Some(42.0)
    );

    // The mirror refuses the sale on a later tick.
    game.replace_blocked_purchases(std::collections::BTreeMap::from([(
        cid,
        std::collections::BTreeSet::from(["unit:warrior".to_string()]),
    )]));

    assert_eq!(
        game.unit_purchase_cost_for_formation(0, cid, "warrior", 0, "gold"),
        None,
        "a refused purchase must read as refused immediately"
    );
}
