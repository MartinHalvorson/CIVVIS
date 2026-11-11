//! `cold-war-then-heritage`: unit tests.
use super::*;
use crate::ai::advanced::StrategicPlan;

/// A Culture empire on Monarchy, turn 160, that knows every civic except
/// the ones the post-Cold-War road and its detours ask for. Cold War is known
/// unless a test removes it.
fn board() -> Game {
    let mut g = Game::new_full(2, 40, 24, 61_427, 250, 0, false);
    g.current = 0;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.apply(0, &crate::game::Action::FoundCity { unit: settler })
        .unwrap();
    g.at_war.clear();
    g.turn = 160;
    let withheld = [
        "conservation",
        "professional_sports",
        "cultural_heritage",
        "space_race",
        "environmentalism",
        "social_media",
        "suffrage",
        "class_struggle",
        "totalitarianism",
        "corporate_libertarianism",
        "digital_democracy",
        "synthetic_technocracy",
    ];
    g.players[0].civics = g
        .rules
        .civics
        .keys()
        .copied()
        .filter(|civic| !withheld.contains(&civic.as_str()))
        .collect();
    assert!(g.players[0].civics.contains(&crate::name!("cold_war")));
    g.players[0].government = Some("monarchy".into());
    g.players[0].civic = None;
    g
}

fn plan(g: &Game, strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    }
}

fn seat(on: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    if on {
        ai.enable_cold_war_then_heritage();
    }
    ai.base.journal = crate::reasoning::Journal::recording();
    ai
}

fn picks(ai: &AdvancedAi, g: &Game, strategy: GrandStrategy) -> Option<String> {
    let mut g = g.clone();
    let plan = plan(&g, strategy);
    ai.advanced_research(&mut g, 0, &plan);
    g.players[0].civic.as_deref().map(str::to_string)
}

fn gene_lines(ai: &AdvancedAi) -> Vec<String> {
    ai.base
        .journal
        .since(0)
        .thoughts
        .iter()
        .filter(|thought| thought.detail.starts_with("cold-war-then-heritage:"))
        .map(|thought| format!("{} | {}", thought.headline, thought.detail))
        .collect()
}

#[test]
fn the_gene_is_an_opt_in_off_in_the_controllers() {
    assert!(!AdvancedAi::targeting(VictoryTarget::Culture).cold_war_then_heritage);
    assert!(!AdvancedAi::new().cold_war_then_heritage);
}

#[test]
fn after_cold_war_the_road_is_conservation_then_heritage() {
    let mut g = board();
    assert_eq!(
        seat(true).cold_war_then_heritage_goal(&g, 0),
        Some("conservation")
    );
    g.players[0].civics.insert(crate::name!("conservation"));
    assert_eq!(
        seat(true).cold_war_then_heritage_goal(&g, 0),
        Some("cultural_heritage")
    );
    g.players[0]
        .civics
        .insert(crate::name!("cultural_heritage"));
    assert_eq!(seat(true).cold_war_then_heritage_goal(&g, 0), None);
}

#[test]
fn the_gene_steps_aside_where_it_should() {
    let g = board();
    assert_eq!(seat(false).cold_war_then_heritage_goal(&g, 0), None);
    // Before Cold War the peers' road owns the slot.
    let mut early = g.clone();
    early.players[0].civics.remove(&crate::name!("cold_war"));
    assert_eq!(seat(true).cold_war_then_heritage_goal(&early, 0), None);
    // Another target, a released lane, or no Culture victory.
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_cold_war_then_heritage();
    assert_eq!(science.cold_war_then_heritage_goal(&g, 0), None);
    let mut released = seat(true);
    released.lane_lost = true;
    assert_eq!(released.cold_war_then_heritage_goal(&g, 0), None);
    let mut no_culture = g.clone();
    no_culture.victory_conditions.culture = false;
    assert_eq!(seat(true).cold_war_then_heritage_goal(&no_culture, 0), None);
}

#[test]
fn the_slot_takes_cultural_heritage_before_the_culture_list() {
    // Conservation known: the culture list asks for Professional Sports
    // next; the gene asks for Cultural Heritage.
    let mut g = board();
    g.players[0].civics.insert(crate::name!("conservation"));
    let off = seat(false);
    let off_pick = picks(&off, &g, GrandStrategy::Culture);
    assert_ne!(off_pick.as_deref(), Some("cultural_heritage"));
    assert!(gene_lines(&off).is_empty());
    let on = seat(true);
    assert_eq!(
        picks(&on, &g, GrandStrategy::Culture).as_deref(),
        Some("cultural_heritage")
    );
    let lines = gene_lines(&on);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("cultural heritage"), "{lines:?}");
}

#[test]
fn a_recovery_plan_on_the_culture_target_keeps_the_road() {
    let g = board();
    for strategy in [GrandStrategy::Culture, GrandStrategy::Recovery] {
        assert_eq!(
            picks(&seat(true), &g, strategy).as_deref(),
            Some("conservation"),
            "{strategy:?}"
        );
    }
}

#[test]
fn before_cold_war_the_pick_is_unchanged() {
    let mut g = board();
    g.players[0].civics.remove(&crate::name!("cold_war"));
    for strategy in [GrandStrategy::Culture, GrandStrategy::Recovery] {
        let on = seat(true);
        assert_eq!(
            picks(&on, &g, strategy),
            picks(&seat(false), &g, strategy),
            "{strategy:?}"
        );
        assert!(gene_lines(&on).is_empty());
    }
}
