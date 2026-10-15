use super::*;

fn watched_frontier() -> (Game, u32) {
    let mut game = Game::new_full(2, 48, 24, 4063, 650, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.clear_mirror_cities();
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let capital = game.found_city_for(0, (12, 10), None);
    let frontier = game.found_city_for(0, (24, 10), None);
    game.found_city_for(1, (35, 10), None);
    assert!(game.cities[&capital].is_capital);
    assert!(!game.cities[&frontier].is_capital);
    game.cities.get_mut(&frontier).unwrap().pop = 4;
    game.current = 0;
    game.turn = 30;
    game.record_contact(0, 1);
    game.players[0].techs.insert(crate::name!("mining"));
    Arc::make_mut(&mut game.observed_military_power).insert(0, 100.0);
    Arc::make_mut(&mut game.observed_military_power).insert(1, 250.0);
    let soldier = (31, 10);
    assert_eq!(game.wdist(game.cities[&frontier].pos, soldier), 7);
    assert!(game.wdist(game.cities[&capital].pos, soldier) > 8);
    game.spawn_test_unit("warrior", 1, soldier);
    game.host_observed = Arc::new(game.wdisk((24, 10), 8).into_iter().collect());
    (game, frontier)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_capital_walls_before_the_surprise();
    ai.enable_frontier_walls_before_the_surprise();
    ai
}

fn walls() -> Item {
    Item::Building {
        building: crate::name!("walls"),
    }
}

#[test]
fn a_watched_frontier_unlocks_masonry_before_a_peaceful_rival_declares() {
    let (game, _) = watched_frontier();
    let mut capital_only = AdvancedAi::new();
    capital_only.enable_capital_walls_before_the_surprise();
    capital_only.observe_turn_start_hostiles(&game, 0);
    assert!(capital_only
        .capital_walls_research_threat(&game, 0)
        .is_none());
    assert!(capital_only
        .frontier_walls_research_goal(&game, 0)
        .is_none());

    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    assert_eq!(
        ai.frontier_walls_research_goal(&game, 0),
        Some(crate::name!("masonry")),
        "the original capital's distant warning cannot prepare this frontier"
    );
}

#[test]
fn the_watched_frontier_claims_legal_walls_before_routine_production() {
    let (mut game, frontier) = watched_frontier();
    game.players[0].techs.insert(crate::name!("masonry"));
    assert!(game.can_produce(0, frontier, &walls()));
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    game.apply(
        0,
        &Action::Produce {
            city: frontier,
            item: builder.clone(),
        },
    )
    .unwrap();
    let mut off = game.clone();
    AdvancedAi::new().claim_frontier_walls_before_the_surprise(&mut off, 0);
    assert_eq!(off.cities[&frontier].queue.first(), Some(&builder));

    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    ai.claim_frontier_walls_before_the_surprise(&mut game, 0);
    assert_eq!(
        game.cities[&frontier].queue.first(),
        Some(&walls()),
        "an available first wall is prepared before the surprise declaration"
    );
}

#[test]
fn the_real_research_picker_takes_the_frontier_warning() {
    let (mut game, _) = watched_frontier();
    let mut ai = armed();
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.observe_turn_start_hostiles(&game, 0);
    let plan = ai.assess(&game, 0);
    game.players[0].research = None;
    ai.advanced_research(&mut game, 0, &plan);
    assert_eq!(game.players[0].research.as_deref(), Some("masonry"));
    assert!(ai.journal().since(0).thoughts.iter().any(|thought| {
        thought
            .detail
            .contains("frontier-walls-before-the-surprise")
    }));
}

#[test]
fn the_full_observed_turn_leaves_the_frontier_on_its_wall() {
    let (mut game, frontier) = watched_frontier();
    game.players[0].techs.insert(crate::name!("masonry"));
    let mut ai = armed();
    ai.plan_observed_turn(&mut game, 0);
    assert_eq!(game.cities[&frontier].queue.first(), Some(&walls()));
}

fn replace_soldier(game: &mut Game, kind: &str, at: Pos) {
    for uid in game.player_unit_ids(1) {
        game.remove_unit(uid);
    }
    game.spawn_test_unit(kind, 1, at);
}

#[test]
fn a_weak_distant_or_unseen_major_does_not_arm_construction() {
    let (game, frontier) = watched_frontier();
    let mut weak = game.clone();
    Arc::make_mut(&mut weak.observed_military_power).insert(1, 150.0);
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&weak, 0);
    assert!(ai.frontier_walls_threat(&weak, 0, frontier).is_none());

    let mut distant = game.clone();
    replace_soldier(&mut distant, "warrior", (33, 10));
    distant.host_observed = Arc::new(distant.wdisk((24, 10), 10).into_iter().collect());
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&distant, 0);
    assert!(ai.frontier_watch_sightings.is_empty());

    let mut unseen = game.clone();
    unseen.host_observed = Arc::new(Default::default());
    assert!(!unseen.sees(&unseen.player_vision_frame(0), (31, 10)));
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&unseen, 0);
    assert!(ai.frontier_watch_sightings.is_empty());
}

#[test]
fn a_recon_civilian_or_naval_unit_does_not_arm_the_watch() {
    for kind in ["scout", "builder", "galley"] {
        let (mut game, _) = watched_frontier();
        if kind == "galley" {
            game.map.tiles.get_mut(&(31, 10)).unwrap().terrain = crate::name!("coast");
        }
        replace_soldier(&mut game, kind, (31, 10));
        let mut ai = armed();
        ai.observe_turn_start_hostiles(&game, 0);
        assert!(ai.frontier_watch_sightings.is_empty(), "{kind}");
    }
}

#[test]
fn a_friend_minor_or_active_war_is_not_a_peaceful_major_warning() {
    let (game, _) = watched_frontier();
    let mut friend = game.clone();
    friend.players[0].friends_until.insert(1, game.turn + 20);
    let mut minor = game.clone();
    minor.players[1].is_minor = true;
    let mut war = game.clone();
    war.at_war.insert((0, 1));
    for case in [friend, minor, war] {
        let mut ai = armed();
        ai.observe_turn_start_hostiles(&case, 0);
        assert!(ai.frontier_watch_sightings.is_empty());
        assert!(ai.frontier_walls_research_goal(&case, 0).is_none());
    }
}

#[test]
fn a_frontier_sighting_cannot_arm_a_different_city() {
    let (mut game, frontier) = watched_frontier();
    let quiet = game.found_city_for(0, (10, 20), None);
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    assert!(ai.frontier_walls_threat(&game, 0, frontier).is_some());
    assert!(ai.frontier_walls_threat(&game, 0, quiet).is_none());
    assert!(ai.frontier_walls_threat(&game, 0, u32::MAX).is_none());
}

#[test]
fn expired_or_future_sightings_do_not_keep_an_unstarted_wall_armed() {
    let (mut game, frontier) = watched_frontier();
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    assert!(ai.frontier_walls_threat(&game, 0, frontier).is_some());
    for uid in game.player_unit_ids(1) {
        game.remove_unit(uid);
    }
    let mut later = game.clone();
    later.turn += later.standard_duration(FRONTIER_WATCH_STANDARD) + 1;
    assert!(ai.frontier_walls_research_goal(&later, 0).is_none());
    ai.remember_frontier_watch_sightings(&later, 0);
    assert!(ai.frontier_watch_sightings.is_empty());

    ai.frontier_watch_sightings
        .insert((frontier, 1), game.turn + 1);
    assert!(ai.frontier_walls_research_goal(&game, 0).is_none());
    ai.remember_frontier_watch_sightings(&game, 0);
    assert!(ai.frontier_watch_sightings.is_empty());
}

#[test]
fn the_original_capital_still_uses_its_separate_policy() {
    let (mut game, _) = watched_frontier();
    let capital = game
        .player_city_ids(0)
        .into_iter()
        .find(|cid| game.cities[cid].is_capital)
        .unwrap();
    replace_soldier(&mut game, "warrior", (5, 10));
    game.host_observed = Arc::new(game.wdisk((12, 10), 8).into_iter().collect());
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    assert!(ai.capital_walls_research_threat(&game, 0).is_some());
    assert!(ai.frontier_watch_sightings.is_empty());
    assert!(ai.frontier_walls_research_goal(&game, 0).is_none());
    assert!(ai.frontier_walls_threat(&game, 0, capital).is_none());
}

#[test]
fn an_invested_wall_finishes_after_the_sighting_expires() {
    let (mut game, frontier) = watched_frontier();
    game.players[0].techs.insert(crate::name!("masonry"));
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    ai.claim_frontier_walls_before_the_surprise(&mut game, 0);
    assert_eq!(game.cities[&frontier].queue.first(), Some(&walls()));
    game.cities.get_mut(&frontier).unwrap().production = 5.0;
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    game.apply(
        0,
        &Action::Produce {
            city: frontier,
            item: builder,
        },
    )
    .unwrap();
    assert!(game.item_invested_production(frontier, &walls()) > 0.0);
    for uid in game.player_unit_ids(1) {
        game.remove_unit(uid);
    }
    game.turn += game.standard_duration(FRONTIER_WATCH_STANDARD) + 1;
    ai.remember_frontier_watch_sightings(&game, 0);
    assert!(ai.frontier_walls_threat(&game, 0, frontier).is_none());
    ai.claim_frontier_walls_before_the_surprise(&mut game, 0);
    assert_eq!(game.cities[&frontier].queue.first(), Some(&walls()));
}

#[test]
fn a_nearly_finished_settler_finishes_before_the_first_wall() {
    let (mut game, frontier) = watched_frontier();
    game.players[0].techs.insert(crate::name!("masonry"));
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    game.apply(
        0,
        &Action::Produce {
            city: frontier,
            item: settler.clone(),
        },
    )
    .unwrap();
    let production = game.city_yields(frontier).production.max(0.1);
    let cost = game.item_cost_for_city(0, frontier, &settler);
    game.cities.get_mut(&frontier).unwrap().production = cost - 1.5 * production;
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    ai.claim_frontier_walls_before_the_surprise(&mut game, 0);
    assert_eq!(game.cities[&frontier].queue.first(), Some(&settler));
    game.cities.get_mut(&frontier).unwrap().production = 0.0;
    ai.claim_frontier_walls_before_the_surprise(&mut game, 0);
    assert_eq!(game.cities[&frontier].queue.first(), Some(&walls()));
}
