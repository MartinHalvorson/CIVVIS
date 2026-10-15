use super::super::siege_train::StageMarch;
use super::super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Action, Game};

fn blocked_road() -> (Game, AdvancedAi, usize, usize, u32, u32) {
    let mut g = Game::new_full(3, 40, 12, 931_036, 300, 2, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.clear_mirror_cities();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if (4..=8).contains(&tile.pos.1) && (2..=36).contains(&tile.pos.0) {
            crate::name!("grassland")
        } else {
            crate::name!("ocean")
        };
        tile.feature = None;
        tile.hills = false;
        tile.owner_city = None;
    }
    let minors: Vec<_> = g
        .players
        .iter()
        .filter(|p| p.is_minor && !p.is_barbarian)
        .map(|p| p.id)
        .collect();
    let (kumasi, fez) = (minors[0], minors[1]);
    g.players[kumasi].civ = "Kumasi".into();
    g.players[fez].civ = "Fez".into();
    g.players[0].civ = "Gran Colombia".into();
    g.players[1].civ = "Portugal".into();
    for pid in 0..3 {
        for &minor in &minors {
            g.record_contact(pid, minor);
        }
    }
    for y in 4..=6 {
        for x in [4, 7, 10] {
            let cid = g.found_city_for(0, (x, y), None);
            let city = g.cities.get_mut(&cid).unwrap();
            city.districts.insert(crate::name!("campus"), (x, y));
            city.buildings.push(crate::name!("library"));
            city.buildings.push(crate::name!("university"));
        }
    }
    let screen = g.found_city_for(kumasi, (18, 6), None);
    g.found_city_for(fez, (3, 8), None);
    let target = g.found_city_for(2, (32, 6), None);
    for tile in g.map.tiles.values_mut() {
        if (14..=22).contains(&tile.pos.0) && (4..=8).contains(&tile.pos.1) {
            tile.owner_city = Some(screen);
        }
    }
    g.players[kumasi].borders_enforced = Some(true);
    g.players[0].envoys = vec![(kumasi, 9), (fez, 5)];
    g.players[1].envoys = vec![(kumasi, 10), (fez, 5)];
    g.players[0].envoys_free = 2;
    g.at_war.insert((0, 2));
    g.at_war.insert((2, 0));
    g.turn = 168;
    g.current = 0;
    let soldier = g.spawn_test_unit("bombard", 0, (11, 6));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_price_the_suzerainty();
    ai.enable_siege_target_needs_a_road();
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(2),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 10,
        assessed_turn: 168,
        rush: false,
    });
    assert_eq!(g.suzerain_of(kumasi), Some(1));
    assert!(g
        .route_step_dry(soldier, g.cities[&target].pos, 5, 128)
        .is_none());
    assert_eq!(
        ai.note_stage_march(&g, soldier, target, &StageMarch::Hold { wet: 13 })
            .as_deref(),
        Some("Kumasi")
    );
    (g, ai, kumasi, fez, target, soldier)
}

fn envoys(g: &Game, from: usize) -> Vec<usize> {
    g.log
        .since(from)
        .filter_map(|(_, action)| match action {
            Action::SendEnvoy { player } => Some(*player),
            _ => None,
        })
        .collect()
}

#[test]
fn envoys_open_the_siege_road_buys_both_placements_before_the_economic_tier() {
    let (g, mut ai, kumasi, fez, target, soldier) = blocked_road();
    let mut off = g.clone();
    let start = off.log.len();
    ai.advanced_envoys(&mut off, 0, GrandStrategy::Conquest, None);
    assert_eq!(
        envoys(&off, start).first(),
        Some(&fez),
        "fixture: the economic tier wins on the old policy"
    );
    assert_ne!(off.suzerain_of(kumasi), Some(0));
    let mut on = g;
    ai.enable_envoys_open_the_siege_road();
    let start = on.log.len();
    ai.advanced_envoys(&mut on, 0, GrandStrategy::Conquest, None);
    assert_eq!(envoys(&on, start), vec![kumasi, kumasi]);
    assert_eq!(on.suzerain_of(kumasi), Some(0));
    assert!(on
        .route_step_dry(soldier, on.cities[&target].pos, 5, 128)
        .is_some());
    assert!(
        !ai.campaign_target_legal(&on, 0, kumasi),
        "our suzerainty cannot become the new war front"
    );
}

#[test]
fn envoys_open_the_siege_road_requires_an_affordable_complete_suzerainty() {
    let (mut g, mut ai, kumasi, fez, _, _) = blocked_road();
    ai.enable_envoys_open_the_siege_road();
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 2), Some(2));
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 1), None);
    g.players[0].envoys_free = 1;
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 8), None);
    let start = g.log.len();
    ai.advanced_envoys(&mut g, 0, GrandStrategy::Conquest, None);
    assert_eq!(
        envoys(&g, start),
        vec![fez],
        "one envoy cannot buy the road"
    );
}

#[test]
fn envoys_open_the_siege_road_ignores_stale_holds_and_finished_wars() {
    let (mut g, mut ai, kumasi, _, _, soldier) = blocked_road();
    ai.enable_envoys_open_the_siege_road();
    g.turn += 1;
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 8), Some(2));
    g.turn += 1;
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 8), None);
    g.turn -= 2;
    g.at_war.remove(&(0, 2));
    g.at_war.remove(&(2, 0));
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 8), None);
    g.at_war.insert((0, 2));
    g.at_war.insert((2, 0));
    g.remove_unit(soldier);
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 8), None);
}

#[test]
fn envoys_open_the_siege_road_requires_a_current_legal_closed_border() {
    let (mut g, mut ai, kumasi, _, _, _) = blocked_road();
    ai.enable_envoys_open_the_siege_road();
    std::sync::Arc::make_mut(&mut g.host_envoy_permissions).insert((0, kumasi), (168, false));
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 8), None);
    std::sync::Arc::make_mut(&mut g.host_envoy_permissions).clear();
    g.players[kumasi].open_borders_until.insert(0, 169);
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 8), None);
    g.players[kumasi].open_borders_until.clear();
    g.at_war.insert((0, kumasi));
    g.at_war.insert((kumasi, 0));
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 8), None);
}

#[test]
fn envoys_open_the_siege_road_prices_physical_placements_with_league() {
    let (mut g, mut ai, kumasi, _, _, _) = blocked_road();
    ai.enable_envoys_open_the_siege_road();
    g.players[0].envoys.retain(|(minor, _)| *minor != kumasi);
    g.players[1].envoys = vec![(kumasi, 2)];
    g.players[0]
        .policies
        .insert(crate::name!("diplomatic_league"));
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 2), Some(2));
    let start = g.log.len();
    ai.advanced_envoys(&mut g, 0, GrandStrategy::Conquest, None);
    assert_eq!(envoys(&g, start), vec![kumasi, kumasi]);
    assert_eq!(g.envoys_at(0, kumasi), 3);
    assert_eq!(g.suzerain_of(kumasi), Some(0));
}

#[test]
fn envoys_open_the_siege_road_containment_stops_at_the_tie() {
    let (mut g, mut ai, kumasi, _, _, _) = blocked_road();
    ai.enable_envoys_open_the_siege_road();
    g.players[0].envoys = vec![(kumasi, 2)];
    g.players[1].envoys = vec![(kumasi, 4)];
    g.players[0].government = Some("democracy".into());
    g.players[1].government = Some("monarchy".into());
    g.players[0].policies.insert(crate::name!("containment"));
    assert_eq!(
        ai.siege_road_envoy_package(&g, 0, kumasi, 1),
        None,
        "a doubled envoy only ties"
    );
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 2), Some(2));
    ai.advanced_envoys(&mut g, 0, GrandStrategy::Conquest, None);
    assert_eq!(g.envoys_at(0, kumasi), 5);
    assert_eq!(g.suzerain_of(kumasi), Some(0));
}

#[test]
fn envoys_open_the_siege_road_records_without_the_older_road_gene() {
    let (g, _, kumasi, _, target, soldier) = blocked_road();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.siege_target_needs_a_road);
    ai.enable_envoys_open_the_siege_road();
    assert_eq!(
        ai.note_stage_march(&g, soldier, target, &StageMarch::Hold { wet: 13 })
            .as_deref(),
        Some("Kumasi")
    );
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 8), Some(2));
}

#[test]
fn envoys_open_the_siege_road_budget_does_not_change_the_off_policy() {
    let (g, mut ai, _, _, _, _) = blocked_road();
    ai.set_envoy_order_budget(Some(0));
    let mut off = g.clone();
    let start = off.log.len();
    ai.advanced_envoys(&mut off, 0, GrandStrategy::Conquest, None);
    assert_eq!(envoys(&off, start).len(), 2);
    ai.enable_envoys_open_the_siege_road();
    let mut on = g;
    let start = on.log.len();
    ai.advanced_envoys(&mut on, 0, GrandStrategy::Conquest, None);
    assert!(envoys(&on, start).is_empty());
    assert_eq!(on.players[0].envoys_free, 2);
}

#[test]
fn envoys_open_the_siege_road_full_turn_respects_the_native_budget() {
    let (mut g, mut ai, kumasi, _, _, _) = blocked_road();
    ai.enable_envoys_open_the_siege_road();
    ai.disable_bank_envoys();
    ai.set_envoy_order_budget(Some(8));
    g.players[0].envoys_free = 20;
    let start = g.log.len();
    ai.plan_observed_turn(&mut g, 0);
    let sent = envoys(&g, start);
    assert!(
        sent.len() <= 8,
        "the ancillary fallback must not consume the unissued balance: {sent:?}"
    );
    assert_eq!(&sent[..2], &[kumasi, kumasi]);
    assert!(g.suzerain_of(kumasi) == Some(0));
    assert!(!g.is_at_war(0, kumasi));
}

#[test]
fn envoys_open_the_siege_road_follows_the_host_unit_across_a_rebuilt_board() {
    let (mut g, mut ai, kumasi, _, _, soldier) = blocked_road();
    ai.enable_envoys_open_the_siege_road();
    let new = g.spawn_test_unit("bombard", 0, g.units[&soldier].pos);
    g.remove_unit(soldier);
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 8), None);
    ai.remap_unit_memory(&[(soldier, new)].into());
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 8), Some(2));
    ai.remap_unit_memory(&Default::default());
    assert_eq!(ai.siege_road_envoy_package(&g, 0, kumasi, 8), None);
}
