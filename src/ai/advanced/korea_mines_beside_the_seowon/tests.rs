//! `korea-mines-beside-the-seowon`: Three Kingdoms on the Builder's side.
use super::*;

/// One founded Korean capital; returns a bare first-ring Grassland Hills
/// plot and an owned neighbour of it (also first-ring) that holds a Seowon.
fn korea_with_a_seowon(seed: u64) -> (Game, Pos, Pos) {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("KOREASEOWON", seed),
        250,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.players[0].civ = "Korea".to_string();
    game.players[0].techs.insert(crate::name!("mining"));
    let cid = game.player_city_ids(0)[0];
    let center = game.cities[&cid].pos;
    let ring: Vec<Pos> = game.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .filter(|pos| *pos != center && game.wdist(center, *pos) == 1)
        .collect();
    let (site, seowon) = ring
        .iter()
        .flat_map(|a| ring.iter().map(move |b| (*a, *b)))
        .find(|(a, b)| a != b && game.wdist(*a, *b) == 1)
        .expect("two adjacent first-ring plots");
    for pos in [site, seowon] {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = true;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
    }
    game.map.tiles.get_mut(&seowon).unwrap().district = Some(crate::name!("seowon"));
    (game, site, seowon)
}

#[test]
fn a_mine_beside_a_seowon_earns_korea_a_science() {
    let (mut game, site, seowon) = korea_with_a_seowon(93_201);
    let off = AdvancedAi::new();
    let mut on = AdvancedAi::new();
    on.enable_korea_mines_beside_the_seowon();
    assert_eq!(
        off.three_kingdoms_improvement_yields(&game, site, "mine"),
        Yields::default(),
        "off, nothing changes"
    );
    let mine = on.three_kingdoms_improvement_yields(&game, site, "mine");
    assert_eq!((mine.science, mine.food), (1.0, 0.0));
    let farm = on.three_kingdoms_improvement_yields(&game, site, "farm");
    assert_eq!((farm.science, farm.food), (0.0, 1.0));
    assert_eq!(
        on.three_kingdoms_improvement_yields(&game, site, "pasture"),
        Yields::default(),
        "only a Mine or a Farm"
    );
    assert!(
        on.improvement_value(&game, site, "mine", GrandStrategy::Science)
            > off.improvement_value(&game, site, "mine", GrandStrategy::Science),
        "the Builder prices the Science"
    );
    // A Seowon of no city of ours, and a civilization other than Korea, pay
    // nothing.
    game.map.tiles.get_mut(&seowon).unwrap().owner_city = None;
    assert_eq!(
        on.three_kingdoms_improvement_yields(&game, site, "mine"),
        Yields::default()
    );
    let (mut rome, site, _) = korea_with_a_seowon(93_202);
    rome.players[0].civ = "Rome".to_string();
    assert_eq!(
        on.three_kingdoms_improvement_yields(&rome, site, "mine"),
        Yields::default()
    );
}
