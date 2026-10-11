use super::*;
use crate::ai::advanced::aqueduct_claims_the_capped_queue::CAPPED_AQUEDUCT_MIN_POP;

/// Whether `cid` would have a legal Aqueduct site with Engineering known.
fn legal_with_engineering(game: &Game, cid: u32) -> bool {
    let mut future = game.clone();
    future.players[0].techs.insert(crate::name!("engineering"));
    let aqueduct = BasicAi::civ_district(game, 0, "aqueduct");
    game.district_sites(cid, aqueduct).into_iter().any(|pos| {
        future.can_produce(
            0,
            cid,
            &Item::District {
                district: aqueduct,
                pos,
            },
        )
    })
}

/// Our `count` cities, each at least four tiles from the others, each with a
/// legal Aqueduct site once Engineering is known, every one capped (population
/// at its housing) with its Granary built. Pottery, Mining, the Wheel, Masonry and
/// Bronze Working known; Engineering not. Every unit removed.
fn capped_empire(count: usize) -> Option<(Game, Vec<u32>)> {
    for seed in [936_221_u64, 424_242, 777_001, 31_337, 12_345] {
        let mut game = Game::new_full(1, 32, 20, seed, 200, 0, false);
        for uid in game.player_unit_ids(0) {
            game.remove_unit(uid);
        }
        for tech in ["pottery", "mining", "wheel", "masonry", "bronze_working"] {
            game.players[0].techs.insert(Name::new(tech));
        }
        let mut sites: Vec<Pos> = game
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| !game.rules.is_water(tile))
            .map(|(pos, _)| *pos)
            .collect();
        sites.sort();
        let mut ours: Vec<u32> = Vec::new();
        for site in sites {
            if ours.len() == count {
                break;
            }
            if ours
                .iter()
                .any(|cid| game.wdist(game.cities[cid].pos, site) < 4)
            {
                continue;
            }
            let mut trial = game.clone();
            let cid = trial.place_city(0, site, None);
            if !legal_with_engineering(&trial, cid) {
                continue;
            }
            game = trial;
            ours.push(cid);
        }
        if ours.len() < count {
            continue;
        }
        for cid in &ours {
            let city = game.cities.get_mut(cid).unwrap();
            city.buildings.push(crate::name!("granary"));
            city.queue.clear();
            let housing = game.city_housing(&game.cities[cid]);
            game.cities.get_mut(cid).unwrap().pop =
                (housing.ceil() as i32).max(CAPPED_AQUEDUCT_MIN_POP);
        }
        return Some((game, ours));
    }
    None
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_engineering_for_capped_fresh_water();
    ai
}

/// Engineering almost paid for: the remaining path is a sliver of a turn.
fn nearly_researched(game: &mut Game) {
    let cost = game.tech_cost("engineering");
    game.players[0].research = Some("engineering".to_string());
    game.players[0].research_progress = (cost - 1.0).max(0.0);
}

#[test]
fn two_capped_cities_with_a_cheap_path_research_engineering() {
    let (mut game, _) = capped_empire(2).expect("a two-city capped fixture");
    nearly_researched(&mut game);
    let pick = armed()
        .capped_fresh_water_engineering_goal(&game, 0)
        .expect("two capped cities and a cheap path");
    assert_eq!(pick.tech, crate::name!("engineering"));
    assert_eq!(pick.cities, 2);
}

#[test]
fn the_gene_off_leaves_the_slot_alone() {
    let (mut game, _) = capped_empire(2).expect("a two-city capped fixture");
    nearly_researched(&mut game);
    assert!(AdvancedAi::new()
        .capped_fresh_water_engineering_goal(&game, 0)
        .is_none());
}

#[test]
fn one_capped_city_is_a_production_problem_not_research() {
    let (mut game, cities) = capped_empire(2).expect("a two-city capped fixture");
    nearly_researched(&mut game);
    game.cities.get_mut(&cities[1]).unwrap().pop = 1;
    assert!(armed()
        .capped_fresh_water_engineering_goal(&game, 0)
        .is_none());
}

#[test]
fn a_known_engineering_ends_the_rule() {
    let (mut game, _) = capped_empire(2).expect("a two-city capped fixture");
    game.players[0].techs.insert(crate::name!("engineering"));
    assert!(armed()
        .capped_fresh_water_engineering_goal(&game, 0)
        .is_none());
}

#[test]
fn a_dear_path_leaves_the_beeline_alone() {
    let (mut game, _) = capped_empire(2).expect("a two-city capped fixture");
    // Nothing invested and the Wheel forgotten: the path is two technologies
    // at a few beakers a turn, far past the cap.
    game.players[0].techs.remove(&crate::name!("wheel"));
    game.players[0].research = None;
    game.players[0].research_progress = 0.0;
    game.players[0].research_overflow = 0.0;
    let science = AdvancedAi::empire_science(&game, 0).max(1.0);
    let owed = game.tech_cost("engineering") + game.tech_cost("wheel");
    assert!(owed / science > game.standard_duration(ENGINEERING_MAX_RESEARCH_STANDARD) as f64);
    assert!(armed()
        .capped_fresh_water_engineering_goal(&game, 0)
        .is_none());
}

#[test]
fn a_city_without_its_granary_does_not_count() {
    let (mut game, cities) = capped_empire(2).expect("a two-city capped fixture");
    nearly_researched(&mut game);
    game.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .retain(|b| *b != crate::name!("granary"));
    assert!(armed()
        .capped_fresh_water_engineering_goal(&game, 0)
        .is_none());
}

/// The live mirror carries the host's menu, which before Engineering lists no
/// Aqueduct anywhere: the counterfactual must not read today's menu as
/// tomorrow's legality.
#[test]
fn a_host_menu_without_the_aqueduct_does_not_hide_the_site() {
    let (mut game, cities) = capped_empire(2).expect("a two-city capped fixture");
    nearly_researched(&mut game);
    let warrior = Item::Unit {
        unit: Name::new("warrior"),
    };
    let menu: std::collections::BTreeMap<String, crate::game::HostMenuEntry> =
        std::collections::BTreeMap::from([(
            Game::production_block_key(&warrior),
            crate::game::HostMenuEntry {
                cost: Some(20.0),
                turns: Some(2.0),
            },
        )]);
    game.host_buildable =
        std::sync::Arc::new(cities.iter().map(|cid| (*cid, menu.clone())).collect());
    let pick = armed()
        .capped_fresh_water_engineering_goal(&game, 0)
        .expect("the site is the ruleset's question, not today's menu");
    assert_eq!(pick.tech, crate::name!("engineering"));
}
