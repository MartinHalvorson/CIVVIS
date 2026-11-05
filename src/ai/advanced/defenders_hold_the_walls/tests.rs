use super::super::AdvancedAi;
use crate::game::Game;

/// Two majors at war, seat 0 with a capital and an Archer beside it; no
/// hostile near yet. Returns the board, the Archer and the capital's tile.
fn walls() -> (Game, u32, crate::Pos) {
    let mut game = Game::new_full(2, 24, 16, 7_961, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    // Dry, open ground, so no unit below stands embarked or on a peak.
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    game.record_contact(0, 1);
    game.at_war.insert((0, 1));
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    let beside = game
        .nbrs(capital)
        .into_iter()
        .find(|pos| game.unit_ids_at(*pos).is_empty())
        .expect("a free tile beside the capital");
    let archer = game.spawn_test_unit("archer", 0, beside);
    (game, archer, capital)
}

/// A free tile exactly `distance` from `from`.
fn free_at(game: &Game, from: crate::Pos, distance: i32) -> crate::Pos {
    game.wdisk(from, distance)
        .into_iter()
        .filter(|pos| game.wdist(*pos, from) == distance && game.unit_ids_at(*pos).is_empty())
        .min()
        .expect("a free tile at that distance")
}

#[test]
fn a_defender_holds_only_under_the_gene_and_a_threat() {
    let (mut game, archer, capital) = walls();
    let mut ai = AdvancedAi::new();
    ai.enable_defenders_hold_the_walls();
    assert!(
        !ai.holds_the_walls(&game, 0, archer),
        "no hostile near the walls"
    );

    let raider = free_at(&game, capital, super::WALL_THREAT_REACH);
    game.spawn_test_unit("warrior", 1, raider);
    assert!(ai.holds_the_walls(&game, 0, archer));
    assert!(!AdvancedAi::new().holds_the_walls(&game, 0, archer), "off");

    // A hostile at peace is no threat.
    game.at_war.remove(&(0, 1));
    assert!(!ai.holds_the_walls(&game, 0, archer), "at peace");
}

#[test]
fn the_wounded_and_the_distant_do_not_hold() {
    let (mut game, archer, capital) = walls();
    let raider = free_at(&game, capital, super::WALL_THREAT_REACH);
    game.spawn_test_unit("warrior", 1, raider);
    let mut ai = AdvancedAi::new();
    ai.enable_defenders_hold_the_walls();

    let far = free_at(&game, capital, super::WALL_DEFENDER_REACH + 1);
    let distant = game.spawn_test_unit("archer", 0, far);
    assert!(!ai.holds_the_walls(&game, 0, distant), "beyond the reach");

    game.units.get_mut(&archer).unwrap().hp = super::ROTATE_HP - 1;
    assert!(!ai.holds_the_walls(&game, 0, archer), "wounded");
}

#[test]
fn walls_release_the_one_blow_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.walls_release_the_one_blow, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "walls_release_the_one_blow"));
    super::super::test_support::opt_in_off_in_both_controllers(
        "walls-release-the-one-blow",
        |ai| ai.walls_release_the_one_blow,
    );
}

/// An Archer beside the walls, wounded to a health the adjacent Swordsman's
/// blow removes at the top of its roll: under the gene it is listed one-blow
/// lethal and no longer holds the walls; without it, it holds; a garrison on
/// the City Center is never listed.
#[test]
fn a_ring_defender_one_blow_removes_is_released() {
    let (mut game, archer, capital) = walls();
    let archer_pos = game.units[&archer].pos;
    let raider_pos = game
        .nbrs(archer_pos)
        .into_iter()
        .find(|pos| {
            *pos != capital
                && game.unit_ids_at(*pos).is_empty()
                && game.wdist(*pos, capital) <= super::WALL_THREAT_REACH
        })
        .expect("a tile beside the archer within the threat reach");
    game.spawn_test_unit("swordsman", 1, raider_pos);
    let garrison = game.spawn_test_unit("warrior", 0, capital);
    // Find a health the swordsman's top blow removes but which still counts
    // as healthy for the walls (at or over ROTATE_HP).
    let mut ai = AdvancedAi::new();
    ai.enable_defenders_hold_the_walls();
    ai.enable_walls_release_the_one_blow();
    let mut released = None;
    for hp in (super::ROTATE_HP..=100).rev() {
        game.units.get_mut(&archer).unwrap().hp = hp;
        let mut field = super::super::battle_planner::DangerField::new(&game, 0);
        let doomed = ai.walls_one_blow_units(&game, 0, &mut field);
        if doomed.contains(&archer) {
            released = Some((hp, doomed));
            break;
        }
    }
    let (hp, doomed) = released.expect("some healthy hp the swordsman's top blow removes");
    assert!(
        !doomed.contains(&garrison),
        "the City Center takes the blow"
    );
    game.units.get_mut(&archer).unwrap().hp = hp;
    let mut keeper = AdvancedAi::new();
    keeper.enable_defenders_hold_the_walls();
    assert!(
        keeper.holds_the_walls(&game, 0, archer),
        "without the gene it holds"
    );
    ai.walls_one_blow = doomed;
    assert!(
        !ai.holds_the_walls(&game, 0, archer),
        "under the gene it is released"
    );
}
