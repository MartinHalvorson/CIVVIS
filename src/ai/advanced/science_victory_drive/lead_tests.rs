use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::{install_test_district, Action};

/// A two-player board with one founded city each, met, on a 250-turn clock,
/// no space project landed by anyone: our city is larger and holds a Campus,
/// and we know ten techs more, so we lead the field outright.
fn leading_board() -> Game {
    let mut g = Game::new(2, 24, 16, 71, 250, 0);
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
        g.players[0].techs.insert(tech);
        if i < 30 {
            g.players[1].techs.insert(tech);
        }
    }
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    let ours = g.player_city_ids(0)[0];
    install_test_district(&mut g, ours, "campus");
    g.cities.get_mut(&ours).unwrap().pop = 15;
    g.turn = g.standard_duration(SCIENCE_LEAD_OPEN_STANDARD);
    g
}

fn seat(lane: VictoryTarget, lead: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(lane);
    ai.enable_science_victory_drive();
    ai.enable_science_drive_answers_the_launch();
    if lead {
        ai.enable_science_drive_leads_beside_the_lane();
    }
    ai
}

#[test]
fn science_drive_leads_beside_the_lane_is_opt_in() {
    opt_in_off_in_both_controllers("science-drive-leads-beside-the-lane", |ai| {
        ai.science_drive_leads_beside_the_lane
    });
}

/// 141505Z / 151815Z: a seat assigned another lane that leads the field
/// outright races beside its lane from Online turn 140 under the gene only,
/// with no rival launch to answer; not a turn earlier.
#[test]
fn an_outright_leader_races_beside_its_lane_from_turn_140() {
    let mut g = leading_board();
    let standing = AdvancedAi::science_standing(&g, 0);
    assert!(
        AdvancedAi::science_lead_is_meaningful(&standing),
        "fixture: we lead outright: {standing:?}"
    );
    assert!(
        !AdvancedAi::rival_landed_a_space_project(&g, 0),
        "fixture: no launch to answer"
    );

    let mut off = seat(VictoryTarget::Religion, false);
    off.maintain_science_drive(&g, 0);
    assert!(
        off.science_drive().is_none(),
        "off: the lane keeps the seat"
    );

    let mut on = seat(VictoryTarget::Religion, true);
    on.maintain_science_drive(&g, 0);
    let drive = on
        .science_drive()
        .expect("an outright lead opens the drive");
    assert!(drive.answering && !drive.assigned);
    assert_eq!(on.raced_target(), Some(VictoryTarget::Science));

    let open = g.turn;
    g.turn = open - 1;
    let mut early = seat(VictoryTarget::Religion, true);
    early.maintain_science_drive(&g, 0);
    assert!(early.science_drive().is_none(), "not before turn 140");
}

/// No meaningful lead, a Diplomatic seat near its own victory, or the
/// answer gene off: no drive.
#[test]
fn a_trailing_or_near_diplomatic_seat_keeps_its_lane() {
    let mut g = leading_board();
    let more: Vec<Name> = g.rules.techs.keys().take(45).copied().collect();
    g.players[1].techs.extend(more);
    let mut trailing = seat(VictoryTarget::Culture, true);
    trailing.maintain_science_drive(&g, 0);
    assert!(trailing.science_drive().is_none(), "behind on techs");

    let g = leading_board();
    let mut diplomat = seat(VictoryTarget::Diplomacy, true);
    let mut near = g.clone();
    near.players[0].dvp = SCIENCE_ANSWER_DIPLO_YIELD;
    diplomat.maintain_science_drive(&near, 0);
    assert!(diplomat.science_drive().is_none(), "near its own victory");

    let mut no_answer = AdvancedAi::targeting(VictoryTarget::Culture);
    no_answer.enable_science_victory_drive();
    no_answer.enable_science_drive_leads_beside_the_lane();
    no_answer.maintain_science_drive(&g, 0);
    assert!(
        no_answer.science_drive().is_none(),
        "needs science-drive-answers-the-launch"
    );
}

/// A Domination seat's lead-opened drive lifts its space-race exclusions
/// under `domination-joins-the-race`, as an answer does.
#[test]
fn a_leading_domination_seat_joins_the_race() {
    let g = leading_board();
    let mut ai = seat(VictoryTarget::Domination, true);
    ai.enable_domination_joins_the_race();
    assert!(ai.space_race_excluded(), "no drive yet");
    ai.maintain_science_drive(&g, 0);
    assert!(ai.science_drive().is_some_and(|drive| drive.answering));
    assert!(!ai.space_race_excluded(), "the lead-opened drive races");
}
