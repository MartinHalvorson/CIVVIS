use super::*;
use crate::rules::Yields;
use std::sync::Arc;

fn board() -> (Game, [u32; 2], Pos) {
    let mut g = Game::new(2, 32, 22, 914_357_400, 250, 0);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
    }
    let cities = [(5, 5), (19, 5)].map(|pos| g.found_city_for(0, pos, None));
    for (cid, production) in cities.into_iter().zip([1.0, 10.0]) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.queue.clear();
        city.buildings = vec![crate::name!("monument")];
        for pos in city.owned_tiles.clone() {
            if pos != city.pos {
                g.map.tiles.get_mut(&pos).unwrap().improvement = Some(crate::name!("farm"));
            }
        }
        let actual = g.city_yields(cid).production;
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            cid,
            Yields {
                production: production - actual,
                ..Default::default()
            },
        );
    }
    let work = g.cities[&cities[1]]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != g.cities[&cities[1]].pos)
        .unwrap();
    g.map.tiles.get_mut(&work).unwrap().improvement = None;
    g.players[0].gold = 2000.0;
    g.players[0].gold_per_turn = 10.0;
    g.current = 0;
    g.turn = 20;
    assert!(g.city_yields(cities[0]).production < g.city_yields(cities[1]).production);
    assert!(g
        .valid_improvements(0, work)
        .contains(&crate::name!("farm")));
    (g, cities, work)
}

fn bought_builder(g: &Game) -> Pos {
    let builders: Vec<_> = g
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "builder")
        .collect();
    assert_eq!(builders.len(), 1);
    builders[0].pos
}

#[test]
fn treasury_successor_buys_near_work_instead_of_the_finished_city() {
    let (g, cities, _) = board();
    let mut old_game = g.clone();
    let mut old = AdvancedAi::new();
    old.enable_treasury_at_work_2();
    assert!(old.young_empire_purchase(&mut old_game, 0, 100.0));
    assert_eq!(bought_builder(&old_game), g.cities[&cities[0]].pos);

    let mut game = g.clone();
    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2_2();
    assert!(ai.young_empire_purchase(&mut game, 0, 100.0));
    assert_eq!(bought_builder(&game), g.cities[&cities[1]].pos);
}

#[test]
fn treasury_successor_counts_repairs_as_local_work() {
    let (mut g, cities, work) = board();
    let tile = g.map.tiles.get_mut(&work).unwrap();
    tile.improvement = Some(crate::name!("farm"));
    tile.pillaged = true;
    // A pillaged Farm may also be rebuilt. Block new improvements here so
    // only the independently legal repair can justify the purchase.
    Arc::make_mut(&mut g.blocked_improvement_sites).insert(work);
    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2_2();
    assert!(g.valid_improvements(0, work).is_empty());
    assert!(ai.young_empire_purchase(&mut g, 0, 100.0));
    assert_eq!(bought_builder(&g), g.cities[&cities[1]].pos);
}

#[test]
fn treasury_successor_waits_for_recent_attack_to_age_out() {
    let (mut g, cities, _) = board();
    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2_2();
    g.cities.get_mut(&cities[1]).unwrap().last_attacked = g.turn - 4;
    assert!(!ai.young_empire_purchase(&mut g, 0, 100.0));
    g.cities.get_mut(&cities[1]).unwrap().last_attacked = g.turn - 5;
    assert!(ai.young_empire_purchase(&mut g, 0, 100.0));
    assert_eq!(bought_builder(&g), g.cities[&cities[1]].pos);
}

#[test]
fn treasury_successor_avoids_a_current_barbarian_alarm() {
    let (mut g, cities, work) = board();
    let barb = g.barb_pid.expect("the board has a barbarian seat");
    let uid = g.spawn_test_unit("warrior", barb, work);
    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2_2();
    assert!(ai
        .base
        .barbarian_local_alarm_for_controller(&g, 0, cities[1]));
    assert!(!ai.young_empire_purchase(&mut g, 0, 100.0));
    g.remove_unit(uid);
    assert!(ai.young_empire_purchase(&mut g, 0, 100.0));
}

#[test]
fn treasury_successor_preserves_reserve_and_monument_fallback() {
    let (mut g, cities, work) = board();
    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2_2();
    let reserve = 100.0;
    let price = g
        .unit_purchase_cost(0, cities[1], "builder", "gold")
        .unwrap();
    g.players[0].gold = reserve + price - 1.0;
    assert!(!ai.young_empire_purchase(&mut g, 0, reserve));
    assert_eq!(g.players[0].gold, reserve + price - 1.0);
    g.players[0].gold = reserve + price;
    assert!(ai.young_empire_purchase(&mut g, 0, reserve));
    assert_eq!(g.players[0].gold, reserve);

    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    g.map.tiles.get_mut(&work).unwrap().improvement = Some(crate::name!("farm"));
    g.cities.get_mut(&cities[0]).unwrap().buildings.clear();
    g.players[0].gold = 2000.0;
    assert!(ai.young_empire_purchase(&mut g, 0, reserve));
    assert!(g.cities[&cities[0]]
        .buildings
        .contains(&crate::name!("monument")));
    assert!(g.units.values().all(|unit| unit.kind != "builder"));
    assert!(g.players[0].gold >= reserve);
}

#[test]
fn treasury_successor_rejects_work_no_longer_owned_by_the_city() {
    let (mut g, _, work) = board();
    g.map.tiles.get_mut(&work).unwrap().owner_city = None;
    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2_2();
    assert!(!ai.young_empire_purchase(&mut g, 0, 100.0));
}

#[test]
fn treasury_successor_is_exclusive_default_off_and_preserves_solvency() {
    test_support::opt_in_off_in_both_controllers("treasury-at-work-2-2", |ai| {
        ai.treasury_at_work_2_2
    });
    let (mut g, _, _) = board();
    let mut ai = AdvancedAi::new();
    ai.enable_treasury_at_work_2();
    let reserve = ai.working_treasury_reserve(&g, 0, 325.0);
    ai.enable_treasury_at_work_2_2();
    assert!(!ai.treasury_at_work_2 && ai.treasury_at_work_2_2);
    assert_eq!(ai.working_treasury_reserve(&g, 0, 325.0), reserve);
    g.players[0].gold_per_turn = -3.0;
    assert!(!ai.treasury_purchase_stays_solvent(
        &g,
        0,
        &Item::Unit {
            unit: crate::name!("archer")
        }
    ));
    ai.enable_treasury_at_work_2();
    assert!(ai.treasury_at_work_2 && !ai.treasury_at_work_2_2);
    ai.disable_treasury_at_work_2();
    ai.enable_treasury_at_work_2_2();
    ai.disable_treasury_at_work_2_2();
    assert!(!ai.treasury_at_work_2 && !ai.treasury_at_work_2_2);
}

#[test]
fn treasury_successor_live_identity_matches_the_selected_flags() {
    for forced in [vec![], vec!["treasury-at-work-2-2"]] {
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge_universe();
        ai.apply_gene_ledger_with_forced_live(&forced);
        let tags = gene_ledger::deployment_treatments_with_forced_live(&forced);
        assert_eq!(tags.contains(&"treasury-at-work-2"), ai.treasury_at_work_2);
        assert_eq!(
            tags.contains(&"treasury-at-work-2-2"),
            ai.treasury_at_work_2_2
        );
        assert_ne!(ai.treasury_at_work_2, ai.treasury_at_work_2_2);
        assert_eq!(ai.treasury_at_work_2_2, !forced.is_empty());
    }
}
