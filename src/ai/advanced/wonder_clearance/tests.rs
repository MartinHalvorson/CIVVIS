use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::name::Name;
use std::collections::BTreeMap;

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 1,
        rush: false,
    }
}

fn board() -> (Game, AdvancedAi, u32, Pos, u32) {
    let mut g = Game::new_full(2, 32, 22, 913_349, 250, 0, true);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_camp_guards.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = Name::new("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.owner_city = None;
        tile.river_edges = [false; 6];
    }
    g.current = 0;
    g.found_city_for(0, (10, 10), None);
    let site = (17, 10);
    g.map.tiles.get_mut(&(18, 10)).unwrap().feature = Some(Name::new("mount_roraima"));
    for pos in g.wdisk(site, 2) {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.hills = true;
        tile.resource = Some(Name::new("wheat"));
    }
    g.map.tiles.get_mut(&site).unwrap().river_edges = [true; 6];
    g.players[0].explored = g.map.tiles.keys().copied().collect();
    g.players[0].techs.insert(Name::new("archery"));
    let settler = g.spawn_test_unit("settler", 0, (11, 10));
    g.spawn_test_unit("archer", 0, (12, 9));
    g.spawn_test_unit("archer", 0, (12, 10));
    g.spawn_test_unit("warrior", 0, (11, 11));
    let raider = g.spawn_test_unit("warrior", g.barb_pid.unwrap(), (14, 10));
    let mut ai = AdvancedAi::new();
    ai.enable_garrison_under_fire();
    ai.enable_wonder_adjacent_sites_2();
    ai.settler_targets.insert(settler, site);
    (g, ai, settler, site, raider)
}

#[test]
fn blocked_wonder_retains_its_site_and_assigns_a_real_force_without_the_board() {
    let (mut g, mut ai, settler, site, raider) = board();
    assert!(!ai.objective_board);
    assert!(ai.settle_value(&g, 0, site) >= MIN_SITE_VALUE);
    assert!(ai.reserve_wonder_clearance(&g, 0, settler, site));
    let roster = ai.wonder_clearance[&settler].units.clone();
    assert_eq!(roster.len(), 3);
    // Ordinary escort recruitment must leave the clearing team together.
    ai.live_formationless_settler_shadow = true;
    assert!(ai.formationless_settler_escort());
    ai.stacked_escort_pace(&mut g, 0, settler);
    assert!(!ai.settler_guards.contains_key(&settler));
    assert_eq!(
        roster
            .iter()
            .filter(|id| g.units[id].kind == "archer")
            .count(),
        2
    );
    assert_eq!(
        ai.best_settler_target(&g, 0, settler, 8, Some(site))
            .unwrap()
            .0,
        site
    );
    // The actual military path must spend an assigned archer on the blocker.
    let archer = roster
        .iter()
        .copied()
        .find(|id| {
            g.units[id].kind == "archer" && g.wdist(g.units[id].pos, g.units[&raider].pos) <= 2
        })
        .unwrap();
    let before = g.units[&raider].hp;
    assert!(ai.advanced_military_step(&mut g, 0, archer, &plan()));
    assert!(g.units.get(&raider).is_none_or(|u| u.hp < before));
    // Clearing the blocker does not discard the valuable destination.
    if g.units.contains_key(&raider) {
        g.remove_unit(raider);
    }
    assert_eq!(ai.wonder_clearance_site(&g, 0, settler), Some(site));
    std::sync::Arc::make_mut(&mut g.blocked_city_sites).insert(site);
    assert_eq!(ai.wonder_clearance_site(&g, 0, settler), None);
}

#[test]
fn clearance_expires_and_cannot_renew_every_frame() {
    let (mut g, mut ai, settler, site, _) = board();
    assert!(ai.reserve_wonder_clearance(&g, 0, settler, site));
    g.turn = ai.wonder_clearance[&settler].until;
    assert_eq!(ai.wonder_clearance_site(&g, 0, settler), None);
    assert!(!ai.reserve_wonder_clearance(&g, 0, settler, site));
    ai.refresh_wonder_clearance(&g, 0);
    assert!(ai.wonder_clearance[&settler].units.is_empty());
}

#[test]
fn clearance_requires_discovery_spare_force_and_live_policy() {
    let (mut g, mut ai, settler, site, _) = board();
    ai.disable_garrison_under_fire();
    assert!(!ai.reserve_wonder_clearance(&g, 0, settler, site));
    ai.enable_garrison_under_fire();
    g.players[0].explored.remove(&(18, 10));
    assert!(!ai.reserve_wonder_clearance(&g, 0, settler, site));
    g.players[0].explored.insert((18, 10));
    let archer = g
        .units
        .values()
        .find(|u| u.owner == 0 && u.kind == "archer")
        .unwrap()
        .id;
    ai.settler_guards.insert(settler, archer);
    assert!(!ai.reserve_wonder_clearance(&g, 0, settler, site));
}

#[test]
fn clearance_tracks_host_unit_identity_and_releases_a_vanished_settler() {
    let (g, mut ai, settler, site, _) = board();
    assert!(ai.reserve_wonder_clearance(&g, 0, settler, site));
    let units = ai.wonder_clearance[&settler].units.clone();
    let mapping: BTreeMap<_, _> = std::iter::once((settler, settler + 1000))
        .chain(units.iter().map(|id| (*id, *id + 1000)))
        .collect();
    ai.remap_unit_memory(&mapping);
    assert_eq!(ai.wonder_clearance[&(settler + 1000)].site, site);
    assert_eq!(
        ai.wonder_clearance[&(settler + 1000)].units,
        units.iter().map(|id| id + 1000).collect::<Vec<_>>()
    );
    ai.remap_unit_memory(&BTreeMap::new());
    assert!(ai.wonder_clearance.is_empty());
}

#[test]
fn opening_archery_preempts_science_beeline_only_with_known_nearby_pressure() {
    let (mut g, mut ai, _, _, raider) = board();
    g.players[0].techs.clear();
    ai.victory_target = Some(VictoryTarget::Science);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    assert_eq!(ai.opening_archery_goal(&g, 0).as_deref(), Some("archery"));
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("animal_husbandry"));
    g.players[0].techs.insert(Name::new("animal_husbandry"));
    g.players[0].research = None;
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("archery"));
    g.players[0].techs.insert(Name::new("archery"));
    assert!(ai.opening_archery_goal(&g, 0).is_none());
    g.players[0].techs.remove(&Name::new("archery"));
    g.remove_unit(raider);
    assert!(ai.opening_archery_goal(&g, 0).is_none());
    g.spawn_test_unit("slinger", 0, (10, 10));
    assert_eq!(ai.opening_archery_goal(&g, 0).as_deref(), Some("archery"));
}

#[test]
fn clearing_force_leaves_city_defenders_in_place() {
    let (mut g, mut ai, settler, site, _) = board();
    g.spawn_test_unit("warrior", g.barb_pid.unwrap(), (10, 9));
    assert!(!ai.reserve_wonder_clearance(&g, 0, settler, site));
}

#[test]
fn known_camp_keeps_opening_archery_urgent_without_a_visible_raider() {
    let (mut g, ai, _, _, raider) = board();
    g.players[0].techs.remove(&Name::new("archery"));
    g.remove_unit(raider);
    let camp = (14, 10);
    g.barb_camps.insert(camp, g.turn);
    g.players[0].explored.remove(&camp);
    assert!(ai.opening_archery_goal(&g, 0).is_none());
    g.players[0].explored.insert(camp);
    assert_eq!(ai.opening_archery_goal(&g, 0).as_deref(), Some("archery"));
}

#[test]
fn clearing_force_advances_after_the_blocker_is_removed() {
    let (mut g, mut ai, settler, site, raider) = board();
    assert!(ai.reserve_wonder_clearance(&g, 0, settler, site));
    g.remove_unit(raider);
    let archer = ai.wonder_clearance[&settler]
        .units
        .iter()
        .copied()
        .find(|uid| g.units[uid].kind == "archer")
        .unwrap();
    let before = g.wdist(g.units[&archer].pos, site);
    assert_eq!(ai.wonder_clearance_step(&mut g, 0, archer), Some(true));
    assert!(g.wdist(g.units[&archer].pos, site) < before);
}
