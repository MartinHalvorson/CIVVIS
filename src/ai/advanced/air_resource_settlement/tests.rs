use super::super::{AdvancedAi, VictoryTarget};
use crate::game::Game;
use crate::name::Name;
use crate::Pos;

fn fixture() -> (Game, AdvancedAi, Pos, u32) {
    let mut g = Game::new_full(3, 40, 24, 378700, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = true;
    }
    let home = g.found_city_for(0, (8, 10), None);
    g.found_city_for(0, (4, 10), None);
    g.found_city_for(1, (30, 10), None);
    g.players[2].is_minor = true;
    g.found_city_for(2, (18, 10), None);
    g.players[0].envoys.push((2, 6));
    g.players[0].techs.extend(
        ["mining", "industrialization", "flight", "radio"]
            .into_iter()
            .map(Name::new),
    );
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    crate::game::install_test_district(&mut g, home, "aerodrome");
    let deposit = (12, 10);
    g.map.tiles.get_mut(&deposit).unwrap().resource = Some(crate::name!("aluminum"));
    let guard = g.spawn_test_unit("musketman", 0, (12, 11));
    g.current = 0;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    (g, ai, deposit, guard)
}

#[test]
fn needed_defended_aluminum_colony_enters_the_actual_site_scan() {
    let (g, ai, deposit, _) = fixture();
    assert_eq!(g.suzerain_of(2), Some(0));
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 0.0);
    assert!(
        !ai.city_state_settlement_exclusion(&g, 0).contains(&deposit),
        "a needed, defended aluminum center beside our city-state remains a candidate"
    );
    assert!(ai
        .settle_ranking(&g, 0, (8, 10), 8)
        .iter()
        .any(|(pos, _)| *pos == deposit));
}

#[test]
fn colony_exception_keeps_need_permission_and_security_vetoes() {
    for case in [
        "off",
        "science",
        "field",
        "unrevealed",
        "unexplored",
        "other_resource",
        "owned",
        "flooded",
        "submerged",
        "blocked",
        "no_guard",
        "hurt_guard",
        "distant_guard",
        "ranged_guard",
        "suzerain",
        "other_minor",
        "sufficient",
        "city_spacing",
        "hostile",
    ] {
        let (mut g, mut ai, deposit, guard) = fixture();
        match case {
            "off" => ai.disable_air_surge_2(),
            "science" => ai = AdvancedAi::targeting(VictoryTarget::Science),
            "field" => {
                for city in g.cities.values_mut().filter(|city| city.owner == 0) {
                    city.districts.clear();
                }
            }
            "unrevealed" => {
                g.players[0].techs.remove(&crate::name!("radio"));
            }
            "unexplored" => {
                g.units.get_mut(&guard).unwrap().pos = (12, 14);
                g.players[0].explored.remove(&deposit);
                assert!(!g.sees(&g.player_vision_frame(0), deposit));
            }
            "other_resource" => {
                g.map.tiles.get_mut(&deposit).unwrap().resource = Some(crate::name!("iron"))
            }
            "owned" => {
                let cid = g.player_city_ids(0)[0];
                g.map.tiles.get_mut(&deposit).unwrap().owner_city = Some(cid);
            }
            "flooded" => g.map.tiles.get_mut(&deposit).unwrap().flooded = true,
            "submerged" => g.map.tiles.get_mut(&deposit).unwrap().submerged = true,
            "blocked" => {
                std::sync::Arc::make_mut(&mut g.blocked_city_sites).insert(deposit);
            }
            "no_guard" => {
                g.remove_unit(guard);
            }
            "hurt_guard" => g.units.get_mut(&guard).unwrap().hp = 74,
            "distant_guard" => g.units.get_mut(&guard).unwrap().pos = (7, 10),
            "ranged_guard" => g.units.get_mut(&guard).unwrap().kind = crate::name!("field_cannon"),
            "suzerain" => g.players[0].envoys.clear(),
            "other_minor" => {
                g.players[1].is_minor = true;
                g.found_city_for(1, (12, 16), None);
            }
            "sufficient" => {
                let tile = g.map.tiles.get_mut(&(8, 10)).unwrap();
                tile.resource = Some(crate::name!("aluminum"));
                assert_eq!(g.strategic_resource_rate(0, "aluminum"), 2.0);
            }
            "city_spacing" => {
                g.found_city_for(0, (12, 13), None);
            }
            "hostile" => {
                g.record_contact(0, 1);
                g.apply(0, &crate::game::Action::DeclareWar { player: 1 })
                    .unwrap();
                g.spawn_test_unit("musketman", 1, (13, 10));
            }
            _ => unreachable!(),
        }
        assert!(
            ai.city_state_settlement_exclusion(&g, 0).contains(&deposit),
            "{case}"
        );
    }
}

#[test]
fn a_resource_colony_must_hold_its_loyalty_without_the_optional_rate_alarm() {
    let (mut g, ai, deposit, _) = fixture();
    let rival = g.found_city_for(1, (17, 14), None);
    g.cities.get_mut(&rival).unwrap().pop = 40;
    let mut forecast = g.speculative_clone();
    let city = forecast.found_city_for(0, deposit, None);
    assert!(forecast.city_loyalty_per_turn(&forecast.cities[&city]) < 0.0);
    assert!(ai.city_state_settlement_exclusion(&g, 0).contains(&deposit));
}

#[test]
fn a_cached_resource_colony_is_rechecked_before_founding() {
    for loses_permission in [false, true] {
        let (mut g, mut ai, deposit, _) = fixture();
        let settler = g.spawn_test_unit("settler", 0, deposit);
        ai.settler_targets.insert(settler, deposit);
        assert!(!ai.air_resource_colony_refused(&g, 0, deposit));
        if loses_permission {
            g.players[0].envoys.clear();
        }
        ai.advanced_settler_step(&mut g, 0, settler);
        assert_eq!(g.city_at(deposit).is_some(), !loses_permission);
    }
}

#[test]
fn stalled_founding_cannot_bypass_lost_colony_permission() {
    let (mut g, mut ai, deposit, _) = fixture();
    let settler = g.spawn_test_unit("settler", 0, deposit);
    ai.enable_settler_founds_when_stalled();
    g.players[0].envoys.clear();
    assert!(!ai.founds_where_it_stands(&mut g, 0, settler, deposit));
    assert!(g.city_at(deposit).is_none());
}

#[test]
fn a_charted_resource_colony_can_be_planned_before_it_reenters_sight() {
    let (mut g, ai, deposit, guard) = fixture();
    g.units.get_mut(&guard).unwrap().pos = (12, 14);
    assert!(g.players[0].explored.contains(&deposit));
    assert!(!g.player_can_see(0, deposit));
    assert!(!ai.city_state_settlement_exclusion(&g, 0).contains(&deposit));
    assert!(ai
        .settle_ranking(&g, 0, (8, 10), 8)
        .iter()
        .any(|(pos, _)| *pos == deposit));
}
