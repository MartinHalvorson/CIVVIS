use super::super::near_rival_deterrence::DETERRENCE_END_STANDARD;
use super::*;

/// Two majors who have met: our founded capital (population 4, idle), the
/// rival's city `gap` tiles off, every unit removed, everything within nine
/// tiles of the capital in sight, the rival at 4 times our power with a
/// Warrior five tiles from the capital, at standard turn 20 (between this
/// gene's start and the opening gene's). Returns the game and our capital.
fn early_board(gap: std::ops::RangeInclusive<i32>) -> (Game, u32) {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a Settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital");
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| gap.contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(1, sites[0], None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 4;
    city.queue.clear();
    game.turn = game.standard_duration(20);
    game.host_observed = Arc::new(game.wdisk(home, 9).into_iter().collect());
    game.players[0].techs.insert(crate::name!("bronze_working"));
    power(&mut game, 0, 50.0);
    power(&mut game, 1, 200.0);
    let at = ground(&game, capital, 5);
    game.spawn_unit("warrior", 1, at);
    (game, capital)
}

fn power(game: &mut Game, pid: usize, value: f64) {
    Arc::make_mut(&mut game.observed_military_power).insert(pid, value);
}

/// A land tile exactly `gap` tiles from the capital not yet holding a unit.
fn ground(game: &Game, capital: u32, gap: i32) -> Pos {
    let home = game.cities[&capital].pos;
    let mut tiles: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.wdist(**pos, home) == gap
                && !game.rules.is_water(tile)
                && !game.units.values().any(|unit| unit.pos == **pos)
        })
        .map(|(pos, _)| *pos)
        .collect();
    tiles.sort();
    tiles[0]
}

fn opening() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_near_rival_deterrence();
    ai
}

fn armed() -> AdvancedAi {
    let mut ai = opening();
    ai.enable_near_rival_deterrence_opens_early();
    ai.base.journal = crate::reasoning::Journal::recording();
    ai
}

fn details(ai: &AdvancedAi) -> Vec<String> {
    ai.base
        .journal
        .since(0)
        .thoughts
        .into_iter()
        .map(|thought| thought.detail)
        .collect()
}

fn land_units_queued(game: &Game) -> usize {
    game.player_city_ids(0)
        .into_iter()
        .filter(|cid| {
            matches!(
                game.cities[cid].queue.first(),
                Some(Item::Unit { unit }) if AdvancedAi::deterrence_land_unit(game, unit)
            )
        })
        .count()
}

/// The live case (Sweden t12-45, Sumer t21-50): a far rival's soldier at the
/// capital before turn 30. The opening gene alone reads nothing; with this
/// gene the watch records the soldier, the rival is a neighbour, one land
/// unit is queued and held, and the journal names this gene.
#[test]
fn a_soldier_at_the_door_opens_the_deterrent_before_its_window() {
    let (game, capital) = early_board(12..=14);

    let mut alone = opening();
    alone.observe_turn_start_hostiles(&game, 0);
    assert!(
        alone.capital_watch_sightings.is_empty(),
        "the opening gene alone keeps no watch"
    );
    assert!(alone.deterrence_threat(&game, 0).is_none(), "before t30");
    let mut off = game.clone();
    let plan = alone.assess(&off, 0);
    alone.claim_deterrence_unit(&mut off, 0, &plan);
    assert_eq!(land_units_queued(&off), 0, "off: no claim");

    let mut ai = armed();
    assert!(ai.deterrence_threat(&game, 0).is_none(), "no sighting yet");
    ai.observe_turn_start_hostiles(&game, 0);
    assert_eq!(ai.capital_watch_sightings.get(&1), Some(&game.turn));
    let (rival, theirs) = ai.deterrence_threat(&game, 0).expect("the door reads");
    assert_eq!(rival, 1);
    assert!((theirs - 200.0).abs() < 1e-9);
    let reason = ai
        .deterrence_opens_early_reason(&game, 0, 1)
        .expect("this gene armed it");
    assert!(reason.contains("its soldier stood within 8 tiles"), "{reason}");
    assert!(reason.contains("before the opening window"), "{reason}");

    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_deterrence_unit(&mut on, 0, &plan);
    assert_eq!(land_units_queued(&on), 1, "one land unit at the door");
    let head = on.cities[&capital].queue.first().cloned().unwrap();
    assert!(
        ai.deterrence_unit_holds(&on, 0, &head),
        "the deterrent is held while out-gunned"
    );
    assert!(
        details(&ai)
            .iter()
            .any(|line| line.starts_with("near-rival-deterrence-opens-early:")),
        "the journal names the gene"
    );

    let mut level = game.clone();
    power(&mut level, 0, 200.0);
    let plan = ai.assess(&level, 0);
    ai.claim_deterrence_unit(&mut level, 0, &plan);
    assert_eq!(land_units_queued(&level), 0, "level power: no claim");
}

/// A known city within ten tiles opens the window early too, with no soldier
/// in sight; inside the opening gene's own window that city is the opening
/// gene's and the journal reason is `None`.
#[test]
fn a_near_city_opens_the_window_early() {
    let (mut game, _capital) = early_board(6..=10);
    game.units.retain(|_, unit| unit.owner != 1);
    let alone = opening();
    assert!(alone.deterrence_threat(&game, 0).is_none(), "before t30");
    let ai = armed();
    assert_eq!(ai.deterrence_threat(&game, 0).map(|(rival, _)| rival), Some(1));
    let reason = ai.deterrence_opens_early_reason(&game, 0, 1).unwrap();
    assert!(reason.starts_with("a city within 10 tiles"), "{reason}");

    let mut window = game.clone();
    window.turn = window.standard_duration(DETERRENCE_START_STANDARD) + 1;
    assert!(alone.deterrence_threat(&window, 0).is_some());
    assert!(ai.deterrence_opens_early_reason(&window, 0, 1).is_none());
}

/// The bounds: the opening gene off, before live turn 10, a sighting older
/// than the watch's window, at war, and past the opening window (the late
/// gene reads known cities only) read nothing.
#[test]
fn the_door_stays_inside_the_opening_window() {
    let (game, _capital) = early_board(12..=14);
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    assert!(ai.deterrence_threat(&game, 0).is_some(), "fixture: the door reads");

    let mut lone = AdvancedAi::new();
    lone.enable_near_rival_deterrence_opens_early();
    lone.observe_turn_start_hostiles(&game, 0);
    assert!(
        lone.deterrence_threat(&game, 0).is_none(),
        "the opening gene off: no deterrent"
    );

    let mut too_soon = game.clone();
    too_soon.turn = too_soon.standard_duration(EARLY_DETERRENCE_START_STANDARD) - 1;
    let mut soon = armed();
    soon.observe_turn_start_hostiles(&too_soon, 0);
    assert!(soon.deterrence_threat(&too_soon, 0).is_none(), "before turn 10");

    let mut stale = game.clone();
    stale.turn += stale.standard_duration(CAPITAL_WATCH_STANDARD) + 1;
    assert!(ai.deterrence_threat(&stale, 0).is_none(), "an old sighting");

    let mut war = game.clone();
    war.at_war.insert((0, 1));
    assert!(ai.deterrence_threat(&war, 0).is_none(), "at war: the war's");

    let mut late_ai = armed();
    late_ai.enable_near_rival_deterrence_holds_late();
    let mut late = game.clone();
    late.turn = late.standard_duration(DETERRENCE_END_STANDARD) + 1;
    late_ai.observe_turn_start_hostiles(&late, 0);
    assert_eq!(late_ai.capital_watch_sightings.get(&1), Some(&late.turn));
    assert!(
        late_ai.deterrence_threat(&late, 0).is_none(),
        "past the window the door is no neighbour"
    );
}
