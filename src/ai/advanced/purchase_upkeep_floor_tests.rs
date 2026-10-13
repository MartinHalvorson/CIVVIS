//! `purchases-keep-the-upkeep-reserve`: every guarded Gold buyer keeps one
//! and a half turns of the army's bill, and each is untouched while the gene
//! is off.

use super::*;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};
use crate::game::HostMaintenance;
use std::sync::Arc;

/// The host's unit bill, as a live seat exports it.
fn bill(g: &mut Game, units: f64) {
    Arc::make_mut(&mut g.host_maintenance).insert(
        0,
        HostMaintenance {
            units: Some(units),
            buildings: None,
            districts: None,
        },
    );
}

fn one_city() -> (Game, u32) {
    let mut g = Game::new_full(1, 20, 14, 320, 30, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    (g, cid)
}

#[test]
fn the_purchase_floor_keeps_the_armys_bill_only_while_on() {
    let (mut g, _) = one_city();
    bill(&mut g, 140.0);
    let mut ai = AdvancedAi::new();
    assert_eq!(
        ai.purchase_upkeep_floor(&g, 0, 30.0),
        30.0,
        "off by default"
    );
    ai.enable_purchases_keep_the_upkeep_reserve();
    assert_eq!(ai.purchase_upkeep_floor(&g, 0, 30.0), 210.0);
    assert_eq!(
        ai.purchase_upkeep_floor(&g, 0, 300.0),
        300.0,
        "a larger floor stands"
    );
    ai.disable_purchases_keep_the_upkeep_reserve();
    assert_eq!(ai.purchase_upkeep_floor(&g, 0, 30.0), 30.0);
}

/// The baseline buyer (the one that bought civvis-20261013T220550Z's Arena
/// and Lighthouse) is handed the floor through `reserve_floor`. A 240 Monument
/// out of 365 leaves 125, the one-city peacetime reserve: bought with the gene
/// off, held with a 100-Gold unit bill (floor 150) and the gene on.
#[test]
fn the_baseline_buyer_keeps_the_upkeep_reserve() {
    let (mut g, cid) = one_city();
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|building| building != "monument");
    bill(&mut g, 100.0);
    let mut ai = AdvancedAi::new();

    ai.enable_purchases_keep_the_upkeep_reserve();
    g.players[0].gold = 365.0;
    ai.base.reserve_floor = ai.purchase_upkeep_floor(&g, 0, 0.0);
    assert_eq!(ai.base.reserve_floor, 150.0);
    assert!(
        !ai.base.spend_gold(&mut g, 0, &[cid], 1, 1, 1, 2, 1, 1),
        "125 left would sit under one and a half turns of a 100-Gold bill"
    );
    assert_eq!(g.players[0].gold, 365.0);
    g.players[0].gold = 390.0;
    assert!(ai.base.spend_gold(&mut g, 0, &[cid], 1, 1, 1, 2, 1, 1));
    assert!(g.cities[&cid].buildings.iter().any(|b| b == "monument"));
    assert_eq!(g.players[0].gold, 150.0, "the bill's floor is left behind");

    let (mut g, cid) = one_city();
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|building| building != "monument");
    bill(&mut g, 100.0);
    ai.disable_purchases_keep_the_upkeep_reserve();
    g.players[0].gold = 365.0;
    ai.base.reserve_floor = ai.purchase_upkeep_floor(&g, 0, 0.0);
    assert_eq!(ai.base.reserve_floor, 0.0, "off, the shipped floor");
    assert!(ai.base.spend_gold(&mut g, 0, &[cid], 1, 1, 1, 2, 1, 1));
    assert_eq!(g.players[0].gold, 125.0);
}

/// The main purchase pass: the Science board of
/// `strategic_gold_purchase_buys_science_tempo_but_preserves_the_reserve`
/// buys its Library out of 1,000 Gold above a 300 reserve. With a 500-Gold
/// unit bill the floor is 750: 1,000 buys no Library and nothing that dips
/// under 750, and the Library comes once the bank covers price and floor.
#[test]
fn the_main_purchase_pass_keeps_the_upkeep_reserve() {
    let (mut game, city) = one_city();
    let campus = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), campus);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .extend([crate::name!("monument"), crate::name!("granary")]);
    game.players[0].techs.insert(crate::name!("writing"));
    game.spawn_test_unit("builder", 0, game.cities[&city].pos);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    let library = |g: &Game| g.cities[&city].buildings.contains(&crate::name!("library"));
    let price = game
        .building_gold_purchase_cost(0, city, "library")
        .expect("the Library is for sale");

    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    let mut off = game.clone();
    off.players[0].gold = 1_000.0;
    assert!(
        ai.advanced_gold_spending(&mut off, 0, &plan),
        "off, the stock buy"
    );
    assert!(library(&off));

    bill(&mut game, 500.0);
    ai.enable_purchases_keep_the_upkeep_reserve();
    game.players[0].gold = 1_000.0;
    ai.advanced_gold_spending(&mut game, 0, &plan);
    assert!(
        !library(&game),
        "a {price:.0} Library out of 1,000 would dip under 750"
    );
    assert!(
        game.players[0].gold >= 750.0,
        "nothing else dips under it either"
    );

    game.players[0].gold = 750.0 + price;
    assert!(ai.advanced_gold_spending(&mut game, 0, &plan));
    assert!(library(&game));
    assert!(game.players[0].gold >= 750.0);
}

/// The city-state levy: one city-state on three of our envoys. Off (floor
/// zero) the levy spends down to the urgent share; on, with a 100-Gold unit
/// bill, it leaves 150 behind.
#[test]
fn the_levy_keeps_the_upkeep_reserve() {
    let mut g = Game::new_full(2, 30, 20, 4_471, 250, 1, false);
    let minor = g
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .expect("one city-state");
    if g.levy_cost(0, minor).is_none() {
        g.players[0].envoys.push((minor, 3));
    }
    if g.levy_cost(0, minor).is_none() {
        let pos = g
            .units
            .values()
            .find(|unit| unit.owner == minor)
            .map(|unit| unit.pos)
            .or_else(|| g.cities.values().find(|c| c.owner == minor).map(|c| c.pos))
            .expect("the city-state stands somewhere");
        g.spawn_test_unit("warrior", minor, pos);
    }
    let cost = g
        .levy_cost(0, minor)
        .expect("we are Suzerain of an armed city-state");
    bill(&mut g, 100.0);
    let mut ai = AdvancedAi::new();
    let levied = |g: &Game| g.units.values().any(|unit| unit.levied_from.is_some());

    let mut off = g.clone();
    off.players[0].gold = cost + 100.0;
    let floor = ai.purchase_upkeep_floor(&off, 0, 0.0);
    assert_eq!(floor, 0.0);
    ai.base.levy_city_state_military(&mut off, 0, true, floor);
    assert!(levied(&off), "off, the urgent share covers {cost:.0}");

    ai.enable_purchases_keep_the_upkeep_reserve();
    g.players[0].gold = cost + 100.0;
    let floor = ai.purchase_upkeep_floor(&g, 0, 0.0);
    assert_eq!(floor, 150.0);
    ai.base.levy_city_state_military(&mut g, 0, true, floor);
    assert!(!levied(&g), "100 left would sit under the 150 floor");
    assert_eq!(g.players[0].gold, cost + 100.0);

    g.players[0].gold = cost + 150.0;
    ai.base.levy_city_state_military(&mut g, 0, true, floor);
    assert!(levied(&g));
    assert!(g.players[0].gold + f64::EPSILON >= 150.0);
}

/// The delegated governor (`delegated_cities`) runs the baseline buyer
/// itself; on civvis-20261013T220550Z it bought the turn-198 Lighthouse. With
/// a 200-Gold unit bill (floor 300) and 365 Gold, off spends below the floor
/// and on leaves it standing; the lent floor is handed back either way.
#[test]
fn the_delegated_governor_keeps_the_upkeep_reserve() {
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: 0,
        rush: false,
    };
    for on in [false, true] {
        let (mut g, cid) = one_city();
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .retain(|building| building != "monument");
        bill(&mut g, 200.0);
        g.players[0].gold = 365.0;
        let mut ai = AdvancedAi::new();
        if on {
            ai.enable_purchases_keep_the_upkeep_reserve();
        }
        ai.delegated_cities(&mut g, 0, &plan);
        assert_eq!(ai.base.reserve_floor, 0.0, "the lent floor is handed back");
        if on {
            assert!(
                g.players[0].gold >= 300.0,
                "on, nothing dips under the floor: {}",
                g.players[0].gold
            );
        } else {
            assert!(
                g.players[0].gold < 300.0,
                "off, the baseline buyer spends below it: {}",
                g.players[0].gold
            );
        }
    }
}
