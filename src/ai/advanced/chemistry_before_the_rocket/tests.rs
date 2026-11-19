use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::install_test_district as install_ai_test_district;

fn science_plan(g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: g.turn,
        rush: false,
    }
}

/// One city with a Campus, Library and University. It holds every ancestor
/// of Rocketry except Chemistry's last three (Sanitation, Economics,
/// Replaceable Parts) and the Radio half's last four (Industrialization,
/// Steam Power, Flight, Radio), plus the ancient side branches, so Sanitation
/// and Economics (toward Chemistry) and Industrialization (toward Radio) are
/// all legal steps toward Rocketry.
fn university_empire(seed: u64) -> Game {
    let mut game = Game::new(2, 32, 24, seed, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    install_ai_test_district(&mut game, city, "campus");
    game.cities.get_mut(&city).unwrap().buildings =
        vec![crate::name!("library"), crate::name!("university")];
    let held_back = [
        "chemistry",
        "sanitation",
        "economics",
        "replaceable_parts",
        "industrialization",
        "steam_power",
        "flight",
        "radio",
    ];
    let ancestors = game.rules.tech_ancestors.get("rocketry").cloned().unwrap();
    for tech in ancestors.iter() {
        let blocked = held_back.contains(&tech.as_str())
            || game
                .rules
                .tech_ancestors
                .get(tech.as_str())
                .is_some_and(|before| held_back.iter().any(|held| before.contains(*held)));
        if !blocked {
            game.players[0].techs.insert(Name::new(tech));
        }
    }
    for tech in [
        "astrology",
        "irrigation",
        "sailing",
        "celestial_navigation",
        "bronze_working",
        "iron_working",
        "the_wheel",
    ] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.players[0].research = None;
    game
}

fn science_seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    if gene {
        ai.enable_chemistry_before_the_rocket();
    }
    ai
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("chemistry-before-the-rocket", |ai| {
        ai.chemistry_before_the_rocket
    });
}

#[test]
fn a_university_empire_on_the_rocketry_goal_names_chemistry_only_with_the_gene() {
    let game = university_empire(8_811);
    assert_eq!(
        AdvancedAi::science_victory_tech_goal(&game, 0, GrandStrategy::Science),
        Some("rocketry"),
        "fixture: Rocketry is the lane's next victory technology"
    );
    for (gene, expected) in [(false, None), (true, Some("chemistry"))] {
        let ai = science_seat(gene);
        assert_eq!(
            ai.chemistry_before_the_rocket_goal(&game, 0, GrandStrategy::Science, true),
            expected,
            "gene {gene}"
        );
    }
}

/// No University, another lane, or a later victory technology: no goal.
#[test]
fn no_university_another_lane_or_a_later_chain_leaves_the_goal_alone() {
    let ai = science_seat(true);
    let mut bare = university_empire(8_812);
    let city = bare.player_city_ids(0)[0];
    bare.cities.get_mut(&city).unwrap().buildings = vec![crate::name!("library")];
    assert_eq!(
        ai.chemistry_before_the_rocket_goal(&bare, 0, GrandStrategy::Science, true),
        None,
        "a Library alone does not open the Research Lab"
    );
    let game = university_empire(8_813);
    assert_eq!(
        ai.chemistry_before_the_rocket_goal(&game, 0, GrandStrategy::Culture, false),
        None,
        "off the Science lane"
    );
    let mut later = university_empire(8_814);
    for tech in ["chemistry", "rocketry"] {
        later.players[0].techs.insert(Name::new(tech));
    }
    assert_eq!(
        ai.chemistry_before_the_rocket_goal(&later, 0, GrandStrategy::Science, true),
        None,
        "past Rocketry the lane's own chain stands"
    );
}

/// The research pick itself: with the gene the seat steps toward Chemistry
/// although a step toward Radio (Industrialization) is legal too, and it
/// journals the pick.
#[test]
fn the_research_pick_steps_toward_chemistry_with_the_gene() {
    let mut game = university_empire(8_815);
    assert!(
        game.available_techs(0)
            .iter()
            .any(|tech| tech == "industrialization"),
        "fixture: a Radio-half step is legal"
    );
    let mut ai = science_seat(true);
    let journal = crate::reasoning::Journal::recording();
    journal.begin_turn(game.turn, 0);
    ai.attach_journal(journal.handle());
    let plan = science_plan(&game);
    ai.advanced_research(&mut game, 0, &plan);
    let selected = game.players[0]
        .research
        .as_deref()
        .expect("a research step is chosen");
    assert!(
        ai.tech_leads_to(&game, selected, "chemistry"),
        "picked {selected}"
    );
    let details: Vec<String> = journal
        .since(0)
        .thoughts
        .into_iter()
        .map(|thought| thought.detail)
        .collect();
    assert!(
        details
            .iter()
            .any(|detail| detail.starts_with("chemistry-before-the-rocket:")),
        "{details:?}"
    );
}
