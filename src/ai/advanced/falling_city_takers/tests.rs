use super::*;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};

/// Our city at (10, 10), theirs at (20, 10) behind fallen walls at `hp`, at
/// war, open grassland, the city seen this frame.
fn fixture(hp: i32) -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(2, 40, 24, 377300, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (10, 10), None);
    let target = g.found_city_for(1, (20, 10), None);
    g.current = 0;
    g.at_war.insert((0, 1));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let city = g.cities.get_mut(&target).unwrap();
    city.wall_hp = 0;
    city.hp = hp;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: 1,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.observe_air_assault_frame(BTreeSet::from([(20, 10)]), 3);
    (g, ai, plan, target)
}

#[test]
fn a_falling_city_calls_a_far_body_one_step_in() {
    let (mut g, mut ai, plan, _) = fixture(20);
    let knight = g.spawn_test_unit("knight", 0, (13, 10));
    ai.falling_city_calls_the_takers = true;
    let reserved = ai.plan_falling_city_takers(&mut g, 0, &plan, &BTreeSet::new());
    assert!(reserved.contains(&knight));
    assert!(
        g.wdist(g.units[&knight].pos, (20, 10)) < 7,
        "the knight closed from 7 tiles to {}",
        g.wdist(g.units[&knight].pos, (20, 10))
    );
}

#[test]
fn a_falling_city_in_reach_is_taken() {
    let (mut g, mut ai, plan, target) = fixture(1);
    let knight = g.spawn_test_unit("knight", 0, (17, 10));
    ai.falling_city_calls_the_takers = true;
    let reserved = ai.plan_falling_city_takers(&mut g, 0, &plan, &BTreeSet::new());
    assert!(reserved.contains(&knight));
    assert_eq!(g.cities[&target].owner, 0);
    assert!(
        g.legal_city_disposition_actions(0).is_empty(),
        "the prepass resolves the capture prompt"
    );
}

#[test]
fn nothing_moves_with_the_gene_off_walls_up_a_wounded_body_or_at_peace() {
    for case in [
        "off",
        "walls",
        "healthy city",
        "wounded",
        "peace",
        "claimed",
    ] {
        let (mut g, mut ai, plan, target) = fixture(20);
        let knight = g.spawn_test_unit("knight", 0, (13, 10));
        ai.falling_city_calls_the_takers = true;
        let mut claimed = BTreeSet::new();
        match case {
            "off" => ai.falling_city_calls_the_takers = false,
            "walls" => g.cities.get_mut(&target).unwrap().wall_hp = 100,
            "healthy city" => g.cities.get_mut(&target).unwrap().hp = 120,
            "wounded" => g.units.get_mut(&knight).unwrap().hp = TAKER_CALL_MIN_HP - 1,
            "peace" => g.at_war.clear(),
            "claimed" => {
                claimed.insert(knight);
            }
            _ => unreachable!(),
        }
        let before = g.log.len();
        assert!(
            ai.plan_falling_city_takers(&mut g, 0, &plan, &claimed)
                .is_empty(),
            "{case}"
        );
        assert_eq!(g.units[&knight].pos, (13, 10), "{case}");
        assert_eq!(g.log.len(), before, "{case}");
    }
}

#[test]
fn a_body_beyond_the_reach_is_not_called() {
    let (mut g, mut ai, plan, _) = fixture(20);
    let far = (20 - TAKER_CALL_REACH - 1, 10);
    let knight = g.spawn_test_unit("knight", 0, far);
    ai.falling_city_calls_the_takers = true;
    assert!(ai
        .plan_falling_city_takers(&mut g, 0, &plan, &BTreeSet::new())
        .is_empty());
    assert_eq!(g.units[&knight].pos, far);
}
