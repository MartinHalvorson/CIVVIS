use super::super::{AdvancedAi, GrandStrategy, VictoryTarget};
use crate::game::Game;
use crate::name::Name;
use crate::Pos;

fn fixture() -> (Game, AdvancedAi, u32, usize, Pos) {
    let mut g = Game::new_full(3, 40, 24, 377900, 300, 0, false);
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
    let home = g.found_city_for(0, (8, 10), None);
    g.found_city_for(0, (4, 10), None);
    g.found_city_for(1, (30, 10), None);
    let minor = 2;
    g.players[minor].is_minor = true;
    g.players[minor].civ = "Taruga".to_string();
    g.found_city_for(minor, (15, 10), None);
    g.players[0].envoys.push((minor, 3));
    g.players[0].techs.extend(
        ["mining", "industrialization", "flight", "radio"]
            .into_iter()
            .map(Name::new),
    );
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    crate::game::install_test_district(&mut g, home, "aerodrome");
    let deposit = (14, 10);
    g.map.tiles.get_mut(&deposit).unwrap().resource = Some(crate::name!("aluminum"));
    let builder = g.spawn_test_unit("builder", 0, (13, 10));
    g.current = 0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    (g, ai, builder, minor, deposit)
}

#[test]
fn nearby_suzerain_aluminum_is_connected_during_the_beeline() {
    let (mut g, mut ai, builder, _, deposit) = fixture();
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 0.0);
    assert!(!g.players[0]
        .techs
        .contains(&crate::name!("advanced_flight")));
    assert!(g
        .valid_improvements(0, deposit)
        .contains(&crate::name!("mine")));
    for _ in 0..2 {
        assert!(ai.advanced_builder_step(&mut g, 0, builder, GrandStrategy::Conquest));
    }
    assert_eq!(
        g.map.tiles[&deposit].improvement,
        Some(crate::name!("mine"))
    );
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 2.0);
}

#[test]
fn air_supply_errand_requires_permission_need_commitment_and_nearby_work() {
    for case in [
        "suzerain",
        "hidden",
        "off",
        "lane",
        "field",
        "sufficient",
        "far",
        "blocked",
        "flooded",
        "submerged",
        "reserved",
        "charges",
        "moves",
    ] {
        let (mut g, mut ai, builder, _, deposit) = fixture();
        match case {
            "suzerain" => g.players[0].envoys.clear(),
            "hidden" => {
                g.players[0].techs.remove(&crate::name!("radio"));
            }
            "off" => ai.disable_air_surge_2(),
            "lane" => ai = AdvancedAi::targeting(VictoryTarget::Science),
            "field" => {
                for city in g.cities.values_mut().filter(|city| city.owner == 0) {
                    city.districts.clear();
                }
            }
            "sufficient" => {
                let pos = g
                    .map
                    .tiles
                    .iter()
                    .find_map(|(pos, tile)| {
                        (tile.owner_city.is_some_and(|cid| g.cities[&cid].owner == 0)
                            && tile.district.is_none()
                            && g.city_at(*pos).is_none())
                        .then_some(*pos)
                    })
                    .unwrap();
                let tile = g.map.tiles.get_mut(&pos).unwrap();
                tile.resource = Some(crate::name!("aluminum"));
                tile.improvement = Some(crate::name!("mine"));
                assert_eq!(g.strategic_resource_rate(0, "aluminum"), 2.0);
            }
            "far" => g.units.get_mut(&builder).unwrap().pos = (1, 1),
            "blocked" => {
                std::sync::Arc::make_mut(&mut g.blocked_improvement_sites).insert(deposit);
            }
            "flooded" => g.map.tiles.get_mut(&deposit).unwrap().flooded = true,
            "submerged" => g.map.tiles.get_mut(&deposit).unwrap().submerged = true,
            "reserved" => {
                let other = g.spawn_test_unit("builder", 0, (13, 11));
                ai.builder_targets.insert(other, deposit);
            }
            "charges" => g.units.get_mut(&builder).unwrap().charges = 0,
            "moves" => g.units.get_mut(&builder).unwrap().moves_left = 0.0,
            _ => unreachable!(),
        }
        assert_eq!(
            ai.air_resource_builder_step(&mut g, 0, builder),
            None,
            "{case}"
        );
        assert_eq!(g.map.tiles[&deposit].improvement, None, "{case}");
    }
}

#[test]
fn repairs_client_mine_without_spending_a_charge() {
    let (mut g, mut ai, builder, _, deposit) = fixture();
    let tile = g.map.tiles.get_mut(&deposit).unwrap();
    tile.improvement = Some(crate::name!("mine"));
    tile.pillaged = true;
    g.units.get_mut(&builder).unwrap().pos = deposit;
    let charges = g.units[&builder].charges;
    assert_eq!(ai.air_resource_builder_step(&mut g, 0, builder), Some(true));
    assert!(!g.map.tiles[&deposit].pillaged);
    assert_eq!(g.units[&builder].charges, charges);
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 2.0);
}

#[test]
fn lost_suzerainty_prevents_the_followup_improvement() {
    let (mut g, mut ai, builder, _, deposit) = fixture();
    assert_eq!(ai.air_resource_builder_step(&mut g, 0, builder), Some(true));
    assert_eq!(g.units[&builder].pos, deposit);
    g.players[0].envoys.clear();
    assert_eq!(ai.air_resource_builder_step(&mut g, 0, builder), None);
    assert_eq!(g.map.tiles[&deposit].improvement, None);
}

#[test]
fn builder_does_not_enter_visible_hostile_reach_for_aluminum() {
    let (mut g, mut ai, builder, _, _) = fixture();
    g.at_war.insert((0, 1));
    g.spawn_test_unit("cavalry", 1, (16, 10));
    let current = g.units[&builder].pos;
    assert_eq!(ai.air_resource_builder_step(&mut g, 0, builder), None);
    assert_eq!(g.units[&builder].pos, current);
}
