use super::super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use super::SIEGE_TRAIN_WAR_LIMIT;
use crate::game::{Game, Item};

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 373200, 300, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (8, 10), None);
    let city = g.found_city_for(1, (20, 10), None);
    g.found_city_for(1, (29, 10), None);
    g.players[1].techs.insert(crate::name!("steel"));
    g.cities.get_mut(&city).unwrap().wall_hp = 400;
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.current = 0;
    g.turn = 120;
    let unit = g.spawn_test_unit("modern_armor", 0, (19, 10));
    g.spawn_test_unit("modern_armor", 0, (18, 11));
    g.spawn_test_unit("warrior", 1, (29, 11));
    // A reserve far from both cities keeps the front short of crushed
    // (`domination_front_crushed`), so a stalled war can still tire.
    g.spawn_test_unit("modern_armor", 1, (36, 20));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.major_war_since = Some(80);
    ai.last_campaign_progress = 80;
    ai.last_city_count = g.player_city_ids(0).len();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(city),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 120,
        rush: false,
    };
    (g, ai, plan, city, unit)
}

fn damage(g: &mut Game, ai: &mut AdvancedAi, city: u32, turn: u32, walls: i32) {
    g.turn = turn;
    g.cities.get_mut(&city).unwrap().wall_hp = walls;
    ai.observe_campaign(g, 0);
}

#[test]
fn committed_wall_breaker_survives_fatigue_until_its_bounded_arrival_window_ends() {
    let (mut g, mut ai, plan, _, _) = fixture();
    // Native King run civvis-20260928T090730Z: the Ottoman war began on
    // turn 107, Cuenca started a Bombard on 122, and the peace desk called
    // the war stalled on 131, before the Bombard emerged on 134.
    ai.major_war_since = Some(g.turn - 25);
    let home = g.player_city_ids(0)[0];
    g.cities.get_mut(&home).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("bombard"),
    }];
    assert!(ai.domination_siege_train_mobilizing(&g, 0, 1, &plan));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!ai.peace_offers.contains(&1));

    let (mut fielded, mut fielded_ai, fielded_plan, _, _) = fixture();
    fielded_ai.major_war_since = Some(fielded.turn - 27);
    let gun = fielded.spawn_test_unit("bombard", 0, (8, 10));
    assert!(fielded_ai.domination_siege_train_mobilizing(&fielded, 0, 1, &fielded_plan));
    fielded_ai.advanced_diplomacy(&mut fielded, 0, &fielded_plan);
    assert!(!fielded_ai.peace_offers.contains(&1));
    fielded.remove_unit(gun);
    assert!(!fielded_ai.domination_siege_train_mobilizing(&fielded, 0, 1, &fielded_plan));

    let (mut expired, mut expired_ai, expired_plan, _, _) = fixture();
    expired_ai.major_war_since = Some(expired.turn - SIEGE_TRAIN_WAR_LIMIT);
    let home = expired.player_city_ids(0)[0];
    expired.cities.get_mut(&home).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("bombard"),
    }];
    assert!(!expired_ai.domination_siege_train_mobilizing(&expired, 0, 1, &expired_plan));
    expired_ai.advanced_diplomacy(&mut expired, 0, &expired_plan);
    assert!(expired_ai.peace_offers.contains(&1));
}

#[test]
fn white_peace_does_not_interrupt_a_committed_domination_siege_train() {
    let (mut g, mut ai, plan, _, _) = fixture();
    ai.major_war_since = Some(g.turn - 25);
    let home = g.player_city_ids(0)[0];
    g.cities.get_mut(&home).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("bombard"),
    }];
    let deal = crate::game::DiplomaticDeal {
        id: 1,
        from: 1,
        to: 0,
        give_gold: 0.0,
        request_gold: 0.0,
        open_borders: false,
        friendship: false,
        peace: true,
        alliance: None,
        defensive_pact: false,
        joint_war_target: None,
        promise: None,
        demand: false,
        expires: g.turn + SIEGE_TRAIN_WAR_LIMIT,
    };
    assert!(ai.incoming_deal_value(&g, 0, &deal, &plan) < 0.0);
    g.turn += SIEGE_TRAIN_WAR_LIMIT - 25;
    assert!(ai.incoming_deal_value(&g, 0, &deal, &plan) > 0.0);
}

#[test]
fn substantial_domination_siege_progress_prevents_stalled_peace() {
    let (mut g, mut ai, plan, city, _) = fixture();
    ai.observe_campaign(&g, 0);
    damage(&mut g, &mut ai, city, 121, 240);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!ai.peace_offers.contains(&1));
}

#[test]
fn a_new_quarter_of_defenses_buys_another_bounded_extension() {
    let (mut g, mut ai, plan, city, _) = fixture();
    ai.observe_campaign(&g, 0);
    damage(&mut g, &mut ai, city, 121, 240);
    for turn in 122..131 {
        damage(&mut g, &mut ai, city, turn, 240);
    }
    damage(&mut g, &mut ai, city, 131, 90);
    g.turn = 133;
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!ai.peace_offers.contains(&1));
}

#[test]
fn small_hits_and_repeated_damage_do_not_postpone_peace_forever() {
    for case in ["small", "repeat", "expired", "first_seen_damaged"] {
        let (mut g, mut ai, plan, city, _) = fixture();
        if case == "first_seen_damaged" {
            g.cities.get_mut(&city).unwrap().wall_hp = 240;
        }
        ai.observe_campaign(&g, 0);
        if case == "small" {
            damage(&mut g, &mut ai, city, 121, 399);
        } else if case != "first_seen_damaged" {
            damage(&mut g, &mut ai, city, 121, 240);
            if case == "repeat" {
                damage(&mut g, &mut ai, city, 129, 400);
                damage(&mut g, &mut ai, city, 130, 240);
            }
            g.turn = 133;
        }
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert!(ai.peace_offers.contains(&1), "{case}");
    }
}

#[test]
fn siege_credit_requires_our_present_army_and_current_campaign() {
    for case in ["absent", "departed", "other_target", "recovery", "lane"] {
        let (mut g, mut ai, mut plan, city, _) = fixture();
        if case == "lane" {
            ai.victory_target = Some(VictoryTarget::Science);
        }
        ai.observe_campaign(&g, 0);
        if case == "absent" {
            for u in g.units.values_mut().filter(|u| u.owner == 0) {
                u.pos = (8, 10);
            }
        }
        damage(&mut g, &mut ai, city, 121, 240);
        match case {
            "departed" => {
                for u in g.units.values_mut().filter(|u| u.owner == 0) {
                    u.pos = (8, 10);
                }
            }
            "other_target" => plan.target_city = None,
            "recovery" => {
                plan.strategy = GrandStrategy::Recovery;
                plan.target_player = None;
            }
            _ => {}
        }
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert!(ai.peace_offers.contains(&1), "{case}");
    }
}

#[test]
fn substantial_progress_does_not_block_an_outmatched_peace_offer() {
    let (mut g, mut ai, plan, city, _) = fixture();
    ai.observe_campaign(&g, 0);
    damage(&mut g, &mut ai, city, 121, 240);
    for pos in [(30, 12), (31, 12), (32, 12), (33, 12)] {
        g.spawn_test_unit("giant_death_robot", 1, pos);
    }
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(ai.peace_offers.contains(&1));
}

#[test]
fn returning_after_a_visibility_gap_does_not_claim_unobserved_damage() {
    let (mut g, mut ai, plan, city, _) = fixture();
    ai.observe_campaign(&g, 0);
    // No intervening observation: the army cannot attribute newly seen damage.
    damage(&mut g, &mut ai, city, 124, 240);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(ai.peace_offers.contains(&1));
}

#[test]
fn incoming_white_peace_does_not_cash_out_a_progressing_siege() {
    let (mut g, mut ai, plan, city, _) = fixture();
    ai.observe_campaign(&g, 0);
    damage(&mut g, &mut ai, city, 121, 240);
    let deal = crate::game::DiplomaticDeal {
        id: 1,
        from: 1,
        to: 0,
        give_gold: 0.0,
        request_gold: 0.0,
        open_borders: false,
        friendship: false,
        peace: true,
        alliance: None,
        defensive_pact: false,
        joint_war_target: None,
        promise: None,
        demand: false,
        expires: 150,
    };
    assert!(ai.incoming_deal_value(&g, 0, &deal, &plan) < 0.0);
    g.turn = 133;
    assert!(ai.incoming_deal_value(&g, 0, &deal, &plan) > 0.0);
}

#[test]
fn a_larger_defense_capacity_is_not_damage_progress() {
    let (mut g, mut ai, plan, city, _) = fixture();
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(city, 200);
    g.cities.get_mut(&city).unwrap().wall_hp = 200;
    ai.observe_campaign(&g, 0);
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(city, 400);
    damage(&mut g, &mut ai, city, 121, 200);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(ai.peace_offers.contains(&1));
}

#[test]
fn peace_clears_old_siege_credit_before_a_later_war() {
    let (mut g, mut ai, plan, city, _) = fixture();
    ai.observe_campaign(&g, 0);
    damage(&mut g, &mut ai, city, 121, 240);
    assert!(ai.domination_siege_is_progressing(&g, 0, 1, &plan));
    g.at_war.clear();
    g.turn = 122;
    ai.observe_campaign(&g, 0);
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.turn = 123;
    ai.observe_campaign(&g, 0);
    assert!(!ai.domination_siege_is_progressing(&g, 0, 1, &plan));
}

#[test]
fn mirror_city_renumbering_preserves_progress_at_the_same_owned_location() {
    let (mut g, mut ai, mut plan, city, _) = fixture();
    ai.observe_campaign(&g, 0);
    g.clear_mirror_cities();
    g.found_city_for(0, (8, 10), None);
    g.found_city_for(1, (29, 10), None);
    let rebuilt = g.found_city_for(1, (20, 10), None);
    assert_ne!(rebuilt, city);
    plan.target_city = Some(rebuilt);
    damage(&mut g, &mut ai, rebuilt, 121, 240);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!ai.peace_offers.contains(&1));
}

#[test]
fn observed_damage_after_a_wall_upgrade_counts_toward_the_fixed_budget() {
    let (mut g, mut ai, plan, city, _) = fixture();
    // Native Kristiansand: 300 walls first observed, upgraded to400,
    // then six consecutive turns of damage before a stalled-war peace offer.
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(city, 300);
    g.cities.get_mut(&city).unwrap().wall_hp = 300;
    ai.observe_campaign(&g, 0);
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(city, 400);
    damage(&mut g, &mut ai, city, 121, 400);
    for (turn, walls) in [
        (122, 377),
        (123, 352),
        (124, 327),
        (125, 304),
        (126, 281),
        (127, 256),
    ] {
        damage(&mut g, &mut ai, city, turn, walls);
    }
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        !ai.peace_offers.contains(&1),
        "144 real damage exceeds a quarter of the initial500HP budget"
    );
    // The same damage cannot hold this war open indefinitely.
    g.turn = 139;
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(ai.peace_offers.contains(&1));
}

#[test]
fn upgraded_defenses_do_not_expand_or_replenish_the_four_milestone_budget() {
    let (mut g, mut ai, plan, city, _) = fixture();
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(city, 0);
    g.cities.get_mut(&city).unwrap().wall_hp = 0;
    ai.observe_campaign(&g, 0);
    std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(city, 400);
    damage(&mut g, &mut ai, city, 121, 400);
    damage(&mut g, &mut ai, city, 122, 300);
    damage(&mut g, &mut ai, city, 123, 0);
    let key = (1, g.cities[&city].pos);
    assert_eq!(ai.domination_siege_milestones[&key].greatest_quarter, 4);
    assert!(ai.domination_siege_is_progressing(&g, 0, 1, &plan));
    damage(&mut g, &mut ai, city, 134, 400);
    damage(&mut g, &mut ai, city, 135, 0);
    assert_eq!(ai.domination_siege_milestones[&key].greatest_quarter, 4);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(ai.peace_offers.contains(&1));
}

/// See `domination_front_crushed`: a stalled war against a rival we outgun
/// four times over keeps going. The fixture's far reserve holds the front
/// short of that; without it the same stall offers no peace.
#[test]
fn a_crushed_front_is_not_offered_stalled_peace() {
    let (mut g, mut ai, plan, _, _) = fixture();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(ai.peace_offers.contains(&1), "the control: a stalled war");

    let (mut g, mut ai, plan, _, _) = fixture();
    for uid in g.player_unit_ids(1) {
        if g.units[&uid].kind == "modern_armor" {
            g.remove_unit(uid);
        }
    }
    assert!(ai.domination_front_crushed(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!ai.peace_offers.contains(&1));

    // Short of crushed, a siege reducing one of its cities keeps the war too.
    let (mut g, mut ai, plan, city, _) = fixture();
    ai.sieges.insert(
        city,
        crate::ai::advanced::siege_train::Siege {
            stage: crate::ai::advanced::siege_train::SiegeStage::Reduce,
            taker: None,
            entered: g.turn,
            assessed: g.turn,
            posts: Default::default(),
            short_since: None,
        },
    );
    assert!(ai.domination_front_crushed(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!ai.peace_offers.contains(&1));
}
