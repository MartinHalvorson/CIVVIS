use super::*;

/// Seat 0 holds one city with a Warrior in it and two Archers beside it,
/// knows Machinery (the Crossbowman), and earns 5 Gold a turn. The counters
/// read the units it was given: best melee 20, best ranged 25.
struct Fixture {
    g: Game,
    ai: AdvancedAi,
    archers: [u32; 2],
}

fn fixture(on: bool) -> Fixture {
    let mut g = Game::new_full(2, 24, 16, 366_400, 250, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (4, 4), None);
    g.found_city_for(1, (18, 10), None);
    g.current = 0;
    g.turn = 60;
    g.players[0].techs.insert(crate::name!("archery"));
    g.players[0].techs.insert(crate::name!("machinery"));
    g.players[0].gold = 1_000.0;
    g.players[0].gold_per_turn = 5.0;
    g.players[0].counters.remove("strongest_unit_built");
    g.players[0].counters.remove("strongest_ranged_built");
    g.spawn_test_unit("warrior", 0, (4, 4));
    let archers = [
        g.spawn_test_unit("archer", 0, (5, 4)),
        g.spawn_test_unit("archer", 0, (4, 5)),
    ];
    for uid in archers {
        assert_eq!(
            g.unit_gold_upgrade_detail(0, uid).map(|(to, _, _)| to),
            Ok(crate::name!("crossbowman")),
            "the fixture's Archer {uid} can upgrade"
        );
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    if on {
        ai.enable_one_upgrade_lifts_every_city();
    }
    Fixture { g, ai, archers }
}

fn crossbow_price(g: &Game) -> f64 {
    g.unit_upgrade_price_in_formation(0, crate::name!("archer"), crate::name!("crossbowman"), 0)
        .unwrap()
        .0
}

fn counter(g: &Game, name: &str) -> i64 {
    g.players[0].counters.get(name).copied().unwrap_or(0)
}

fn crossbowmen(g: &Game) -> usize {
    g.player_unit_ids(0)
        .into_iter()
        .filter(|uid| g.units[uid].kind == crate::name!("crossbowman"))
        .count()
}

#[test]
fn the_first_crossbowman_lifts_every_city_and_the_second_waits() {
    let Fixture {
        mut g,
        mut ai,
        archers,
    } = fixture(true);
    let journal = crate::reasoning::Journal::recording();
    ai.base.journal = journal.handle();
    assert_eq!(AdvancedAi::city_lift_basis(&g, 0), (20.0, 25.0));
    let strike = g.city_ranged_strength(g.player_city_ids(0)[0]);
    let gold = g.players[0].gold;

    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 1);

    // One upgrade, the counters raised by 10 (defence) and 15 (strike), and
    // every city's strike with them; the second Archer is no longer a lift.
    assert_eq!(crossbowmen(&g), 1);
    assert_eq!(counter(&g, "strongest_unit_built"), 30);
    assert_eq!(counter(&g, "strongest_ranged_built"), 40);
    assert_eq!(
        g.city_ranged_strength(g.player_city_ids(0)[0]),
        strike + 15.0
    );
    assert!((g.players[0].gold - (gold - crossbow_price(&g))).abs() < 1e-6);
    assert!(archers
        .iter()
        .any(|uid| g.units[uid].kind == crate::name!("archer")));
    // The lower id goes first on a tie.
    assert_eq!(g.units[&archers[0]].kind, crate::name!("crossbowman"));
    let lines: Vec<String> = journal
        .since(0)
        .thoughts
        .iter()
        .filter(|thought| thought.headline == "One upgrade lifts every city")
        .map(|thought| thought.detail.clone())
        .collect();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].contains("defence +10") && lines[0].contains("ranged strike +15"),
        "{}",
        lines[0]
    );

    // Asked again the same turn, nothing qualifies.
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 0);
    assert_eq!(crossbowmen(&g), 1);
}

#[test]
fn it_goes_ahead_of_the_ordinary_floor() {
    // 20 Gold above the bill: under both the peacetime (120) and the wartime
    // (30) floor of the ordinary pass.
    for on in [false, true] {
        let Fixture { mut g, ai, .. } = fixture(on);
        g.players[0].gold = crossbow_price(&g) + 20.0;
        ai.upgrade_units_preserving_air_wing(&mut g, 0);
        assert_eq!(crossbowmen(&g), usize::from(on), "gene on: {on}");
    }
}

#[test]
fn the_treasury_may_reach_zero_and_no_lower() {
    let Fixture { mut g, ai, .. } = fixture(true);
    let price = crossbow_price(&g);
    g.players[0].gold = price;
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 1);
    assert!(g.players[0].gold.abs() < 1e-6);

    let Fixture { mut g, ai, .. } = fixture(true);
    g.players[0].gold = price - 1.0;
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 0);
    assert_eq!(crossbowmen(&g), 0);
}

#[test]
fn a_negative_income_takes_nothing() {
    let Fixture { mut g, ai, .. } = fixture(true);
    g.players[0].gold_per_turn = -0.5;
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 0);
    g.players[0].gold_per_turn = f64::NAN;
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 0);
    g.players[0].gold_per_turn = 0.0;
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 1);
}

#[test]
fn off_it_takes_nothing() {
    let Fixture { mut g, ai, .. } = fixture(false);
    let gold = g.players[0].gold;
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 0);
    assert_eq!(crossbowmen(&g), 0);
    assert_eq!(counter(&g, "strongest_ranged_built"), 25);
    assert_eq!(g.players[0].gold, gold);
}

#[test]
fn a_tier_already_owned_lifts_nothing() {
    // Owned now, with the counters intact.
    let Fixture { mut g, ai, .. } = fixture(true);
    g.spawn_test_unit("crossbowman", 0, (3, 4));
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 0);
    assert_eq!(crossbowmen(&g), 1);

    // Owned now, the counters lost (a resumed live mirror).
    let Fixture { mut g, ai, .. } = fixture(true);
    g.spawn_test_unit("crossbowman", 0, (3, 4));
    g.players[0].counters.remove("strongest_unit_built");
    g.players[0].counters.remove("strongest_ranged_built");
    assert_eq!(AdvancedAi::city_lift_basis(&g, 0), (30.0, 40.0));
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 0);

    // Owned once and gone: the counters remember it.
    let Fixture { mut g, ai, .. } = fixture(true);
    let gone = g.spawn_test_unit("crossbowman", 0, (3, 4));
    g.remove_unit(gone);
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 0);
    assert_eq!(crossbowmen(&g), 0);
}

#[test]
fn an_observed_strike_counts_as_owned() {
    // No counters and no Crossbowman, but a Swordsman holds the melee basis
    // above the Crossbowman's 30: only the strike half could lift.
    let Fixture { mut g, ai, .. } = fixture(true);
    g.spawn_test_unit("swordsman", 0, (3, 4));
    g.players[0].counters.remove("strongest_unit_built");
    g.players[0].counters.remove("strongest_ranged_built");
    assert_eq!(AdvancedAi::city_lift_basis(&g, 0), (35.0, 25.0));
    // The host reports the city striking at 40 (plus any card's bonus).
    let city = g.player_city_ids(0)[0];
    let card = g.policy_effect(0, "city_ranged");
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(city, 40.0 + card);
    assert_eq!(AdvancedAi::city_lift_basis(&g, 0), (35.0, 40.0));
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 0);
    // Without the observation the Crossbowman lifts the strike by 15.
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).clear();
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 1);
}
