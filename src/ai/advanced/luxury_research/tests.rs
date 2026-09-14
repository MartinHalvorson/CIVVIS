use super::*;
use crate::ai::advanced::{genes, GrandStrategy, StrategicPlan};
use crate::game::{Action, ActiveTradeDeal, CongressEffect, DealItems};
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn board() -> (Game, u32, Vec<Pos>) {
    let mut g = Game::new_full(2, 32, 24, 914_3567, 250, 0, false);
    g.players[0].civ = "Rome".to_string();
    g.current = 0;
    let pos = g.units[&g.player_unit_ids(0)[0]].pos;
    let cid = g.found_city_for(0, pos, None);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.resource = None;
    }
    let plots = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .filter(|plot| *plot != pos)
        .collect::<Vec<_>>();
    assert!(plots.len() >= 3);
    for pos in &plots {
        let tile = g.map.tiles.get_mut(pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.flooded = false;
        tile.submerged = false;
    }
    g.players[0].techs = [crate::name!("pottery"), crate::name!("mining")].into();
    g.players[0].research = None;
    g.players[0].boosted_techs.clear();
    g.cities.get_mut(&cid).unwrap().pop = 6;
    Arc::make_mut(&mut g.observed_city_amenity_adjustments).insert(cid, -10);
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        Yields {
            science: 20.0,
            ..Yields::default()
        },
    );
    g.map.tiles.get_mut(&plots[0]).unwrap().resource = Some(crate::name!("silk"));
    (g, cid, plots)
}

fn candidate() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_connect_the_luxury_2();
    ai
}

#[test]
fn versions_are_registered_reversible_and_mutually_exclusive() {
    let gene = genes::gene("connect-the-luxury-2").unwrap();
    assert!(gene.opt_in() && gene.screenable());
    let mut ai = AdvancedAi::new();
    assert!(!ai.connect_the_luxury && !ai.connect_the_luxury_2);
    assert!(!AdvancedAi::legacy().connect_the_luxury_2);
    ai.enable_connect_the_luxury();
    (gene.enable)(&mut ai);
    assert!(!ai.connect_the_luxury && ai.connect_the_luxury_2);
    ai.enable_connect_the_luxury();
    assert!(ai.connect_the_luxury && !ai.connect_the_luxury_2);
    (gene.enable)(&mut ai);
    (gene.disable)(&mut ai);
    assert!(!ai.connect_the_luxury && !ai.connect_the_luxury_2);
}

#[test]
fn research_step_selects_a_legal_missing_luxury_and_preserves_the_board() {
    let (mut g, cid, plots) = board();
    let ai = candidate();
    assert!(!AdvancedAi::luxury_connectable(
        &g,
        0,
        plots[0],
        crate::name!("silk")
    ));
    assert_eq!(ai.unconnected_luxury_tech(&g, 0), Some("irrigation"));
    assert!(!g.players[0].techs.contains(&crate::name!("irrigation")));
    assert!(g.map.tiles[&plots[0]].improvement.is_none());
    let plan = StrategicPlan {
        strategy: GrandStrategy::Diplomacy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].research.as_deref(), Some("irrigation"));
    g.players[0].techs.insert(crate::name!("irrigation"));
    assert!(AdvancedAi::luxury_connectable(
        &g,
        0,
        plots[0],
        crate::name!("silk")
    ));
    assert_eq!(ai.unconnected_luxury_tech(&g, 0), None);
    assert_eq!(g.cities[&cid].owner, 0);
}

#[test]
fn content_empire_keeps_its_lane_but_v1_retains_its_original_detour() {
    let (mut g, cid, _) = board();
    Arc::make_mut(&mut g.observed_city_amenity_adjustments).insert(cid, 20);
    assert_eq!(candidate().unconnected_luxury_tech(&g, 0), None);
    let mut old = AdvancedAi::new();
    old.enable_connect_the_luxury();
    assert_eq!(old.unconnected_luxury_tech(&g, 0), Some("irrigation"));
}

#[test]
fn an_existing_connected_copy_or_import_removes_the_research_detour() {
    let (mut g, cid, _) = board();
    let home = g.cities[&cid].pos;
    g.map.tiles.get_mut(&home).unwrap().resource = Some(crate::name!("silk"));
    assert!(g.resource_access_count(0, "silk") > 0);
    assert_eq!(candidate().unconnected_luxury_tech(&g, 0), None);
    g.map.tiles.get_mut(&home).unwrap().resource = None;
    let mut offer = DealItems::default();
    offer.resources.insert("silk".to_string(), 1);
    g.active_trade_deals.push(ActiveTradeDeal {
        id: 1,
        from: 1,
        to: 0,
        offer,
        request: DealItems::default(),
        started: g.turn,
        ends: g.turn + 30,
    });
    assert_eq!(candidate().unconnected_luxury_tech(&g, 0), None);
    g.active_trade_deals.clear();
    assert_eq!(
        candidate().unconnected_luxury_tech(&g, 0),
        Some("irrigation")
    );
}

#[test]
fn banned_luxuries_do_not_redirect_research() {
    let (mut g, _, _) = board();
    g.active_congress_effects.push(CongressEffect {
        resolution: "luxury_policy".to_string(),
        outcome: "B".to_string(),
        target: "silk".to_string(),
        expires: g.turn + 30,
    });
    assert_eq!(candidate().unconnected_luxury_tech(&g, 0), None);
}

#[test]
fn unlock_must_pass_the_engines_tile_and_host_legality_checks() {
    let (g, _, plots) = board();
    for reason in 0..6 {
        let mut blocked = g.clone();
        let tile = blocked.map.tiles.get_mut(&plots[0]).unwrap();
        match reason {
            0 => tile.flooded = true,
            1 => tile.submerged = true,
            2 => tile.terrain = crate::name!("coast"),
            3 => tile.district = Some(crate::name!("campus")),
            4 => {
                Arc::make_mut(&mut blocked.blocked_improvement_sites).insert(plots[0]);
            }
            _ => tile.owner_city = Some(u32::MAX),
        }
        assert_eq!(
            candidate().unconnected_luxury_tech(&blocked, 0),
            None,
            "block {reason}"
        );
    }
}

#[test]
fn a_legal_copy_of_the_same_resource_makes_a_second_unlock_unnecessary() {
    let (mut g, _, plots) = board();
    g.map.tiles.get_mut(&plots[0]).unwrap().resource = Some(crate::name!("amber"));
    let sea = g.map.tiles.get_mut(&plots[1]).unwrap();
    sea.resource = Some(crate::name!("amber"));
    sea.terrain = crate::name!("coast");
    // Land Amber is a Mine; sea Amber is Fishing Boats. Mining is already in.
    assert!(AdvancedAi::luxury_connectable(
        &g,
        0,
        plots[0],
        crate::name!("amber")
    ));
    assert_eq!(candidate().unconnected_luxury_tech(&g, 0), None);
}

#[test]
fn boosts_and_prerequisites_change_which_connection_is_cheapest() {
    let (mut g, _, plots) = board();
    let sea = g.map.tiles.get_mut(&plots[1]).unwrap();
    sea.resource = Some(crate::name!("pearls"));
    sea.terrain = crate::name!("coast");
    assert_eq!(
        candidate().unconnected_luxury_tech(&g, 0),
        Some("irrigation"),
        "stable name tie break"
    );
    g.players[0].boosted_techs.insert(crate::name!("sailing"));
    assert_eq!(candidate().unconnected_luxury_tech(&g, 0), Some("sailing"));
    g.players[0].boosted_techs.clear();
    g.players[0].techs.remove(&crate::name!("pottery"));
    assert_eq!(
        candidate().unconnected_luxury_tech(&g, 0),
        Some("sailing"),
        "Irrigation now owes Pottery too"
    );
}

#[test]
fn research_bill_matches_engine_boost_credit_and_does_not_count_it_twice() {
    let (mut g, _, _) = board();
    g.game_speed = GameSpeed::Online;
    g.players[0].civ = "China".to_string();
    let goal = crate::name!("irrigation");
    let path = AdvancedAi::luxury_research_path(&g, 0, goal);
    g.players[0].boosted_techs.insert(goal);
    let before = AdvancedAi::luxury_research_cost(&g, 0, &path);
    g.apply(0, &Action::Research { tech: goal }).unwrap();
    assert_eq!(
        before,
        g.tech_cost("irrigation") - g.players[0].research_progress
    );
    assert_eq!(AdvancedAi::luxury_research_cost(&g, 0, &path), before);
    g.players[0].research_progress += 3.0;
    assert_eq!(AdvancedAi::luxury_research_cost(&g, 0, &path), before - 3.0);
}

#[test]
fn two_distinct_luxuries_outweigh_one_bargain_but_duplicate_plots_do_not() {
    let (mut g, _, plots) = board();
    let sea = g.map.tiles.get_mut(&plots[1]).unwrap();
    sea.resource = Some(crate::name!("pearls"));
    sea.terrain = crate::name!("coast");
    g.players[0].boosted_techs.insert(crate::name!("sailing"));
    g.map.tiles.get_mut(&plots[2]).unwrap().resource = Some(crate::name!("silk"));
    assert_eq!(candidate().unconnected_luxury_tech(&g, 0), Some("sailing"));
    g.map.tiles.get_mut(&plots[2]).unwrap().resource = Some(crate::name!("spices"));
    assert_eq!(
        candidate().unconnected_luxury_tech(&g, 0),
        Some("irrigation")
    );
}

#[test]
fn unknown_civics_and_foreign_unique_improvements_are_not_unlock_promises() {
    let (mut g, _, _) = board();
    Arc::make_mut(&mut g.rules)
        .improvements
        .get_mut("plantation")
        .unwrap()
        .civic = Some(crate::name!("feudalism"));
    assert_eq!(candidate().unconnected_luxury_tech(&g, 0), None);
    g.players[0].civics.insert(crate::name!("feudalism"));
    assert_eq!(
        candidate().unconnected_luxury_tech(&g, 0),
        Some("irrigation")
    );
    Arc::make_mut(&mut g.rules)
        .improvements
        .get_mut("plantation")
        .unwrap()
        .unique_to = Some("China".to_string());
    assert_eq!(candidate().unconnected_luxury_tech(&g, 0), None);
}

#[test]
fn unaffordable_research_yields_to_the_lane() {
    let (mut g, cid, _) = board();
    let science = g.city_yields(cid).science;
    let adjustment = g.observed_city_yield_adjustments[&cid].science;
    Arc::make_mut(&mut g.observed_city_yield_adjustments)
        .get_mut(&cid)
        .unwrap()
        .science = adjustment - science;
    assert_eq!(g.city_yields(cid).science, 0.0);
    assert_eq!(candidate().unconnected_luxury_tech(&g, 0), None);
}
