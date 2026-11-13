use super::*;

#[test]
fn score_seat_founds_early_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers("score-seat-founds-early", |ai| {
        ai.score_seat_founds_early
    });
}

/// Two majors, every unit removed: our capital, turn 30, and a Settler on a
/// land tile five to seven tiles out on which a city may be founded. No
/// hostile is on the board. Returns the game, the Settler and its tile.
fn walking_settler() -> (Game, u32, Pos) {
    let mut game = Game::new_full(2, 32, 20, 936_431, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    for pid in 0..2 {
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
        .filter(|(pos, tile)| {
            (5..=7).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.turn = 30;
    game.current = 0;
    for site in sites {
        let uid = game.spawn_unit("settler", 0, site);
        if game.can_found_city(uid) {
            return (game, uid, site);
        }
        game.remove_unit(uid);
    }
    panic!("fixture: a foundable tile five to seven tiles out");
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_score_seat_founds_early();
    }
    ai
}

/// The Settler set out `turns` turns ago.
fn set_out(ai: &mut AdvancedAi, game: &Game, uid: u32, turns: u32) {
    ai.settler_walk_started
        .insert(uid, game.turn.saturating_sub(turns));
}

/// Off, on another target, or inside its patience, the gene leaves the
/// Settler to the ordinary step.
#[test]
fn the_ordinary_step_runs_off_elsewhere_and_inside_the_patience() {
    let (game, uid, _) = walking_settler();
    let patience = game.standard_duration(SCORE_FOUNDS_EARLY_PATIENCE_STANDARD);
    for (mut ai, walked, why) in [
        (seat(VictoryTarget::Score, false), patience + 5, "off"),
        (
            seat(VictoryTarget::Domination, true),
            patience + 5,
            "another target",
        ),
        (
            seat(VictoryTarget::Score, true),
            patience - 1,
            "inside its patience",
        ),
    ] {
        set_out(&mut ai, &game, uid, walked);
        let mut board = game.clone();
        assert_eq!(
            ai.score_seat_founds_early_step(&mut board, 0, uid),
            None,
            "{why}"
        );
        assert_eq!(board.player_city_ids(0).len(), 1, "{why}");
        assert!(board.units.contains_key(&uid), "{why}");
    }
}

/// Past its patience on a Score seat, with no hostile in sight, the Settler
/// founds on the best site in reach: underfoot it founds at once, otherwise
/// it takes that site within four tiles and steps toward it.
#[test]
fn a_settler_past_its_patience_takes_the_best_safe_site_in_reach() {
    let (mut game, uid, here) = walking_settler();
    let patience = game.standard_duration(SCORE_FOUNDS_EARLY_PATIENCE_STANDARD);
    let mut ai = seat(VictoryTarget::Score, true);
    set_out(&mut ai, &game, uid, patience);
    let acted = ai.score_seat_founds_early_step(&mut game, 0, uid);
    assert_eq!(acted, Some(true), "the Settler acts for the score");
    let founded_here = game
        .player_city_ids(0)
        .iter()
        .any(|cid| game.cities[cid].pos == here);
    let retargeted = ai
        .settler_targets
        .get(&uid)
        .is_some_and(|site| game.wdist(*site, here) <= 4);
    assert!(
        founded_here || retargeted,
        "it founds underfoot or takes a site within four tiles"
    );
}

/// After the early window the gene stands aside.
#[test]
fn the_gene_stands_aside_after_the_early_window() {
    let (mut game, uid, _) = walking_settler();
    game.turn = game.standard_duration(SCORE_FOUNDS_EARLY_UNTIL_STANDARD) + 1;
    let patience = game.standard_duration(SCORE_FOUNDS_EARLY_PATIENCE_STANDARD);
    let mut ai = seat(VictoryTarget::Score, true);
    set_out(&mut ai, &game, uid, patience + 5);
    assert_eq!(ai.score_seat_founds_early_step(&mut game, 0, uid), None);
    assert_eq!(game.player_city_ids(0).len(), 1);
}
