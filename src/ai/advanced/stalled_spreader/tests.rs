use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::VictoryTarget;
use std::collections::HashSet;

/// A cleared two-player board with a rival Orthodox Apostle and our Buddhist
/// Missionary on a land tile its strike envelope reaches (the fixture of
/// `apostle_evasion::tests`). Returns the game, our unit and the rival.
fn chased() -> Option<(Game, u32, u32)> {
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
        .filter(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    land.sort();
    for rival_at in land.iter().copied() {
        let mut probe = game.clone();
        let rival = probe.spawn_unit("apostle", 1, rival_at);
        probe.units.get_mut(&rival)?.religion = Some("orthodoxy".into());
        let mut stands: HashSet<Pos> = probe.threat_reach(rival).into_iter().collect();
        stands.insert(rival_at);
        let edge = land.iter().copied().find(|pos| {
            !stands.contains(pos)
                && stands
                    .iter()
                    .any(|from| *from != *pos && probe.wdist(*from, *pos) <= 1)
        });
        if let Some(ours_at) = edge {
            let mine = probe.spawn_unit("missionary", 0, ours_at);
            probe.units.get_mut(&mine)?.religion = Some("buddhism".into());
            return Some((probe, mine, rival));
        }
    }
    None
}

fn religion_seat() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    ai.enable_spreader_steps_out_of_the_apostle();
    ai.enable_stalled_spreader_braves_the_march();
    ai
}

fn hold_on(ai: &AdvancedAi, g: &mut Game, uid: u32, turns: &[u32]) {
    for turn in turns {
        g.turn = *turn;
        ai.record_spreader_hold(g, uid);
    }
}

#[test]
fn stalled_spreader_braves_the_march_is_opt_in() {
    opt_in_off_in_both_controllers("stalled-spreader-braves-the-march", |ai| {
        ai.stalled_spreader_braves_the_march
    });
}

/// Three consecutive held turns (several holds in one turn count once) make
/// the unit brave for [`BRAVE_TURNS`] after its last hold; a gap restarts the
/// streak; only a Religion seat with the gene braves.
#[test]
fn three_held_turns_running_make_the_spreader_brave() {
    let Some((mut g, mine, _)) = chased() else {
        panic!("fixture: a land tile in the Apostle's reach");
    };
    let ai = religion_seat();
    hold_on(&ai, &mut g, mine, &[40, 40, 41]);
    assert!(!ai.spreader_braves_the_march(&g, mine), "two turns");
    hold_on(&ai, &mut g, mine, &[42]);
    assert!(ai.spreader_braves_the_march(&g, mine), "three turns");
    g.turn = 42 + BRAVE_TURNS;
    assert!(ai.spreader_braves_the_march(&g, mine), "within the window");
    g.turn = 43 + BRAVE_TURNS;
    assert!(!ai.spreader_braves_the_march(&g, mine), "past it");
    hold_on(&ai, &mut g, mine, &[60]);
    assert!(
        !ai.spreader_braves_the_march(&g, mine),
        "a gap restarts the streak"
    );

    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_stalled_spreader_braves_the_march();
    hold_on(&culture, &mut g, mine, &[70, 71, 72]);
    assert!(
        !culture.spreader_braves_the_march(&g, mine),
        "a Culture seat"
    );

    let mut off = AdvancedAi::targeting(VictoryTarget::Religion);
    off.enable_spreader_steps_out_of_the_apostle();
    hold_on(&off, &mut g, mine, &[70, 71, 72]);
    assert!(!off.spreader_braves_the_march(&g, mine), "gene off");
}

/// A brave spreader reads the rival as a threat on its tile only when the
/// rival's blow there would kill it.
#[test]
fn a_brave_spreader_yields_only_to_a_lethal_blow() {
    let Some((mut g, mine, rival)) = chased() else {
        panic!("fixture: a land tile in the Apostle's reach");
    };
    let at = g.units[&mine].pos;
    let ai = religion_seat();
    let Some((dealt, received, hp)) =
        g.theological_exchange_preview_with_defender_at(rival, mine, at)
    else {
        panic!("fixture: the rival can strike our tile");
    };
    let wins = dealt >= f64::from(hp) || dealt > received;
    let threats = ai.theological_threats(&g, 0, mine);
    assert_eq!(
        AdvancedAi::theological_threats_reaching(&g, mine, at, &threats),
        usize::from(wins),
        "not yet stalled: the shipped reading"
    );
    hold_on(&ai, &mut g, mine, &[40, 41, 42]);
    let brave = ai.theological_threats(&g, 0, mine);
    assert_eq!(
        AdvancedAi::theological_threats_reaching(&g, mine, at, &brave),
        usize::from(dealt >= f64::from(hp)),
        "stalled: only a kill counts"
    );
}
