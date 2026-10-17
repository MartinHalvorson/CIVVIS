use super::super::siege_train::{Siege, SiegeStage};
use super::super::*;
use super::tests::siege_gap_case;

fn stalled_health_case() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let (mut game, mut controller, plan, home_city, target_city) = siege_gap_case();
    controller.enable_lane_delegates_production_2();
    controller.enable_siege_train();
    controller.enable_siege_positive_damage_budget();
    controller.enable_breakers_match_the_walls();
    controller.enable_one_war_at_a_time();
    game.cities.get_mut(&target_city).unwrap().wall_hp = 0;
    std::sync::Arc::make_mut(&mut game.observed_city_strength).insert(target_city, 50.0);
    controller.one_war_observe(&game, 0);
    assert_eq!(controller.one_war_front(), Some(1));
    let target_position = game.cities[&target_city].pos;
    controller.front_city_low.insert(
        target_position,
        (200, game.turn - game.standard_duration(4)),
    );
    controller.sieges.insert(
        target_city,
        Siege {
            stage: SiegeStage::Reduce,
            taker: None,
            entered: game.turn - game.standard_duration(4),
            assessed: game.turn,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
    assert!(game.players[1].techs.is_empty());
    assert_eq!(game.cities[&target_city].hp, 200);
    assert!(game.is_at_war(0, 1));
    assert!(game.can_produce(
        0,
        home_city,
        &Item::Unit {
            unit: crate::name!("catapult")
        }
    ));
    (game, controller, plan, home_city, target_city)
}

#[test]
fn a_stalled_breached_city_prices_fire_against_its_remaining_health() {
    let (game, controller, _, _, target_city) = stalled_health_case();
    let matched = controller
        .breakers_matched_to_walls(&game, 0, target_city)
        .expect("a live siege has stopped reducing the city's health");
    let (guns, breach_turns, take_turns) = matched.ask.unwrap();
    assert_eq!(guns, 3);
    assert_eq!(breach_turns, 0.0);
    assert!(take_turns <= matched.target_turns);
    assert_eq!(
        controller.breakers_match_the_row(&game, 0, target_city, 0),
        guns
    );
}

#[test]
fn a_stalled_unwalled_front_orders_fire_without_private_enemy_technology() {
    let (mut game, controller, plan, home_city, target_city) = stalled_health_case();
    std::sync::Arc::make_mut(&mut game.observed_city_max_wall_hp).insert(target_city, 0);
    assert_eq!(game.city_max_wall_hp(&game.cities[&target_city]), 0);
    assert!(!game.players[1].techs.contains(&crate::name!("masonry")));
    let weapon = Item::Unit {
        unit: crate::name!("catapult"),
    };
    assert_eq!(
        controller.reserve_delegated_domination_siege(&mut game, 0, &plan),
        Some((home_city, weapon.clone()))
    );
    assert_eq!(game.cities[&home_city].queue.first(), Some(&weapon));
    assert!(game.log.since(0).any(|(owner, action)| {
        *owner == 0
            && matches!(action, Action::Produce { city, item } if *city == home_city && *item == weapon)
    }));
}

#[test]
fn the_recovery_governor_keeps_a_missing_health_weapon_reservation() {
    let (game, controller, mut plan, home_city, target_city) = stalled_health_case();
    plan.strategy = GrandStrategy::Recovery;
    let counts = controller.counts(&game, 0);
    assert_eq!(counts.siege, 0);
    assert_eq!(game.cities[&target_city].wall_hp, 0);
    assert!(controller.missing_domination_siege(
        &game,
        0,
        home_city,
        &plan,
        &counts,
        &game.rules.units[&crate::name!("catapult")]
    ));
}

#[test]
fn a_health_fire_shortfall_can_supply_more_than_the_first_gun() {
    let (mut game, controller, plan, home_city, target_city) = stalled_health_case();
    let home_position = game.cities[&home_city].pos;
    game.spawn_unit("catapult", 0, home_position);
    assert_eq!(controller.counts(&game, 0).siege, 1);
    assert_eq!(
        controller.breakers_matched_guns(&game, 0, target_city),
        Some(3)
    );
    assert!(controller
        .reserve_delegated_domination_siege(&mut game, 0, &plan)
        .is_some());
}

#[test]
fn enough_existing_health_guns_close_the_reservation() {
    let (mut game, controller, plan, home_city, _) = stalled_health_case();
    let home_position = game.cities[&home_city].pos;
    for _ in 0..3 {
        game.spawn_unit("catapult", 0, home_position);
    }
    assert_eq!(controller.counts(&game, 0).siege, 3);
    assert!(controller
        .reserve_delegated_domination_siege(&mut game, 0, &plan)
        .is_none());
}

#[test]
fn a_committed_gun_keeps_its_health_role_while_the_city_is_being_reduced() {
    let (mut game, mut controller, _, _, target_city) = stalled_health_case();
    let target_position = game.cities[&target_city].pos;
    let firing_post = game.nbrs(target_position)[0];
    game.map.tiles.get_mut(&firing_post).unwrap().terrain = crate::name!("grassland");
    game.map.tiles.get_mut(&firing_post).unwrap().feature = None;
    let gun = game.spawn_unit("catapult", 0, firing_post);
    controller
        .sieges
        .get_mut(&target_city)
        .unwrap()
        .posts
        .insert(gun, firing_post);
    game.cities.get_mut(&target_city).unwrap().hp = 120;
    controller
        .front_city_low
        .insert(target_position, (120, game.turn));
    assert_eq!(
        controller.breakers_matched_guns(&game, 0, target_city),
        Some(2)
    );
    assert!(controller.breakers_match_the_row(&game, 0, target_city, 0) > 0);
}

#[test]
fn a_health_fire_ask_requires_an_active_stalled_hostile_front() {
    for condition in [
        "recent_progress",
        "no_siege",
        "held_capture",
        "stale_siege",
        "peace",
        "low_health",
        "no_front",
        "disabled_matching",
        "disabled_budget",
        "disabled_train",
        "unobserved_breach",
        "archer_post",
    ] {
        let (mut game, mut controller, _, _, target_city) = stalled_health_case();
        let target_position = game.cities[&target_city].pos;
        match condition {
            "recent_progress" => {
                controller
                    .front_city_low
                    .insert(target_position, (200, game.turn));
            }
            "no_siege" => {
                controller.sieges.remove(&target_city);
            }
            "held_capture" => {
                controller.sieges.get_mut(&target_city).unwrap().stage = SiegeStage::Hold;
            }
            "stale_siege" => {
                controller.sieges.get_mut(&target_city).unwrap().assessed = game.turn - 2;
            }
            "peace" => {
                game.at_war.clear();
            }
            "low_health" => {
                game.cities.get_mut(&target_city).unwrap().hp = 100;
            }
            "no_front" => controller.disable_one_war_at_a_time(),
            "disabled_matching" => controller.disable_breakers_match_the_walls(),
            "disabled_budget" => controller.disable_siege_positive_damage_budget(),
            "disabled_train" => controller.disable_siege_train(),
            "unobserved_breach" => {
                controller.front_city_low.insert(
                    target_position,
                    (400, game.turn - game.standard_duration(4)),
                );
            }
            "archer_post" => {
                let archer = game.spawn_unit("archer", 0, target_position);
                controller
                    .sieges
                    .get_mut(&target_city)
                    .unwrap()
                    .posts
                    .insert(archer, target_position);
                game.cities.get_mut(&target_city).unwrap().hp = 100;
            }
            _ => unreachable!(),
        }
        assert_eq!(
            controller.breakers_matched_to_walls(&game, 0, target_city),
            None,
            "{condition} must not create a health-fire requirement"
        );
    }
}
