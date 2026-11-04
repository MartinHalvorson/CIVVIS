use super::*;

/// A capital and a second city four or more tiles away, every owned plot a
/// bare grassland hill (a Mine apiece with Mining), four citizens a city, no
/// Builder and no Settler anywhere, both queues training a routine Warrior at
/// peace. The fixture of `backlog-builder-takes-the-queue`'s tests.
fn builderless_empire() -> (Game, u32, u32) {
    builderless_empire_with(false)
}

/// [`builderless_empire`], with the barbarian seat when `barbarians`.
fn builderless_empire_with(barbarians: bool) -> (Game, u32, u32) {
    let mut game = Game::new_full(1, 24, 16, 936_221, 200, 0, barbarians);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let sites: Vec<Pos> = game.map.tiles.keys().copied().collect();
    let mut second = None;
    for site in sites {
        if game.wdist(site, home) < 4 {
            continue;
        }
        let settler = game.spawn_unit("settler", 0, site);
        if game.apply(0, &Action::FoundCity { unit: settler }).is_ok() {
            second = game
                .player_city_ids(0)
                .into_iter()
                .find(|cid| game.cities[cid].pos == site);
            break;
        }
        game.remove_unit(settler);
    }
    let second = second.expect("a legal second site");
    game.players[0].techs.insert(crate::name!("mining"));
    for cid in [capital, second] {
        let center = game.cities[&cid].pos;
        let owned: Vec<Pos> = game.cities[&cid].owned_tiles.to_vec();
        for position in owned {
            if position == center || game.map.tiles[&position].district.is_some() {
                continue;
            }
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.hills = true;
            tile.feature = None;
            tile.resource = None;
            tile.improvement = None;
        }
        let city = game.cities.get_mut(&cid).unwrap();
        city.pop = 4;
        city.queue = vec![unit("warrior")];
    }
    (game, capital, second)
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn plan_for(game: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    }
}

fn builders_at_head(game: &Game) -> Vec<u32> {
    game.player_city_ids(0)
        .into_iter()
        .filter(|cid| game.cities[cid].queue.first() == Some(&unit("builder")))
        .collect()
}

/// Off, nothing moves. On, exactly one city -- the faster -- puts a Builder at
/// the head, its Warrior kept behind it, and the claim does not repeat once a
/// Builder is held.
#[test]
fn an_empire_without_a_builder_trains_one_in_its_fastest_city() {
    let (mut off, ..) = builderless_empire();
    assert!(BasicAi::unimproved_worked_tiles(&off, 0) >= BUILDERLESS_MIN_BACKLOG);
    let plan = plan_for(&off);
    AdvancedAi::new().claim_builderless_empire_builder(&mut off, 0, &plan);
    assert!(builders_at_head(&off).is_empty(), "off: nothing moves");

    let (mut on, capital, second) = builderless_empire();
    let mut ai = AdvancedAi::new();
    ai.enable_builderless_empire_trains_one();
    let builder = unit("builder");
    let fastest = [capital, second]
        .into_iter()
        .min_by(|a, b| {
            ai.production_build_turns(&on, 0, *a, &builder)
                .total_cmp(&ai.production_build_turns(&on, 0, *b, &builder))
                .then(a.cmp(b))
        })
        .unwrap();
    ai.claim_builderless_empire_builder(&mut on, 0, &plan);
    assert_eq!(builders_at_head(&on), vec![fastest]);
    assert!(
        on.cities[&fastest].queue.contains(&unit("warrior")),
        "the displaced Warrior keeps its place behind the Builder"
    );
    ai.claim_builderless_empire_builder(&mut on, 0, &plan);
    assert_eq!(builders_at_head(&on), vec![fastest], "one Builder, not two");
}

/// A Builder afield is a Builder held: the claim stands down.
#[test]
fn a_builder_afield_stands_the_claim_down() {
    let (mut game, capital, _) = builderless_empire();
    let pos = game.cities[&capital].pos;
    game.spawn_unit("builder", 0, pos);
    let plan = plan_for(&game);
    let mut ai = AdvancedAi::new();
    ai.enable_builderless_empire_trains_one();
    ai.claim_builderless_empire_builder(&mut game, 0, &plan);
    assert!(builders_at_head(&game).is_empty());
}

/// A Settler at the head keeps its queue, and so does a city with a
/// barbarian raider in its home ring; with both cities so held, nothing
/// moves.
#[test]
fn a_settler_and_a_raided_city_keep_their_queues() {
    let (mut game, capital, second) = builderless_empire();
    game.cities.get_mut(&capital).unwrap().queue = vec![unit("settler")];
    let plan = plan_for(&game);
    let mut ai = AdvancedAi::new();
    ai.enable_builderless_empire_trains_one();
    ai.claim_builderless_empire_builder(&mut game, 0, &plan);
    assert_eq!(
        builders_at_head(&game),
        vec![second],
        "the Settler's city keeps its queue; the other city trains the Builder"
    );

    let (mut raided, capital, second) = builderless_empire_with(true);
    raided.cities.get_mut(&capital).unwrap().queue = vec![unit("settler")];
    let barb = raided.barb_pid.expect("the barbarian seat");
    let near = raided.cities[&second].pos;
    let raider = raided
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            let d = raided.wdist(**pos, near);
            (2..=3).contains(&d) && raided.rules.is_passable(tile) && !raided.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .next()
        .expect("a land tile near the second city");
    let uid = raided.spawn_unit("warrior", barb, raider);
    assert!(BasicAi::is_barbarian_raider(&raided, &raided.units[&uid]));
    ai.claim_builderless_empire_builder(&mut raided, 0, &plan);
    assert!(
        builders_at_head(&raided).is_empty(),
        "the raided city keeps its queue"
    );
}

/// Version two: a Settler with nothing invested yields the head to the
/// empire's only Builder; one with production in it keeps its queue.
#[test]
fn version_two_lets_a_fresh_settler_yield_to_the_only_builder() {
    let (mut game, capital, second) = builderless_empire();
    // Both cities train a Settler; the second has production in its own.
    for cid in [capital, second] {
        game.cities.get_mut(&cid).unwrap().queue = vec![unit("settler")];
    }
    game.cities.get_mut(&second).unwrap().production = 5.0;
    assert!(game.item_invested_production(second, &unit("settler")) > 0.0);
    assert_eq!(
        game.item_invested_production(capital, &unit("settler")),
        0.0
    );
    let plan = plan_for(&game);

    let mut v1 = AdvancedAi::new();
    v1.enable_builderless_empire_trains_one();
    let mut one = game.clone();
    v1.claim_builderless_empire_builder(&mut one, 0, &plan);
    assert!(
        builders_at_head(&one).is_empty(),
        "v1: every Settler keeps its queue"
    );

    let mut v2 = AdvancedAi::new();
    v2.enable_builderless_empire_trains_one_2();
    let mut two = game.clone();
    v2.claim_builderless_empire_builder(&mut two, 0, &plan);
    assert_eq!(
        builders_at_head(&two),
        vec![capital],
        "v2: the fresh Settler yields; the invested one keeps its queue"
    );
}
