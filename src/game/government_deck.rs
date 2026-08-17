use super::*;

/// ⚠ A DECK LEGAL UNDER THE OLD CONSTITUTION SURVIVED THE CHANGE.
///
/// `policies_fit` is enforced when a card is SLOTTED and never again, so on
/// the live bridge CIVVIS re-sent a stale deck every turn and the host
/// refused it every turn. Run `civvis-20260805T192809Z` switched
/// Theocracy -> Merchant Republic on t155 and was still asking for the
/// Theocracy shape (ECONOMIC x3, MILITARY x2, DIPLOMATIC x1) at t160-163,
/// while the host sat at FIVE cards in SIX slots.
#[test]
fn changing_government_drops_cards_the_new_slots_cannot_hold() {
    let mut game = Game::new(2, 24, 16, 1, 200, 0);

    // Theocracy is military 2, economic 2, diplomatic 1, wildcard 1.
    game.players[0].government = Some("theocracy".to_string());
    // Build the deck FROM THE RULES by slot type rather than by name, so the
    // fixture cannot silently stop testing anything when a card is renamed.
    // Theocracy holds military 2 + economic 2 + diplomatic 1 + wildcard 1, so
    // MILITARY x2 and ECONOMIC x3 is legal there (one economic on the
    // wildcard) and needs TWO wildcards under Merchant Republic's military 1.
    let pick = |game: &Game, slot: &str, want: usize| -> Vec<Name> {
        let mut found: Vec<Name> = game
            .rules
            .policies
            .iter()
            .filter(|(_, spec)| spec.slot == slot)
            .map(|(name, _)| *name)
            .collect();
        found.sort();
        found.truncate(want);
        found
    };
    game.players[0].policies.clear();
    for card in pick(&game, "military", 2)
        .into_iter()
        .chain(pick(&game, "economic", 3))
    {
        game.players[0].policies.insert(card);
    }
    let before = game.players[0].policies.len();
    assert_eq!(before, 5, "the fixture needs all five cards to exist");
    assert!(
        game.policies_fit(0, &game.players[0].policies.clone()),
        "the fixture deck must be LEGAL under Theocracy, or it tests nothing"
    );

    // Merchant Republic is military 1, economic 2, diplomatic 2, wildcard 1:
    // the SAME six slots in a different shape, which is exactly why a
    // slot-COUNT comparison cannot catch this and only the SHAPE can.
    game.players[0].government = Some("merchant_republic".to_string());
    let dropped = game.prune_policies_to_government(0);

    let held = game.players[0].policies.clone();
    // ⚠ A TEST THAT CANNOT FAIL IS NOT A TEST. If the starting deck already
    // fitted Merchant Republic nothing would be pruned and the assertions
    // below would pass vacuously, so require that pruning actually bit.
    assert!(
        dropped > 0,
        "the fixture must construct a deck the NEW government cannot hold"
    );
    assert!(
        game.policies_fit(0, &held),
        "after pruning the deck must fit the new government"
    );
    assert_eq!(
        held.len() + dropped,
        before,
        "pruning must only remove cards, never invent them"
    );
}

/// A deck that already fits must be left completely alone.
#[test]
fn a_deck_that_fits_is_not_pruned() {
    let mut game = Game::new(2, 24, 16, 1, 200, 0);
    game.players[0].government = Some("merchant_republic".to_string());
    let before = game.players[0].policies.clone();
    assert_eq!(game.prune_policies_to_government(0), 0);
    assert_eq!(game.players[0].policies, before);
}
