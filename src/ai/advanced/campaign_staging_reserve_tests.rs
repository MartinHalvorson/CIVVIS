use super::*;

fn fixture() -> (Game, AdvancedAi, Pos, Vec<u32>, Vec<u32>) {
    let mut g = Game::new_full(2, 40, 24, 3523, 500, 0, false);
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
        tile.owner_city = None;
    }
    let objective = (10, 10);
    g.found_city_for(0, (21, 10), None);
    g.found_city_for(1, objective, None);
    g.at_war.clear();
    g.turn = 150;
    g.current = 0;
    g.record_contact(0, 1);
    let front = [(15, 10), (15, 9), (15, 8)]
        .into_iter()
        .map(|p| g.spawn_test_unit("warrior", 0, p))
        .collect();
    let rear = [(16, 10), (16, 9), (17, 8)]
        .into_iter()
        .map(|p| g.spawn_test_unit("giant_death_robot", 0, p))
        .collect();
    for p in [(14, 10), (14, 9)] {
        g.spawn_test_unit("modern_armor", 1, p);
    }
    (
        g,
        AdvancedAi::targeting(VictoryTarget::Domination),
        objective,
        front,
        rear,
    )
}

#[test]
fn reachable_reserves_supply_the_actual_launch_strength_check() {
    let (mut g, ai, objective, front, rear) = fixture();
    let staged = ai.staged_campaign_units(&g, 0, 1, objective);
    assert!(front.iter().chain(&rear).all(|uid| staged.contains(uid)));
    assert!(ai.campaign_staged_for_war(&g, 0, 1, objective, true));
    for uid in rear {
        g.remove_unit(uid);
    }
    assert!(
        !ai.campaign_staged_for_war(&g, 0, 1, objective, true),
        "the same small vanguard cannot launch without its supporting army"
    );
}

#[test]
fn a_nearby_but_trapped_reserve_does_not_count() {
    let (mut g, ai, objective, _, rear) = fixture();
    let trapped = rear[2];
    for pos in g.nbrs(g.units[&trapped].pos) {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
    }
    assert!(!ai
        .staged_campaign_units(&g, 0, 1, objective)
        .contains(&trapped));
}

#[test]
fn reserves_need_an_assembled_front_and_stay_out_of_other_victory_lanes() {
    let (mut g, ai, objective, front, rear) = fixture();
    let other = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(other.staged_campaign_units(&g, 0, 1, objective), front);
    for uid in front {
        g.remove_unit(uid);
    }
    assert!(ai.staged_campaign_units(&g, 0, 1, objective).is_empty());
    assert!(!ai.campaign_staged_for_war(&g, 0, 1, objective, true));
    assert_eq!(rear.len(), 3);
}

#[test]
#[ignore = "requires a caller-supplied recorded game"]
fn recorded_staging_reserve_readback() {
    let path = std::env::var("CIVVIS_STAGING_SNAPSHOT").unwrap();
    let g: Game = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let v = g.player_decision_view(2);
    let city = v.cities.values().find(|c| c.name == "Nidaros").unwrap();
    let units = ai.staged_campaign_units(&v, 2, 0, city.pos);
    eprintln!(
        "T{} staged {:?} strength {} ready {}",
        g.turn,
        units,
        AdvancedAi::campaign_strength_of(&v, &units),
        ai.campaign_staged_for_war(&v, 2, 0, city.pos, true)
    );
}
