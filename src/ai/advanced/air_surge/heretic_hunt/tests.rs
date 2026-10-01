use super::*;

fn fixture() -> (Game, AdvancedAi) {
    let mut g = Game::new_full(2, 40, 24, 936301, 650, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (10, 12), None);
    g.found_city_for(1, (26, 12), None);
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.current = 0;
    g.turn = 130;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    (g, ai)
}

fn spreader(g: &mut Game, kind: &str, pos: Pos) -> u32 {
    let uid = g.spawn_test_unit(kind, 1, pos);
    g.units.get_mut(&uid).unwrap().religion = Some("catholicism".to_string());
    uid
}

/// Live King 20261001T022028Z issued one condemnation in the game Brazil's
/// missionaries won. A spreader three tiles from a garrison is hunted down.
#[test]
fn a_spreader_near_home_is_hunted_and_condemned() {
    let (mut g, mut ai) = fixture();
    let apostle = spreader(&mut g, "apostle", (13, 12));
    let hunter = g.spawn_test_unit("cavalry", 0, (10, 13));
    let hunters = ai.plan_heretic_hunt(&mut g, 0, &BTreeSet::new());
    assert_eq!(hunters, BTreeSet::from([hunter]));
    assert!(!g.units.contains_key(&apostle), "the apostle is gone");
    assert_eq!(g.units[&hunter].pos, (13, 12));
}

#[test]
fn no_hunt_at_peace_far_from_home_out_of_reach_or_off_the_lane() {
    for case in ["peace", "far", "slow", "lane", "reserved"] {
        let (mut g, mut ai) = fixture();
        let at = if case == "far" { (20, 12) } else { (13, 12) };
        let target = spreader(&mut g, "missionary", at);
        let kind = if case == "slow" { "warrior" } else { "cavalry" };
        let start = if case == "slow" { (10, 13) } else { (10, 13) };
        let hunter = g.spawn_test_unit(kind, 0, start);
        let mut reserved = BTreeSet::new();
        match case {
            "peace" => g.at_war.clear(),
            "lane" => ai.victory_target = Some(VictoryTarget::Science),
            "reserved" => {
                reserved.insert(hunter);
            }
            _ => {}
        }
        let hunters = ai.plan_heretic_hunt(&mut g, 0, &reserved);
        assert!(hunters.is_empty(), "{case}");
        assert!(g.units.contains_key(&target), "{case}");
    }
}
