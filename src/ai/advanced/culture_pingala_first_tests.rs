use super::super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Action, Game};

fn board(seed: u64) -> Game {
    let mut game = Game::new_full(1, 18, 10, seed, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.players[0]
        .civics
        .insert(crate::name!("state_workforce"));
    game
}

fn plan(game: &Game, strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    }
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_culture_seat_appoints_pingala_first();
    }
    ai
}

fn roster(game: &Game) -> Vec<String> {
    game.players[0]
        .governor_roster
        .keys()
        .map(|name| name.to_string())
        .collect()
}

#[test]
fn an_expansion_plan_on_a_culture_seat_appoints_pingala_with_the_gene() {
    let mut game = board(9_101);
    let opening = plan(&game, GrandStrategy::Expansion);
    seat(VictoryTarget::Culture, true).strategic_governors(&mut game, 0, &opening);
    assert_eq!(roster(&game), ["pingala"]);
}

#[test]
fn the_same_seat_appoints_magnus_without_the_gene() {
    let mut game = board(9_101);
    let opening = plan(&game, GrandStrategy::Expansion);
    seat(VictoryTarget::Culture, false).strategic_governors(&mut game, 0, &opening);
    assert_eq!(roster(&game), ["magnus"]);
}

#[test]
fn another_victory_target_keeps_magnus() {
    let mut game = board(9_102);
    let opening = plan(&game, GrandStrategy::Expansion);
    seat(VictoryTarget::Science, true).strategic_governors(&mut game, 0, &opening);
    assert_eq!(roster(&game), ["magnus"]);
}

#[test]
fn a_recovery_keeps_its_own_first_appointee() {
    let mut game = board(9_103);
    let recovery = plan(&game, GrandStrategy::Recovery);
    seat(VictoryTarget::Culture, true).strategic_governors(&mut game, 0, &recovery);
    assert_eq!(roster(&game), ["victor"]);
}

#[test]
fn pingala_takes_connoisseur_first_even_under_a_recovery_plan() {
    let game = board(9_104);
    let ai = seat(VictoryTarget::Culture, true);
    assert_eq!(
        ai.pingala_title_strategy(&game, GrandStrategy::Recovery, "pingala"),
        GrandStrategy::Culture
    );
    assert_eq!(
        ai.pingala_title_strategy(&game, GrandStrategy::Recovery, "victor"),
        GrandStrategy::Recovery
    );
    let off = seat(VictoryTarget::Culture, false);
    assert_eq!(
        off.pingala_title_strategy(&game, GrandStrategy::Recovery, "pingala"),
        GrandStrategy::Recovery
    );
}

#[test]
fn the_title_after_the_appointment_is_connoisseur() {
    // Early Empire and State Workforce: two titles, an appointment and the
    // first promotion.
    let mut game = board(9_105);
    game.players[0].civics.insert(crate::name!("early_empire"));
    let opening = plan(&game, GrandStrategy::Expansion);
    seat(VictoryTarget::Culture, true).strategic_governors(&mut game, 0, &opening);
    let pingala = &game.players[0].governor_roster["pingala"];
    assert!(
        pingala.promotions.contains("connoisseur") && !pingala.promotions.contains("researcher"),
        "{:?}",
        pingala.promotions
    );
}
