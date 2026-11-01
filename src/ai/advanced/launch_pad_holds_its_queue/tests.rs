use super::*;
use crate::game::install_test_district;
use crate::world::DistrictFoundation;

#[test]
fn launch_pad_holds_its_queue_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "launch-pad-holds-its-queue",
        |ai| ai.launch_pad_holds_its_queue,
    );
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

/// A late-game city that placed the empire's first pad, then lent its queue
/// to a Builder: the foundation stays on the tile and the Builder is at the
/// head.
fn board() -> (Game, u32, Item, Item) {
    let mut game = Game::new(2, 32, 24, 5_414, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let cid = game.player_city_ids(0)[0];
    game.players[0].techs = game.rules.techs.keys().cloned().collect();
    game.max_turns = 100_000;
    game.turn = game.max_turns / 2;
    game.current = 0;
    install_test_district(&mut game, cid, "industrial_zone");
    game.cities.get_mut(&cid).unwrap().pop = 14;
    let site = *game
        .district_sites(cid, crate::name!("spaceport"))
        .first()
        .expect("the city can place a Spaceport");
    let pad = Item::District {
        district: crate::name!("spaceport"),
        pos: site,
    };
    game.map.tiles.get_mut(&site).unwrap().district_foundation = Some(DistrictFoundation {
        district: crate::name!("spaceport"),
        cost: 900.0,
    });
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    game.cities.get_mut(&cid).unwrap().queue.clear();
    game.apply(
        0,
        &Action::Produce {
            city: cid,
            item: builder.clone(),
        },
    )
    .expect("the Builder borrows the queue");
    assert_eq!(game.cities[&cid].queue.first(), Some(&builder));
    assert_eq!(AdvancedAi::unfinished_launch_pad_site(&game, 0), Some(cid));
    (game, cid, pad, builder)
}

#[test]
fn the_first_pad_goes_back_to_the_head_of_its_city() {
    let (mut game, cid, pad, builder) = board();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.claim_launch_pad_holds_its_queue(&mut game, 0, &plan());
    assert_eq!(
        game.cities[&cid].queue.first(),
        Some(&builder),
        "off, the Builder keeps the queue"
    );

    ai.enable_launch_pad_holds_its_queue();
    assert_eq!(ai.launch_pad_hold(&game, 0), Some((cid, pad.clone())));
    ai.claim_launch_pad_holds_its_queue(&mut game, 0, &plan());
    assert_eq!(game.cities[&cid].queue.first(), Some(&pad));
}

#[test]
fn a_threatened_pad_city_is_left_to_the_defence() {
    let (mut game, cid, _, builder) = board();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_launch_pad_holds_its_queue();
    let threatened = StrategicPlan {
        threatened_city: Some(cid),
        ..plan()
    };
    ai.claim_launch_pad_holds_its_queue(&mut game, 0, &threatened);
    assert_eq!(game.cities[&cid].queue.first(), Some(&builder));

    game.cities.get_mut(&cid).unwrap().last_attacked = game.turn - 2;
    ai.claim_launch_pad_holds_its_queue(&mut game, 0, &plan());
    assert_eq!(
        game.cities[&cid].queue.first(),
        Some(&builder),
        "attacked two turns ago"
    );
}

#[test]
fn the_spy_trainer_skips_the_pad_city_until_a_pad_stands() {
    let (mut game, cid, _, _) = board();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    assert!(!ai.launch_pad_holds_city(&game, 0, cid), "off");
    ai.enable_launch_pad_holds_its_queue();
    assert!(ai.launch_pad_holds_city(&game, 0, cid));

    let site = match ai.launch_pad_hold(&game, 0) {
        Some((_, Item::District { pos, .. })) => pos,
        other => panic!("the pad is held: {other:?}"),
    };
    game.map.tiles.get_mut(&site).unwrap().district_foundation = None;
    install_test_district(&mut game, cid, "spaceport");
    assert!(
        !ai.launch_pad_holds_city(&game, 0, cid),
        "a standing pad ends it"
    );
    assert_eq!(ai.launch_pad_hold(&game, 0), None);
}
