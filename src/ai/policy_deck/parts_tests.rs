use super::*;

/// A seat with four cities, a garrison, every civic and so every card.
fn empire() -> Game {
    let mut game = Game::new_full(2, 30, 20, 91_517, 160, 0, false);
    let land: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.unit_ids_at(**pos).is_empty()
        })
        .map(|(pos, _)| *pos)
        .collect();
    let mut sites: Vec<Pos> = Vec::new();
    for pos in land {
        if sites.iter().all(|site| game.wdist(*site, pos) >= 5) {
            sites.push(pos);
        }
        if sites.len() == 4 {
            break;
        }
    }
    for site in &sites {
        game.found_city_for(0, *site, None);
    }
    game.spawn_test_unit("spearman", 0, sites[0]);
    let civics: Vec<Name> = game.rules.civics.keys().copied().collect();
    game.players[0].civics.extend(civics);
    game.players[0].government = Some("chiefdom".to_string());
    game.current = 0;
    game
}

/// Every card scored by parts reads exactly as scored by whole sweeps, in
/// every combination of the two empire-wide terms, and the unchanged
/// reading is the same number.
#[test]
fn a_card_scored_by_parts_reads_exactly_as_a_whole_sweep_does() {
    let mut game = empire();
    let weights = Weights {
        policy_deck: PolicyDeck::Live,
        ..Weights::default()
    };
    let mut cards = game.available_policies(0);
    cards.sort();
    cards.dedup();
    assert!(
        cards.len() > 10,
        "the fixture offers a real deck: {}",
        cards.len()
    );
    if let Some(first) = cards.first().copied() {
        game.players[0].policies.insert(first);
    }
    let mut checked = 0;
    for (net_maintenance, player_income) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let (current, reads) =
            reading_with_policy_trace(&game, 0, &weights, net_maintenance, player_income);
        let parts = ReadingParts::new(&game, 0, &weights, net_maintenance, player_income);
        assert_eq!(parts.reading(&weights).to_bits(), current.to_bits());
        for card in &cards {
            let Some(spec) = game.rules.policies.get_interned(*card) else {
                continue;
            };
            let candidate = (0usize, spec.slot.clone(), *card);
            let whole = policy_card_score(
                &mut game,
                0,
                &weights,
                &candidate,
                current,
                &reads,
                net_maintenance,
                player_income,
            );
            let by_parts = policy_card_score_by_parts(
                &mut game,
                0,
                &weights,
                &candidate,
                &parts,
                current,
                net_maintenance,
                player_income,
            );
            assert_eq!(whole.0.to_bits(), by_parts.0.to_bits(), "{}", card.as_str());
            assert_eq!(
                (whole.1, &whole.2, whole.3),
                (by_parts.1, &by_parts.2, by_parts.3)
            );
            checked += 1;
        }
    }
    assert!(checked > 40);
}
