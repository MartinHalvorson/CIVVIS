use super::*;
use crate::game::{Action, Game};

/// Our Inquisitor beside a rival Apostle of another faith. Returns the game,
/// our unit and the rival's tile.
fn duel(ours: &str, theirs: &str) -> Option<(Game, u32, Pos)> {
    let mut game = Game::new_full(2, 32, 20, 4_242, 200, 0, false);
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let land: Vec<Pos> = {
        let mut tiles: Vec<Pos> = game
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| !game.rules.is_water(tile))
            .map(|(pos, _)| *pos)
            .collect();
        tiles.sort();
        tiles
    };
    let (a, b) = land.iter().find_map(|a| {
        land.iter()
            .find(|b| game.wdist(*a, **b) == 1)
            .map(|b| (*a, *b))
    })?;
    let mine = game.spawn_unit(ours, 0, a);
    let rival = game.spawn_unit(theirs, 1, b);
    game.units.get_mut(&mine)?.religion = Some("buddhism".into());
    game.units.get_mut(&rival)?.religion = Some("orthodoxy".into());
    Some((game, mine, b))
}

#[test]
fn a_weaker_inquisitor_does_not_start_a_fight_it_loses() {
    let Some((game, mine, target)) = duel("inquisitor", "apostle") else {
        panic!("fixture: two adjacent land tiles");
    };
    let (dealt, received, _) = game
        .theological_exchange_preview(mine, target)
        .expect("fixture: the exchange reads");
    assert!(dealt < received, "fixture: the Inquisitor is the weaker side");
    assert!(!AdvancedAi::theology_fight_is_winnable(&game, mine, target));
}

#[test]
fn a_wounded_defender_the_blow_kills_is_still_taken() {
    let Some((mut game, mine, target)) = duel("inquisitor", "apostle") else {
        panic!("fixture: two adjacent land tiles");
    };
    let rival = game.units_at(target)[0];
    game.units.get_mut(&rival).unwrap().hp = 5;
    assert!(AdvancedAi::theology_fight_is_winnable(&game, mine, target));
}

#[test]
fn the_gene_drops_the_losing_attack_from_the_step() {
    let Some((game, mine, target)) = duel("inquisitor", "apostle") else {
        panic!("fixture: two adjacent land tiles");
    };
    let legal = game.legal_actions(0);
    assert!(
        legal
            .iter()
            .any(|action| matches!(action, Action::TheologicalAttack { unit, target: t } if *unit == mine && *t == target)),
        "fixture: the attack is legal"
    );
    let mut on = AdvancedAi::new();
    on.enable_theology_fights_only_to_win();
    let mut fought = game.clone();
    on.advanced_religious_step(&mut fought, 0, mine, false);
    let rival = game.units_at(target)[0];
    assert_eq!(
        fought.units.get(&rival).map(|u| u.hp),
        Some(100),
        "on: the losing fight is not started"
    );

    let off = AdvancedAi::new();
    let mut stock = game.clone();
    off.advanced_religious_step(&mut stock, 0, mine, false);
    let hurt = stock.units.get(&rival).map_or(0, |u| u.hp);
    assert!(hurt < 100, "off: the stock step starts the fight");
}
