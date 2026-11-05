//! `prod-tiles-call-the-builders`: the production ground the quota counts,
//! the quota itself, and the Builder it orders where the stock 60% cover is
//! already met.
use super::*;

/// One founded capital (ten Production a turn) whose six first-ring tiles
/// are bare Grassland, with Mining and Construction researched. A live-style
/// worked assignment is set per test through `observed_city_worked_tiles`.
fn ring_fixture(tag: &str, seed: u64) -> (Game, u32, Vec<Pos>) {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed(tag, seed),
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
    let cid = game.player_city_ids(0)[0];
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            production: 10.0,
            ..Default::default()
        },
    );
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    game.players[0].techs.insert(crate::name!("mining"));
    game.players[0].techs.insert(crate::name!("construction"));
    let center = game.cities[&cid].pos;
    let ring: Vec<Pos> = game.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .filter(|pos| *pos != center && game.wdist(center, *pos) == 1)
        .collect();
    assert_eq!(ring.len(), 6, "the founded city owns its first ring");
    for pos in &ring {
        let tile = game.map.tiles.get_mut(pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
    }
    (game, cid, ring)
}

fn set_worked(game: &mut Game, cid: u32, worked: &[Pos]) {
    game.cities.get_mut(&cid).unwrap().pop = worked.len() as i32;
    std::sync::Arc::make_mut(&mut game.observed_city_worked_tiles).insert(cid, worked.to_vec());
}

fn hills(game: &mut Game, pos: Pos, terrain: Name) {
    let tile = game.map.tiles.get_mut(&pos).unwrap();
    tile.terrain = terrain;
    tile.hills = true;
}

fn forest(game: &mut Game, pos: Pos) {
    game.map.tiles.get_mut(&pos).unwrap().feature = Some(crate::name!("forest"));
}

/// Worked hills and forests count, whatever they pay; an idle tile counts
/// only when it already yields Food. Flat Grassland (a Farm, no Production)
/// never counts, and neither does an improved tile.
#[test]
fn the_backlog_counts_worked_production_ground_and_good_idle_ground() {
    let (mut game, cid, ring) = ring_fixture("PRODTILES", 93_101);
    let [worked_hills, worked_desert_hills, worked_forest, idle_hills, idle_desert_hills, worked_flat] =
        [ring[0], ring[1], ring[2], ring[3], ring[4], ring[5]];
    hills(&mut game, worked_hills, crate::name!("grassland"));
    hills(&mut game, worked_desert_hills, crate::name!("desert"));
    forest(&mut game, worked_forest);
    hills(&mut game, idle_hills, crate::name!("grassland"));
    hills(&mut game, idle_desert_hills, crate::name!("desert"));
    set_worked(
        &mut game,
        cid,
        &[
            worked_hills,
            worked_desert_hills,
            worked_forest,
            worked_flat,
        ],
    );
    assert!(
        game.workable_tile_yields(idle_desert_hills).food < 1.0,
        "Desert Hills pay no Food"
    );
    assert!(game.workable_tile_yields(idle_hills).food >= 1.0);
    for pos in [
        worked_hills,
        worked_desert_hills,
        worked_forest,
        idle_hills,
        idle_desert_hills,
    ] {
        assert!(
            BasicAi::production_improvement_fits(&game, 0, pos),
            "a Mine or Lumber Mill fits {pos:?}"
        );
    }
    assert!(
        !BasicAi::production_improvement_fits(&game, 0, worked_flat),
        "only a Farm fits bare flat Grassland"
    );
    assert_eq!(
        BasicAi::production_tile_backlog(&game, 0),
        ProductionTileBacklog { worked: 3, idle: 1 },
        "the worked Desert Hills count; the idle Desert Hills do not"
    );
    // An improved tile is no backlog.
    game.map.tiles.get_mut(&worked_hills).unwrap().improvement = Some(crate::name!("mine"));
    game.map.tiles.get_mut(&idle_hills).unwrap().improvement = Some(crate::name!("mine"));
    assert_eq!(
        BasicAi::production_tile_backlog(&game, 0),
        ProductionTileBacklog { worked: 2, idle: 0 }
    );
    // A pillaged Mine is repaired without a charge, so it asks for none.
    game.map.tiles.get_mut(&worked_hills).unwrap().pillaged = true;
    game.map.tiles.get_mut(&idle_hills).unwrap().pillaged = true;
    assert_eq!(
        BasicAi::production_tile_backlog(&game, 0),
        ProductionTileBacklog { worked: 2, idle: 0 }
    );
    // Without Construction a worked forest takes no Lumber Mill.
    game.players[0].techs.remove(&crate::name!("construction"));
    assert_eq!(BasicAi::production_tile_backlog(&game, 0).worked, 2);
}

/// Off, the quota is the stock 60% of the backlog. On, one charge per worked
/// production tile, 60% of the rest, and half a charge per good idle tile,
/// and never below the stock quota.
#[test]
fn the_quota_asks_a_charge_per_worked_production_tile() {
    let (mut game, cid, ring) = ring_fixture("PRODQUOTA", 93_102);
    for pos in &ring[..3] {
        hills(&mut game, *pos, crate::name!("grassland"));
    }
    hills(&mut game, ring[5], crate::name!("plains"));
    // Three worked hills, one worked flat (a Farm), one idle plains hills.
    set_worked(&mut game, cid, &[ring[0], ring[1], ring[2], ring[3]]);
    let backlog = BasicAi::unimproved_worked_tiles(&game, 0);
    assert_eq!(backlog, 4, "four worked tiles take an improvement");
    let stock = BasicAi::new();
    let (wanted, production) = stock.worked_backlog_charges_wanted(&game, 0, backlog);
    assert_eq!(production, None);
    assert!((wanted - 4.0 * BasicAi::WORKED_BACKLOG_COVER).abs() < 1e-9);
    let mut gene = BasicAi::new();
    gene.prod_tiles_call_the_builders = true;
    let (wanted, production) = gene.worked_backlog_charges_wanted(&game, 0, backlog);
    assert_eq!(
        production,
        Some(ProductionTileBacklog { worked: 3, idle: 1 })
    );
    let expected = 3.0 + BasicAi::WORKED_BACKLOG_COVER + IDLE_PRODUCTION_TILE_CHARGE;
    assert!(
        (wanted - expected).abs() < 1e-9,
        "{wanted} against {expected}"
    );
    assert!(gene.production_tile_quota_note(&game, 0).is_some());
    assert!(stock.production_tile_quota_note(&game, 0).is_none());
}

/// The stock step stops once the charges in hand cover 60% of the worked
/// backlog. Under the gene, worked hills still bare call a further Builder.
/// The gene acts only through `builders-cover-the-worked-backlog`, and a due
/// Settler and charges that meet the gene's own quota keep the stock pick.
#[test]
fn bare_worked_hills_call_a_builder_past_the_stock_cover() {
    let (mut game, cid, ring) = ring_fixture("PRODCOVER", 93_103);
    for pos in &ring {
        hills(&mut game, *pos, crate::name!("grassland"));
    }
    set_worked(&mut game, cid, &ring);
    let backlog = BasicAi::unimproved_worked_tiles(&game, 0);
    assert_eq!(backlog, 6);
    // Four charges in hand: the stock quota (3.6) is met, the gene's (6) is not.
    let center = game.cities[&cid].pos;
    let uid = game.spawn_unit("builder", 0, center);
    game.units.get_mut(&uid).unwrap().charges = 4;
    assert_eq!(BasicAi::builder_charge_cover(&game, 0), 4);
    let pick = |game: &Game, cover: bool, gene: bool, settlers: usize| {
        let mut ai = BasicAi::new();
        ai.builder_before_the_army_3 = true;
        ai.builders_cover_the_worked_backlog = cover;
        ai.prod_tiles_call_the_builders = gene;
        ai.pick_item(game, 0, cid, 3, settlers, 2, 1, 0, 0, 0, 0)
    };
    let is_builder =
        |item: &Option<Item>| matches!(item, Some(Item::Unit { unit }) if *unit == "builder");
    assert!(
        !is_builder(&pick(&game, true, false, 1)),
        "the stock cover is met: {:?}",
        pick(&game, true, false, 1)
    );
    assert!(
        is_builder(&pick(&game, true, true, 1)),
        "{:?}",
        pick(&game, true, true, 1)
    );
    assert_eq!(
        pick(&game, false, true, 1),
        pick(&game, false, false, 1),
        "without the backlog step the gene changes nothing"
    );
    assert!(BasicAi::new().settler_due(&game, 0, cid, 3, 0));
    assert_eq!(
        pick(&game, true, true, 0),
        pick(&game, true, false, 0),
        "a due Settler keeps the stock pick"
    );
    let mut covered = game.clone();
    covered.units.get_mut(&uid).unwrap().charges = 6;
    assert_eq!(
        pick(&covered, true, true, 1),
        pick(&covered, true, false, 1),
        "charges that meet the gene's quota keep the stock pick"
    );
}
