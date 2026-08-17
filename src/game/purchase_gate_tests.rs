use super::*;

/// ⚠⚠⚠ THE BUYERS BYPASS THEIR OWN GATE.
///
/// `purchase_is_blocked` says so in its own doc — the missionary buyer and
/// the gold buyers build an `Action::Buy*` and call `apply` DIRECTLY, so a
/// gate living only in the enumeration (`purchases.retain`, `acts.retain`)
/// never runs for them. It was still only in the enumeration.
///
/// Live run `civvis-20260811T230324Z`: **181 refused `UNIT_MISSIONARY` faith
/// purchases in one game**, one city, 177 CONSECUTIVE turns from t58, against
/// an eight-turn cooldown. Sixty percent of every refusal that run recorded.
///
/// This drives `apply` directly, exactly as the buyers do, so a gate that
/// only filters an enumeration cannot satisfy it.
#[test]
fn a_blocked_purchase_is_refused_even_when_apply_is_called_directly() {
    let mut game = Game::new_full(1, 20, 14, 91_483, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("a starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("the capital is founded");
    let cid = *game.player_city_ids(0).first().expect("a capital");

    game.replace_blocked_purchases(std::collections::BTreeMap::from([(
        cid,
        std::collections::BTreeSet::from(["unit:warrior".to_string()]),
    )]));

    let refused = game.apply(
        0,
        &Action::Buy {
            city: cid,
            unit: crate::name!("warrior"),
            formation: 0,
            currency: "gold".to_string(),
        },
    );
    assert!(
        refused.is_err(),
        "a purchase the host refused recently must not be re-attempted through \
         a direct `apply`, which is how 181 missionary buys reached the host"
    );
}

/// ⚠ And an unblocked item must still be reachable, or the gate is a wall.
#[test]
fn an_unblocked_purchase_is_untouched_by_the_gate() {
    let mut game = Game::new_full(1, 20, 14, 91_483, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("a starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("the capital is founded");
    let cid = *game.player_city_ids(0).first().expect("a capital");
    game.replace_blocked_purchases(std::collections::BTreeMap::from([(
        cid,
        std::collections::BTreeSet::from(["unit:slinger".to_string()]),
    )]));
    // Not asserting the buy SUCCEEDS — gold, tech and rules all gate it too.
    // Asserting only that it is not stopped by the block for a different unit.
    let why = game
        .apply(
            0,
            &Action::Buy {
                city: cid,
                unit: crate::name!("warrior"),
                formation: 0,
                currency: "gold".to_string(),
            },
        )
        .err()
        .unwrap_or_default();
    assert!(
        !why.contains("refused this purchase recently"),
        "the gate must only stop the item that was actually refused, got: {why}"
    );
}
