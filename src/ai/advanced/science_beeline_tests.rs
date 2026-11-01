//! `science-buildings-lead-the-beeline`: the Campus building our cities are
//! equipped for leads the Science victory beeline when its technology lies on
//! the beeline's own path. See `AdvancedAi::science_buildings_lead_the_beeline`.
use super::*;
use crate::game::install_test_district;

#[test]
fn science_buildings_lead_the_beeline_is_a_native_opt_in_off_in_both_controllers() {
    super::test_support::opt_in_off_in_both_controllers(
        "science-buildings-lead-the-beeline",
        |ai| ai.science_buildings_lead_the_beeline,
    );
}

/// Every technology known except Sanitation, Industrialization, Square
/// Rigging, Cartography and what they lead to: Chemistry is two steps away
/// (Sanitation, then Chemistry), and the rest of the road to Rocketry runs
/// through the Radio branch. A University stands.
fn university_empire_short_of_chemistry() -> (Game, StrategicPlan) {
    let mut game = Game::new(2, 32, 24, 8_811, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    install_test_district(&mut game, city, "campus");
    game.cities.get_mut(&city).unwrap().buildings =
        vec![crate::name!("library"), crate::name!("university")];
    let ai = AdvancedAi::new();
    let withheld: Vec<Name> = game
        .rules
        .techs
        .keys()
        .filter(|tech| {
            [
                "sanitation",
                "industrialization",
                "square_rigging",
                "cartography",
            ]
            .into_iter()
            .any(|root| ai.tech_leads_to(&game, root, tech))
        })
        .cloned()
        .collect();
    game.players[0].techs = game
        .rules
        .techs
        .keys()
        .filter(|tech| !withheld.contains(tech))
        .cloned()
        .collect();
    game.players[0].research = None;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    (game, plan)
}

#[test]
fn the_research_lab_leads_the_rocketry_beeline() {
    let (game, plan) = university_empire_short_of_chemistry();
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(
        AdvancedAi::science_victory_tech_goal(&game, 0, GrandStrategy::Science),
        Some("rocketry")
    );
    assert_eq!(
        ai.unreachable_science_building_tech(&game, 0),
        Some("chemistry")
    );
    assert!(
        ai.tech_leads_to(&game, "chemistry", "rocketry"),
        "Chemistry is on the beeline's own path, so only the order moves"
    );
    fn walk_to_chemistry(ai: &AdvancedAi, mut game: Game, plan: &StrategicPlan) -> Vec<String> {
        let mut order = Vec::new();
        while !game.players[0].techs.contains(&crate::name!("chemistry")) && order.len() < 20 {
            game.players[0].research = None;
            ai.advanced_research(&mut game, 0, plan);
            let tech = game.players[0]
                .research
                .clone()
                .expect("the beeline always researches something");
            game.players[0].techs.insert(Name::new(&tech));
            order.push(tech);
        }
        order
    }
    let mut treated = ai.clone();
    treated.enable_science_buildings_lead_the_beeline();
    let treated_order = walk_to_chemistry(&treated, game.clone(), &plan);
    let stock_order = walk_to_chemistry(&ai, game, &plan);
    assert_eq!(
        treated_order,
        vec!["sanitation".to_string(), "chemistry".to_string()],
        "the treated beeline takes Chemistry's own path first"
    );
    // Stock, on this board: Cartography, Square Rigging, Industrialization,
    // Sanitation, Steam Power, Flight, then Chemistry, the live order.
    assert!(
        stock_order.len() > treated_order.len() + 2,
        "stock, the beeline crosses the other branch first: {stock_order:?}"
    );
    assert_eq!(stock_order.last().map(String::as_str), Some("chemistry"));
}
