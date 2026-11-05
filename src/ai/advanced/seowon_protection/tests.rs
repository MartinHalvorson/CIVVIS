use super::*;
use crate::ai::advanced::{StrategicPlan, VictoryTarget};
use crate::rules::Yields;
use crate::world::DistrictFoundation;

fn fixture() -> (Game, u32, Pos, Pos) {
    let mut g = Game::new_full(2, 28, 18, 91779, 250, 0, false);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.owner_city = None;
        tile.river_edges = [false; 6];
    }
    g.players[0].civ = "Korea".to_string();
    let center = (8, 8);
    let cid = g.found_city_for(0, center, None);
    let positions = g.wdisk(center, 3);
    for pos in &positions {
        g.map.tiles.get_mut(pos).unwrap().owner_city = Some(cid);
    }
    let city = g.cities.get_mut(&cid).unwrap();
    city.owned_tiles = positions;
    city.pop = 10;
    let seowon = (10, 8);
    let site = (10, 7);
    let tile = g.map.tiles.get_mut(&seowon).unwrap();
    tile.hills = true;
    tile.district = Some(crate::name!("seowon"));
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("seowon"), seowon);
    g.players[0].techs.insert(crate::name!("writing"));
    g.players[0].civics.insert(crate::name!("drama_poetry"));
    (g, cid, seowon, site)
}

fn on() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_districts_protect_seowon_science();
    ai
}

fn loss(ai: &AdvancedAi, g: &Game, site: Pos, district: &str) -> f64 {
    ai.seowon_site_loss_value(g, 0, Name::new(district), site, GrandStrategy::Science)
}

#[test]
fn seowon_protection_uses_the_engine_loss_and_policy_multiplier() {
    let (mut g, _, seowon, site) = fixture();
    let ai = on();
    let one = ai.yield_value(
        Yields {
            science: 1.0,
            ..Yields::default()
        },
        GrandStrategy::Science,
    );
    assert!((loss(&ai, &g, site, "theater_square") - one).abs() < 1e-9);
    g.players[0]
        .policies
        .insert(crate::name!("natural_philosophy"));
    let quoted = loss(&ai, &g, site, "theater_square");
    assert!((quoted - 2.0 * one).abs() < 1e-9);
    let before = g.district_yields(crate::name!("seowon"), seowon).science;
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("theater_square"));
    let after = g.district_yields(crate::name!("seowon"), seowon).science;
    assert_eq!(
        before - after,
        2.0,
        "the projected loss equals actual engine output"
    );
}

#[test]
fn seowon_protection_keeps_campus_science_pricing_through_expansion() {
    let (g, _, _, site) = fixture();
    let mut ai = on();
    ai.enable_campus_through_expansion();
    let science = loss(&ai, &g, site, "theater_square");
    let expansion = ai.seowon_site_loss_value(
        &g,
        0,
        crate::name!("theater_square"),
        site,
        GrandStrategy::Expansion,
    );
    assert_eq!(
        science, expansion,
        "the district cannot discount the Science it removes"
    );
    let mut other_lane = AdvancedAi::new();
    other_lane.enable_districts_protect_seowon_science();
    other_lane.enable_campus_through_expansion();
    let unchanged = other_lane.seowon_site_loss_value(
        &g,
        0,
        crate::name!("theater_square"),
        site,
        GrandStrategy::Expansion,
    );
    assert!(
        unchanged < science,
        "the Science-only pricing must not change other lanes"
    );
}

#[test]
fn seowon_protection_leaves_plazas_distant_sites_and_control_unchanged() {
    let (g, _, _, site) = fixture();
    let ai = on();
    assert_eq!(loss(&ai, &g, site, "government_plaza"), 0.0);
    assert_eq!(loss(&ai, &g, (6, 8), "theater_square"), 0.0);
    let control = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(loss(&control, &g, site, "theater_square"), 0.0);
}

#[test]
fn seowon_protection_prices_started_seowons_without_recharging_placed_districts() {
    let (mut g, cid, seowon, site) = fixture();
    let ai = on();
    let before = loss(&ai, &g, site, "theater_square");
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .remove(crate::name!("seowon"));
    let tile = g.map.tiles.get_mut(&seowon).unwrap();
    tile.district = None;
    tile.district_foundation = Some(DistrictFoundation {
        district: crate::name!("seowon"),
        cost: 27.0,
    });
    assert_eq!(loss(&ai, &g, site, "theater_square"), before);
    g.map.tiles.get_mut(&site).unwrap().district_foundation = Some(DistrictFoundation {
        district: crate::name!("theater_square"),
        cost: 54.0,
    });
    assert_eq!(loss(&ai, &g, site, "theater_square"), 0.0);
}

#[test]
fn seowon_protection_counts_multiple_owned_neighbors_but_not_rivals() {
    let (mut g, cid, _, site) = fixture();
    let ai = on();
    let one = loss(&ai, &g, site, "theater_square");
    let second = (11, 7);
    let tile = g.map.tiles.get_mut(&second).unwrap();
    tile.district_foundation = Some(DistrictFoundation {
        district: crate::name!("seowon"),
        cost: 27.0,
    });
    assert!(
        (loss(&ai, &g, site, "theater_square") - 2.0 * one).abs() < 1e-9,
        "two owned Seowons each pay the same one-point adjacency loss"
    );
    g.cities.get_mut(&cid).unwrap().owner = 1;
    assert_eq!(loss(&ai, &g, site, "theater_square"), 0.0);
}

#[test]
fn seowon_protection_survives_planner_floors_without_double_charging() {
    use crate::ai::advanced::district_planning::PlannedDistrict;
    let (g, cid, _, site) = fixture();
    let mut ai = on();
    ai.enable_campus_through_expansion();
    ai.enable_district_planning_3();
    let mut control = AdvancedAi::targeting(VictoryTarget::Science);
    control.enable_campus_through_expansion();
    control.enable_district_planning_3();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    let district = crate::name!("theater_square");
    let items = vec![Item::District {
        district,
        pos: site,
    }];
    let rows = vec![PlannedDistrict {
        district,
        family: district,
        pos: site,
        support: false,
        order: 0,
        purchase: None,
        owned_fallback: None,
    }];
    let mut ordinary = vec![-500.0];
    let mut protected = ordinary.clone();
    control.district_plan_adjust_menu_scores(&g, &plan, cid, &rows, &items, &mut ordinary);
    ai.district_plan_adjust_menu_scores(&g, &plan, cid, &rows, &items, &mut protected);
    let charge = ai.seowon_site_loss_score(&g, 0, cid, &items[0], plan.strategy);
    assert!(charge > 0.0);
    let counts = ai.counts(&g, 0);
    let raw_difference = control.production_value(&g, 0, cid, &items[0], &plan, &counts)
        - ai.production_value(&g, 0, cid, &items[0], &plan, &counts);
    assert!(
        (raw_difference - charge).abs() < 1e-9,
        "the floor charge must match the ordinary production scorer's loss"
    );
    assert!(
        (ordinary[0] - protected[0] - charge).abs() < 1e-9,
        "the planner floor must not erase the Seowon charge"
    );
    let already_priced = 1_000.0 - charge;
    let mut high = vec![already_priced];
    ai.district_plan_adjust_menu_scores(&g, &plan, cid, &rows, &items, &mut high);
    assert_eq!(
        high[0], already_priced,
        "a raw score above the floor must not pay twice"
    );
}

#[test]
fn seowon_protection_reaches_the_culture_defense_reservation() {
    let (mut g, cid, _, _) = fixture();
    std::sync::Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        1,
        Yields {
            culture: 200.0,
            ..Yields::default()
        },
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut control = AdvancedAi::targeting(VictoryTarget::Science);
    control.enable_culture_defense_theater();
    control.base.w.city_target = 1.0;
    let mut protected = control.clone();
    protected.enable_districts_protect_seowon_science();
    let mut off = g.clone();
    let mut on = g.clone();
    control.reserve_culture_defense_theater(&mut off, 0, &plan);
    protected.reserve_culture_defense_theater(&mut on, 0, &plan);
    let site = |board: &Game| match board.cities[&cid].queue.first() {
        Some(Item::District { district, pos }) if district == "theater_square" => *pos,
        other => panic!("the reservation must still build its Theater: {other:?}"),
    };
    let original = site(&off);
    let clean = site(&on);
    assert!(loss(&protected, &g, original, "theater_square") > 0.0);
    assert_eq!(loss(&protected, &g, clean, "theater_square"), 0.0);
    assert_ne!(
        original, clean,
        "the reservation must reach the site protection"
    );
    let item = off.cities[&cid].queue[0].clone();
    assert_eq!(
        control.seowon_protection_reserved_site(&g, 0, cid, item.clone(), &plan),
        item,
        "the disabled gene must preserve the original reservation"
    );
}

#[test]
fn seowon_protection_changes_production_site_preference_and_exposes_alternatives() {
    let (g, cid, _, site) = fixture();
    let ai = on();
    let control = AdvancedAi::targeting(VictoryTarget::Science);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    let counts = ai.counts(&g, 0);
    let crowded = Item::District {
        district: crate::name!("theater_square"),
        pos: site,
    };
    let clean = Item::District {
        district: crate::name!("theater_square"),
        pos: (6, 8),
    };
    let a = control.production_value(&g, 0, cid, &crowded, &plan, &counts);
    let b = ai.production_value(&g, 0, cid, &crowded, &plan, &counts);
    assert!(
        b < a,
        "the production scorer actually charges the externality"
    );
    assert_eq!(
        control.production_value(&g, 0, cid, &clean, &plan, &counts),
        ai.production_value(&g, 0, cid, &clean, &plan, &counts)
    );
    assert!(
        ai.production_value(&g, 0, cid, &clean, &plan, &counts) > b,
        "the clean district site must beat the crowded site in the Science lane"
    );
    let mut menu = vec![crowded.clone()];
    control.seowon_protection_expand_menu(&g, 0, cid, &mut menu);
    assert_eq!(menu, vec![crowded], "the control retains its exact menu");
    ai.seowon_protection_expand_menu(&g, 0, cid, &mut menu);
    assert!(
        menu.contains(&clean),
        "a clean legal site survives the two-site shortlist"
    );
    assert!(menu.iter().all(|item| g.can_produce(0, cid, item)));
}

fn actual_placement_science_loss(g: &Game, cid: u32, site: Pos) -> f64 {
    let mut placed = g.clone();
    placed.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("theater_square"));
    placed
        .cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), site);
    g.city_yields(cid).science - placed.city_yields(cid).science
}

#[test]
fn rationalism_threshold_loss_matches_powered_and_scientist_building_yields() {
    for pop in [10, 15] {
        for powered in [false, true] {
            for pillaged_lab in [false, true] {
                let (mut g, cid, _, site) = fixture();
                let ai = on();
                g.cities.get_mut(&cid).unwrap().pop = pop;
                g.cities.get_mut(&cid).unwrap().buildings = vec![
                    crate::name!("library"),
                    crate::name!("university"),
                    crate::name!("research_lab"),
                ];
                if pillaged_lab {
                    g.cities
                        .get_mut(&cid)
                        .unwrap()
                        .pillaged_buildings
                        .insert(crate::name!("research_lab"));
                }
                // A synthetic zero-demand laboratory isolates the powered
                // branch without introducing a power plant's regional yields.
                std::sync::Arc::make_mut(&mut g.rules)
                    .buildings
                    .get_mut("research_lab")
                    .unwrap()
                    .power = if powered { 0.0 } else { 3.0 };
                std::sync::Arc::make_mut(&mut g.rules)
                    .districts
                    .get_mut("seowon")
                    .unwrap()
                    .effects
                    .insert("building_science".to_string(), 2.0);
                for (counter, amount) in [
                    ("great_person:library_science", 1),
                    ("great_person:university_science", 2),
                    ("great_person:research_lab_science", 3),
                ] {
                    g.players[0].counters.insert(counter.to_string(), amount);
                }
                g.players[0]
                    .policies
                    .insert(crate::name!("natural_philosophy"));
                let unmultiplied = actual_placement_science_loss(&g, cid, site);
                assert!(unmultiplied > 0.0);
                // Normalize only the city's final amenity/empire multiplier;
                // the production scorer prices raw yield, as it did before.
                let city_multiplier = unmultiplied / 2.0;
                g.players[0].policies.insert(crate::name!("rationalism"));
                let expected = actual_placement_science_loss(&g, cid, site) / city_multiplier;
                let one = ai.yield_value(
                    Yields {
                        science: 1.0,
                        ..Yields::default()
                    },
                    GrandStrategy::Science,
                );
                assert!(
                    (loss(&ai, &g, site, "theater_square") / one - expected).abs() < 1e-8,
                    "pop={pop} powered={powered} pillaged_lab={pillaged_lab}, expected {expected}"
                );
                assert!(
                    expected > 2.0,
                    "the card loses more than just doubled adjacency"
                );
                assert_eq!(loss(&AdvancedAi::new(), &g, site, "theater_square"), 0.0);
            }
        }
    }
}

#[test]
fn rationalism_population_half_and_doubled_subthreshold_adjacency_are_not_lost() {
    let (mut g, cid, seowon, site) = fixture();
    g.cities.get_mut(&cid).unwrap().pop = 15;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    g.players[0]
        .policies
        .insert(crate::name!("natural_philosophy"));
    g.players[0].policies.insert(crate::name!("rationalism"));
    let neighbor = g.nbrs(seowon).into_iter().find(|p| *p != site).unwrap();
    g.map.tiles.get_mut(&neighbor).unwrap().district = Some(crate::name!("aqueduct"));
    let ai = on();
    let one = ai.yield_value(
        Yields {
            science: 1.0,
            ..Yields::default()
        },
        GrandStrategy::Science,
    );
    assert!(
        (loss(&ai, &g, site, "theater_square") / one - 2.0).abs() < 1e-9,
        "raw3->2 is doubled6->4; the adjacency half is absent and population's half remains"
    );
}
