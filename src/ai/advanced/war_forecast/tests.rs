use super::*;

/// Two met majors, one city each, the rival's within `gap` tiles of ours,
/// every unit removed. Returns the game, our city and our city's tile.
fn neighbours(gap: i32) -> Option<(Game, u32, Pos)> {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")?;
    game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let ours = game.player_city_ids(0)[0];
    let home = game.cities[&ours].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (5..=gap).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(1, *sites.first()?, None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    Some((game, ours, home))
}

fn power(game: &mut Game, pid: usize, value: f64) {
    std::sync::Arc::make_mut(&mut game.observed_military_power).insert(pid, value);
}

/// Land tiles at exactly `ring` tiles from `home`, in a fixed order.
fn ring(game: &Game, home: Pos, ring: i32) -> Vec<Pos> {
    let mut tiles: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| game.wdist(**pos, home) == ring && !game.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    tiles.sort();
    tiles
}

/// The damage curve: an army of Warriors counts its units, and four
/// Pikemen (45) outweigh ten Warriors although their plain power is lower.
#[test]
fn quality_power_follows_the_damage_curve() {
    let warriors = AdvancedAi::quality_power_at(200.0, 20.0);
    assert!((warriors - 10.0).abs() < 1e-9, "ten Warriors count ten");
    let pikemen = AdvancedAi::quality_power_at(180.0, 45.0);
    assert!(
        (pikemen - 4.0 * std::f64::consts::E).abs() < 1e-9,
        "four Pikemen weigh four times e"
    );
    assert!(pikemen > warriors, "quality outweighs plain power");
}

/// A rival army out of sight is read from its technologies; once two of
/// its soldiers are in sight, from them.
#[test]
fn an_unseen_army_is_read_from_its_era() {
    let Some((mut game, _ours, home)) = neighbours(7) else {
        panic!("fixture: two cities within seven tiles");
    };
    game.players[1]
        .techs
        .insert(crate::name!("military_tactics"));
    let era = AdvancedAi::army_quality(&game, 0, 1);
    assert!(
        (era - UNSEEN_ARMY_ERA_SHARE * 45.0).abs() < 1e-9,
        "unseen: {era} is the era share of the Pikeman"
    );
    for pos in ring(&game, home, 1).into_iter().take(2) {
        game.spawn_unit("warrior", 1, pos);
    }
    let seen = AdvancedAi::army_quality(&game, 0, 1);
    assert!((seen - 20.0).abs() < 1e-9, "seen: two Warriors read {seen}");
}

/// Soldiers at our door raise the hazard; a declared friendship lowers it;
/// a rival at war or unmet has no forecast.
#[test]
fn the_hazard_reads_the_door_and_the_friendship() {
    let Some((mut game, _ours, home)) = neighbours(7) else {
        panic!("fixture: two cities within seven tiles");
    };
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 100.0);
    let ai = AdvancedAi::new();
    let quiet = ai
        .threat_forecast(&game, 0, 1)
        .expect("a met major at peace");
    assert!(quiet.nearest <= 7, "fixture: a near neighbour");
    assert_eq!(quiet.massed, 0);

    let mut door = game.clone();
    for pos in ring(&door, home, 2).into_iter().take(3) {
        door.spawn_unit("warrior", 1, pos);
    }
    let massed = ai.threat_forecast(&door, 0, 1).expect("still at peace");
    assert_eq!(massed.massed, 3, "three soldiers at our door");
    assert!(massed.hazard > quiet.hazard, "the door raises the hazard");
    assert!(massed.hazard >= FORECAST_HAZARD_FLOOR);

    let mut friends = door.clone();
    friends.players[0]
        .observed_diplomatic_state
        .insert(1, "DECLARED_FRIEND".to_string());
    let friendly = ai.threat_forecast(&friends, 0, 1).expect("still at peace");
    assert!(
        friendly.hazard < massed.hazard,
        "a friendship lowers the hazard"
    );

    let mut war = game.clone();
    war.at_war.insert((0, 1));
    assert!(
        ai.threat_forecast(&war, 0, 1).is_none(),
        "at war: no forecast"
    );
    let mut unmet = game.clone();
    unmet.players[0].met.remove(&1);
    assert!(
        ai.threat_forecast(&unmet, 0, 1).is_none(),
        "unmet: no forecast"
    );
}
