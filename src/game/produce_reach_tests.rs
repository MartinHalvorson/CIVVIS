//! Kept city yields across a `Produce`: what it drops and what it leaves.
//! See the `Produce` note in `Game::apply`.

use super::*;

/// Three cities for seat 0 and one for seat 1, with seat 0 to act, keeping
/// both seats' yields.
fn empire() -> (Game, [u32; 3], u32) {
    let mut game = Game::new_full(2, 30, 20, 91_171, 100, 0, false);
    let land: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.unit_ids_at(**pos).is_empty()
        })
        .map(|(pos, _)| *pos)
        .collect();
    let mut sites: Vec<Pos> = Vec::new();
    for pos in land {
        if sites.iter().all(|site| game.wdist(*site, pos) >= 5) {
            sites.push(pos);
        }
        if sites.len() == 4 {
            break;
        }
    }
    let ours = [
        game.found_city_for(0, sites[0], None),
        game.found_city_for(0, sites[1], None),
        game.found_city_for(0, sites[2], None),
    ];
    let theirs = game.found_city_for(1, sites[3], None);
    game.current = 0;
    game.keep_rival_yields(true);
    game.keep_own_yields(true);
    (game, ours, theirs)
}

fn kept(game: &Game) -> (Vec<u32>, Vec<u32>) {
    let keys = |held: &std::cell::RefCell<KeptDerivations>| {
        held.borrow().yields.keys().copied().collect::<Vec<_>>()
    };
    (
        keys(&game.query_memo.rival_yields),
        keys(&game.query_memo.own_yields),
    )
}

fn read_all(game: &Game, ours: [u32; 3], theirs: u32) -> Vec<Yields> {
    let mut all: Vec<Yields> = ours.iter().map(|cid| game.city_yields(*cid)).collect();
    all.push(game.city_yields(theirs));
    all
}

#[test]
fn a_produce_drops_only_its_own_city() {
    let (mut game, ours, theirs) = empire();
    let before = read_all(&game, ours, theirs);
    let mut own = ours.to_vec();
    own.sort();
    assert_eq!(kept(&game), (vec![theirs], own.clone()));

    // A Warrior needs no strategic resource and places nothing.
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    game.apply(
        0,
        &Action::Produce {
            city: ours[1],
            item: warrior,
        },
    )
    .expect("every city can train a Warrior");
    let mut others = vec![ours[0], ours[2]];
    others.sort();
    assert_eq!(kept(&game), (vec![theirs], others));

    // The others answer from what was kept, the producer is derived again,
    // and every answer is the one a fresh derivation gives.
    let (reused, derived) = game.rival_yields_reused();
    let after = read_all(&game, ours, theirs);
    assert_eq!(game.rival_yields_reused(), (reused + 3, derived + 1));
    let mut fresh = game.clone();
    fresh.keep_rival_yields(false);
    assert_eq!(after, read_all(&fresh, ours, theirs));
    assert_eq!(after[0], before[0]);
    assert_eq!(after[2], before[2]);
    assert_eq!(after[3], before[3]);
}

#[test]
fn a_produce_that_takes_from_the_stockpile_drops_every_own_city() {
    let (mut game, ours, theirs) = empire();
    // Every technology and a deep stockpile of every strategic resource, so
    // some unit the city can train takes from it when it is queued.
    let techs: Vec<Name> = game.rules.techs.keys().copied().collect();
    game.players[0].techs.extend(techs);
    let mut needing: Vec<(Name, Name)> = game
        .rules
        .units
        .iter()
        .filter_map(|(unit, spec)| Some((*unit, spec.requires_resource?)))
        .collect();
    needing.sort();
    for (_, resource) in &needing {
        game.players[0].strategic_resources.insert(*resource, 500.0);
    }
    game.board_written();
    let (unit, resource) = needing
        .into_iter()
        .find(|(unit, _)| game.can_produce(0, ours[1], &Item::Unit { unit: *unit }))
        .expect("some unit that takes a strategic resource can be trained");
    read_all(&game, ours, theirs);
    let stock = game.players[0].strategic_resources[&resource];
    game.apply(
        0,
        &Action::Produce {
            city: ours[1],
            item: Item::Unit { unit },
        },
    )
    .expect("the city can train it");
    assert!(
        game.players[0].strategic_resources[&resource] < stock,
        "{} took nothing from the stockpile",
        unit.as_str()
    );
    assert_eq!(kept(&game), (vec![theirs], vec![]));
}

/// A project can power its city, and a powered city's Factory reaches its
/// neighbours' production: replacing one keeps the old, empire-wide reach.
#[test]
fn a_produce_that_replaces_a_project_drops_every_own_city() {
    let (mut game, ours, theirs) = empire();
    let project = *game
        .rules
        .projects
        .keys()
        .find(|project| project.as_str() == "industrial_zone_logistics")
        .expect("the ruleset has Industrial Zone Logistics");
    game.cities.get_mut(&ours[1]).unwrap().queue = vec![Item::Project { project }];
    game.board_written();
    read_all(&game, ours, theirs);
    game.apply(
        0,
        &Action::Produce {
            city: ours[1],
            item: Item::Unit {
                unit: crate::name!("warrior"),
            },
        },
    )
    .expect("every city can train a Warrior");
    assert_eq!(kept(&game), (vec![theirs], vec![]));
}
