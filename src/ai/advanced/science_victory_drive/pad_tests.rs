use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::Action;

/// A two-player board with one founded city each, met, on a 200-turn clock:
/// the rival knows more techs, so we do not lead outright.
fn trailing_board() -> Game {
    let mut g = Game::new(2, 24, 16, 71, 200, 0);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.current = pid;
        g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
    }
    let techs: Vec<Name> = g.rules.techs.keys().take(40).copied().collect();
    for (i, tech) in techs.into_iter().enumerate() {
        if i < 30 {
            g.players[0].techs.insert(tech);
        }
        g.players[1].techs.insert(tech);
    }
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.turn = 150;
    g
}

fn seat(lane: VictoryTarget, pad: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(lane);
    ai.enable_science_victory_drive();
    ai.enable_science_drive_answers_the_launch();
    if pad {
        ai.enable_science_drive_answers_from_the_pad();
    }
    ai
}

#[test]
fn science_drive_answers_from_the_pad_is_opt_in() {
    opt_in_off_in_both_controllers("science-drive-answers-from-the-pad", |ai| {
        ai.science_drive_answers_from_the_pad
    });
}

/// 075446Z: a Religion seat level with the racer on landed projects, behind
/// on techs, answers the launch from its pads under the gene only; a seat
/// with no project, or behind the racer on projects, does not.
#[test]
fn a_seat_level_on_the_pads_answers_the_launch() {
    let mut g = trailing_board();
    g.players[1]
        .science_projects
        .insert("launch_earth_satellite".to_string());
    assert!(
        !AdvancedAi::science_standing(&g, 0).leads_outright(),
        "fixture: we do not lead outright"
    );

    // No project of ours: no answer, gene or not.
    let mut ai = seat(VictoryTarget::Religion, true);
    ai.maintain_science_drive(&g, 0);
    assert!(ai.science_drive().is_none(), "no pad of ours yet");

    // Level on the pads: the gene answers, the shipped answer does not.
    for project in ["launch_earth_satellite", "launch_moon_landing"] {
        g.players[0].science_projects.insert(project.to_string());
    }
    let mut off = seat(VictoryTarget::Religion, false);
    off.maintain_science_drive(&g, 0);
    assert!(
        off.science_drive().is_none(),
        "off: the science reading holds it"
    );
    let mut on = seat(VictoryTarget::Religion, true);
    on.maintain_science_drive(&g, 0);
    let drive = on.science_drive().expect("level on the pads answers");
    assert!(drive.answering && !drive.assigned);
    assert_eq!(on.raced_target(), Some(VictoryTarget::Science));

    // The racer pulls a project ahead: the pad reading no longer holds.
    for project in ["launch_moon_landing", "launch_mars_colony"] {
        g.players[1].science_projects.insert(project.to_string());
    }
    assert!(!on.answer_holds_the_pad(&g, 0), "the racer is ahead");
    let mut fresh = seat(VictoryTarget::Religion, true);
    fresh.maintain_science_drive(&g, 0);
    assert!(fresh.science_drive().is_none(), "behind on the pads");
}
