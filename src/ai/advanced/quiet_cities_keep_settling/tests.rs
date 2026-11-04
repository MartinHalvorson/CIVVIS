use super::*;
use crate::game::Action;

/// Two cities of ours at war with player 1: the capital is the threatened
/// city and a second city stands well out of its relief ring. No barbarians,
/// no units but three of our Warriors garrisoned in the capital. Returns the
/// game, the capital and the distant city.
fn recovery_board(seed: u64) -> (Game, u32, u32) {
    let mut game = Game::new_full(2, 74, 46, seed, 200, 0, false);
    game.current = 0;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    let site = game
        .map
        .tiles
        .values()
        .filter(|tile| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && tile.owner_city.is_none()
                && game.city_at(tile.pos).is_none()
                && (THREAT_RELIEF_RADIUS + 4..=THREAT_RELIEF_RADIUS + 8)
                    .contains(&game.wdist(home, tile.pos))
        })
        .map(|tile| tile.pos)
        .min()
        .expect("a land site out of the capital's relief ring");
    let walker = game.spawn_test_unit("settler", 0, site);
    game.apply(0, &Action::FoundCity { unit: walker }).unwrap();
    let distant = game.city_at(site).unwrap();
    game.cities.get_mut(&distant).unwrap().pop = 3;
    for _ in 0..3 {
        game.spawn_test_unit("warrior", 0, home);
    }
    game.at_war.insert((0, 1));
    game.current = 0;
    (game, capital, distant)
}

fn threatened_recovery(game: &Game, capital: u32) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: Some(1),
        target_city: None,
        threatened_city: Some(capital),
        desired_cities: 6,
        assessed_turn: game.turn,
        rush: false,
    }
}

fn settler() -> Item {
    Item::Unit {
        unit: crate::name!("settler"),
    }
}

#[test]
fn off_the_hold_stays_empire_wide() {
    let (game, capital, distant) = recovery_board(41_104);
    let plan = threatened_recovery(&game, capital);
    let ai = AdvancedAi::new();
    assert!(ai.plan_city_target);
    let counts = ai.counts(&game, 0);
    assert!(!ai.quiet_city_keeps_settling(&game, 0, distant, &plan, &counts));
    assert!(
        ai.production_value(&game, 0, distant, &settler(), &plan, &counts) < -1_000.0,
        "with the gene off a threatened Recovery holds the Settler in every city"
    );
}

#[test]
fn a_quiet_distant_city_may_train_one_settler() {
    let (game, capital, distant) = recovery_board(41_104);
    let plan = threatened_recovery(&game, capital);
    let mut ai = AdvancedAi::new();
    ai.enable_quiet_cities_keep_settling();
    let counts = ai.counts(&game, 0);
    assert!(ai.quiet_city_keeps_settling(&game, 0, distant, &plan, &counts));
    assert!(
        !ai.quiet_city_keeps_settling(&game, 0, capital, &plan, &counts),
        "the threatened city keeps the hold"
    );
    assert!(
        ai.production_value(&game, 0, capital, &settler(), &plan, &counts) < -1_000.0,
        "the threatened city still may not train a Settler"
    );
}

#[test]
fn a_visible_hostile_within_reach_keeps_the_hold() {
    let (mut game, capital, distant) = recovery_board(41_104);
    let plan = threatened_recovery(&game, capital);
    let mut ai = AdvancedAi::new();
    ai.enable_quiet_cities_keep_settling();
    let at = game.cities[&distant].pos;
    let near = game
        .map
        .tiles
        .values()
        .filter(|tile| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.city_at(tile.pos).is_none()
                && game.wdist(at, tile.pos) == 2
        })
        .map(|tile| tile.pos)
        .min()
        .expect("a land tile two from the distant city");
    game.spawn_test_unit("warrior", 1, near);
    let counts = ai.counts(&game, 0);
    assert!(
        !ai.quiet_city_keeps_settling(&game, 0, distant, &plan, &counts),
        "an enemy soldier two tiles out is within the war's reach"
    );
}

#[test]
fn a_settler_in_flight_or_a_thin_army_keeps_the_hold() {
    let (mut game, capital, distant) = recovery_board(41_104);
    let plan = threatened_recovery(&game, capital);
    let mut ai = AdvancedAi::new();
    ai.enable_quiet_cities_keep_settling();
    let home = game.cities[&capital].pos;

    let mut walking = game.clone();
    walking.spawn_test_unit("settler", 0, home);
    let counts = ai.counts(&walking, 0);
    assert!(counts.settlers > 0);
    assert!(
        !ai.quiet_city_keeps_settling(&walking, 0, distant, &plan, &counts),
        "one Settler at a time under the exemption"
    );

    let soldiers = game
        .player_unit_ids(0)
        .into_iter()
        .filter(|unit| game.units[unit].kind == "warrior")
        .collect::<Vec<_>>();
    game.remove_unit(soldiers[0]);
    let counts = ai.counts(&game, 0);
    assert!(
        !ai.quiet_city_keeps_settling(&game, 0, distant, &plan, &counts),
        "two soldiers for two cities leave no escort to spare"
    );
}

#[test]
fn a_city_inside_the_relief_ring_or_lately_hit_keeps_the_hold() {
    let (mut game, capital, distant) = recovery_board(41_104);
    let plan = threatened_recovery(&game, capital);
    let mut ai = AdvancedAi::new();
    ai.enable_quiet_cities_keep_settling();

    let mut hit = game.clone();
    hit.turn = 40;
    hit.cities.get_mut(&distant).unwrap().last_attacked = 38;
    let counts = ai.counts(&hit, 0);
    assert!(!ai.quiet_city_keeps_settling(&hit, 0, distant, &plan, &counts));

    // Move the threatened city's role to a city founded beside the distant one.
    let at = game.cities[&distant].pos;
    let beside = game
        .map
        .tiles
        .values()
        .filter(|tile| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && tile.owner_city.is_none()
                && game.city_at(tile.pos).is_none()
                && game.wdist(at, tile.pos) == 4
        })
        .map(|tile| tile.pos)
        .min();
    if let Some(beside) = beside {
        let walker = game.spawn_test_unit("settler", 0, beside);
        game.apply(0, &Action::FoundCity { unit: walker }).unwrap();
        let front = game.city_at(beside).unwrap();
        let home = game.cities[&capital].pos;
        game.spawn_test_unit("warrior", 0, home);
        let plan = threatened_recovery(&game, front);
        let counts = ai.counts(&game, 0);
        assert!(
            !ai.quiet_city_keeps_settling(&game, 0, distant, &plan, &counts),
            "four tiles from the threatened city is inside its relief ring"
        );
    }
}

#[test]
fn the_quiet_city_founds_only_on_quiet_ground() {
    let (mut game, capital, distant) = recovery_board(41_104);
    let plan = threatened_recovery(&game, capital);
    let ai = AdvancedAi::new();
    let home = game.cities[&capital].pos;
    let far = game.cities[&distant].pos;
    assert!(!ai.quiet_settle_site(&game, 0, &plan, home));
    assert!(ai.quiet_settle_site(&game, 0, &plan, far));
    game.spawn_test_unit("warrior", 1, far);
    assert!(
        !ai.quiet_settle_site(&game, 0, &plan, far),
        "a visible enemy on the site is not quiet ground"
    );
}

#[test]
fn the_production_scorer_prices_the_quiet_citys_settler() {
    let (game, capital, distant) = recovery_board(41_104);
    let plan = threatened_recovery(&game, capital);
    let mut ai = AdvancedAi::new();
    ai.enable_quiet_cities_keep_settling();
    let counts = ai.counts(&game, 0);
    assert!(game.can_produce(0, distant, &settler()));
    assert!(
        ai.production_value(&game, 0, distant, &settler(), &plan, &counts) > 0.0,
        "past the Recovery hold the quiet city's Settler is priced like any other"
    );
}
