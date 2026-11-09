use super::*;

const FAITH: &str = "buddhism";

/// Our Guru with land to stand other units on, its faith set and no other
/// units on the board.
fn board() -> (Game, u32, Vec<Pos>) {
    let mut game = Game::new_full(2, 32, 20, 4_242, 200, 0, false);
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let mut land: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !game.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    land.sort();
    let guru = game.spawn_unit("guru", 0, land[0]);
    game.units.get_mut(&guru).unwrap().religion = Some(FAITH.into());
    (game, guru, land)
}

/// The first land tile at least `at_least` tiles from the Guru.
fn tile_away(game: &Game, guru: u32, land: &[Pos], at_least: i32) -> Pos {
    let here = game.units[&guru].pos;
    *land
        .iter()
        .find(|pos| game.wdist(here, **pos) >= at_least)
        .expect("fixture: land that far away")
}

fn spawn(game: &mut Game, kind: &str, owner: usize, at: Pos, faith: &str, hp: i32) -> u32 {
    let unit = game.spawn_unit(kind, owner, at);
    let body = game.units.get_mut(&unit).unwrap();
    body.religion = Some(faith.into());
    body.hp = hp;
    unit
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_guru_walks_to_the_wounded();
    ai
}

#[test]
fn the_gene_is_off_by_default() {
    let (game, guru, _) = board();
    assert_eq!(AdvancedAi::new().guru_walk_target(&game, 0, guru), None);
}

#[test]
fn a_wounded_friend_outranks_a_healthy_one_and_any_rival() {
    let (mut game, guru, land) = board();
    let healthy_at = tile_away(&game, guru, &land, 2);
    let wounded_at = tile_away(&game, guru, &land, 5);
    spawn(&mut game, "apostle", 0, healthy_at, FAITH, 100);
    spawn(&mut game, "apostle", 0, wounded_at, FAITH, 40);
    spawn(&mut game, "apostle", 1, healthy_at, "orthodoxy", 100);
    assert_eq!(armed().guru_walk_target(&game, 0, guru), Some(wounded_at));
}

#[test]
fn with_nobody_wounded_it_stays_with_the_corps() {
    let (mut game, guru, land) = board();
    let healthy_at = tile_away(&game, guru, &land, 3);
    spawn(&mut game, "missionary", 0, healthy_at, FAITH, 100);
    assert_eq!(armed().guru_walk_target(&game, 0, guru), Some(healthy_at));
}

#[test]
fn another_faith_and_another_guru_are_not_patients() {
    let (mut game, guru, land) = board();
    let at = tile_away(&game, guru, &land, 3);
    spawn(&mut game, "apostle", 0, at, "orthodoxy", 40);
    spawn(&mut game, "guru", 0, at, FAITH, 40);
    assert_eq!(armed().guru_walk_target(&game, 0, guru), None);
}

#[test]
fn a_guru_with_no_heals_ignores_the_wounded() {
    let (mut game, guru, land) = board();
    let at = tile_away(&game, guru, &land, 3);
    spawn(&mut game, "apostle", 0, at, FAITH, 40);
    game.units.get_mut(&guru).unwrap().charges = 0;
    assert_eq!(armed().guru_walk_target(&game, 0, guru), None);
}

#[test]
fn only_a_guru_is_steered() {
    let (mut game, _, land) = board();
    let apostle = spawn(&mut game, "apostle", 0, land[1], FAITH, 100);
    assert_eq!(armed().guru_walk_target(&game, 0, apostle), None);
}
