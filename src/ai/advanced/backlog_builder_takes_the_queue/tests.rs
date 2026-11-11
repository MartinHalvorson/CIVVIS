use super::*;

/// A founded capital and a second city four or more tiles away, every owned
/// plot a bare grassland hill (a Mine apiece with Mining), four citizens a
/// city, a Settler already walking (no city is due another one), no Builder,
/// and both queues training a routine Warrior.
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

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_builders_cover_the_worked_backlog();
    ai.enable_backlog_builder_takes_the_queue();
    ai
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn heads(game: &Game) -> Vec<Option<Item>> {
    game.player_city_ids(0)
        .into_iter()
        .map(|cid| game.cities[&cid].queue.first().cloned())
        .collect()
}

fn builders_at_head(game: &Game) -> usize {
    heads(game)
        .iter()
        .filter(|head| **head == Some(unit("builder")))
        .count()
}

/// The blocked case: both cities train a routine Warrior at peace while the
/// worked backlog has no charge against it. Off (and with only the backlog
/// gate's idle-queue claim armed), nothing moves; on, one city puts a Builder
/// first per pass, the governor keeps it while the backlog still wants it,
/// and once the queued charges cover the backlog no further Builder comes.
#[test]
fn a_backlog_claims_a_builder_over_a_routine_unit() {
    let (game, _capital, _second) = backlogged_empire();
    let backlog = BasicAi::unimproved_worked_tiles(&game, 0);
    assert!(backlog >= 4, "fixture backlog {backlog}");
    assert_eq!(
        BasicAi::builder_charge_cover(&game, 0),
        0,
        "fixture: no charges"
    );

    let stock = AdvancedAi::new();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_backlog_builder(&mut off, 0, &plan);
    assert_eq!(builders_at_head(&off), 0, "off: no claim");

    let mut idle_only = AdvancedAi::new();
    idle_only.enable_builders_cover_the_worked_backlog();
    let mut busy = game.clone();
    let plan = idle_only.assess(&busy, 0);
    idle_only.claim_idle_queue_for_worked_backlog(&mut busy, 0, &plan);
    assert_eq!(
        builders_at_head(&busy),
        0,
        "the idle-queue claim cannot reach a busy queue"
    );

    let ai = armed();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_backlog_builder(&mut on, 0, &plan);
    assert_eq!(builders_at_head(&on), 1, "one Builder per pass");
    let claimed = on
        .player_city_ids(0)
        .into_iter()
        .find(|cid| on.cities[cid].queue.first() == Some(&unit("builder")))
        .unwrap();
    assert!(
        heads(&on).contains(&Some(unit("warrior"))),
        "the other city keeps its Warrior: one claim a pass"
    );
    let plan = ai.assess(&on, 0);
    assert!(
        ai.backlog_builder_holds(&on, 0, claimed, &plan, &unit("builder")),
        "the governor keeps the claimed Builder while the backlog wants it"
    );
    assert!(
        !stock.backlog_builder_holds(&on, 0, claimed, &plan, &unit("builder")),
        "off: no hold"
    );

    // Charges in hand that cover the backlog stop the claim.
    let mut covered = game.clone();
    let home = covered.cities[&claimed].pos;
    let uid = covered.spawn_unit("builder", 0, home);
    covered.units.get_mut(&uid).unwrap().charges = backlog as i32;
    let plan = ai.assess(&covered, 0);
    ai.claim_backlog_builder(&mut covered, 0, &plan);
    assert_eq!(
        builders_at_head(&covered),
        0,
        "a covered backlog claims nothing"
    );
}

/// A Settler, a Builder, a Trader, Walls and a freshly attacked city keep
/// their queues; at war with a major the gate stands down for a short army.
#[test]
fn settlers_traders_walls_and_attacked_cities_keep_their_queues() {
    let (game, _capital, _second) = backlogged_empire();
    let ai = armed();
    let walls = Item::Building {
        building: crate::name!("walls"),
    };
    for keep in [unit("settler"), unit("trader"), walls] {
        let mut busy = game.clone();
        for cid in busy.player_city_ids(0) {
            busy.cities.get_mut(&cid).unwrap().queue = vec![keep.clone()];
        }
        let plan = ai.assess(&busy, 0);
        ai.claim_backlog_builder(&mut busy, 0, &plan);
        assert!(
            heads(&busy).iter().all(|head| *head == Some(keep.clone())),
            "{keep:?} stays"
        );
    }
    let mut attacked = game.clone();
    let turn = attacked.turn.max(1);
    for cid in attacked.player_city_ids(0) {
        attacked.cities.get_mut(&cid).unwrap().last_attacked = turn;
    }
    let plan = ai.assess(&attacked, 0);
    ai.claim_backlog_builder(&mut attacked, 0, &plan);
    assert_eq!(
        builders_at_head(&attacked),
        0,
        "attacked cities keep their queues"
    );
}

/// The gene is registered and off by default.
#[test]
fn the_gene_is_registered_and_off_by_default() {
    let gene = crate::ai::advanced::gene("backlog-builder-takes-the-queue").expect("registered");
    assert_eq!(gene.field, "backlog_builder_takes_the_queue");
    assert!(!AdvancedAi::new().backlog_builder_takes_the_queue);
    assert!(armed().backlog_builder_takes_the_queue);
}

/// `prod-tiles-call-the-builders`: charges that meet the stock 60% cover
/// stop the stock claim. Under the gene the bare worked hills still call a
/// Builder, and the governor holds it under the same quota.
#[test]
fn production_ground_claims_and_holds_past_the_stock_cover() {
    let (mut game, capital, _second) = backlogged_empire();
    let backlog = BasicAi::unimproved_worked_tiles(&game, 0);
    let stock_quota = (backlog as f64 * BasicAi::WORKED_BACKLOG_COVER).ceil() as i32;
    assert!(
        stock_quota < backlog as i32,
        "the fixture's stock cover sits below the backlog: {stock_quota} of {backlog}"
    );
    let home = game.cities[&capital].pos;
    let uid = game.spawn_unit("builder", 0, home);
    game.units.get_mut(&uid).unwrap().charges = stock_quota;

    let stock = armed();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_backlog_builder(&mut off, 0, &plan);
    assert_eq!(builders_at_head(&off), 0, "the stock cover is met");

    let mut ai = armed();
    ai.enable_prod_tiles_call_the_builders();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_backlog_builder(&mut on, 0, &plan);
    assert_eq!(builders_at_head(&on), 1, "the production ground calls one");
    let claimed = on
        .player_city_ids(0)
        .into_iter()
        .find(|cid| on.cities[cid].queue.first() == Some(&unit("builder")))
        .unwrap();
    let plan = ai.assess(&on, 0);
    assert!(
        ai.backlog_builder_holds(&on, 0, claimed, &plan, &unit("builder")),
        "the governor keeps the Builder the gene claimed"
    );
    assert!(
        !stock.backlog_builder_holds(&on, 0, claimed, &plan, &unit("builder")),
        "the stock quota alone would not hold it"
    );
}

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

/// `science-heads-keep-their-queue`: a Campus-family district (Korea's
/// Seowon too) and its buildings are science, matched by family.
#[test]
fn science_heads_are_the_campus_family() {
    let (game, capital, _second) = backlogged_empire();
    let pos = game.cities[&capital].pos;
    let district = |name: &str| Item::District {
        district: Name::new(name),
        pos,
    };
    for item in [
        building("library"),
        building("university"),
        building("research_lab"),
        district("campus"),
        district("seowon"),
    ] {
        assert!(
            AdvancedAi::science_head(&game, &item),
            "{item:?} is science"
        );
    }
    for item in [
        building("monument"),
        building("workshop"),
        district("theater_square"),
        unit("warrior"),
    ] {
        assert!(!AdvancedAi::science_head(&game, &item), "{item:?} is not");
    }
}

/// Science in progress keeps its queue: with both cities on a Library the
/// guarded claim waits rather than displace one, and with one city on a
/// Library and the other on a Warrior it takes the Warrior's queue. Off,
/// the stock claim takes a Library's queue.
#[test]
fn the_backlog_builder_spares_science_in_progress() {
    let (mut game, capital, second) = backlogged_empire();
    for cid in [capital, second] {
        game.cities.get_mut(&cid).unwrap().queue = vec![building("library")];
    }
    let stock = armed();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_backlog_builder(&mut off, 0, &plan);
    assert_eq!(
        builders_at_head(&off),
        1,
        "off: the claim takes a Library's queue"
    );

    let mut guarded = armed();
    guarded.enable_science_heads_keep_their_queue();
    let mut both = game.clone();
    let plan = guarded.assess(&both, 0);
    guarded.claim_backlog_builder(&mut both, 0, &plan);
    assert_eq!(
        builders_at_head(&both),
        0,
        "every city on science: the claim waits"
    );
    assert!(heads(&both)
        .iter()
        .all(|head| *head == Some(building("library"))));

    let mut mixed = game.clone();
    mixed.cities.get_mut(&second).unwrap().queue = vec![unit("warrior")];
    let plan = guarded.assess(&mixed, 0);
    guarded.claim_backlog_builder(&mut mixed, 0, &plan);
    assert_eq!(builders_at_head(&mixed), 1);
    assert_eq!(
        mixed.cities[&capital].queue.first(),
        Some(&building("library")),
        "the Library keeps its queue"
    );
    assert_eq!(mixed.cities[&second].queue.first(), Some(&unit("builder")));
}

/// One pass over the cities takes the empire-wide backlog census once, not
/// once per city, and both idle-queue passes do.
///
/// Every city of the pass reaches `worked_backlog_builder`, which counts the
/// unimproved worked tiles of the whole empire to decide about one city. With
/// no query scope around the loop each of those counts was taken afresh: C
/// cities, C censuses, C x C citizen assignments. The answer was never wrong,
/// which is why nothing but a profile could see it.
#[test]
fn a_pass_over_the_cities_counts_the_backlog_once() {
    let (game, _capital, _second) = backlogged_empire();
    let ai = armed();
    let plan = ai.assess(&game, 0);
    let unscoped = {
        let before = Game::worked_backlog_censuses();
        for cid in game.player_city_ids(0) {
            assert!(ai.backlog_builder_item(&game, 0, cid, &plan).is_some());
        }
        Game::worked_backlog_censuses() - before
    };
    assert!(
        unscoped >= 2,
        "fixture: both cities reach the census, and unscoped each takes its own ({unscoped})"
    );
    let before = Game::worked_backlog_censuses();
    let candidate = ai.backlog_builder_claim_candidate(&game, 0, &plan, None);
    assert!(candidate.is_some(), "fixture: the pass names a city");
    assert_eq!(Game::worked_backlog_censuses() - before, 1);

    // The idle-queue claim's own pass, over two idle queues.
    let mut idle = game.clone();
    for cid in idle.player_city_ids(0) {
        idle.cities.get_mut(&cid).unwrap().queue.clear();
    }
    let mut idle_only = AdvancedAi::new();
    idle_only.enable_builders_cover_the_worked_backlog();
    let plan = idle_only.assess(&idle, 0);
    let before = Game::worked_backlog_censuses();
    idle_only.claim_idle_queue_for_worked_backlog(&mut idle, 0, &plan);
    assert_eq!(
        builders_at_head(&idle),
        1,
        "fixture: an idle queue is claimed"
    );
    assert_eq!(
        Game::worked_backlog_censuses() - before,
        1,
        "two idle cities, one census"
    );
}
