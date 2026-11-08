//! Tests of `advanced/production_values.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

#[test]
fn support_search_respects_ram_and_tower_wall_eras() {
    let mut game = Game::new_full(2, 24, 16, 71_008, 160, 0, false);
    for pid in 0..2 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    let home = game.player_city_ids(0)[0];
    let target = game.player_city_ids(1)[0];
    let position = game.cities[&home].pos;
    game.spawn_test_unit("warrior", 0, position);
    game.spawn_test_unit("warrior", 0, position);
    game.spawn_test_unit("warrior", 0, position);
    game.cities.get_mut(&target).unwrap().buildings =
        vec![crate::name!("walls"), crate::name!("medieval_walls")];
    game.cities.get_mut(&target).unwrap().wall_hp = 200;
    game.at_war.insert((0, 1));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let counts = ai.counts(&game, 0);

    assert!(ai.support_unit_value(&game, 0, home, "battering_ram", &plan, &counts) < -9_000.0);
    assert!(ai.support_unit_value(&game, 0, home, "siege_tower", &plan, &counts) > 0.0);
}

#[test]
fn foreign_city_assault_reserves_breach_support_for_a_small_infantry_group() {
    let mut game = Game::new_full(2, 24, 16, 71_012, 160, 0, false);
    for pid in 0..2 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    game.current = 0;
    let home = game.player_city_ids(0)[0];
    let target = game.player_city_ids(1)[0];
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    let position = game.cities[&home].pos;
    game.spawn_test_unit("warrior", 0, position);
    game.spawn_test_unit("warrior", 0, position);
    game.players[0].techs.insert(crate::name!("masonry"));
    game.cities.get_mut(&home).unwrap().queue.clear();
    game.cities.get_mut(&target).unwrap().buildings = vec![crate::name!("walls")];
    game.cities.get_mut(&target).unwrap().wall_hp = 100;
    game.at_war.insert((0, 1));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.book_pos = 4;

    let counts = ai.counts(&game, 0);
    assert!(
        ai.support_unit_value(&game, 0, home, "battering_ram", &plan, &counts) > 0.0,
        "two eligible infantry on a walled city objective need their breach element"
    );
    assert!(ai.reserve_foreign_city_assault_support(&mut game, 0, &plan));
    assert!(matches!(
        game.cities[&home].queue.first(),
        Some(Item::Unit { unit }) if unit == "battering_ram"
    ));

    game.cities.get_mut(&home).unwrap().queue.clear();
    game.spawn_test_unit("battering_ram", 0, position);
    assert!(
        !ai.reserve_foreign_city_assault_support(&mut game, 0, &plan),
        "an existing breach element must not open a duplicate support queue"
    );
    game.at_war.clear();
    assert!(
        !ai.reserve_foreign_city_assault_support(&mut game, 0, &plan),
        "a foreign city is not an assault objective after peace"
    );
}

/// Rule 2 of `strategic_wonder_value`: a rate effect is priced as the flat
/// yield it is equivalent to, so the same wonder is a different build in a
/// different city. The arm as written could not tell them apart — it read
/// only the data sheet, which is identical everywhere.
#[test]
fn a_science_multiplier_is_worth_what_the_city_it_multiplies_makes() {
    let (mut game, city) = strategic_wonder_fixture(6_403, "oxford_university");
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    let spec = game.rules.wonders["oxford_university"].clone();
    let plan = wonder_plan(GrandStrategy::Science, game.turn);

    let quiet = ai.strategic_wonder_value(&game, 0, city, &spec, GrandStrategy::Science);
    install_ai_test_district(&mut game, city, "campus");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    game.cities.get_mut(&city).unwrap().pop += 6;
    let research = ai.strategic_wonder_value(&game, 0, city, &spec, GrandStrategy::Science);
    assert!(
        research > quiet,
        "Oxford is worth more where there is more science to multiply: \
         {research} vs {quiet}"
    );
    let _ = plan;
}

#[test]
fn native_campus_finish_library_one_turn_before_slow_defender() {
    let (mut g, cid, ai, committed, _) = native_campus_finish_fixture("library");
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
    assert_eq!(
        g.cities[&cid].queue.first(),
        Some(&committed),
        "finish the one-turn Library behind undamaged walls"
    );
    assert_eq!(
        g.cities[&cid].production, 33.0,
        "keep its banked progress active"
    );
}

#[test]
fn native_campus_finish_university_one_turn_before_slow_defender() {
    let (mut g, cid, ai, committed, _) = native_campus_finish_fixture("university");
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
    assert_eq!(g.cities[&cid].queue.first(), Some(&committed));
}

#[test]
fn native_campus_finish_city_damage_keeps_emergency_defense() {
    let (mut g, cid, ai, committed, _) = native_campus_finish_fixture("library");
    g.cities.get_mut(&cid).unwrap().hp -= 10;
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
    assert_ne!(g.cities[&cid].queue.first(), Some(&committed));
}

#[test]
fn native_campus_finish_wall_damage_keeps_emergency_defense() {
    let (mut g, cid, ai, committed, _) = native_campus_finish_fixture("library");
    g.cities.get_mut(&cid).unwrap().wall_hp -= 10;
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
    assert_ne!(g.cities[&cid].queue.first(), Some(&committed));
}

#[test]
fn native_campus_finish_two_turn_building_keeps_emergency_defense() {
    let (mut g, cid, ai, committed, _) = native_campus_finish_fixture("library");
    std::sync::Arc::make_mut(&mut g.host_buildable)
        .get_mut(&cid)
        .unwrap()
        .get_mut(&Game::production_block_key(&committed))
        .unwrap()
        .turns = Some(2.0);
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
    assert_ne!(g.cities[&cid].queue.first(), Some(&committed));
}

#[test]
fn native_campus_finish_unknown_timing_keeps_emergency_defense() {
    for missing_defender in [false, true] {
        let (mut g, cid, ai, committed, defence) = native_campus_finish_fixture("library");
        let unknown = if missing_defender {
            &defence
        } else {
            &committed
        };
        std::sync::Arc::make_mut(&mut g.host_buildable)
            .get_mut(&cid)
            .unwrap()
            .get_mut(&Game::production_block_key(unknown))
            .unwrap()
            .turns = None;
        ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
        assert_ne!(g.cities[&cid].queue.first(), Some(&committed));
    }
}

#[test]
fn native_campus_finish_fast_defender_keeps_emergency_defense() {
    let (mut g, cid, ai, committed, defence) = native_campus_finish_fixture("library");
    std::sync::Arc::make_mut(&mut g.host_buildable)
        .get_mut(&cid)
        .unwrap()
        .get_mut(&Game::production_block_key(&defence))
        .unwrap()
        .turns = Some(1.0);
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
    assert_ne!(g.cities[&cid].queue.first(), Some(&committed));
}

#[test]
fn native_campus_finish_other_victory_keeps_emergency_defense() {
    let (mut g, cid, mut ai, committed, _) = native_campus_finish_fixture("library");
    ai.victory_target = Some(VictoryTarget::Science);
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
    assert_ne!(g.cities[&cid].queue.first(), Some(&committed));
}

#[test]
fn native_campus_finish_unwalled_city_keeps_emergency_defense() {
    let (mut g, cid, ai, committed, _) = native_campus_finish_fixture("library");
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|b| *b != "walls");
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
    assert_ne!(g.cities[&cid].queue.first(), Some(&committed));
}

#[test]
fn native_campus_finish_claim_survives_a_later_civilian_queue_writer() {
    let (mut g, cid, ai, committed, _) = native_campus_finish_fixture("library");
    let claim = ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
    assert_eq!(claim, Some((cid, committed.clone())));
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    std::sync::Arc::make_mut(&mut g.host_buildable)
        .get_mut(&cid)
        .unwrap()
        .insert(
            Game::production_block_key(&builder),
            crate::game::HostMenuEntry {
                cost: Some(41.0),
                turns: Some(2.0),
            },
        );
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: builder,
        },
    )
    .unwrap();
    ai.reapply_confirmed_defense_queue(&mut g, 0, claim.as_ref());
    assert_eq!(g.cities[&cid].queue.first(), Some(&committed));
    assert_eq!(g.cities[&cid].production, 33.0);
}

#[test]
fn native_campus_finish_yields_priority_to_another_damaged_city() {
    let (mut g, cid, ai, committed, _) = native_campus_finish_fixture("library");
    let anchor = g.cities[&cid].pos;
    let threatened = found_nearby_test_city(&mut g, 0, anchor);
    g.cities.get_mut(&threatened).unwrap().hp -= 20;
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    g.apply(
        0,
        &Action::Produce {
            city: threatened,
            item: builder.clone(),
        },
    )
    .unwrap();
    let claim = ai
        .redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid))
        .expect("the damaged city needs a defensive queue");
    assert_eq!(claim.0, threatened);
    assert_ne!(claim.1, builder);
    assert_eq!(g.cities[&cid].queue.first(), Some(&committed));
    assert_eq!(g.cities[&threatened].queue.first(), Some(&claim.1));
}

fn culture_resort_sites_fixture() -> (Game, u32, Pos, Pos) {
    let mut game = Game::new_full(2, 24, 16, 71_079, 250, 0, false);
    let cid = found_test_city(&mut game, 0);
    let positions: Vec<_> = game.map.tiles.keys().copied().take(2).collect();
    let resort = positions[0];
    let farm = positions[1];
    game.map.tiles.get_mut(&resort).unwrap().improvement = Some(crate::name!("seaside_resort"));
    game.map.tiles.get_mut(&resort).unwrap().pillaged = true;
    game.map.tiles.get_mut(&farm).unwrap().improvement = Some(crate::name!("farm"));
    for pos in [resort, farm] {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.flooded = false;
        tile.submerged = false;
    }
    std::sync::Arc::make_mut(&mut game.host_district_sites)
        .entry(cid)
        .or_default()
        .insert(crate::name!("campus"), [resort, farm].into_iter().collect());
    game.players[0].techs.extend([
        crate::name!("flight"),
        crate::name!("radio"),
        crate::name!("computers"),
    ]);
    (game, cid, resort, farm)
}

#[test]
fn culture_keeps_repairable_resort_when_native_quotes_a_farm_for_campus() {
    let (game, cid, resort, farm) = culture_resort_sites_fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    assert!(ai.culture_preserves_resort_site(&game, 0, cid, crate::name!("campus"), resort));
    assert!(!ai.culture_preserves_resort_site(&game, 0, cid, crate::name!("campus"), farm));
    // This is a preference; the original Native legal site is retained.
    assert!(game.district_sites(cid, "campus").contains(&resort));
    assert!(!BasicAi::culture_resort_site_is_displaced(
        &game,
        cid,
        crate::name!("theater_square"),
        resort,
        &[resort, farm],
    ));
}

#[test]
fn culture_resort_preference_requires_all_tourism_techs_and_culture_target() {
    let (mut game, cid, resort, _) = culture_resort_sites_fixture();
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Domination,
        VictoryTarget::Religion,
    ] {
        assert!(
            !AdvancedAi::targeting(target).culture_preserves_resort_site(
                &game,
                0,
                cid,
                crate::name!("campus"),
                resort,
            )
        );
    }
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    game.players[0].techs.remove(&crate::name!("computers"));
    assert!(!ai.culture_preserves_resort_site(&game, 0, cid, crate::name!("campus"), resort));
    game.players[0].techs.insert(crate::name!("computers"));
    game.victory_conditions.culture = false;
    assert!(!ai.culture_preserves_resort_site(&game, 0, cid, crate::name!("campus"), resort));
}

#[test]
fn culture_resort_preference_retains_fallback_without_quoted_safe_alternative() {
    let (mut game, cid, resort, farm) = culture_resort_sites_fixture();
    let displaced = |game: &Game| {
        BasicAi::culture_resort_site_is_displaced(
            game,
            cid,
            crate::name!("campus"),
            resort,
            &[resort, farm],
        )
    };
    game.map.tiles.get_mut(&farm).unwrap().submerged = true;
    assert!(!displaced(&game));
    game.map.tiles.get_mut(&farm).unwrap().submerged = false;
    game.map.tiles.get_mut(&farm).unwrap().improvement = Some(crate::name!("seaside_resort"));
    assert!(!displaced(&game));
    game.map.tiles.get_mut(&farm).unwrap().improvement = Some(crate::name!("farm"));
    std::sync::Arc::make_mut(&mut game.host_district_sites)
        .get_mut(&cid)
        .unwrap()
        .get_mut(&crate::name!("campus"))
        .unwrap()
        .remove(&farm);
    assert!(!displaced(&game));
    std::sync::Arc::make_mut(&mut game.host_district_sites).clear();
    assert!(!displaced(&game));
}
