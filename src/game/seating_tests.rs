use super::*;

/// The official pool is the default and retains the stable seating order
/// established by `CIV_NAMES`.
#[test]
fn civ6_seating_is_the_default_and_keeps_the_stable_head() {
    let known: BTreeSet<Name> = Rules::shared().civs.keys().cloned().collect();
    assert_eq!(
        GameOptions::new(4, 20, 14, 1, 20, 0).leader_pool,
        LeaderPool::Civ6
    );
    assert_eq!(
        &CIV_NAMES[..8],
        ["Rome", "Egypt", "Greece", "China", "Sumeria", "Aztec", "Nubia", "Scythia"],
        "an existing seed seats its majors by position, so the roster only grows at the end"
    );
    for players in [1usize, 2, 4, 6, 8, 12] {
        let seated = seat_civs(players, &[], &known, LeaderPool::Civ6);
        let stock: Vec<String> = (0..players)
            .map(|i| CIV6_LEADER_POOL[i % CIV6_LEADER_POOL.len()].to_string())
            .collect();
        assert_eq!(seated, stock, "Civ VI seating changed at {players} players");
    }
}

/// Neither pool repeats until every civilization it contains has a seat.
#[test]
fn each_leader_pool_uses_every_entry_before_it_repeats() {
    let known: BTreeSet<Name> = Rules::shared().civs.keys().cloned().collect();
    for pool in [LeaderPool::Civ6, LeaderPool::ExpandedHistorical] {
        for players in 1..=pool.entries().count() {
            let seated = seat_civs(players, &[], &known, pool);
            assert_eq!(
                seated.iter().collect::<BTreeSet<_>>().len(),
                players,
                "duplicate civilization in {pool:?} at {players} players: {seated:?}"
            );
            for civ in &seated {
                assert!(
                    known.contains(&Name::new(civ)),
                    "{civ} is not in the ruleset"
                );
            }
        }
    }
}

/// A chosen civilization takes its seat, and nobody else is handed the
/// same one — two majors sharing a civilization would share its unique
/// unit, ability and agenda.
#[test]
fn a_chosen_civilization_takes_its_seat_and_is_not_duplicated() {
    let known: BTreeSet<Name> = Rules::shared().civs.keys().cloned().collect();
    let seated = seat_civs(4, &["Egypt".to_string()], &known, LeaderPool::Civ6);
    assert_eq!(seated[0], "Egypt");
    assert_eq!(
        seated.iter().collect::<BTreeSet<_>>().len(),
        4,
        "duplicate civilization in {seated:?}"
    );

    let two = seat_civs(
        4,
        &["Nubia".to_string(), "Rome".to_string()],
        &known,
        LeaderPool::Civ6,
    );
    assert_eq!(&two[..2], ["Nubia".to_string(), "Rome".to_string()]);
    assert_eq!(two.iter().collect::<BTreeSet<_>>().len(), 4);

    let rejected_historical_pick = seat_civs(3, &["Denmark".to_string()], &known, LeaderPool::Civ6);
    assert_eq!(rejected_historical_pick[0], CIV6_LEADER_POOL[0]);

    let historical_pick = seat_civs(
        3,
        &["Denmark".to_string()],
        &known,
        LeaderPool::ExpandedHistorical,
    );
    assert_eq!(historical_pick[0], "Denmark");
}

/// A name from another ruleset seats a stock civilization rather than
/// taking the process down: saves and clients outlive rulesets.
#[test]
fn an_unknown_civilization_falls_back_to_the_stock_roster() {
    let known: BTreeSet<Name> = Rules::shared().civs.keys().cloned().collect();
    let seated = seat_civs(3, &["Atlantis".to_string()], &known, LeaderPool::Civ6);
    assert_eq!(seated[0], CIV_NAMES[0]);
    assert_eq!(seated.iter().collect::<BTreeSet<_>>().len(), 3);
}

/// A lobby that names the same civilization for two seats means it. Only
/// the automatic fill has to avoid a collision, because only the automatic
/// fill can produce one nobody asked for.
#[test]
fn a_lobby_that_names_a_civilization_twice_seats_it_twice() {
    let known: BTreeSet<Name> = Rules::shared().civs.keys().cloned().collect();
    let twice = seat_civs(
        3,
        &["Egypt".to_string(), "Egypt".to_string()],
        &known,
        LeaderPool::Civ6,
    );
    assert_eq!(&twice[..2], ["Egypt".to_string(), "Egypt".to_string()]);
    // The seat nobody asked about still steers clear of the mirror match.
    assert_ne!(twice[2], "Egypt");
}

/// Past the end of the roster there is nothing left to hand out, so the
/// stock fill starts again at the top rather than running out of seats.
#[test]
fn a_lobby_larger_than_the_roster_wraps_instead_of_failing() {
    let known: BTreeSet<Name> = Rules::shared().civs.keys().cloned().collect();
    let players = CIV6_LEADER_POOL.len() + 3;
    let seated = seat_civs(players, &[], &known, LeaderPool::Civ6);
    assert_eq!(seated.len(), players);
    assert_eq!(
        &seated[..CIV6_LEADER_POOL.len()],
        &CIV6_LEADER_POOL.map(String::from)[..]
    );
    assert_eq!(
        &seated[CIV6_LEADER_POOL.len()..],
        &CIV6_LEADER_POOL.map(String::from)[..3]
    );
}

#[test]
fn randomized_seating_is_seeded_unique_and_preserves_explicit_picks() {
    let known: BTreeSet<Name> = Rules::shared().civs.keys().cloned().collect();
    let chosen = ["Egypt".to_string()];
    let mut first_rng = Rng::new(71);
    let mut repeat_rng = Rng::new(71);
    let first = seat_civs_randomized(8, &chosen, &known, LeaderPool::Civ6, &mut first_rng);
    let repeat = seat_civs_randomized(8, &chosen, &known, LeaderPool::Civ6, &mut repeat_rng);
    assert_eq!(first, repeat);
    assert_eq!(first[0], "Egypt");
    assert_eq!(first.iter().collect::<BTreeSet<_>>().len(), 8);
    let stock_tail: Vec<String> = CIV_NAMES[1..8]
        .iter()
        .map(|civilization| (*civilization).to_string())
        .collect();
    assert_ne!(
        &first[1..],
        stock_tail,
        "open seats should not silently retain stock order"
    );
}

#[test]
fn each_random_pool_contains_exactly_its_selected_roster() {
    let known: BTreeSet<Name> = Rules::shared().civs.keys().cloned().collect();
    for pool in [LeaderPool::Civ6, LeaderPool::ExpandedHistorical] {
        let mut seen = BTreeSet::new();
        for seed in 0..1_000 {
            let mut rng = Rng::new(seed);
            seen.extend(seat_civs_randomized(8, &[], &known, pool, &mut rng));
        }
        assert_eq!(
            seen,
            pool.entries().map(|entry| entry.civ.clone()).collect(),
            "randomized {pool:?} roster differs from its declared pool"
        );
    }
    assert!(CIV6_LEADER_POOL.contains(&"Byzantium"));
    assert!(CIV6_LEADER_POOL.contains(&"Babylon"));
    assert!(CIV6_LEADER_POOL.contains(&"Cree"));
    assert!(CIV6_LEADER_POOL.contains(&"Gran Colombia"));
    assert!(CIV6_LEADER_POOL.contains(&"Indonesia"));
    assert!(CIV6_LEADER_POOL.contains(&"Macedon"));
    assert!(!CIV6_LEADER_POOL.contains(&"Denmark"));
}

#[test]
fn historical_leaders_are_neutral_even_when_legacy_rules_describe_them() {
    let legacy = Rules::embedded().civs[&Name::new("Denmark")].clone();
    let mut options = GameOptions::new(1, 42, 28, 74_101, 40, 0);
    options.leader_pool = LeaderPool::ExpandedHistorical;
    options.civs = vec!["Denmark".to_string()];
    let game = Game::new_with(options);

    assert_eq!(game.players[0].civ, "Denmark");
    assert!(!game.uses_civ6_content(0));
    assert!(!game.has_ability(0, &legacy.ability));
    assert!(legacy
        .effects
        .keys()
        .all(|effect| game.civ_effect(0, effect) == 0.0));
    assert!(game.rules.civs[&Name::new("Denmark")].unique_unit.is_none());
    assert!(game.agenda_of(0).is_none());
}
