use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn founder(target: VictoryTarget) -> (Game, AdvancedAi) {
    let mut g = Game::new_full(2, 40, 24, 936_314, 250, 0, false);
    g.players[0].religion = Some("Buddhism".into());
    g.players[0].civics.insert(crate::name!("theology"));
    (g, AdvancedAi::targeting(target))
}

#[test]
fn the_gene_is_opt_in() {
    opt_in_off_in_both_controllers("religion-seat-reaches-reformed-church", |ai| {
        ai.religion_seat_reaches_reformed_church
    });
}

/// A founder with Theology reaches for Reformed Church, and stops once it has it.
#[test]
fn a_founder_reaches_for_reformed_church() {
    let (mut g, mut ai) = founder(VictoryTarget::Religion);
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), None, "gene off");
    ai.enable_religion_seat_reaches_reformed_church();
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), Some("reformed_church"));
    g.players[0].civics.insert(crate::name!("reformed_church"));
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), None, "already adopted");
}

/// Before Theology or a founding, and on another lane, there is no such goal.
#[test]
fn the_goal_waits_for_the_founding_and_the_lane() {
    let (mut g, mut ai) = founder(VictoryTarget::Religion);
    ai.enable_religion_seat_reaches_reformed_church();
    g.players[0].civics.remove(&crate::name!("theology"));
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), None, "no Theology");

    let (mut g, mut ai) = founder(VictoryTarget::Religion);
    ai.enable_religion_seat_reaches_reformed_church();
    g.players[0].religion = None;
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), None, "not founded");

    let (g, mut ai) = founder(VictoryTarget::Culture);
    ai.enable_religion_seat_reaches_reformed_church();
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), None, "a Culture seat");
}

fn before_theology() -> (Game, AdvancedAi) {
    let (mut g, mut ai) = founder(VictoryTarget::Religion);
    g.turn = 49;
    g.players[0].civ = "Russia".into();
    g.players[0].civics = [
        "code_of_laws",
        "foreign_trade",
        "craftsmanship",
        "early_empire",
        "state_workforce",
        "political_philosophy",
    ]
    .into_iter()
    .map(Name::new)
    .collect();
    g.players[0].pantheon = Some("divine_spark".into());
    g.players[0]
        .boosted_civics
        .extend([crate::name!("mysticism"), crate::name!("theology")]);
    g.players[0].government = Some("classical_republic".into());
    g.players[0].civic = None;
    let city = g.found_city_for(0, (8, 8), None);
    g.cities.get_mut(&city).unwrap().pop = 4;
    ai.skip_opening_book();
    ai.enable_wide_map_capacity();
    ai.enable_government_ladder_2();
    ai.enable_beeline_orders_by_value();
    (g, ai)
}

#[test]
fn the_apostle_treatment_reaches_its_unlock_then_retires() {
    let (mut g, mut ai) = before_theology();
    ai.disable_religion_seat_fields_apostles();
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), None);
    ai.enable_religion_seat_fields_apostles();
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), Some("theology"));
    g.players[0].civics.insert(crate::name!("theology"));
    assert_eq!(
        ai.religion_seat_civic_goal(&g, 0),
        None,
        "the Apostle treatment does not enable the later-civic treatment"
    );
    ai.enable_religion_seat_reaches_reformed_church();
    assert_eq!(ai.religion_seat_civic_goal(&g, 0), Some("reformed_church"));
}

#[test]
fn the_apostle_unlock_keeps_its_founding_and_lane_gates() {
    let (g, mut ai) = before_theology();
    ai.enable_religion_seat_fields_apostles();
    for case in ["not founded", "other target", "religious victory disabled"] {
        let mut board = g.clone();
        let mut controller = ai.clone();
        match case {
            "not founded" => board.players[0].religion = None,
            "other target" => {
                controller = AdvancedAi::targeting(VictoryTarget::Science);
                controller.enable_religion_seat_fields_apostles();
            }
            "religious victory disabled" => board.victory_conditions.religious = false,
            _ => unreachable!(),
        }
        assert_eq!(
            controller.religion_seat_civic_goal(&board, 0),
            None,
            "{case}"
        );
    }
}

#[test]
fn the_real_civic_chooser_reaches_theology_before_the_later_government() {
    let (g, mut ai) = before_theology();
    let mut plan = ai.assess(&g, 0);
    plan.strategy = GrandStrategy::Expansion;
    assert_eq!(
        ai.government_ladder_goal(&g, 0, GrandStrategy::Religion),
        Some("divine_right")
    );
    let mut control = g.clone();
    ai.disable_religion_seat_fields_apostles();
    ai.advanced_research(&mut control, 0, &plan);
    let old = control.players[0].civic.unwrap();
    assert!(
        !ai.civic_leads_to(&g, old.as_str(), "theology"),
        "the control must actually take the later-government detour: {old}"
    );
    ai.enable_religion_seat_fields_apostles();
    let mut candidate = g.clone();
    ai.advanced_research(&mut candidate, 0, &plan);
    let selected = candidate.players[0].civic.unwrap();
    assert!(
        ai.civic_leads_to(&g, selected.as_str(), "theology"),
        "Expansion posture must still reach the Religion seat's unlock: {selected}"
    );
}

#[test]
fn the_apostle_unlock_preserves_the_first_government_and_active_study() {
    let (g, mut ai) = before_theology();
    ai.enable_religion_seat_fields_apostles();
    let plan = ai.assess(&g, 0);
    let mut first_government = g.clone();
    first_government.players[0]
        .civics
        .remove(&crate::name!("political_philosophy"));
    first_government.players[0].government = Some("chiefdom".into());
    ai.advanced_research(&mut first_government, 0, &plan);
    assert_eq!(
        first_government.players[0].civic.as_deref(),
        Some("political_philosophy")
    );
    let mut ongoing = g;
    ongoing.players[0].civic = Some("games_recreation".into());
    ongoing.players[0].civic_progress = 5.0;
    ai.advanced_research(&mut ongoing, 0, &plan);
    assert_eq!(
        ongoing.players[0].civic.as_deref(),
        Some("games_recreation")
    );
    assert_eq!(ongoing.players[0].civic_progress, 5.0);
}

#[test]
fn the_apostle_unlock_selects_theology_when_its_prerequisites_are_done() {
    let (mut g, mut ai) = before_theology();
    ai.enable_religion_seat_fields_apostles();
    g.players[0]
        .civics
        .extend([crate::name!("drama_poetry"), crate::name!("mysticism")]);
    let plan = ai.assess(&g, 0);
    ai.advanced_research(&mut g, 0, &plan);
    assert_eq!(g.players[0].civic.as_deref(), Some("theology"));
}
