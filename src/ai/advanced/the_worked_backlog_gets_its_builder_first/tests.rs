use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// `backlog-builder-takes-the-queue`'s fixture: a founded capital and a
/// second city four or more tiles away, every owned plot a bare grassland
/// hill (a Mine apiece with Mining), four citizens a city, a Settler already
/// walking (no city is due another one), no Builder, and both queues training
/// a routine Warrior.
fn backlogged_empire() -> (Game, u32, u32) {
    let mut game = Game::new_full(1, 24, 16, 936_221, 200, 0, false);
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
    game.spawn_unit("settler", 0, home);
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

/// The two genes this one rides on, as the live seat arms them.
fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_builders_cover_the_worked_backlog();
    ai.enable_backlog_builder_takes_the_queue();
    ai
}

fn first() -> AdvancedAi {
    let mut ai = armed();
    ai.enable_the_worked_backlog_gets_its_builder_first();
    ai
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn builders_at_head(game: &Game) -> usize {
    game.player_city_ids(0)
        .into_iter()
        .filter(|cid| game.cities[cid].queue.first() == Some(&unit("builder")))
        .count()
}

fn claimed_city(game: &Game) -> u32 {
    game.player_city_ids(0)
        .into_iter()
        .find(|cid| game.cities[cid].queue.first() == Some(&unit("builder")))
        .expect("a city trains the Builder")
}

/// Give a Builder of `charges` charges to the capital.
fn with_charges(game: &mut Game, capital: u32, charges: i32) {
    let home = game.cities[&capital].pos;
    let uid = game.spawn_unit("builder", 0, home);
    game.units.get_mut(&uid).unwrap().charges = charges;
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(
        !ai.base.the_worked_backlog_gets_its_builder_first,
        "an opt-in ships off"
    );
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "the_worked_backlog_gets_its_builder_first"));
    opt_in_off_in_both_controllers("the-worked-backlog-gets-its-builder-first", |ai| {
        ai.base.the_worked_backlog_gets_its_builder_first
    });
    assert!(
        !ai.base.worked_backlog_runs_ahead(10, 0),
        "off: never ahead"
    );
    let on = first();
    assert!(on.base.worked_backlog_runs_ahead(10, 0));
    assert!(on.base.worked_backlog_runs_ahead(5, 3), "two short");
    assert!(!on.base.worked_backlog_runs_ahead(5, 4), "one short");
    assert!(!on.base.worked_backlog_runs_ahead(2, 6), "covered");
}

/// Charges at the stock 60% cover stop the stock claim. With two or more
/// worked tiles still uncovered the gene's quota calls the Builder, the
/// governor holds it, and the claim names the gene.
#[test]
fn a_backlog_past_the_stock_cover_still_calls_its_builder() {
    let (mut game, capital, _second) = backlogged_empire();
    let backlog = BasicAi::unimproved_worked_tiles(&game, 0);
    let stock_quota = (backlog as f64 * BasicAi::WORKED_BACKLOG_COVER).ceil() as i32;
    assert!(
        backlog as i64 - i64::from(stock_quota) >= BACKLOG_FIRST_UNCOVERED_TILES,
        "fixture: the stock cover {stock_quota} leaves two of {backlog} uncovered"
    );
    with_charges(&mut game, capital, stock_quota);

    let stock = armed();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_backlog_builder(&mut off, 0, &plan);
    assert_eq!(builders_at_head(&off), 0, "off: the stock cover is met");
    stock.claim_worked_backlog_builder_first(&mut off, 0, &plan);
    assert_eq!(builders_at_head(&off), 0, "off: the early claim is a no-op");

    let ai = first();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_backlog_builder(&mut on, 0, &plan);
    assert_eq!(builders_at_head(&on), 1, "on: the uncovered tiles call one");
    let claimed = claimed_city(&on);
    let plan = ai.assess(&on, 0);
    assert!(
        ai.backlog_builder_holds(&on, 0, claimed, &plan, &unit("builder")),
        "on: the governor keeps the Builder the quota wanted"
    );
    assert!(
        !stock.backlog_builder_holds(&on, 0, claimed, &plan, &unit("builder")),
        "off: the stock quota alone would not hold it"
    );
    let cover = BasicAi::builder_charge_cover(&game, 0);
    let note = ai
        .worked_backlog_first_note(backlog, cover)
        .expect("on: the claim names the gene");
    assert!(note.starts_with("the-worked-backlog-gets-its-builder-first:"));
    assert!(note.ends_with("would not ask"), "{note}");
    assert!(stock.worked_backlog_first_note(backlog, cover).is_none());

    // One tile short of the charges is covered: no Builder either way.
    let (mut covered, capital, _second) = backlogged_empire();
    with_charges(&mut covered, capital, backlog as i32 - 1);
    assert_eq!(BasicAi::unimproved_worked_tiles(&covered, 0), backlog);
    let plan = ai.assess(&covered, 0);
    ai.claim_backlog_builder(&mut covered, 0, &plan);
    ai.claim_worked_backlog_builder_first(&mut covered, 0, &plan);
    assert_eq!(builders_at_head(&covered), 0, "one tile of slack");
}

/// Fewer than `WORKED_BACKLOG_MIN_TILES` unimproved worked tiles and no
/// charge: the stock step asks for nothing; the gene asks for a Builder.
#[test]
fn a_short_backlog_with_no_charge_calls_its_builder() {
    let (mut game, capital, second) = backlogged_empire();
    // Mine owned hills one at a time until fewer than the stock minimum of
    // worked tiles stand unimproved; each Mine removes at most one.
    let owned: Vec<Pos> = [capital, second]
        .into_iter()
        .flat_map(|cid| {
            let center = game.cities[&cid].pos;
            game.cities[&cid]
                .owned_tiles
                .iter()
                .copied()
                .filter(move |pos| *pos != center)
                .collect::<Vec<_>>()
        })
        .collect();
    for pos in owned {
        if BasicAi::unimproved_worked_tiles(&game, 0) < BasicAi::WORKED_BACKLOG_MIN_TILES {
            break;
        }
        if game.map.tiles[&pos].district.is_none() {
            game.map.tiles.get_mut(&pos).unwrap().improvement = Some(crate::name!("mine"));
        }
    }
    let backlog = BasicAi::unimproved_worked_tiles(&game, 0);
    assert!(
        backlog < BasicAi::WORKED_BACKLOG_MIN_TILES
            && backlog as i64 >= BACKLOG_FIRST_UNCOVERED_TILES,
        "fixture: a short backlog of {backlog}"
    );
    assert_eq!(BasicAi::builder_charge_cover(&game, 0), 0);

    let stock = armed();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_backlog_builder(&mut off, 0, &plan);
    assert_eq!(builders_at_head(&off), 0, "off: below the stock minimum");

    let ai = first();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_backlog_builder(&mut on, 0, &plan);
    assert_eq!(builders_at_head(&on), 1, "on: the short backlog calls one");
}

/// The early claim takes one queue ahead of the later claims while the
/// backlog runs ahead, keeps the heads the stock claim keeps, and is a no-op
/// with the gene off.
#[test]
fn the_early_claim_takes_one_queue_and_keeps_the_held_heads() {
    let (game, _capital, _second) = backlogged_empire();
    assert_eq!(BasicAi::builder_charge_cover(&game, 0), 0);

    let stock = armed();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_worked_backlog_builder_first(&mut off, 0, &plan);
    assert_eq!(builders_at_head(&off), 0, "off: no early claim");

    let ai = first();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_worked_backlog_builder_first(&mut on, 0, &plan);
    assert_eq!(builders_at_head(&on), 1, "one Builder per early pass");
    let claimed = claimed_city(&on);
    let other = on
        .player_city_ids(0)
        .into_iter()
        .find(|cid| *cid != claimed)
        .unwrap();
    assert_eq!(
        on.cities[&other].queue.first(),
        Some(&unit("warrior")),
        "the other city keeps its Warrior"
    );

    // A Settler, a Trader and Walls keep their queues, as under the stock claim.
    let walls = Item::Building {
        building: crate::name!("walls"),
    };
    for keep in [unit("settler"), unit("trader"), walls] {
        let mut busy = game.clone();
        for cid in busy.player_city_ids(0) {
            busy.cities.get_mut(&cid).unwrap().queue = vec![keep.clone()];
        }
        let plan = ai.assess(&busy, 0);
        ai.claim_worked_backlog_builder_first(&mut busy, 0, &plan);
        assert_eq!(builders_at_head(&busy), 0, "{keep:?} stays");
    }

    // Without `backlog-builder-takes-the-queue` it rides on nothing.
    let mut alone = AdvancedAi::new();
    alone.enable_builders_cover_the_worked_backlog();
    alone.enable_the_worked_backlog_gets_its_builder_first();
    let mut bare = game.clone();
    let plan = alone.assess(&bare, 0);
    alone.claim_worked_backlog_builder_first(&mut bare, 0, &plan);
    assert_eq!(builders_at_head(&bare), 0, "no claim without its carrier");
}
