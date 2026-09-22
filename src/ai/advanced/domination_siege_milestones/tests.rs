use super::super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::Game;

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
