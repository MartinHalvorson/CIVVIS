use super::super::{GrandStrategy, StrategicPlan};
use super::*;
use crate::game::Game;
use crate::name;

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

/// Two empires on a flat, fully explored board, met and at peace. The
/// rival's original capital at (28, 11) is then taken off the board and its
/// surroundings put back in fog — what the live mirror shows of a capital
/// the seat has never seen — leaving one known rival city at (24, 11).
fn board() -> (Game, Pos) {
    let mut g = Game::new_full(2, 40, 24, 5_112_202, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    g.found_city_for(0, at(6, 11), None);
    let capital_pos = at(28, 11);
    let capital = g.found_city_for(1, capital_pos, None);
    assert!(
        g.cities[&capital].is_capital,
        "fixture: the rival's first city is its capital"
    );
    g.found_city_for(1, at(24, 11), None);
    g.record_contact(0, 1);
    g.players[0].explored = g.map.tiles.keys().copied().collect();
    // Unseen: the capital is not on the board, its plots belong to no known
    // city, and the ground around it was never revealed.
    g.cities.remove(&capital);
    for tile in g.map.tiles.values_mut() {
        if tile.owner_city == Some(capital) {
            tile.owner_city = None;
        }
    }
    for pos in g.wdisk(capital_pos, 2) {
        g.players[0].explored.remove(&pos);
    }
    g.at_war.clear();
    g.turn = 120;
    g.current = 0;
    (g, capital_pos)
}

fn domination() -> AdvancedAi {
    AdvancedAi::targeting(VictoryTarget::Domination)
}

fn on() -> AdvancedAi {
    let mut ai = domination();
    ai.enable_find_the_capital();
    ai
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    assert!(!AdvancedAi::new().find_the_capital);
    let gene = super::super::genes::GENES
        .iter()
        .find(|gene| gene.tag == "find-the-capital")
        .expect("registered");
    let mut ai = AdvancedAi::new();
    (gene.enable)(&mut ai);
    assert!(ai.find_the_capital);
    (gene.disable)(&mut ai);
    assert!(!ai.find_the_capital);
}

#[test]
fn off_nothing_changes() {
    let (mut g, _) = board();
    let ai = domination();
    assert!(ai.unfound_capital_rivals(&g, 0).is_empty());
    assert_eq!(ai.find_the_capital_passage_gold(&g, 0, 1), None);
    g.at_war.insert((0, 1));
    assert!(ai.find_capital_row(&g, 0).is_none());
}

#[test]
fn an_unseen_capital_prices_the_passage_at_peace() {
    let (g, _) = board();
    let ai = on();
    assert_eq!(ai.unfound_capital_rivals(&g, 0), vec![1]);
    assert_eq!(
        ai.find_the_capital_passage_gold(&g, 0, 1),
        Some(FIND_CAPITAL_PASSAGE_GOLD)
    );
    // The book alone sits under the border-buy lane's 30 Gold minimum.
    assert!(g.passage_gold_value(0) < 30.0);
    // Closed ground: no hunt row yet, the scout's lead and the purchase do
    // the work at peace.
    assert!(ai.find_capital_row(&g, 0).is_none());
}

#[test]
fn only_a_domination_plan_hunts() {
    let (g, _) = board();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_find_the_capital();
    assert!(ai.unfound_capital_rivals(&g, 0).is_empty());
    assert_eq!(ai.find_the_capital_passage_gold(&g, 0, 1), None);
}

#[test]
fn a_seen_or_remembered_capital_is_not_hunted() {
    let (mut g, capital_pos) = board();
    let ai = on();
    // Remembered from an earlier sighting.
    let remembered = crate::game::RememberedCity {
        id: 999,
        name: "Mbanza Kongo".to_string(),
        owner: 1,
        pos: capital_pos,
        pop: 8,
        hp: 200,
        is_capital: true,
        original_owner: 1,
        captured_from: None,
        occupied_from: None,
        wall_hp: 0,
        wall_max: 0,
        encampment_hp: 0,
        encampment_wall_hp: 0,
        encampment_pillaged: false,
        defending_districts: Vec::new(),
        religion: None,
        seen_turn: 100,
    };
    g.players[0].remembered_cities.insert(999, remembered);
    assert!(ai.unfound_capital_rivals(&g, 0).is_empty());
    assert_eq!(ai.find_the_capital_passage_gold(&g, 0, 1), None);
    g.players[0].remembered_cities.clear();
    // On the board.
    let capital = g.found_city_for(1, capital_pos, None);
    g.cities.get_mut(&capital).unwrap().is_capital = true;
    assert!(ai.unfound_capital_rivals(&g, 0).is_empty());
}

#[test]
fn war_opens_the_ground_and_the_row_aims_at_the_fog_in_the_middle() {
    let (mut g, capital_pos) = board();
    let ai = on();
    g.at_war.insert((0, 1));
    // At war the passage is not bought: the ground is already open.
    assert_eq!(ai.find_the_capital_passage_gold(&g, 0, 1), None);
    let row = ai.find_capital_row(&g, 0).expect("a hunt row at war");
    assert_eq!(row.key, ObjectiveKey::FindCapital(1));
    assert_eq!(row.kind, ObjectiveKind::Recon);
    assert!(row.value > 100.0, "worth more than a sector");
    assert!(!g.players[0].explored.contains(&row.at), "it aims into fog");
    assert!(
        g.wdist(row.at, capital_pos) <= 2,
        "the fog beside the known ground, where the capital stands"
    );
}

#[test]
fn granted_open_borders_open_the_ground_too() {
    let (mut g, _) = board();
    let ai = on();
    g.players[1].open_borders_until.insert(0, g.turn + 30);
    assert!(ai.find_capital_row(&g, 0).is_some());
    // An expired grant does not.
    g.players[1].open_borders_until.insert(0, g.turn);
    assert!(ai.find_capital_row(&g, 0).is_none());
}

#[test]
fn no_fog_left_means_no_row() {
    let (mut g, _) = board();
    let ai = on();
    g.at_war.insert((0, 1));
    g.players[0].explored = g.map.tiles.keys().copied().collect();
    // Still unseen — say the Free Cities hold it, which the export does not
    // carry as a city — but there is nowhere left to look.
    assert_eq!(ai.unfound_capital_rivals(&g, 0), vec![1]);
    assert!(ai.find_capital_row(&g, 0).is_none());
}

#[test]
fn the_board_sends_a_fast_body_when_no_scout_is_free() {
    let (mut g, _) = board();
    g.at_war.insert((0, 1));
    let horse = g.spawn_test_unit("horseman", 0, at(10, 11));
    assert_eq!(
        crate::ai::BasicAi::unit_doctrine(&g, horse),
        crate::ai::UnitDoctrine::Mobile
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    let assigned = |ai: &AdvancedAi| {
        ai.objective_board()
            .forces
            .iter()
            .find(|force| force.objective_key == ObjectiveKey::FindCapital(1))
            .is_some_and(|force| force.units.contains(&horse))
    };

    let mut off = domination();
    off.enable_objective_board();
    off.board_rebuild_force_groups(&g, 0, &plan);
    assert!(off
        .objective_board()
        .rows
        .iter()
        .all(|row| row.key != ObjectiveKey::FindCapital(1)));
    assert!(!assigned(&off));

    let mut ai = on();
    ai.enable_objective_board();
    ai.board_rebuild_force_groups(&g, 0, &plan);
    assert!(ai
        .objective_board()
        .rows
        .iter()
        .any(|row| row.key == ObjectiveKey::FindCapital(1)));
    assert!(assigned(&ai), "the horseman takes the hunt");
}

#[test]
fn the_unseen_capital_gene_ships_off_and_is_registered() {
    assert!(!AdvancedAi::new().unseen_capital_is_unseen());
    let gene = super::super::genes::GENES
        .iter()
        .find(|gene| gene.tag == "unseen-capital-is-unseen")
        .expect("registered");
    let mut ai = AdvancedAi::new();
    (gene.enable)(&mut ai);
    assert!(ai.unseen_capital_is_unseen());
    (gene.disable)(&mut ai);
    assert!(!ai.unseen_capital_is_unseen());
}

#[test]
fn under_the_unseen_capital_gene_a_slow_body_takes_the_hunt() {
    let (mut g, _) = board();
    g.at_war.insert((0, 1));
    let warrior = g.spawn_test_unit("warrior", 0, at(10, 11));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    let assigned = |ai: &AdvancedAi| {
        ai.objective_board()
            .forces
            .iter()
            .find(|force| force.objective_key == ObjectiveKey::FindCapital(1))
            .is_some_and(|force| force.units.contains(&warrior))
    };

    let mut v1 = on();
    v1.enable_objective_board();
    v1.board_rebuild_force_groups(&g, 0, &plan);
    assert!(
        !assigned(&v1),
        "find-the-capital alone takes only scouts and fast bodies"
    );

    let mut ai = on();
    ai.enable_unseen_capital_is_unseen();
    ai.enable_objective_board();
    ai.board_rebuild_force_groups(&g, 0, &plan);
    assert!(assigned(&ai), "the warrior takes the hunt");
}

fn competing_scout_goals() -> (Game, AdvancedAi, u32, StrategicPlan, Pos) {
    let (mut g, _) = board();
    g.disaster_intensity = 0;
    g.storms.clear();
    g.at_war.insert((0, 1));
    let scout = g.spawn_test_unit("scout", 0, at(10, 11));
    let generic = at(4, 4);
    for pos in g.wdisk(generic, 2) {
        g.players[0].explored.remove(&pos);
    }
    let mut ai = on();
    ai.enable_objective_board();
    ai.base
        .explore_goal
        .borrow_mut()
        .insert(scout, (generic, g.turn));
    let goal = ai.find_capital_goal(&g, 0, 1).expect("capital-search fog");
    assert_ne!(goal, generic);
    assert!(ai.base.has_exploration_target(&g, 0, scout));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, scout, plan, goal)
}

#[test]
fn a_free_scout_follows_its_capital_hunt_through_the_production_unit_pass() {
    let (mut g, mut ai, scout, plan, goal) = competing_scout_goals();
    let start = g.units[&scout].pos;
    let before = g.wdist(start, goal);
    ai.advanced_units(&mut g, 0, &plan);
    let after = g.units[&scout].pos;
    assert!(g.wdist(after, goal) < before,
        "the capital hunt must beat the held generic frontier: {start:?} -> {after:?}, goal {goal:?}");
    assert!(
        ai.objective_board().forces.iter().any(|force| {
            force.objective_key == ObjectiveKey::FindCapital(1) && force.units.contains(&scout)
        }),
        "an exploring Scout must be available for the named capital row"
    );
}

#[test]
fn the_capital_hunt_does_not_take_a_bound_civilian_guard() {
    let (mut g, mut ai, scout, plan, _) = competing_scout_goals();
    let settler = g.spawn_test_unit("settler", 0, at(6, 11));
    ai.settler_guards.insert(settler, scout);
    ai.board_rebuild_force_groups(&g, 0, &plan);
    assert!(!ai.objective_board().forces.iter().any(|force| {
        force.objective_key == ObjectiveKey::FindCapital(1) && force.units.contains(&scout)
    }));
    assert_eq!(ai.distance_scout_step(&mut g, 0, scout), None);
}

#[test]
fn the_disabled_capital_hunt_keeps_the_generic_exploration_commitment() {
    let (mut g, mut ai, scout, plan, _) = competing_scout_goals();
    ai.disable_find_the_capital();
    let generic = ai.base.explore_goal.borrow()[&scout].0;
    let before = g.wdist(g.units[&scout].pos, generic);
    ai.advanced_units(&mut g, 0, &plan);
    assert!(g.wdist(g.units[&scout].pos, generic) < before);
    assert!(ai
        .objective_board()
        .forces
        .iter()
        .all(|force| { !matches!(force.objective_key, ObjectiveKey::FindCapital(_)) }));
}

#[test]
fn a_capital_lead_retires_when_its_ground_closes() {
    let (mut g, mut ai, scout, plan, goal) = competing_scout_goals();
    ai.board_rebuild_force_groups(&g, 0, &plan);
    assert_eq!(ai.rival_lead_goal(&g, 0, scout), Some(goal));
    g.at_war.clear();
    assert_eq!(
        ai.rival_lead_goal(&g, 0, scout),
        None,
        "the retained assignment cannot overrule closed borders"
    );
}

#[test]
fn a_capital_lead_respects_the_explorers_retired_goals() {
    let (g, mut ai, scout, plan, goal) = competing_scout_goals();
    ai.board_rebuild_force_groups(&g, 0, &plan);
    assert_eq!(ai.rival_lead_goal(&g, 0, scout), Some(goal));
    ai.base
        .explore_dead
        .borrow_mut()
        .entry(scout)
        .or_default()
        .insert(goal, g.turn + 5);
    assert_eq!(ai.rival_lead_goal(&g, 0, scout), None);
}
