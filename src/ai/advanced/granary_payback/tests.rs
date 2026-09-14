use super::*;
use crate::game::Action;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn board() -> (Game, u32, StrategicPlan) {
    let mut g = Game::new_full(1, 24, 16, 914_371_002, 250, 0, false);
    g.game_speed = GameSpeed::Online;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.players[0].techs.insert(crate::name!("pottery"));
    let cid = g.player_city_ids(0)[0];
    g.cities.get_mut(&cid).unwrap().pop = 4;
    g.cities.get_mut(&cid).unwrap().food = 0.0;
    g.cities.get_mut(&cid).unwrap().loyalty = 100.0;
    set_housing(&mut g, cid, 4.0);
    set_amenities(&mut g, cid, 0);
    set_yields(&mut g, cid, 18.0, 10.0);
    let plan = StrategicPlan {
        strategy: super::super::GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, cid, plan)
}

fn set_housing(g: &mut Game, cid: u32, housing: f64) {
    Arc::make_mut(&mut g.observed_city_housing_adjustments).remove(&cid);
    let current = g.city_housing(&g.cities[&cid]);
    Arc::make_mut(&mut g.observed_city_housing_adjustments).insert(cid, housing - current);
}

fn set_amenities(g: &mut Game, cid: u32, amenities: i64) {
    Arc::make_mut(&mut g.observed_city_amenity_adjustments).remove(&cid);
    let current = g.city_amenity_surplus(&g.cities[&cid]);
    Arc::make_mut(&mut g.observed_city_amenity_adjustments).insert(cid, amenities - current);
}

fn set_yields(g: &mut Game, cid: u32, food: f64, production: f64) {
    Arc::make_mut(&mut g.observed_city_yield_adjustments).remove(&cid);
    let current = g.city_yields(cid);
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        Yields {
            food: food - current.food,
            production: production - current.production,
            ..Yields::default()
        },
    );
}

fn ai() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_first_granary_reserve_2();
    ai
}

#[test]
fn granary_payback_reserves_a_prompt_citizen_and_keeps_versions_exclusive() {
    let (g, cid, plan) = board();
    let mut ai = ai();
    assert!(ai.granary_growth_pays(&g, 0, cid, &plan));
    assert!(!ai.first_granary_reserve);
    ai.enable_first_granary_reserve();
    assert!(!ai.first_granary_reserve_2);
    assert!(!ai.granary_growth_pays(&g, 0, cid, &plan));
    ai.enable_first_granary_reserve_2();
    ai.disable_first_granary_reserve_2();
    assert!(!ai.granary_growth_pays(&g, 0, cid, &plan));
    super::super::test_support::opt_in_off_in_both_controllers("first-granary-reserve-2", |ai| {
        ai.first_granary_reserve_2
    });
}

#[test]
fn granary_payback_yields_when_housing_cannot_restore_growth() {
    for reason in ["food", "amenities", "loyalty", "housing_still_binds"] {
        let (mut g, cid, plan) = board();
        match reason {
            "food" => set_yields(&mut g, cid, 8.0, 10.0),
            "amenities" => set_amenities(&mut g, cid, -5),
            "loyalty" => g.cities.get_mut(&cid).unwrap().loyalty = 25.0,
            "housing_still_binds" => set_housing(&mut g, cid, 1.0),
            _ => unreachable!(),
        }
        assert!(!ai().granary_growth_pays(&g, 0, cid, &plan), "{reason}");
    }
}

#[test]
fn granary_payback_requires_time_to_build_then_grow() {
    let (mut g, cid, mut plan) = board();
    g.turn = g.max_turns - 5;
    assert!(!ai().granary_growth_pays(&g, 0, cid, &plan));
    g.turn = 50;
    plan.threatened_city = Some(cid);
    assert!(!ai().granary_growth_pays(&g, 0, cid, &plan));
    plan.threatened_city = None;
    g.cities.get_mut(&cid).unwrap().last_attacked = 49;
    assert!(!ai().granary_growth_pays(&g, 0, cid, &plan));
    g.cities.get_mut(&cid).unwrap().last_attacked = 0;
    set_yields(&mut g, cid, 18.0, 1.0);
    assert!(!ai().granary_growth_pays(&g, 0, cid, &plan));
    // A nearly completed building has a different investment bill.
    let item = Item::Building {
        building: crate::name!("granary"),
    };
    g.cities.get_mut(&cid).unwrap().production = g.item_cost_for_city(0, cid, &item) - 1.0;
    assert!(ai().granary_growth_pays(&g, 0, cid, &plan));
}

#[test]
fn granary_payback_waits_for_a_citizen_already_about_to_arrive() {
    let (mut g, cid, plan) = board();
    g.cities.get_mut(&cid).unwrap().food = g.growth_cost(4) - 1.0;
    assert!(!ai().granary_growth_pays(&g, 0, cid, &plan));
    assert_eq!(growth_arrivals(12.0, 2.0, 4.0, 3.0), Some((6.0, 5.0)));
    assert_eq!(growth_arrivals(6.0, 2.0, 4.0, 3.0), None);
    assert_eq!(
        growth_arrivals(12.0, 0.0, 4.0, 3.0),
        Some((f64::INFINITY, 6.0))
    );
}

#[test]
fn granary_payback_uses_the_same_policy_at_both_speeds() {
    for speed in [GameSpeed::Online, GameSpeed::Standard] {
        let (mut g, cid, plan) = board();
        g.game_speed = speed;
        assert!(ai().granary_growth_pays(&g, 0, cid, &plan), "{speed:?}");
    }
}

#[test]
fn granary_payback_changes_the_reservation_before_an_owed_library() {
    let (mut original, cid, plan) = board();
    crate::game::install_test_district(&mut original, cid, "campus");
    original.players[0].techs.insert(crate::name!("writing"));
    set_yields(&mut original, cid, 8.0, 10.0);
    let mut candidate = original.clone();
    let mut controller = AdvancedAi::new();
    controller.enable_first_granary_reserve();
    controller.advanced_production(&mut original, 0, &plan, false);
    controller.enable_first_granary_reserve_2();
    controller.advanced_production(&mut candidate, 0, &plan, false);
    assert_eq!(
        original.cities[&cid].queue.first(),
        Some(&Item::Building {
            building: crate::name!("granary")
        })
    );
    assert_eq!(
        candidate.cities[&cid].queue.first(),
        Some(&Item::Building {
            building: crate::name!("library")
        })
    );
}
