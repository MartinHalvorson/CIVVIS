use super::super::*;
use std::collections::BTreeMap;

/// One city of ours and a rival objective, `stock` Oil banked and `income`
/// a turn, our technology at `techs`, a Domination seat with the gene on.
fn fixture(stock: f64, income: f64, techs: &[&str]) -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 40, 24, 380_201, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let city = g.found_city_for(0, (6, 12), None);
    let objective = g.found_city_for(1, (24, 12), None);
    for goal in techs {
        g.players[0].techs.extend(
            g.rules.tech_ancestors[*goal]
                .iter()
                .map(|tech| Name::new(tech)),
        );
        g.players[0].techs.insert(Name::new(goal));
    }
    g.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), stock);
    std::sync::Arc::make_mut(&mut g.observed_strategic_income_adjustments)
        .insert(0, BTreeMap::from([(crate::name!("oil"), income)]));
    assert_eq!(g.strategic_resource_rate(0, "oil"), income);
    g.current = 0;
    g.turn = 150;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_guns_keep_the_oil();
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(objective),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    });
    (g, ai, city)
}

const STEEL_AGE: &[&str] = &["steel", "replaceable_parts", "chemistry"];

fn unit(kind: &str) -> Item {
    Item::Unit {
        unit: Name::new(kind),
    }
}

fn off(ai: &AdvancedAi) -> AdvancedAi {
    let mut off = ai.clone();
    off.disable_guns_keep_the_oil();
    off
}

#[test]
fn the_next_gun_is_the_artillery_and_burns_oil() {
    let (g, ai, _) = fixture(14.0, 0.0, STEEL_AGE);
    assert_eq!(
        ai.siege_gun_fuel(&g, 0),
        Some((crate::name!("oil"), 1.0, 1.0))
    );
    assert_eq!(off(&ai).siege_gun_fuel(&g, 0), None);
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_guns_keep_the_oil();
    assert_eq!(science.siege_gun_fuel(&g, 0), None);
    // Before Steel the train's best gun is the Bombard, whose Niter is paid
    // up front and burns nothing a turn.
    let (bombard_age, ai, _) = fixture(14.0, 0.0, &["metal_casting", "replaceable_parts"]);
    assert_eq!(ai.siege_gun_fuel(&bombard_age, 0), None);
}

#[test]
fn an_infantry_waits_while_the_guns_need_the_oil() {
    // 021437Z at t153: 7 Oil, no income, two Infantry already burning it.
    let (mut g, ai, city) = fixture(7.0, 0.0, STEEL_AGE);
    let pos = g.cities[&city].pos;
    for _ in 0..2 {
        g.spawn_test_unit("infantry", 0, pos);
    }
    for kind in ["infantry", "tank"] {
        assert!(
            !ai.guns_keep_the_oil_item(&g, 0, city, &unit(kind)),
            "{kind}"
        );
        assert!(
            off(&ai).guns_keep_the_oil_item(&g, 0, city, &unit(kind)),
            "{kind}"
        );
    }
    // The guns themselves, and a capture body that burns nothing, still go.
    for kind in ["artillery", "at_crew", "machine_gun"] {
        assert!(
            ai.guns_keep_the_oil_item(&g, 0, city, &unit(kind)),
            "{kind}"
        );
    }
    let plan = ai.plan.clone().unwrap();
    let counts = ai.counts(&g, 0);
    assert_eq!(
        ai.production_value(&g, 0, city, &unit("infantry"), &plan, &counts),
        -10_000.0
    );
}

#[test]
fn a_first_infantry_waits_on_a_bank_the_first_gun_needs() {
    // 021437Z at t147-149: 14 Oil banked, no income, no gun fielded yet.
    // 14 - 1 + 10 * (0 - 1) = 3 against the next gun's 1 + 10 * 1 = 11.
    let (g, ai, city) = fixture(14.0, 0.0, STEEL_AGE);
    assert!(!ai.guns_keep_the_oil_item(&g, 0, city, &unit("infantry")));
    // With the bank deep enough for both, it trains: 25 - 1 - 10 = 14 >= 11.
    let (rich, ai, city) = fixture(25.0, 0.0, STEEL_AGE);
    assert!(ai.guns_keep_the_oil_item(&rich, 0, city, &unit("infantry")));
}

#[test]
fn an_empire_rich_in_oil_trains_its_infantry() {
    // 230117Z at t170: 25 Oil and +9 a turn feed two Artillery, three
    // Infantry and a Tank: 25 - 1 + 10 * (9 - 4 - 1) = 64 against 31.
    let (mut g, ai, city) = fixture(25.0, 9.0, STEEL_AGE);
    let pos = g.cities[&city].pos;
    for kind in [
        "artillery",
        "artillery",
        "infantry",
        "infantry",
        "infantry",
        "tank",
    ] {
        g.spawn_test_unit(kind, 0, pos);
    }
    assert!(ai.guns_keep_the_oil_item(&g, 0, city, &unit("infantry")));
    assert!(ai.guns_keep_the_oil_item(&g, 0, city, &unit("tank")));
}

#[test]
fn the_line_infantry_keeps_its_rifle_while_the_guns_are_short() {
    // 230117Z at t135: two Artillery fielded, 10 Oil, +3 a turn.
    // 10 - 1 + 10 * (3 - 0 - 1) = 29 against 1 + 10 * (2 + 1) = 31.
    let (mut g, ai, city) = fixture(10.0, 3.0, STEEL_AGE);
    let pos = g.cities[&city].pos;
    for _ in 0..2 {
        g.spawn_test_unit("artillery", 0, pos);
    }
    let rifle = g.spawn_test_unit("line_infantry", 0, pos);
    let infantry = crate::name!("infantry");
    assert_eq!(g.unit_upgrade_target(0, "line_infantry"), Some(infantry));
    assert!(!ai.air_resource_upgrade_preserves_wing(&g, 0, rifle, infantry, 1.0));
    assert!(off(&ai).air_resource_upgrade_preserves_wing(&g, 0, rifle, infantry, 1.0));
    // A Bombard becoming an Artillery is the guns' own upgrade.
    let bombard = g.spawn_test_unit("bombard", 0, pos);
    let artillery = crate::name!("artillery");
    assert_eq!(g.unit_upgrade_target(0, "bombard"), Some(artillery));
    assert!(ai.air_resource_upgrade_preserves_wing(&g, 0, bombard, artillery, 1.0));
}

#[test]
fn a_queued_infantry_past_half_paid_finishes() {
    let (mut g, ai, city) = fixture(14.0, 0.0, STEEL_AGE);
    let infantry = unit("infantry");
    g.cities.get_mut(&city).unwrap().queue = vec![infantry.clone()];
    g.cities.get_mut(&city).unwrap().production = 0.0;
    // Its Oil is paid, but its upkeep still leaves the next gun short:
    // 14 + 10 * (0 - 1) = 4 against 11.
    assert!(!ai.guns_keep_the_oil_item(&g, 0, city, &infantry));
    let cost = g.item_cost_for_city(0, city, &infantry);
    g.cities.get_mut(&city).unwrap().production = 0.6 * cost;
    assert!(ai.guns_keep_the_oil_item(&g, 0, city, &infantry));
}

#[test]
fn the_guns_fuel_is_never_sold_before_the_first_gun() {
    let (g, ai, _) = fixture(14.0, 0.0, STEEL_AGE);
    assert!(ai
        .siege_gun_resources_held(&g, 0, &[])
        .contains("RESOURCE_OIL"));
    assert!(off(&ai).siege_gun_resources_held(&g, 0, &[]).is_empty());
}

#[test]
fn the_journal_says_when_the_oil_is_held() {
    let held = |ai: &AdvancedAi| {
        ai.journal()
            .since(0)
            .thoughts
            .iter()
            .filter(|thought| thought.detail.contains("guns-keep-the-oil"))
            .count()
    };
    let (g, mut ai, _) = fixture(14.0, 0.0, STEEL_AGE);
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.note_guns_keep_the_oil(&g, 0);
    assert_eq!(held(&ai), 1);
    // A bank that carries the guns and a new consumer is silent.
    let (rich, mut ai, _) = fixture(25.0, 0.0, STEEL_AGE);
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.note_guns_keep_the_oil(&rich, 0);
    assert_eq!(held(&ai), 0);
    let mut off = off(&ai);
    off.base.journal = crate::reasoning::Journal::recording();
    off.note_guns_keep_the_oil(&g, 0);
    assert_eq!(held(&off), 0);
}
