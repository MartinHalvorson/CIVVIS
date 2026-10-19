use super::*;

/// Two majors at war: our city at (4, 4), theirs far off, every unit gone,
/// a full treasury, and an enemy Warrior `distance` tiles from our city.
fn threatened(distance: i32) -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 24, 16, 93_501, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
    }
    let home = g.found_city_for(0, (4, 4), None);
    g.found_city_for(1, (18, 10), None);
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 40;
    g.players[0].gold = 500.0;
    for tech in ["bronze_working", "archery", "mining", "pottery"] {
        g.players[0].techs.insert(Name::new(tech));
    }
    let at = g
        .wring(g.cities[&home].pos, distance)
        .into_iter()
        .find(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
        })
        .expect("a tile for the raider");
    g.spawn_test_unit("warrior", 1, at);
    let mut ai = AdvancedAi::new();
    ai.enable_threatened_city_buys_its_defender();
    (g, ai, home)
}

fn soldiers_in(g: &Game, cid: u32) -> usize {
    let at = g.cities[&cid].pos;
    g.units
        .values()
        .filter(|unit| {
            unit.owner == 0 && unit.pos == at && g.rules.units[unit.kind].class == "military"
        })
        .count()
}

#[test]
fn a_threatened_city_buys_its_defender_before_the_blow() {
    let (mut g, mut ai, home) = threatened(2);
    assert!(g
        .player_visibility(0)
        .contains(&g.units.values().find(|u| u.owner == 1).unwrap().pos));
    let gold = g.players[0].gold;
    assert!(ai.threatened_city_defender_purchase(&mut g, 0, Some(home)));
    assert_eq!(
        soldiers_in(&g, home),
        1,
        "one defender bought into the city"
    );
    assert!(g.players[0].gold < gold);
    // The cooldown: none again this turn.
    assert!(!ai.threatened_city_defender_purchase(&mut g, 0, Some(home)));
}

#[test]
fn off_nothing_is_bought() {
    let (mut g, mut ai, home) = threatened(2);
    ai.disable_threatened_city_buys_its_defender();
    assert!(!ai.threatened_city_defender_purchase(&mut g, 0, Some(home)));
    assert_eq!(soldiers_in(&g, home), 0);
}

#[test]
fn a_distant_raider_buys_nothing() {
    let (mut g, mut ai, home) = threatened(THREATENED_PURCHASE_REACH + 3);
    assert!(!ai.threatened_city_defender_purchase(&mut g, 0, Some(home)));
    assert_eq!(soldiers_in(&g, home), 0);
}

#[test]
fn an_empty_treasury_buys_nothing() {
    let (mut g, mut ai, home) = threatened(2);
    g.players[0].gold = 10.0;
    assert!(!ai.threatened_city_defender_purchase(&mut g, 0, Some(home)));
    assert_eq!(soldiers_in(&g, home), 0);
}

#[test]
fn no_named_threat_buys_nothing() {
    let (mut g, mut ai, home) = threatened(2);
    assert!(!ai.threatened_city_defender_purchase(&mut g, 0, None));
    assert_eq!(soldiers_in(&g, home), 0);
}

/// The fixture's city can train Pikemen (45) but the reserve only reaches a
/// Warrior (20).
fn era_gap() -> (Game, AdvancedAi, u32) {
    let (mut g, ai, home) = threatened(2);
    for tech in [
        "currency",
        "mathematics",
        "military_tactics",
        "writing",
        "archery",
    ] {
        g.players[0].techs.insert(Name::new(tech));
    }
    let pike = Item::Unit {
        unit: crate::name!("pikeman"),
    };
    assert!(g.can_produce(0, home, &pike), "the city trains Pikemen");
    let warrior = g
        .unit_purchase_cost(0, home, "warrior", "gold")
        .expect("a Warrior is for sale");
    let pikeman = g
        .unit_purchase_cost(0, home, "pikeman", "gold")
        .expect("a Pikeman is for sale");
    assert!(warrior + 10.0 < pikeman);
    g.players[0].gold = ai.purchase_upkeep_floor(&g, 0, 0.0) + warrior + 5.0;
    (g, ai, home)
}

fn bought_kinds(g: &Game, cid: u32) -> Vec<String> {
    let at = g.cities[&cid].pos;
    g.units
        .values()
        .filter(|unit| {
            unit.owner == 0 && unit.pos == at && g.rules.units[unit.kind].class == "military"
        })
        .map(|unit| unit.kind.to_string())
        .collect()
}

#[test]
fn version_one_buys_the_body_the_reserve_reaches() {
    let (mut g, mut ai, home) = era_gap();
    assert!(ai.threatened_city_defender_purchase(&mut g, 0, Some(home)));
    assert_eq!(bought_kinds(&g, home), vec!["warrior".to_string()]);
}

#[test]
fn version_two_keeps_the_gold_short_of_a_defender_of_its_era() {
    let (mut g, mut ai, home) = era_gap();
    ai.disable_threatened_city_buys_its_defender();
    ai.enable_threatened_city_buys_its_defender_2();
    let gold = g.players[0].gold;
    assert!(!ai.threatened_city_defender_purchase(&mut g, 0, Some(home)));
    assert!(bought_kinds(&g, home).is_empty());
    assert_eq!(g.players[0].gold, gold);
}

#[test]
fn version_two_buys_a_defender_of_its_era_when_the_gold_reaches_one() {
    let (mut g, mut ai, home) = era_gap();
    ai.disable_threatened_city_buys_its_defender();
    ai.enable_threatened_city_buys_its_defender_2();
    g.players[0].gold = 5_000.0;
    assert!(ai.threatened_city_defender_purchase(&mut g, 0, Some(home)));
    let bought = bought_kinds(&g, home);
    assert_eq!(bought.len(), 1);
    let power = |kind: &str| {
        let spec = &g.rules.units[kind];
        spec.strength.max(spec.ranged_attack_strength())
    };
    assert!(power(&bought[0]) >= super::DEFENDER_ERA_SHARE * power("pikeman"));
}
