use super::tests::walled_city;
use super::*;

fn separated_firing_tiles(kind: &str) -> (Game, u32, u32, Pos, Pos) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    let at = |x, y| (city.0 + x, city.1 + y);
    let start = at(-4, 0);
    let pocket = at(-2, 0);
    let reachable = at(-2, 2);
    let ground = [
        city,
        start,
        pocket,
        at(-1, 0), // Sight into the pocket crosses the reserved inner ring.
        at(-1, 1), // Sight from the reachable firing tile.
        at(-4, 1),
        at(-4, 2),
        at(-3, 2),
        reachable,
    ];
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if ground.contains(&tile.pos) {
            crate::name!("grassland")
        } else {
            crate::name!("mountain")
        };
        tile.feature = None;
        tile.hills = false;
    }
    let uid = g.spawn_unit(kind, 0, start);
    assert_eq!(g.unit_attack_range(uid), 2);
    assert!(g.wdist(start, pocket) < g.wdist(start, reachable));
    assert!(g.unit_has_line_of_sight_from(uid, pocket, city));
    assert!(g.unit_has_line_of_sight_from(uid, reachable, city));
    assert_eq!(siege_route_step(&g, 0, uid, pocket, city), None);
    assert!(siege_route_step(&g, 0, uid, reachable, city).is_some());
    (g, cid, uid, pocket, reachable)
}

fn reaches_and_fires(kind: &str) {
    let (mut g, cid, uid, _, reachable) = separated_firing_tiles(kind);
    let city = CityView::of(&g, cid).unwrap();
    let post = siege_posts(&g, 0, &city, &[uid], None)[&uid];
    assert_eq!(
        post, reachable,
        "a clear but sealed firing pocket must not displace a reachable post"
    );
    let mut ai = AdvancedAi::new();
    ai.enable_recorded_tactical_step();
    for _ in 0..4 {
        ai.approach(&mut g, 0, uid, post, city.pos);
        g.apply(0, &Action::EndTurn).unwrap();
        g.apply(1, &Action::EndTurn).unwrap();
        if g.units[&uid].pos == post {
            break;
        }
    }
    assert_eq!(
        g.units[&uid].pos, post,
        "the assigned post is physically reached"
    );
    let walls = g.cities[&cid].wall_hp;
    g.apply(
        0,
        &Action::Ranged {
            unit: uid,
            target: city.pos,
        },
    )
    .unwrap();
    assert!(
        g.cities[&cid].wall_hp < walls,
        "the arrived unit fires at the wall"
    );
}

#[test]
fn a_siege_gun_reaches_and_fires_from_the_available_post() {
    reaches_and_fires("catapult");
}

#[test]
fn a_ranged_shooter_reaches_and_fires_from_the_available_post() {
    reaches_and_fires("archer");
}

#[test]
fn a_gun_without_any_reachable_firing_tile_gets_no_post() {
    let (mut g, cid, uid, _, _) = separated_firing_tiles("catapult");
    let city = CityView::of(&g, cid).unwrap();
    let start = g.units[&uid].pos;
    for pos in g.nbrs(start) {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
    }
    assert_eq!(
        siege_route_step(&g, 0, uid, (city.pos.0 - 2, city.pos.1 + 2), city.pos),
        None
    );
    assert!(
        !siege_posts(&g, 0, &city, &[uid], None).contains_key(&uid),
        "an unreachable clear shot must not be assigned as a firing post"
    );
    assert_eq!(g.units[&uid].pos, start);
}

fn column_firing_tile(kind: &str) -> (Game, u32, u32, u32, Pos) {
    let (mut game, city_id) = walled_city();
    let city = game.cities[&city_id].pos;
    let start = (city.0 - 4, city.1);
    let screen_pos = (city.0 - 3, city.1);
    let post = (city.0 - 2, city.1);
    for tile in game.map.tiles.values_mut() {
        tile.terrain = if tile.pos.1 == city.1 && (start.0..=city.0).contains(&tile.pos.0) {
            crate::name!("grassland")
        } else {
            crate::name!("mountain")
        };
        tile.feature = None;
        tile.hills = false;
    }
    let shooter = game.spawn_unit(kind, 0, start);
    let screen = game.spawn_unit("swordsman", 0, screen_pos);
    assert_eq!(game.unit_attack_range(shooter), 2);
    assert!(game.unit_has_line_of_sight_from(shooter, post, city));
    assert_eq!(siege_route_step(&game, 0, shooter, post, city), None);
    assert_eq!(game.pass_through_destination(shooter, post, 0), Some(post));
    assert_eq!(game.path_to(shooter, post), Some(vec![screen_pos, post]));
    (game, city_id, shooter, screen, post)
}

fn column_arrives_and_fires(kind: &str) {
    let (mut game, city_id, shooter, screen, post) = column_firing_tile(kind);
    let city = CityView::of(&game, city_id).unwrap();
    let screen_before = game.units[&screen].clone();
    let posts = siege_posts(&game, 0, &city, &[shooter], None);
    assert_eq!(
        posts.get(&shooter),
        Some(&post),
        "a legal firing post across our own screen must be assigned"
    );
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_recorded_tactical_step();
    ai.sieges.insert(
        city_id,
        Siege {
            stage: SiegeStage::Reduce,
            taker: None,
            entered: game.turn,
            assessed: game.turn,
            posts,
            short_since: None,
        },
    );
    assert!(ai.siege_shooter_step(&mut game, 0, shooter, &city));
    assert_eq!(game.units[&shooter].pos, post);
    assert_eq!(game.units[&screen].pos, screen_before.pos);
    assert_eq!(game.units[&screen].moves_left, screen_before.moves_left);
    assert!(game.log.since(0).any(|(seat, action)| {
        *seat == 0
            && matches!(action, Action::MoveTo { unit, to } if *unit == shooter && *to == post)
    }));
    game.apply(0, &Action::EndTurn).unwrap();
    game.apply(1, &Action::EndTurn).unwrap();
    let city = CityView::of(&game, city_id).unwrap();
    let walls_before = game.cities[&city_id].wall_hp;
    assert!(ai.siege_shooter_step(&mut game, 0, shooter, &city));
    assert!(
        game.cities[&city_id].wall_hp < walls_before,
        "the naturally posted shooter must actually damage the wall"
    );
}

#[test]
fn a_catapult_is_posted_across_its_screen_then_arrives_and_fires() {
    column_arrives_and_fires("catapult");
}

#[test]
fn an_archer_is_posted_across_its_screen_then_arrives_and_fires() {
    column_arrives_and_fires("archer");
}

#[test]
fn a_firing_post_is_kept_when_this_turn_can_cross_the_friendly_column() {
    let (game, city_id, shooter, _, post) = column_firing_tile("catapult");
    let city = CityView::of(&game, city_id).unwrap();
    let previous = BTreeMap::from([(shooter, post)]);
    let posts = siege_posts_keeping(&game, 0, &city, &[shooter], None, &previous);
    assert_eq!(posts.get(&shooter), Some(&post));
}

#[test]
fn a_column_post_is_not_assigned_without_moves_to_cross_the_screen() {
    let (mut game, city_id, shooter, _, post) = column_firing_tile("catapult");
    game.units.get_mut(&shooter).unwrap().moves_left = 1.0;
    let city = CityView::of(&game, city_id).unwrap();
    assert_eq!(game.path_to(shooter, post), None);
    assert!(!siege_posts(&game, 0, &city, &[shooter], None).contains_key(&shooter));
}

#[test]
fn an_enemy_screen_does_not_make_a_firing_post_reachable() {
    let (mut game, city_id, shooter, screen, post) = column_firing_tile("catapult");
    game.units.get_mut(&screen).unwrap().owner = 1;
    let city = CityView::of(&game, city_id).unwrap();
    assert_eq!(game.path_to(shooter, post), None);
    assert!(!siege_posts(&game, 0, &city, &[shooter], None).contains_key(&shooter));
}

#[test]
fn a_water_crossing_does_not_make_a_dry_firing_post_reachable() {
    let (mut game, city_id, shooter, screen, post) = column_firing_tile("catapult");
    let screen_pos = game.units[&screen].pos;
    game.remove_unit(screen);
    game.map.tiles.get_mut(&screen_pos).unwrap().terrain = crate::name!("coast");
    let city = CityView::of(&game, city_id).unwrap();
    assert_eq!(siege_route_step(&game, 0, shooter, post, city.pos), None);
    assert!(!siege_posts(&game, 0, &city, &[shooter], None).contains_key(&shooter));
}

#[test]
fn the_siege_gun_handler_crosses_the_column_then_fires_after_the_turn_reset() {
    let (mut game, city_id, gun, screen, post) = column_firing_tile("catapult");
    let city = CityView::of(&game, city_id).unwrap();
    let screen_pos = game.units[&screen].pos;
    let posts = siege_posts(&game, 0, &city, &[gun], None);
    assert_eq!(arm_of(&game, gun), Arm::Siege);
    assert_eq!(posts.get(&gun), Some(&post));
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_recorded_tactical_step();
    ai.sieges.insert(
        city_id,
        Siege {
            stage: SiegeStage::Reduce,
            taker: None,
            entered: game.turn,
            assessed: game.turn,
            posts,
            short_since: None,
        },
    );
    let walls_before = game.cities[&city_id].wall_hp;
    assert!(ai.siege_gun_step(&mut game, 0, gun, &city));
    assert_eq!(game.units[&gun].pos, post);
    assert_eq!(game.units[&screen].pos, screen_pos);
    assert_eq!(game.cities[&city_id].wall_hp, walls_before);
    assert!(game.log.since(0).any(|(seat, action)| {
        *seat == 0 && matches!(action, Action::MoveTo { unit, to } if *unit == gun && *to == post)
    }));
    assert!(!ai.siege_gun_step(&mut game, 0, gun, &city));
    assert_eq!(game.cities[&city_id].wall_hp, walls_before);
    game.apply(0, &Action::EndTurn).unwrap();
    game.apply(1, &Action::EndTurn).unwrap();
    let city = CityView::of(&game, city_id).unwrap();
    let walls_before = game.cities[&city_id].wall_hp;
    assert!(ai.siege_gun_step(&mut game, 0, gun, &city));
    assert!(game.cities[&city_id].wall_hp < walls_before);
}
