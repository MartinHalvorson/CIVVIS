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
    assert_eq!(BasicAi::builder_charge_cover(&game, 0), 0, "fixture: no charges");

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
    assert_eq!(builders_at_head(&busy), 0, "the idle-queue claim cannot reach a busy queue");

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
    assert!(!stock.backlog_builder_holds(&on, 0, claimed, &plan, &unit("builder")), "off: no hold");

    // Charges in hand that cover the backlog stop the claim.
    let mut covered = game.clone();
    let home = covered.cities[&claimed].pos;
    let uid = covered.spawn_unit("builder", 0, home);
    covered.units.get_mut(&uid).unwrap().charges = backlog as i32;
    let plan = ai.assess(&covered, 0);
    ai.claim_backlog_builder(&mut covered, 0, &plan);
    assert_eq!(builders_at_head(&covered), 0, "a covered backlog claims nothing");
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
        assert!(heads(&busy).iter().all(|head| *head == Some(keep.clone())), "{keep:?} stays");
    }
    let mut attacked = game.clone();
    let turn = attacked.turn.max(1);
    for cid in attacked.player_city_ids(0) {
        attacked.cities.get_mut(&cid).unwrap().last_attacked = turn;
    }
    let plan = ai.assess(&attacked, 0);
    ai.claim_backlog_builder(&mut attacked, 0, &plan);
    assert_eq!(builders_at_head(&attacked), 0, "attacked cities keep their queues");
}

/// The gene is registered and off by default.
#[test]
fn the_gene_is_registered_and_off_by_default() {
    let gene = crate::ai::advanced::gene("backlog-builder-takes-the-queue").expect("registered");
    assert_eq!(gene.field, "backlog_builder_takes_the_queue");
    assert!(!AdvancedAi::new().backlog_builder_takes_the_queue);
    assert!(armed().backlog_builder_takes_the_queue);
}
