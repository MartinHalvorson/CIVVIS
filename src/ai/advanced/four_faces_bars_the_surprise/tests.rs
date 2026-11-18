use super::*;
use crate::ai::advanced::near_rival_deterrence::DETERRENCE_START_STANDARD;
use crate::ai::garrison_before_the_surprise::FOUR_FACES_CIVILIZATION;

const CAPITAL: Pos = (12, 10);
const FRONTIER: Pos = (24, 10);
const RIVAL_CITY: Pos = (33, 10);
/// Six tiles from our capital and six from the frontier city.
const AT_THE_CAPITAL: Pos = (18, 10);
/// Seven tiles from the frontier city, past the capital's watch.
const AT_THE_FRONTIER: Pos = (31, 10);

/// A flat board inside the deterrence window: our capital and an unwalled
/// frontier city, a met major at peace with a city nine tiles from the
/// frontier and 2.5 times our power, one of its Warriors in sight at each of
/// our cities. Returns the game and our frontier city.
fn board() -> (Game, u32) {
    let mut game = Game::new_full(2, 48, 24, 4063, 650, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.clear_mirror_cities();
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let capital = game.found_city_for(0, CAPITAL, None);
    let frontier = game.found_city_for(0, FRONTIER, None);
    game.found_city_for(1, RIVAL_CITY, None);
    assert!(game.cities[&capital].is_capital);
    assert_eq!(game.wdist(FRONTIER, RIVAL_CITY), 9);
    game.current = 0;
    game.turn = game.standard_duration(DETERRENCE_START_STANDARD) + 5;
    game.record_contact(0, 1);
    Arc::make_mut(&mut game.observed_military_power).insert(0, 100.0);
    Arc::make_mut(&mut game.observed_military_power).insert(1, 250.0);
    assert_eq!(game.wdist(CAPITAL, AT_THE_CAPITAL), 6);
    assert_eq!(game.wdist(FRONTIER, AT_THE_FRONTIER), 7);
    assert!(game.wdist(CAPITAL, AT_THE_FRONTIER) > 8);
    game.spawn_test_unit("warrior", 1, AT_THE_CAPITAL);
    game.spawn_test_unit("warrior", 1, AT_THE_FRONTIER);
    game.host_observed = Arc::new(
        game.wdisk(CAPITAL, 9)
            .into_iter()
            .chain(game.wdisk(FRONTIER, 8))
            .collect(),
    );
    for seat in [0, 1] {
        assert_ne!(game.players[seat].civ, FOUR_FACES_CIVILIZATION);
    }
    (game, frontier)
}

fn canada(game: &mut Game, seat: usize) {
    game.players[seat].civ = FOUR_FACES_CIVILIZATION.to_string();
    assert!(game.uses_civ6_content(seat), "fixture: Canada is seated");
}

/// Every peacetime surprise reader armed, with or without the gene.
fn armed(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_near_rival_deterrence();
    ai.enable_capital_walls_before_the_surprise();
    ai.enable_frontier_walls_before_the_surprise();
    ai.enable_garrison_before_the_surprise();
    if gene {
        ai.enable_four_faces_bars_the_surprise();
    }
    ai
}

/// What the readers say of the rival this turn: the deterrence neighbour,
/// the capital's wall threat, the frontier's wall threat, the soldiers that
/// call a garrison, and the cities they call it to.
#[derive(Debug, PartialEq)]
struct Reading {
    deterrence: bool,
    capital_walls: bool,
    frontier_walls: bool,
    soldiers: usize,
    called_cities: usize,
}

const ALL: Reading = Reading {
    deterrence: true,
    capital_walls: true,
    frontier_walls: true,
    soldiers: 2,
    called_cities: 2,
};

const NONE: Reading = Reading {
    deterrence: false,
    capital_walls: false,
    frontier_walls: false,
    soldiers: 0,
    called_cities: 0,
};

fn read(gene: bool, game: &Game) -> Reading {
    let mut ai = armed(gene);
    ai.remember_capital_watch_sightings(game, 0);
    ai.remember_frontier_watch_sightings(game, 0);
    Reading {
        deterrence: ai
            .deterrence_threat(game, 0)
            .is_some_and(|(rival, _)| rival == 1),
        capital_walls: ai.capital_walls_threat(game, 0).is_some(),
        frontier_walls: ai.frontier_walls_research_threat(game, 0).is_some(),
        soldiers: ai.base.surprise_soldiers(game, 0).len(),
        called_cities: ai.base.garrison_called_cities(game, 0).len(),
    }
}

/// The blocked case, the live Kongo game: a stronger Canada beside us arms
/// every reader with the gene off and none with it on.
#[test]
fn canada_beside_us_arms_no_surprise_reader() {
    let (mut game, _frontier) = board();
    assert_eq!(read(false, &game), ALL, "fixture: every reader arms");
    assert_eq!(read(true, &game), ALL, "on: any other rival is still read");

    canada(&mut game, 1);
    assert_eq!(
        read(false, &game),
        ALL,
        "off: Canada is read like any rival"
    );
    assert_eq!(read(true, &game), NONE, "on: Canada cannot surprise us");
}

/// The other half, the live Canada game: when we play Canada no rival can
/// open by surprise, so no reader arms against it.
#[test]
fn when_we_are_canada_no_rival_arms_a_surprise_reader() {
    let (mut game, _frontier) = board();
    canada(&mut game, 0);
    assert_eq!(read(false, &game), ALL);
    assert_eq!(read(true, &game), NONE);
}

/// A denouncement in either direction puts the pair in the state every casus
/// belli needs, and the readers arm again that turn; when it lapses they
/// stand down again.
#[test]
fn a_denouncement_either_way_arms_the_readers_again() {
    let (mut game, _frontier) = board();
    canada(&mut game, 1);
    let turn = game.turn;

    let mut theirs = game.clone();
    theirs.players[1].denounced_until.insert(0, turn + 20);
    assert_eq!(read(true, &theirs), ALL, "Canada has denounced us");

    let mut ours = game.clone();
    ours.players[0].denounced_until.insert(1, turn + 20);
    assert_eq!(read(true, &ours), ALL, "we have denounced Canada");

    let mut lapsed = game.clone();
    lapsed.players[1].denounced_until.insert(0, turn);
    assert_eq!(read(true, &lapsed), NONE, "a lapsed denouncement");
}

/// A war that is on is never spared: the capital watch, which keeps reading
/// a major at war with us, reads Canada at war as it does with the gene off.
#[test]
fn a_war_that_is_on_is_never_spared() {
    let (mut game, _frontier) = board();
    canada(&mut game, 1);
    let ai = armed(true);
    assert!(ai.four_faces_spares(&game, 0, 1));
    assert!(!armed(false).four_faces_spares(&game, 0, 1), "off");
    assert!(!ai.four_faces_spares(&game, 0, 0), "never ourselves");

    game.at_war.insert((0, 1));
    assert!(game.is_at_war(0, 1), "fixture: the war reads");
    assert!(!ai.four_faces_spares(&game, 0, 1));
    assert!(read(true, &game).capital_walls);
    assert_eq!(
        read(true, &game).capital_walls,
        read(false, &game).capital_walls
    );
}

/// The capital's door leg (two soldiers in sight within six tiles) reads the
/// soldiers themselves, not the watch's memory: against Canada it trains no
/// defender, and it trains one again once Canada has denounced us.
#[test]
fn the_capital_door_trains_no_defender_against_canada() {
    let (mut game, _frontier) = board();
    game.spawn_test_unit("warrior", 1, (17, 10));
    game.players[0].gold = 0.0;
    canada(&mut game, 1);
    let capital = game.player_city_ids(0)[0];
    assert!(game.cities[&capital].is_capital);
    assert!(
        game.cities[&capital].queue.is_empty(),
        "fixture: idle capital"
    );
    let defender_queued = |gene: bool, game: &Game| {
        let mut game = game.clone();
        armed(gene).claim_capital_walls_before_the_surprise(&mut game, 0);
        matches!(
            game.cities[&capital].queue.first(),
            Some(Item::Unit { unit }) if game.rules.units[unit].class == "military"
        )
    };
    assert!(
        defender_queued(false, &game),
        "off: the door trains a defender"
    );
    assert!(!defender_queued(true, &game), "on: none against Canada");

    game.players[1].denounced_until.insert(0, game.turn + 20);
    assert!(
        defender_queued(true, &game),
        "denounced: the door reads again"
    );
}

/// The toggle sets and clears the one flag, which lives on the base agent
/// because the garrison call reads it there.
#[test]
fn the_toggle_sets_the_base_flag() {
    let mut ai = AdvancedAi::new();
    assert!(!ai.base.four_faces_bars_the_surprise, "off by default");
    ai.enable_four_faces_bars_the_surprise();
    assert!(ai.base.four_faces_bars_the_surprise);
    ai.disable_four_faces_bars_the_surprise();
    assert!(!ai.base.four_faces_bars_the_surprise);
}
