use super::*;
use crate::ai::VictoryTarget;
use crate::game::Action;

fn fixture() -> (Game, u32, Name, Pos) {
    let mut game = Game::new(2, 32, 24, 5_418, 250, 0);
    game.game_speed = crate::setup::GameSpeed::Online;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = game.player_city_ids(0)[0];
    game.cities.get_mut(&cid).unwrap().pop = 6;
    game.players[0].techs.insert(crate::name!("apprenticeship"));
    game.turn = 70;
    let (district, pos) = game
        .producible_items(0, cid)
        .into_iter()
        .find_map(|item| match item {
            Item::District { district, pos }
                if game.district_family(district) == "industrial_zone" =>
            {
                Some((district, pos))
            }
            _ => None,
        })
        .expect("a legal Industrial Zone site");
    (game, cid, district, pos)
}

#[test]
fn named_lanes_price_the_workshop_an_industrial_zone_unlocks() {
    let (game, cid, district, pos) = fixture();
    for target in VictoryTarget::ALL {
        let ai = AdvancedAi::targeting(target);
        assert!(
            ai.industrial_district_path_value(&game, 0, cid, &district, pos, target.strategy())
                > 0.0,
            "{} should price the repayable Workshop path",
            target.as_str()
        );
    }
    let adaptive = AdvancedAi::new();
    assert_eq!(
        adaptive.industrial_district_path_value(
            &game,
            0,
            cid,
            &district,
            pos,
            GrandStrategy::Science
        ),
        0.0
    );
}

#[test]
fn a_late_or_locked_follow_on_building_cannot_fund_a_district() {
    let (mut game, cid, district, pos) = fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    game.turn = 249;
    assert_eq!(
        ai.industrial_district_path_value(&game, 0, cid, &district, pos, GrandStrategy::Science),
        0.0
    );
    game.turn = 70;
    game.players[0]
        .techs
        .remove(&crate::name!("apprenticeship"));
    assert_eq!(
        ai.industrial_district_path_value(&game, 0, cid, &district, pos, GrandStrategy::Science),
        0.0
    );
}

#[test]
fn no_credit_for_a_paid_workshop_or_an_unrelated_district() {
    let (mut game, cid, district, pos) = fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    assert_eq!(
        ai.industrial_district_path_value(
            &game,
            0,
            cid,
            &crate::name!("campus"),
            pos,
            GrandStrategy::Culture
        ),
        0.0
    );
    game.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("workshop"));
    assert_eq!(
        ai.industrial_district_path_value(&game, 0, cid, &district, pos, GrandStrategy::Culture),
        0.0
    );
}

#[test]
fn finishing_a_placed_district_earlier_leaves_more_time_to_compound() {
    let (mut game, cid, district, pos) = fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    let before =
        ai.industrial_district_path_value(&game, 0, cid, &district, pos, GrandStrategy::Science);
    let cost = game.item_cost_for_city(0, cid, &Item::District { district, pos });
    game.cities.get_mut(&cid).unwrap().production = cost;
    let paid =
        ai.industrial_district_path_value(&game, 0, cid, &district, pos, GrandStrategy::Science);
    assert!(
        paid > before,
        "{paid} > {before}: actual build delay belongs in the forecast"
    );
}

#[test]
fn the_production_scorer_receives_the_follow_on_value() {
    let (game, cid, district, pos) = fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    let mut adaptive = ai.clone();
    adaptive.victory_target = None;
    let plan = crate::ai::StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: game.turn,
        rush: false,
    };
    let item = Item::District { district, pos };
    let counts = ai.counts(&game, 0);
    assert!(
        ai.production_value(&game, 0, cid, &item, &plan, &counts)
            > adaptive.production_value(&game, 0, cid, &item, &plan, &counts),
        "the production decision must read the new valuation"
    );
}
