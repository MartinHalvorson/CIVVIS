use super::*;
use crate::ai::{AdvancedAi, BasicAi, GrandStrategy};
use crate::name::Name;

fn game_with_capitals(players: usize, seed: u64) -> (Game, Vec<u32>) {
    let mut game = Game::new_full(players, 28, 18, seed, 300, 0, false);
    let mut cities = Vec::new();
    for pid in 0..players {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let city = game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
        cities.push(city);
    }
    (game, cities)
}

fn add_city_state(game: &mut Game, name: &str) -> usize {
    let id = game.players.len();
    game.players.push(Player::new(id, name, true));
    id
}

fn make_suzerain(game: &mut Game, leader: usize, minor: usize) {
    match game.players[leader]
        .envoys
        .iter_mut()
        .find(|(city_state, _)| *city_state == minor)
    {
        Some((_, count)) => *count = 3,
        None => game.players[leader].envoys.push((minor, 3)),
    }
}

fn install_alliance(game: &mut Game, first: usize, second: usize, kind: &str, level: i32) {
    let alliance = AllianceState {
        kind: kind.to_string(),
        points: match level {
            3.. => 240.0,
            2 => 80.0,
            _ => 0.0,
        },
        level,
        ends: game.turn + 60,
    };
    game.players[first]
        .alliances
        .insert(second, alliance.clone());
    game.players[second].alliances.insert(first, alliance);
}

fn install_district(game: &mut Game, city_id: u32, district: &str) -> Pos {
    let center = game.cities[&city_id].pos;
    let position = game.cities[&city_id]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            *position != center
                && game.map.tiles[position].district.is_none()
                && game.map.tiles[position].wonder.is_none()
        })
        .unwrap();
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.feature = None;
    tile.resource = None;
    tile.improvement = None;
    tile.district = Some(Name::new(district));
    tile.pillaged = false;
    game.cities
        .get_mut(&city_id)
        .unwrap()
        .districts
        .insert(Name::new(district), position);
    position
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn gathering_storm_district_production_rows_apply_to_every_owner() {
    let (mut game, cities) = game_with_capitals(1, 89_094);
    let city = cities[0];
    let pos = owned_non_center_site(&game, city);
    let district = |family: &str| Item::District {
        district: Name::new(family),
        pos,
    };

    // MINOR_CIV_PRODUCTION_HARBORS applies to every type, and each of the
    // six type traits pays the matching specialty-district row.
    for (city_state, specialty) in [
        ("Geneva", "campus"),
        ("Mohenjo-Daro", "theater_square"),
        ("Yerevan", "holy_site"),
        ("Kabul", "encampment"),
        ("Auckland", "industrial_zone"),
        ("Zanzibar", "commercial_hub"),
    ] {
        game.players[0].is_minor = true;
        game.players[0].civ = city_state.to_string();
        assert_close(game.item_prod_mult(0, city, Some(&district("harbor"))), 6.0);
        assert_close(
            game.item_prod_mult(0, city, Some(&district(specialty))),
            6.0,
        );
        assert_close(game.item_prod_mult(0, city, Some(&district("dam"))), 1.0);
    }

    game.players[0].is_minor = false;
    game.players[0].civ = "Japan".to_string();
    for specialty in ["encampment", "holy_site", "theater_square"] {
        assert_close(
            game.item_prod_mult(0, city, Some(&district(specialty))),
            2.0,
        );
    }
    assert_close(game.item_prod_mult(0, city, Some(&district("campus"))), 1.0);

    game.players[0].civ = "Netherlands".to_string();
    assert_close(game.item_prod_mult(0, city, Some(&district("dam"))), 1.5);
    assert_close(game.item_prod_mult(0, city, Some(&district("campus"))), 1.0);

    // Veterans' two rows were already represented by the shared policy key;
    // keep that behavior pinned while adding the remaining eleven rows.
    game.players[0].civ = "Rome".to_string();
    game.players[0].policies.insert(crate::name!("veterancy"));
    for specialty in ["encampment", "harbor"] {
        assert_close(
            game.item_prod_mult(0, city, Some(&district(specialty))),
            1.3,
        );
    }
}

fn owned_non_center_site(game: &Game, city: u32) -> Pos {
    let center = game.cities[&city].pos;
    game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center && game.map.tiles.contains_key(position))
        .expect("a founded capital owns a non-center tile")
}

fn prepare_improvement_site(game: &mut Game, position: Pos, terrain: &str, hills: bool) {
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.terrain = Name::new(terrain);
    tile.hills = hills;
    tile.feature = None;
    tile.resource = None;
    tile.improvement = None;
    tile.pillaged = false;
    tile.flooded = false;
    tile.district = None;
    tile.district_foundation = None;
    tile.wonder = None;
}

#[test]
fn rapa_nui_moai_are_suzerain_actions_with_real_adjacency_yields() {
    let (mut game, cities) = game_with_capitals(1, 89_101);
    let city = cities[0];
    let target = owned_non_center_site(&game, city);
    prepare_improvement_site(&mut game, target, "plains", false);
    for neighbor in game.nbrs(target) {
        if let Some(tile) = game.map.tiles.get_mut(&neighbor) {
            tile.feature = None;
        }
    }

    let rapa_nui = add_city_state(&mut game, "Rapa Nui");
    assert!(
        !game
            .valid_improvements(0, target)
            .contains(&crate::name!("moai")),
        "a normal civilization cannot place Rapa Nui's unique improvement"
    );
    make_suzerain(&mut game, 0, rapa_nui);
    assert!(game
        .valid_improvements(0, target)
        .contains(&crate::name!("moai")));

    let blocked_neighbor = game.nbrs(target)[0];
    game.map.tiles.get_mut(&blocked_neighbor).unwrap().feature = Some(crate::name!("forest"));
    assert!(
        !game
            .valid_improvements(0, target)
            .contains(&crate::name!("moai")),
        "Moai next to Woods must be refused"
    );
    game.map.tiles.get_mut(&blocked_neighbor).unwrap().feature = None;

    let builder = game.spawn_test_unit("builder", 0, target);
    let action = Action::Improve {
        unit: builder,
        improvement: crate::name!("moai"),
    };
    assert!(game.legal_actions(0).contains(&action));
    game.apply(0, &action).unwrap();

    let neighbor = game
        .nbrs(target)
        .into_iter()
        .find(|position| {
            game.cities[&city].owned_tiles.contains(position) && *position != game.cities[&city].pos
        })
        .expect("the capital ring contains adjacent owned sites");
    prepare_improvement_site(&mut game, neighbor, "plains", false);
    game.map.tiles.get_mut(&neighbor).unwrap().improvement = Some(crate::name!("moai"));
    let before = game.player_tile_yields(0, target, &game.map.tiles[&target]);
    game.players[0]
        .civics
        .insert(crate::name!("medieval_faires"));
    let after = game.player_tile_yields(0, target, &game.map.tiles[&target]);
    assert_close(after.culture, before.culture + 1.0);
}

#[test]
fn portugal_nau_builds_feitorias_only_with_foreign_open_borders_and_pays_routes() {
    let (mut game, cities) = game_with_capitals(2, 89_102);
    let foreign_city = cities[1];
    let target = owned_non_center_site(&game, foreign_city);
    prepare_improvement_site(&mut game, target, "coast", false);
    let shore = game
        .nbrs(target)
        .into_iter()
        .find(|position| *position != game.cities[&foreign_city].pos)
        .expect("a map tile has a neighbouring shore site");
    prepare_improvement_site(&mut game, shore, "grassland", false);
    game.map.tiles.get_mut(&shore).unwrap().resource = Some(crate::name!("wheat"));
    game.players[0].civ = "Portugal".to_string();
    game.players[0].techs.insert(crate::name!("cartography"));
    game.players[1].civics.insert(crate::name!("early_empire"));

    assert!(
        !game
            .valid_improvements(0, target)
            .contains(&crate::name!("feitoria")),
        "foreign coast remains closed until its owner grants Open Borders"
    );
    game.players[1].open_borders_until.insert(0, game.turn + 30);
    assert!(game
        .valid_improvements(0, target)
        .contains(&crate::name!("feitoria")));

    let nau = game.spawn_test_unit("nau", 0, target);
    let action = Action::Improve {
        unit: nau,
        improvement: crate::name!("feitoria"),
    };
    assert!(game.legal_actions(0).contains(&action));
    let before = game.trade_route_yields(0, foreign_city);
    game.apply(0, &action).unwrap();
    let after = game.trade_route_yields(0, foreign_city);
    assert_close(after.gold, before.gold + 4.0);
    assert_close(after.production, before.production + 1.0);
}

#[test]
fn special_improver_units_and_incan_mountain_roads_are_actionable() {
    let (mut game, cities) = game_with_capitals(1, 89_103);
    let city = cities[0];
    let center = game.cities[&city].pos;
    let sites: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .take(3)
        .collect();
    assert_eq!(sites.len(), 3, "the capital needs three improvement sites");

    game.players[0].civ = "Maori".to_string();
    prepare_improvement_site(&mut game, sites[0], "plains", true);
    let toa = game.spawn_test_unit("toa", 0, sites[0]);
    let pa = Action::Improve {
        unit: toa,
        improvement: crate::name!("maori_pa"),
    };
    assert!(game.legal_actions(0).contains(&pa));
    game.apply(0, &pa).unwrap();
    assert_eq!(
        game.map.tiles[&sites[0]].improvement.as_deref(),
        Some("maori_pa")
    );

    game.players[0].civ = "Rome".to_string();
    prepare_improvement_site(&mut game, sites[1], "grassland", false);
    let legion = game.spawn_test_unit("legion", 0, sites[1]);
    let fort = Action::Improve {
        unit: legion,
        improvement: crate::name!("roman_fort"),
    };
    assert!(game.legal_actions(0).contains(&fort));
    game.apply(0, &fort).unwrap();
    assert_eq!(
        game.map.tiles[&sites[1]].improvement.as_deref(),
        Some("roman_fort")
    );

    game.players[0].civ = "Inca".to_string();
    game.players[0].civics.insert(crate::name!("foreign_trade"));
    prepare_improvement_site(&mut game, sites[2], "mountain", false);
    let approach = game.spawn_test_unit("builder", 0, center);
    assert!(
        game.can_enter(approach, center, sites[2]),
        "Foreign Trade lets Incan Builders reach a Qhapaq Nan site"
    );
    let builder = game.spawn_test_unit("builder", 0, sites[2]);
    let road = Action::Improve {
        unit: builder,
        improvement: crate::name!("qhapaq_nan"),
    };
    assert!(game.legal_actions(0).contains(&road));
    game.apply(0, &road).unwrap();
    assert_eq!(
        game.map.tiles[&sites[2]].improvement.as_deref(),
        Some("qhapaq_nan")
    );
}

#[test]
fn basic_ai_spends_toa_and_legion_charges_on_their_unique_improvements() {
    let (mut game, cities) = game_with_capitals(1, 89_104);
    let city = cities[0];
    let sites: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != game.cities[&city].pos)
        .take(2)
        .collect();
    assert_eq!(sites.len(), 2, "the capital needs two usable sites");

    game.players[0].civ = "Maori".to_string();
    prepare_improvement_site(&mut game, sites[0], "plains", true);
    let toa = game.spawn_test_unit("toa", 0, sites[0]);
    assert_eq!(
        BasicAi::new().special_improver_step(&mut game, 0, toa),
        Some(true)
    );
    assert_eq!(
        game.map.tiles[&sites[0]].improvement.as_deref(),
        Some("maori_pa")
    );

    game.players[0].civ = "Rome".to_string();
    prepare_improvement_site(&mut game, sites[1], "grassland", false);
    let legion = game.spawn_test_unit("legion", 0, sites[1]);
    assert_eq!(
        BasicAi::new().special_improver_step(&mut game, 0, legion),
        Some(true)
    );
    assert_eq!(
        game.map.tiles[&sites[1]].improvement.as_deref(),
        Some("roman_fort")
    );
}

#[test]
fn basic_ai_routes_a_charged_toa_to_its_only_legal_pa_site() {
    let (mut game, cities) = game_with_capitals(1, 89_106);
    let city = cities[0];
    let center = game.cities[&city].pos;
    let sites: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .collect();
    let target = sites
        .iter()
        .copied()
        .find(|position| {
            game.nbrs(*position)
                .into_iter()
                .any(|neighbor| neighbor != center && sites.contains(&neighbor))
        })
        .expect("the capital ring contains adjacent non-center sites");
    let start = game
        .nbrs(target)
        .into_iter()
        .find(|position| *position != center && sites.contains(position))
        .expect("the Pa site has an adjacent owned approach");
    for position in &sites {
        prepare_improvement_site(&mut game, *position, "grassland", false);
    }
    prepare_improvement_site(&mut game, target, "plains", true);
    game.players[0].civ = "Maori".to_string();
    let toa = game.spawn_test_unit("toa", 0, start);

    assert_eq!(
        BasicAi::new().special_improver_step(&mut game, 0, toa),
        Some(true)
    );
    assert_eq!(game.units[&toa].pos, target);
    assert!(
        game.map.tiles[&target].improvement.is_none(),
        "the first action should route the Toa; a later action spends the charge"
    );
}

#[test]
fn advanced_ai_spends_nau_charge_only_on_an_open_foreign_feitoria_site() {
    let (mut game, cities) = game_with_capitals(2, 89_105);
    let foreign_city = cities[1];
    let target = owned_non_center_site(&game, foreign_city);
    prepare_improvement_site(&mut game, target, "coast", false);
    let shore = game
        .nbrs(target)
        .into_iter()
        .find(|position| *position != game.cities[&foreign_city].pos)
        .expect("the foreign coast needs an adjacent land resource");
    prepare_improvement_site(&mut game, shore, "grassland", false);
    game.map.tiles.get_mut(&shore).unwrap().resource = Some(crate::name!("wheat"));
    game.players[0].civ = "Portugal".to_string();
    game.players[0].techs.insert(crate::name!("cartography"));
    game.players[1].civics.insert(crate::name!("early_empire"));
    let nau = game.spawn_test_unit("nau", 0, target);
    let mut ai = AdvancedAi::new();

    assert_eq!(
        ai.advanced_special_improver_step(&mut game, 0, nau, GrandStrategy::Diplomacy),
        None,
        "without Open Borders the Nau must keep its charge and fall back to military behavior"
    );
    assert!(game.map.tiles[&target].improvement.is_none());

    game.players[1].open_borders_until.insert(0, game.turn + 30);
    assert_eq!(
        ai.advanced_special_improver_step(&mut game, 0, nau, GrandStrategy::Diplomacy),
        Some(true)
    );
    assert_eq!(
        game.map.tiles[&target].improvement.as_deref(),
        Some("feitoria")
    );
}

#[test]
fn advanced_ai_routes_a_nau_to_the_best_reachable_feitoria_site() {
    let (mut game, cities) = game_with_capitals(2, 89_107);
    let foreign_city = cities[1];
    let center = game.cities[&foreign_city].pos;
    let sites: Vec<Pos> = game.cities[&foreign_city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .collect();
    let (target, start, shore) = sites
        .iter()
        .copied()
        .find_map(|target| {
            sites
                .iter()
                .copied()
                .filter(|start| game.nbrs(target).contains(start))
                .find_map(|start| {
                    game.nbrs(target)
                        .into_iter()
                        .find(|shore| {
                            *shore != center && *shore != start && !game.nbrs(start).contains(shore)
                        })
                        .map(|shore| (target, start, shore))
                })
        })
        .expect("the capital ring needs a coast, approach, and exclusive shore");
    for position in &sites {
        prepare_improvement_site(&mut game, *position, "grassland", false);
    }
    for position in game.nbrs(target) {
        prepare_improvement_site(&mut game, position, "grassland", false);
    }
    prepare_improvement_site(&mut game, target, "coast", false);
    prepare_improvement_site(&mut game, start, "coast", false);
    game.map.tiles.get_mut(&shore).unwrap().resource = Some(crate::name!("wheat"));
    game.players[0].civ = "Portugal".to_string();
    game.players[0].techs.insert(crate::name!("cartography"));
    game.players[1].civics.insert(crate::name!("early_empire"));
    game.players[1].open_borders_until.insert(0, game.turn + 30);
    let nau = game.spawn_test_unit("nau", 0, start);
    let mut ai = AdvancedAi::new();

    assert!(
        !game
            .valid_improvements(0, start)
            .contains(&crate::name!("feitoria")),
        "the approach is water but lacks the required adjacent resource"
    );
    assert!(game
        .valid_improvements(0, target)
        .contains(&crate::name!("feitoria")));
    assert_eq!(
        ai.advanced_special_improver_step(&mut game, 0, nau, GrandStrategy::Diplomacy),
        Some(true)
    );
    assert_eq!(game.units[&nau].pos, target);
    assert!(game.map.tiles[&target].improvement.is_none());
}

#[test]
fn economic_level_three_shares_unique_suzerain_bonuses_without_relays() {
    let (mut game, _) = game_with_capitals(3, 89_001);
    let geneva = add_city_state(&mut game, "Geneva");
    make_suzerain(&mut game, 1, geneva);

    install_alliance(&mut game, 0, 1, "economic", 2);
    assert!(!game.grants_city_state_unique_bonus(0, "Geneva"));
    game.players[0].alliances.get_mut(&1).unwrap().level = 3;
    game.players[1].alliances.get_mut(&0).unwrap().level = 3;
    assert!(game.grants_city_state_unique_bonus(0, "Geneva"));
    assert!(game.grants_city_state_unique_bonus(1, "Geneva"));
    assert!(!game.grants_city_state_unique_bonus(2, "Geneva"));

    install_alliance(&mut game, 1, 2, "economic", 3);
    assert!(game.grants_city_state_unique_bonus(2, "Geneva"));
    game.players[0].alliances.clear();
    game.players[1].alliances.remove(&0);
    assert!(game.grants_city_state_unique_bonus(2, "Geneva"));
    assert!(!game.grants_city_state_unique_bonus(0, "Geneva"));

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert!(restored.grants_city_state_unique_bonus(2, "Geneva"));
}

#[test]
fn city_state_trade_route_and_sovereignty_match_gathering_storm_host() {
    let (mut game, _) = game_with_capitals(2, 89_008);
    let scientific = add_city_state(&mut game, "Geneva");
    let position = game
        .map
        .tiles
        .iter()
        .find_map(|(position, tile)| {
            (tile.owner_city.is_none()
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile))
            .then_some(*position)
        })
        .expect("the map has room for a city-state route destination");
    let city = game.found_city_for(scientific, position, None);

    let ordinary = game.trade_route_yields(0, city);
    assert_close(ordinary.gold, 3.0);
    // The installed Gathering Storm host does not apply its shipped
    // city-state SEND_TRADE_ROUTE_BONUS trait rows to route origins. This
    // destination has no district, so there is no other source of Science.
    assert_close(ordinary.science, 0.0);

    game.active_congress_effects.push(CongressEffect {
        resolution: "sovereignty".to_string(),
        outcome: "A".to_string(),
        target: "scientific".to_string(),
        expires: game.turn + 1,
    });
    let sovereign = game.trade_route_yields(0, city);
    assert_close(sovereign.gold, ordinary.gold);
    // Sovereignty A modifies that host-side city-state bonus, which is already
    // absent in Gathering Storm; it must not invent a Science yield.
    assert_close(sovereign.science, ordinary.science);

    let suzerain = add_city_state(&mut game, "Hattusa");
    make_suzerain(&mut game, 0, suzerain);
    assert!(game.grants_city_state_unique_bonus(0, "Hattusa"));
    game.active_congress_effects.clear();
    game.active_congress_effects.push(CongressEffect {
        resolution: "sovereignty".to_string(),
        outcome: "B".to_string(),
        target: "scientific".to_string(),
        expires: game.turn + 1,
    });
    assert!(
        !game.grants_city_state_unique_bonus(0, "Hattusa"),
        "Sovereignty B disables a matching type's unique Suzerain bonus"
    );
}

#[test]
fn carthage_mohenjo_daro_and_auckland_modify_their_native_systems() {
    let (mut game, cities) = game_with_capitals(2, 89_002);
    let city = cities[0];
    game.players[0].civics.insert(crate::name!("foreign_trade"));
    let encampment = install_district(&mut game, city, "encampment");
    let carthage = add_city_state(&mut game, "Carthage");
    let base_capacity = game.trade_capacity(0);
    make_suzerain(&mut game, 0, carthage);
    assert_eq!(game.trade_capacity(0), base_capacity + 1);
    assert_eq!(game.cs_type("Carthage"), "militaristic");

    let center = game.cities[&city].pos;
    game.map.tiles.get_mut(&center).unwrap().river_edges = [false; 6];
    for neighbor in game.nbrs(center) {
        let tile = game.map.tiles.get_mut(&neighbor).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.river_edges = [false; 6];
    }
    let housing_without = game.city_housing(&game.cities[&city]);
    game.players[carthage].civ = "Mohenjo-Daro".to_string();
    assert_close(
        game.city_housing(&game.cities[&city]),
        housing_without + 3.0,
    );

    let water = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center && *position != encampment)
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&water).unwrap();
        tile.terrain = crate::name!("coast");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
    }
    let baseline = game.player_tile_yields(0, water, &game.map.tiles[&water]);
    game.players[carthage].civ = "Auckland".to_string();
    let ancient = game.player_tile_yields(0, water, &game.map.tiles[&water]);
    assert_close(ancient.production, baseline.production + 1.0);
    game.world_era = 4;
    let industrial = game.player_tile_yields(0, water, &game.map.tiles[&water]);
    assert_close(industrial.production, baseline.production + 2.0);
}

#[test]
fn geneva_kabul_and_yerevan_apply_yields_experience_and_promotion_choice() {
    let (mut game, cities) = game_with_capitals(2, 89_003);
    let minor = add_city_state(&mut game, "Hattusa");
    make_suzerain(&mut game, 0, minor);
    let science_without = game.city_yields(cities[0]).science;
    game.players[minor].civ = "Geneva".to_string();
    assert_close(game.city_yields(cities[0]).science, science_without * 1.15);
    game.at_war.insert(pair(0, 1));
    assert_close(game.city_yields(cities[0]).science, science_without);
    game.at_war.clear();

    game.players[minor].civ = "Carthage".to_string();
    let attacker = game.spawn_test_unit("warrior", 0, game.cities[&cities[0]].pos);
    let defender = game.spawn_test_unit("warrior", 1, game.cities[&cities[1]].pos);
    let opponent = game.units[&defender].clone();
    game.award_unit_combat_xp(attacker, &opponent, false, true, false);
    assert_eq!(game.units[&attacker].xp, 4);
    game.units.get_mut(&attacker).unwrap().xp = 0;
    game.players[minor].civ = "Kabul".to_string();
    game.award_unit_combat_xp(attacker, &opponent, false, true, false);
    assert_eq!(game.units[&attacker].xp, 8);
    game.units.get_mut(&attacker).unwrap().xp = 0;
    game.award_initiated_combat_xp(attacker, 3.0);
    assert_eq!(
        game.units[&attacker].xp, 6,
        "Kabul also doubles fixed XP from initiated district combat"
    );

    game.players[minor].civ = "Kandy".to_string();
    let apostle = game.spawn_test_unit("apostle", 0, game.cities[&cities[0]].pos);
    assert_eq!(game.available_promotions(apostle).len(), 3);
    game.players[minor].civ = "Yerevan".to_string();
    assert_eq!(game.available_promotions(apostle).len(), 9);
}

#[test]
fn hattusa_stockholm_and_vilnius_use_resources_gpp_and_real_adjacency() {
    let (mut game, cities) = game_with_capitals(2, 89_004);
    let city = cities[0];
    let minor = add_city_state(&mut game, "Geneva");
    make_suzerain(&mut game, 0, minor);
    game.players[0].techs.insert(crate::name!("bronze_working"));
    for position in game.cities[&city].owned_tiles.clone() {
        game.map.tiles.get_mut(&position).unwrap().resource = None;
    }
    assert_close(game.strategic_resource_rate(0, "iron"), 0.0);
    game.players[minor].civ = "Hattusa".to_string();
    assert_close(game.strategic_resource_rate(0, "iron"), 2.0);

    let campus = install_district(&mut game, city, "campus");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    let mut without_stockholm = game.clone();
    without_stockholm.players[minor].civ = "Geneva".to_string();
    without_stockholm.process_great_people(0);
    game.players[minor].civ = "Stockholm".to_string();
    game.process_great_people(0);
    assert_close(
        game.players[0].gpp["scientist"],
        without_stockholm.players[0].gpp["scientist"] + 1.0,
    );

    let theater = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            *position != game.cities[&city].pos
                && *position != campus
                && game.map.tiles[position].district.is_none()
        })
        .unwrap();
    game.map.tiles.get_mut(&theater).unwrap().district = Some(crate::name!("theater_square"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), theater);
    let wonder = game
        .nbrs(theater)
        .into_iter()
        .find(|position| *position != game.cities[&city].pos && *position != campus)
        .unwrap();
    game.map.tiles.get_mut(&wonder).unwrap().wonder = Some(crate::name!("pyramids"));
    install_alliance(&mut game, 0, 1, "research", 2);
    game.players[minor].civ = "Mohenjo-Daro".to_string();
    let ordinary = game
        .district_yields(crate::name!("theater_square"), theater)
        .culture;
    assert!(ordinary > 0.0);
    game.players[minor].civ = "Vilnius".to_string();
    assert_close(
        game.district_yields(crate::name!("theater_square"), theater)
            .culture,
        ordinary * 2.0,
    );
}

#[test]
fn zanzibar_and_kandy_supply_luxuries_relics_and_relic_faith() {
    let (mut game, cities) = game_with_capitals(1, 89_005);
    let city = cities[0];
    let minor = add_city_state(&mut game, "Zanzibar");
    let luxuries_without = game.empire_luxuries(0);
    let amenities_without = game.city_amenity_surplus(&game.cities[&city]);
    make_suzerain(&mut game, 0, minor);
    assert_eq!(game.empire_luxuries(0), luxuries_without + 2);
    assert_eq!(
        game.city_amenity_surplus(&game.cities[&city]),
        amenities_without + 2
    );

    game.players[minor].civ = "Yerevan".to_string();
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("temple"));
    game.players[0]
        .counters
        .insert("great_work:relic".to_string(), 1);
    let faith_without = game.city_yields(city).faith;
    game.players[minor].civ = "Kandy".to_string();
    let multiplier = game.amenity_yield_mult(&game.cities[&city]);
    assert_close(
        game.city_yields(city).faith,
        faith_without + 2.0 * multiplier,
    );

    let natural_wonder = game
        .map
        .tiles
        .iter()
        .find_map(|(position, tile)| {
            tile.feature.as_ref().and_then(|feature| {
                game.rules.features[feature]
                    .natural_wonder
                    .then_some((*position, *feature))
            })
        })
        .unwrap();
    game.players[0].explored.remove(&natural_wonder.0);
    game.players[0]
        .discovered_natural_wonders
        .remove(&natural_wonder.1);
    let relics_before = game.players[0].counters["great_work:relic"];
    game.reveal(0, natural_wonder.0, 0);
    assert_eq!(
        game.players[0].counters["great_work:relic"],
        relics_before + 1
    );
}

#[test]
fn zanzibar_luxuries_each_supply_six_cities() {
    let (mut game, _) = game_with_capitals(1, 89_012);
    while game.player_city_ids(0).len() < 7 {
        let existing: Vec<Pos> = game
            .player_city_ids(0)
            .into_iter()
            .map(|city| game.cities[&city].pos)
            .collect();
        let position = game
            .map
            .tiles
            .iter()
            .find_map(|(position, tile)| {
                (tile.owner_city.is_none()
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && existing
                        .iter()
                        .all(|city| game.wdist(*city, *position) >= 3))
                .then_some(*position)
            })
            .expect("map has room for the Zanzibar allocation test");
        game.found_city_for(0, position, None);
    }
    let before: i64 = game.luxury_amenity_allocations(0).values().sum();
    let zanzibar = add_city_state(&mut game, "Zanzibar");
    make_suzerain(&mut game, 0, zanzibar);
    let after: i64 = game.luxury_amenity_allocations(0).values().sum();
    assert_eq!(
        after - before,
        12,
        "Cinnamon and Cloves provide six Amenities each"
    );
}

#[test]
fn suzerains_improve_repair_and_accumulate_city_state_resources() {
    let (mut game, _) = game_with_capitals(1, 89_013);
    let minor = add_city_state(&mut game, "Geneva");
    let position = game
        .map
        .tiles
        .iter()
        .find_map(|(position, tile)| {
            (tile.owner_city.is_none()
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile))
            .then_some(*position)
        })
        .unwrap();
    let city = game.found_city_for(minor, position, None);
    let resource = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|tile| *tile != position)
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&resource).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = Some(crate::name!("iron"));
        tile.improvement = None;
        tile.pillaged = false;
    }
    game.players[0]
        .techs
        .extend(["mining", "bronze_working"].into_iter().map(Name::new));
    let builder = game.spawn_test_unit("builder", 0, resource);

    assert!(!game
        .valid_improvements(0, resource)
        .contains(&crate::name!("mine")));
    make_suzerain(&mut game, 0, minor);
    assert!(game
        .valid_improvements(0, resource)
        .contains(&crate::name!("mine")));
    game.apply(
        0,
        &Action::Improve {
            unit: builder,
            improvement: crate::name!("mine"),
        },
    )
    .unwrap();
    assert_close(game.strategic_resource_rate(0, "iron"), 2.0);

    game.map.tiles.get_mut(&resource).unwrap().pillaged = true;
    game.units.get_mut(&builder).unwrap().moves_left = 2.0;
    let repair = Action::RepairImprovement { unit: builder };
    assert!(game.legal_actions(0).contains(&repair));
    game.apply(0, &repair).unwrap();
    assert!(!game.map.tiles[&resource].pillaged);
}

#[test]
fn every_shipped_city_state_has_its_shipped_type() {
    // Civilization VI's own forty-eight city-states and the
    // `MinorCivBonuses` type each one pays its 1/3/6 Envoy thresholds in,
    // read out of the shipped `Leaders_XP2.MinorCivBonusType` rows. Eight per
    // type, which is the balance the real roster ships.
    let expected = [
        ("Kabul", "militaristic"),
        ("Geneva", "scientific"),
        ("Hattusa", "scientific"),
        ("Mohenjo-Daro", "cultural"),
        ("Yerevan", "religious"),
        ("Zanzibar", "trade"),
        ("Auckland", "industrial"),
        ("Valletta", "militaristic"),
        ("Vilnius", "cultural"),
        ("Kandy", "religious"),
        ("Jerusalem", "religious"),
        ("Brussels", "industrial"),
        ("Preslav", "militaristic"),
        ("Antananarivo", "cultural"),
        ("Mogadishu", "trade"),
        ("Cahokia", "trade"),
        ("Akkad", "militaristic"),
        ("Anshan", "scientific"),
        ("Armagh", "religious"),
        ("Ayutthaya", "cultural"),
        ("Bandar Brunei", "trade"),
        ("Bologna", "scientific"),
        ("Buenos Aires", "industrial"),
        ("Caguana", "cultural"),
        ("Cardiff", "industrial"),
        ("Chinguetti", "religious"),
        ("Fez", "scientific"),
        ("Granada", "militaristic"),
        ("Hong Kong", "industrial"),
        ("Hunza", "trade"),
        ("Johannesburg", "industrial"),
        ("Kumasi", "cultural"),
        ("La Venta", "religious"),
        ("Lahore", "militaristic"),
        ("Mexico City", "industrial"),
        ("Mitla", "scientific"),
        ("Muscat", "trade"),
        ("Nalanda", "scientific"),
        ("Nan Madol", "cultural"),
        ("Nazca", "religious"),
        ("Ngazargamu", "militaristic"),
        ("Rapa Nui", "cultural"),
        ("Samarkand", "trade"),
        ("Singapore", "industrial"),
        ("Taruga", "scientific"),
        ("Vatican City", "religious"),
        ("Venice", "trade"),
        ("Wolin", "militaristic"),
    ];
    let game = Game::new_full(2, 24, 16, 77_001, 300, 0, false);
    for (city_state, kind) in expected {
        assert_eq!(game.cs_type(city_state), kind, "{city_state}");
    }
    for kind in [
        "scientific",
        "cultural",
        "religious",
        "militaristic",
        "industrial",
        "trade",
    ] {
        assert_eq!(
            expected.iter().filter(|(_, have)| *have == kind).count(),
            8,
            "{kind}"
        );
    }
    assert_eq!(expected.len(), 48);
}

#[test]
fn the_roster_seats_the_shipped_forty_eight_before_any_other_name() {
    // Seating order is what decides which city-states an ordinary game meets.
    // The shipped forty-eight come first, so only the largest maps ever reach
    // the extra identities.
    let rules = crate::rules::Rules::shipped();
    let roster = &rules.city_states.roster;
    let shipped = roster.iter().filter(|seat| seat.shipped).count();
    assert_eq!(shipped, 48);
    assert!(
        roster[..48].iter().all(|seat| seat.shipped),
        "a name the game never seats was placed inside the first forty-eight"
    );
    assert!(
        roster[48..].iter().all(|seat| !seat.shipped),
        "a shipped city-state was pushed past the first forty-eight"
    );
    // Identity is the name, and two seats sharing one would share a Suzerain
    // bonus.
    let mut names: Vec<&str> = roster.iter().map(|seat| seat.name.as_str()).collect();
    names.sort_unstable();
    let unique = names.len();
    names.dedup();
    assert_eq!(names.len(), unique, "two city-state seats share a name");
    // Every roster name must be a known identity, and every identity seatable.
    for seat in roster {
        assert!(
            crate::game::CITY_STATE_NAMES.contains(&seat.name.as_str()),
            "{} is not a city-state identity",
            seat.name
        );
    }
    assert_eq!(roster.len(), crate::game::CITY_STATE_NAMES.len());
}

#[test]
fn no_city_state_seat_claims_a_suzerain_bonus_the_engine_does_not_have() {
    // `implemented` is what `cs_bonus` gates on. A seat that declares a bonus
    // it does not have would otherwise read as a working bonus that silently
    // does nothing.
    let rules = crate::rules::Rules::shipped();
    let game = Game::new_full(2, 24, 16, 77_002, 300, 0, false);
    for seat in &rules.city_states.roster {
        if seat.implemented {
            assert!(
                seat.bonus.is_some(),
                "{} is marked implemented with no bonus key",
                seat.name
            );
            assert_eq!(
                game.cs_bonus(&seat.name),
                seat.bonus.as_deref(),
                "{}",
                seat.name
            );
        } else {
            assert_eq!(game.cs_bonus(&seat.name), None, "{}", seat.name);
        }
    }
}

/// Valletta sells the city centre for Faith. It does not sell the walls.
///
/// This test used to assert `walls` cost 80 Faith and then buy them. The
/// shipped ruleset disagrees: no base or expansion file gives any wall tier a
/// `PurchaseYield`, so Civilization VI offers them for no currency, and
/// Valletta's `ENABLE_BUILDING_FAITH_PURCHASE` changes the currency of a
/// purchasable building rather than making an unpurchasable one buyable. The
/// live seat issued 99 such purchases across the two Valletta runs and the host
/// refused every one.
#[test]
fn valletta_purchases_city_center_and_encampment_buildings_but_never_the_walls() {
    let (mut game, cities) = game_with_capitals(2, 89_006);
    let city = cities[0];
    let valletta = add_city_state(&mut game, "Valletta");
    make_suzerain(&mut game, 1, valletta);
    install_alliance(&mut game, 0, 1, "economic", 3);
    game.players[0].techs.extend(
        ["pottery", "masonry", "bronze_working"]
            .into_iter()
            .map(Name::new),
    );
    game.players[0].faith = 1_000.0;

    assert_eq!(
        game.building_faith_purchase_cost(0, city, "granary"),
        Some(130.0)
    );
    assert_eq!(game.building_faith_purchase_cost(0, city, "library"), None);
    // A city defence is Production-only in both currencies, which is what
    // `building_gold_purchase_cost` has always said and what this path used to
    // contradict.
    for defence in ["walls", "medieval_walls", "renaissance_walls", "tsikhe"] {
        assert_eq!(
            game.building_faith_purchase_cost(0, city, defence),
            None,
            "{defence} has no PurchaseYield in the shipped ruleset, so no \
             currency buys it -- not even a Valletta suzerain's Faith"
        );
        assert_eq!(
            game.building_gold_purchase_cost(0, city, defence),
            None,
            "{defence} must stay Production-only for Gold too"
        );
    }
    let purchase = Action::BuyBuilding {
        city,
        building: crate::name!("walls"),
        currency: "faith".to_string(),
    };
    assert!(
        !game.legal_actions(0).contains(&purchase),
        "buying walls with Faith must not be offered as a legal action"
    );
    assert!(!game.cities[&city]
        .buildings
        .contains(&crate::name!("walls")));

    install_district(&mut game, city, "encampment");
    assert_eq!(
        game.building_faith_purchase_cost(0, city, "barracks"),
        Some(180.0)
    );
    game.players[0].alliances.clear();
    game.players[1].alliances.clear();
    assert_eq!(game.building_faith_purchase_cost(0, city, "granary"), None);
}

#[test]
fn final_patch_envoy_thresholds_follow_active_building_tiers() {
    let (mut game, cities) = game_with_capitals(1, 89_007);
    let city = cities[0];
    let scientific = add_city_state(&mut game, "Hattusa");
    install_district(&mut game, city, "campus");
    install_district(&mut game, city, "diplomatic_quarter");
    game.cities.get_mut(&city).unwrap().buildings.extend(
        [
            "library",
            "university",
            "research_lab",
            "consulate",
            "chancery",
        ]
        .into_iter()
        .map(Name::new),
    );

    game.players[0].envoys = vec![(scientific, 1)];
    assert_close(game.envoy_yields(0, &game.cities[&city]).science, 2.0);
    game.players[0].envoys = vec![(scientific, 3)];
    assert_close(game.envoy_yields(0, &game.cities[&city]).science, 6.0);
    game.players[0].envoys = vec![(scientific, 6)];
    assert_close(game.envoy_yields(0, &game.cities[&city]).science, 12.0);

    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("library"));
    assert_close(game.envoy_yields(0, &game.cities[&city]).science, 11.0);
}

#[test]
fn trade_envoys_double_each_independent_commercial_and_harbor_tier() {
    let (mut game, cities) = game_with_capitals(1, 89_008);
    let city = cities[0];
    let trade = add_city_state(&mut game, "Zanzibar");
    install_district(&mut game, city, "commercial_hub");
    install_district(&mut game, city, "harbor");
    install_district(&mut game, city, "diplomatic_quarter");
    game.cities.get_mut(&city).unwrap().buildings.extend(
        [
            "market",
            "lighthouse",
            "bank",
            "shipyard",
            "stock_exchange",
            "seaport",
            "consulate",
            "chancery",
        ]
        .into_iter()
        .map(Name::new),
    );

    game.players[0].envoys = vec![(trade, 1)];
    assert_close(game.envoy_yields(0, &game.cities[&city]).gold, 6.0);
    game.players[0].envoys = vec![(trade, 3)];
    assert_close(game.envoy_yields(0, &game.cities[&city]).gold, 18.0);
    game.players[0].envoys = vec![(trade, 6)];
    assert_close(game.envoy_yields(0, &game.cities[&city]).gold, 36.0);
}

#[test]
fn production_envoys_obey_unit_and_infrastructure_queues() {
    let (mut game, cities) = game_with_capitals(1, 89_009);
    let city = cities[0];
    let state = add_city_state(&mut game, "Carthage");
    game.players[0].envoys = vec![(state, 6)];
    install_district(&mut game, city, "encampment");
    install_district(&mut game, city, "industrial_zone");
    install_district(&mut game, city, "diplomatic_quarter");
    game.cities.get_mut(&city).unwrap().buildings.extend(
        [
            "barracks",
            "armory",
            "military_academy",
            "consulate",
            "chancery",
        ]
        .into_iter()
        .map(Name::new),
    );
    game.cities.get_mut(&city).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("warrior"),
    }];
    assert_close(game.envoy_yields(0, &game.cities[&city]).production, 12.0);
    game.cities.get_mut(&city).unwrap().queue = vec![Item::Building {
        building: crate::name!("granary"),
    }];
    assert_close(game.envoy_yields(0, &game.cities[&city]).production, 0.0);

    game.players[state].civ = "Auckland".to_string();
    game.cities.get_mut(&city).unwrap().buildings = [
        "workshop",
        "factory",
        "coal_power_plant",
        "consulate",
        "chancery",
    ]
    .into_iter()
    .map(Name::new)
    .collect();
    assert_close(game.envoy_yields(0, &game.cities[&city]).production, 12.0);
    game.cities.get_mut(&city).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("warrior"),
    }];
    assert_close(game.envoy_yields(0, &game.cities[&city]).production, 0.0);
}

#[test]
fn kilwa_scales_total_type_yields_and_matching_production_categories() {
    let (mut game, cities) = game_with_capitals(1, 89_010);
    let host = cities[0];
    let second_position = game
        .map
        .tiles
        .iter()
        .find_map(|(position, tile)| {
            (tile.owner_city.is_none()
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.wdist(game.cities[&host].pos, *position) >= 4)
                .then_some(*position)
        })
        .unwrap();
    let second = game.found_city_for(0, second_position, Some("Kilwa Reach".to_string()));
    let first_state = add_city_state(&mut game, "Hattusa");
    let second_state = add_city_state(&mut game, "Stockholm");
    game.players[0].envoys = vec![(first_state, 3), (second_state, 3)];
    let host_position = game.cities[&host].pos;
    game.cities
        .get_mut(&host)
        .unwrap()
        .wonders
        .insert(crate::name!("kilwa_kisiwani"), host_position);

    let mut without_kilwa = game.clone();
    without_kilwa
        .cities
        .get_mut(&host)
        .unwrap()
        .wonders
        .remove(&Name::new("kilwa_kisiwani"));
    // Percentage modifiers SUM (Firaxis's `ADJUST_CITY_YIELD_MODIFIER`), so
    // Kilwa's 30 / 15 join whatever band and handicap the city already
    // carries rather than multiplying on top of them.
    let other_pct = |game: &Game, city: u32| {
        (game.amenity_yield_mult(&game.cities[&city]) - 1.0) * 100.0
            + (Game::loyalty_yield_mult(game.cities[&city].loyalty) - 1.0) * 100.0
            + game.handicap_yield_pct(0).science
    };
    for (city, kilwa_pct) in [(host, 30.0), (second, 15.0)] {
        let other = other_pct(&without_kilwa, city);
        let base = without_kilwa.city_yields(city).science / (1.0 + other / 100.0);
        assert_close(
            game.city_yields(city).science,
            base * (1.0 + (other + kilwa_pct) / 100.0),
        );
    }

    game.players[first_state].civ = "Kabul".to_string();
    game.players[second_state].civ = "Carthage".to_string();
    let unit = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let mut no_production_kilwa = game.clone();
    no_production_kilwa
        .cities
        .get_mut(&host)
        .unwrap()
        .wonders
        .remove(&Name::new("kilwa_kisiwani"));
    assert_close(
        game.item_prod_mult(0, host, Some(&unit)),
        no_production_kilwa.item_prod_mult(0, host, Some(&unit)) + 0.30,
    );
    assert_close(
        game.item_prod_mult(0, second, Some(&unit)),
        no_production_kilwa.item_prod_mult(0, second, Some(&unit)) + 0.15,
    );
}

#[test]
fn a_city_state_at_war_with_the_seat_suspends_its_envoy_bonuses() {
    // Ostia's "+2 from Consulate" Culture (Caguana, three Envoys) went to
    // nothing the turn Caguana was brought into a war against us and came back
    // with the peace (run civvis-20260816T223457Z t90 / t98); the capital's
    // point from the same city-state moved with it.
    let (mut game, cities) = game_with_capitals(1, 89_012);
    let capital = cities[0];
    let kumasi = add_city_state(&mut game, "Kumasi");
    make_suzerain(&mut game, 0, kumasi);
    let plain = game.envoy_yields(0, &game.cities[&capital]);
    assert!(
        plain.culture > 0.0,
        "a cultural city-state at three Envoys pays the capital"
    );
    game.at_war.insert(pair(0, kumasi));
    let at_war = game.envoy_yields(0, &game.cities[&capital]);
    assert_eq!(at_war, Yields::default(), "nothing while at war");
    game.at_war.remove(&pair(0, kumasi));
    assert_eq!(
        game.envoy_yields(0, &game.cities[&capital]),
        plain,
        "and all of it back at peace"
    );
}

#[test]
fn leading_sent_envoys_expand_borders_and_strengthen_the_city_state() {
    let (mut game, major_cities) = game_with_capitals(2, 89_011);
    let minor = add_city_state(&mut game, "Geneva");
    let minor_position = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| {
            tile.owner_city.is_none() && game.rules.is_passable(tile) && !game.rules.is_water(tile)
        })
        .map(|(position, _)| *position)
        .max_by_key(|position| {
            major_cities
                .iter()
                .map(|city| game.wdist(game.cities[city].pos, *position))
                .sum::<i32>()
        })
        .unwrap();
    let minor_city = game.found_city_for(minor, minor_position, None);
    let initial_tiles = game.cities[&minor_city].owned_tiles.len();
    game.record_contact(0, minor);
    game.record_contact(1, minor);
    // The border-growth scenario starts from deliberate sends only.
    game.players[0].envoys.clear();
    game.players[1].envoys.clear();

    game.players[0].envoys_free = 2;
    game.do_send_envoy(0, minor).unwrap();
    assert_eq!(game.cities[&minor_city].owned_tiles.len(), initial_tiles);
    game.do_send_envoy(0, minor).unwrap();
    assert_eq!(
        game.cities[&minor_city].owned_tiles.len(),
        initial_tiles + 1
    );

    game.players[1].envoys_free = 3;
    game.do_send_envoy(1, minor).unwrap();
    game.do_send_envoy(1, minor).unwrap();
    assert_eq!(
        game.cities[&minor_city].owned_tiles.len(),
        initial_tiles + 1,
        "a first Envoy and a later tie do not expand borders"
    );
    game.do_send_envoy(1, minor).unwrap();
    assert_eq!(
        game.cities[&minor_city].owned_tiles.len(),
        initial_tiles + 2
    );
    assert_eq!(game.suzerain_of(minor), Some(1));

    install_district(&mut game, minor_city, "encampment");
    {
        let city = game.cities.get_mut(&minor_city).unwrap();
        city.encampment_hp = 100;
        city.encampment_wall_hp = 100;
    }
    let warrior = game.spawn_unit("warrior", minor, minor_position);
    let mut without_envoys = game.clone();
    without_envoys.players[1].envoys.clear();
    assert_close(
        game.unit_strength(&game.units[&warrior], true)
            - without_envoys.unit_strength(&without_envoys.units[&warrior], true),
        3.0,
    );
    assert_close(
        game.city_strength(minor_city) - without_envoys.city_strength(minor_city),
        3.0,
    );
    assert_close(
        game.encampment_strength(minor_city) - without_envoys.encampment_strength(minor_city),
        3.0,
    );

    game.players[1].gold = 1_000.0;
    game.do_levy_military(1, minor).unwrap();
    assert_eq!(game.units[&warrior].owner, 1);
    assert_eq!(game.units[&warrior].levied_from, Some(minor));
    let mut levied_without_envoys = game.clone();
    levied_without_envoys.players[1].envoys.clear();
    assert_close(
        game.unit_strength(&game.units[&warrior], true)
            - levied_without_envoys.unit_strength(&levied_without_envoys.units[&warrior], true),
        3.0,
    );
}

#[test]
fn brussels_hong_kong_and_muscat_pay_wonders_projects_and_amenities() {
    let (mut game, cities) = game_with_capitals(2, 89_010);
    let city = cities[0];
    let brussels = add_city_state(&mut game, "Brussels");

    // +15% Production towards wonders.
    let wonder = Item::Wonder {
        wonder: crate::name!("pyramids"),
        pos: game.cities[&city].pos,
    };
    let before = game.item_prod_mult(0, city, Some(&wonder));
    make_suzerain(&mut game, 0, brussels);
    assert_close(game.item_prod_mult(0, city, Some(&wonder)), before + 0.15);

    // +20% Production towards city projects, and not towards wonders.
    game.players[brussels].civ = "Hong Kong".to_string();
    let project = Item::Project {
        project: crate::name!("campus_research_grants"),
    };
    let base = {
        game.players[0].envoys.clear();
        game.item_prod_mult(0, city, Some(&project))
    };
    make_suzerain(&mut game, 0, brussels);
    assert_close(game.item_prod_mult(0, city, Some(&project)), base + 0.20);
    assert_close(game.item_prod_mult(0, city, Some(&wonder)), before);

    // +1 Amenity in cities with a Commercial Hub, and nothing without one.
    game.players[brussels].civ = "Muscat".to_string();
    let without = game.city_local_amenities(&game.cities[&city]);
    install_district(&mut game, city, "commercial_hub");
    let with_hub = game.city_local_amenities(&game.cities[&city]);
    game.players[0].envoys.clear();
    let unsuzerained = game.city_local_amenities(&game.cities[&city]);
    assert_eq!(
        with_hub,
        unsuzerained + 1,
        "Muscat paid nothing for the Commercial Hub"
    );
    assert!(with_hub > without || unsuzerained > without);
}

#[test]
fn preslav_arms_only_cavalry_and_only_on_the_high_ground() {
    let (mut game, cities) = game_with_capitals(2, 89_011);
    let preslav = add_city_state(&mut game, "Preslav");
    let centre = game.cities[&cities[0]].pos;
    let hill = game
        .nbrs(centre)
        .into_iter()
        .find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| !game.rules.is_water(tile))
        })
        .unwrap();
    game.map.tiles.get_mut(&hill).unwrap().hills = true;
    game.map.tiles.get_mut(&centre).unwrap().hills = false;

    let horseman = game.spawn_unit("horseman", 0, hill);
    let warrior = game.spawn_unit("warrior", 0, centre);
    let cavalry_flat = {
        let unit = game.units[&horseman].clone();
        game.unit_unembarked_strength(&unit)
    };
    let footman = {
        let unit = game.units[&warrior].clone();
        game.unit_unembarked_strength(&unit)
    };
    make_suzerain(&mut game, 0, preslav);
    let cavalry_hill = {
        let unit = game.units[&horseman].clone();
        game.unit_unembarked_strength(&unit)
    };
    assert_close(cavalry_hill, cavalry_flat + 5.0);
    // A melee unit gets nothing, hill or not.
    let footman_after = {
        let unit = game.units[&warrior].clone();
        game.unit_unembarked_strength(&unit)
    };
    assert_close(footman_after, footman);
    // And the cavalry loses it the moment it steps off the hill.
    game.map.tiles.get_mut(&hill).unwrap().hills = false;
    let cavalry_off = {
        let unit = game.units[&horseman].clone();
        game.unit_unembarked_strength(&unit)
    };
    assert_close(cavalry_off, cavalry_flat);
}

#[test]
fn mitla_grows_campus_cities_and_taruga_counts_resource_kinds_not_tiles() {
    let (mut game, cities) = game_with_capitals(2, crate::rng::fixture_seed("MITLA", 89_029));
    let city = cities[0];
    let minor = add_city_state(&mut game, "Mitla");
    make_suzerain(&mut game, 0, minor);

    // Mitla pays nothing until the city actually has a Campus.
    assert!(!game.city_has_active_district_family(&game.cities[&city], crate::name!("campus")));
    install_district(&mut game, city, "campus");
    assert!(game.city_has_active_district_family(&game.cities[&city], crate::name!("campus")));

    // Taruga scales on distinct improved Strategic resources.
    game.players[minor].civ = "Taruga".to_string();
    let owned = game.cities[&city].owned_tiles.to_vec();
    let plain: Vec<Pos> = owned
        .into_iter()
        .filter(|pos| *pos != game.cities[&city].pos)
        .filter(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| !game.rules.is_water(tile) && tile.district.is_none())
        })
        .take(3)
        .collect();
    assert!(
        plain.len() >= 3,
        "the capital needs three workable land tiles"
    );
    // Level the city's whole workable ring, not just the three tiles about to
    // carry the resources. Taruga's bonus is a *percentage* of the city's
    // Science, and `city_yields` re-picks which tiles the Citizens work every
    // time it is called: improving one tile can move a Citizen off a lake or a
    // rainforest and swamp the 5% step this case is measuring. With every ring
    // tile paying the same, an assignment that reshuffles cannot move Science
    // and only the percentage is left.
    let ring: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|pos| *pos != game.cities[&city].pos)
        .collect();
    for pos in &ring {
        let Some(tile) = game.map.tiles.get_mut(pos) else {
            continue;
        };
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
    }
    // And take its Citizens off the ring entirely. This case is about the
    // *percentage*: Taruga pays 5% per distinct improved Strategic the city
    // owns, whether or not anybody is standing on it. A city with Citizens to
    // place re-picks where they stand every time `city_yields` runs, and a
    // fresh Mine is enough to move one — off the Campus, onto the ore — which
    // changes the Science being multiplied and swamps the step.
    game.cities.get_mut(&city).unwrap().pop = 0;
    let base = game.city_yields(city).science;

    // Two mined Iron is one kind, so one 5% step.
    for pos in &plain[..2] {
        let tile = game.map.tiles.get_mut(pos).unwrap();
        tile.resource = Some(crate::name!("iron"));
        tile.improvement = Some(Name::new(&game.rules.resources["iron"].improvement));
    }
    let one_kind = game.city_yields(city).science;

    // A second kind is a second step.
    let third = plain[2];
    let tile = game.map.tiles.get_mut(&third).unwrap();
    tile.resource = Some(crate::name!("niter"));
    tile.improvement = Some(Name::new(&game.rules.resources["niter"].improvement));
    let two_kinds = game.city_yields(city).science;
    assert!(
        two_kinds > one_kind && one_kind > base,
        "Taruga did not scale: base {base}, one kind {one_kind}, two kinds {two_kinds}"
    );
    // The second step is the same size as the first: 5% of the pre-bonus total.
    assert_close(two_kinds - one_kind, one_kind - base);
}

/// Nan Madol's `MODIFIER_PLAYER_DISTRICTS_ADJUST_YIELD_CHANGE` reaches every
/// district plot on or beside Coast — the City Center and each wonder's plot
/// included. The host's culture ledger on live run civvis-20260816T155856Z read
/// "+2 from City Center" in Rome and "+2 from Wonder" in Mediolanum beside the
/// specialty districts the model already paid.
#[test]
fn nan_madol_pays_the_city_center_and_wonder_plots_too() {
    let (mut game, cities) = game_with_capitals(2, 77_101);
    let city = cities[0];
    let center = game.cities[&city].pos;
    // A landlocked capital first: suzerainty alone brings the envoy-tier
    // Culture, and nothing from Nan Madol's own bonus.
    for pos in game.cities[&city].owned_tiles.clone() {
        if game.rules.is_water(&game.map.tiles[&pos]) {
            let tile = game.map.tiles.get_mut(&pos).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.resource = None;
            tile.improvement = None;
        }
    }
    let minor = add_city_state(&mut game, "Nan Madol");
    make_suzerain(&mut game, 0, minor);
    let before = game.city_yields(city).culture;
    // Now put the sea beside the centre and beside one owned plot that will
    // hold a wonder.
    let coast_pos = game.nbrs(center)[0];
    let wonder_pos = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| {
            *pos != center
                && *pos != coast_pos
                && game.map.tiles[pos].district.is_none()
                && game.nbrs(*pos).contains(&coast_pos)
        })
        .expect("a land plot beside the new coast");
    let sea = game.map.tiles.get_mut(&coast_pos).unwrap();
    sea.terrain = crate::name!("coast");
    sea.feature = None;
    sea.resource = None;
    sea.improvement = None;
    let with_center = game.city_yields(city).culture;
    assert!(
        (with_center - before - 2.0).abs() < 1e-9,
        "the coastal City Center is a district plot: {before} -> {with_center}"
    );
    // A wonder with no Culture and no Amenity of its own, so the only Culture
    // that moves is Nan Madol's.
    game.cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .insert(crate::name!("great_library"), wonder_pos);
    game.map.tiles.get_mut(&wonder_pos).unwrap().wonder = Some(crate::name!("great_library"));
    let with_wonder = game.city_yields(city).culture;
    assert!(
        (with_wonder - with_center - 2.0).abs() < 1e-9,
        "the coastal wonder plot pays Nan Madol's 2: {with_center} -> {with_wonder}"
    );
}

/// Merchant Confederation's Gold per Envoy and Raj's yields per tributary are
/// player-level income (`..._PER_USED_INFLUENCE_TOKEN`, `..._PER_TRIBUTARY`),
/// banked with the founder-belief income and reported beside the city sum —
/// never inside the Palace city, whose host ledger carried none of the 27
/// Envoys' Gold on live run civvis-20260816T155856Z.
#[test]
fn envoy_and_tributary_policy_income_is_the_players_not_the_capitals() {
    let (mut game, cities) = game_with_capitals(2, 77_102);
    let capital = cities[0];
    let minor = add_city_state(&mut game, "Nan Madol");
    let second = add_city_state(&mut game, "Muscat");
    game.players[0].envoys.push((minor, 5));
    game.players[0].envoys.push((second, 2));
    let quiet_city = game.city_yields(capital);
    let quiet_player = game.player_policy_yields(0);
    game.players[0]
        .policies
        .insert(crate::name!("merchant_confederation"));
    assert!((game.player_policy_yields(0).gold - quiet_player.gold - 7.0).abs() < 1e-9);
    assert!(
        (game.city_yields(capital).gold - quiet_city.gold).abs() < 1e-9,
        "not in the capital"
    );
    make_suzerain(&mut game, 0, minor);
    make_suzerain(&mut game, 0, second);
    let before_raj = game.player_policy_yields(0);
    game.players[0].policies.insert(crate::name!("raj"));
    let with_raj = game.player_policy_yields(0);
    assert!(
        (with_raj.science - before_raj.science - 4.0).abs() < 1e-9,
        "two tributaries at 2 each"
    );
    assert!((with_raj.gold - before_raj.gold - 4.0).abs() < 1e-9);
    // And the per-turn extras every reader adds carry it.
    assert!((game.player_yield_extras(0).science - with_raj.science).abs() < 1e-9);
}
