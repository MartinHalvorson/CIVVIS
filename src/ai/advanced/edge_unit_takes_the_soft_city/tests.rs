use super::super::*;
use super::{
    edge_candidate, edge_unit, EdgeTarget, EdgeWindow, EDGE_LAST_TURN, EDGE_MUSTER_LAST_TURN,
    EDGE_WAR_PATIENCE,
};

/// Our capital, our second city, and the rival's city eight tiles east.
const CAPITAL: Pos = (4, 8);
const SECOND: Pos = (4, 14);
const RIVAL: Pos = (12, 8);
/// A tile beside our capital, inside its borders.
const IRON: Pos = (5, 8);

/// Seat 0 is Rome with two cities, knowing `techs`; major 1 holds one city
/// eight tiles from our capital, met and explored, with no soldier: it
/// defends at about 13, so a Legion (40) outclasses it by 27. Online speed,
/// turn 40.
fn fixture(techs: &[&str]) -> (Game, u32) {
    let mut g = Game::new_full(2, 30, 20, 5_267_001, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.players[0].civ = "Rome".to_string();
    g.found_city_for(0, CAPITAL, None);
    g.found_city_for(0, SECOND, None);
    let city = g.found_city_for(1, RIVAL, None);
    g.record_contact(0, 1);
    g.players[0].explored.insert(RIVAL);
    g.players[0].explored.insert(IRON);
    g.players[0].techs.clear();
    for tech in techs {
        g.players[0].techs.insert(Name::new(tech));
    }
    for city in g.cities.values_mut() {
        city.queue.clear();
    }
    g.players[0].gold = 0.0;
    g.current = 0;
    g.game_speed = crate::setup::GameSpeed::Online;
    g.turn = 40;
    (g, city)
}

/// An Iron source on a Hill beside our capital.
fn with_iron(g: &mut Game) {
    let tile = g.map.tiles.get_mut(&IRON).unwrap();
    tile.resource = Some(crate::name!("iron"));
    tile.hills = true;
}

fn gene_on() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_edge_unit_takes_the_soft_city();
    ai
}

fn counter(g: &Game, key: &str) -> i64 {
    g.players[0]
        .counters
        .get(&format!("edge_unit:{key}"))
        .copied()
        .unwrap_or(0)
}

fn legion() -> Name {
    crate::name!("legion")
}

fn window(ai: &AdvancedAi) -> EdgeWindow {
    ai.edge_unit_window.clone().expect("the window is kept")
}

#[test]
fn off_reads_nothing() {
    let (mut g, _) = fixture(&["mining", "bronze_working", "iron_working"]);
    with_iron(&mut g);
    let warrior = g.spawn_test_unit("warrior", 0, CAPITAL);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(ai.edge_unit_window, None);
    assert_eq!(ai.edge_unit_research_goal(&g, 0), None);
    ai.claim_edge_unit_queue(&mut g, 0);
    assert!(g
        .player_city_ids(0)
        .iter()
        .all(|cid| g.cities[cid].queue.is_empty()));
    assert_eq!(ai.edge_unit_staging_step(&mut g, 0, warrior), None);
    assert_eq!(ai.edge_unit_staging_rally(&g, 0), None);
    assert!(!ai.edge_unit_force_member(&g, warrior));
    assert!(!g.players[0]
        .counters
        .keys()
        .any(|key| key.starts_with("edge_unit:")));
}

#[test]
fn rome_s_edge_unit_is_the_legion() {
    let (g, _) = fixture(&[]);
    let edge = edge_unit(&g, 0).expect("an edge unit");
    assert_eq!(edge.unit, legion());
    assert_eq!(edge.strength, 40.0);
    assert_eq!(
        edge.missing,
        vec![
            crate::name!("mining"),
            crate::name!("bronze_working"),
            crate::name!("iron_working")
        ]
    );
    let (g, _) = fixture(&["mining", "bronze_working"]);
    assert_eq!(
        edge_unit(&g, 0).map(|edge| edge.missing),
        Some(vec![crate::name!("iron_working")])
    );
    // Rome never trains the Swordsman the Legion replaces.
    assert_eq!(edge_candidate(&g, 0, crate::name!("swordsman")), None);
    // Another civilization has no Legion.
    let (mut g, _) = fixture(&["mining", "bronze_working"]);
    g.players[0].civ = "America".to_string();
    assert_eq!(edge_candidate(&g, 0, legion()), None);
    assert!(edge_unit(&g, 0).is_some_and(|edge| edge.unit != legion()));
}

#[test]
fn the_unit_chosen_first_is_held_for_the_game() {
    // A Writing seat finds a stronger Medieval unit within three
    // technologies; a window that chose the Legion keeps it.
    let writing = ["mining", "pottery", "writing", "currency", "mathematics"];
    let (mut g, _) = fixture(&writing);
    let later = edge_unit(&g, 0).expect("an edge unit");
    assert!(later.strength > 40.0, "{later:?}");
    let mut ai = gene_on();
    ai.edge_unit_window = Some(EdgeWindow {
        unit: Some(legion()),
        ..EdgeWindow::default()
    });
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(window(&ai).unit, Some(legion()));
}

#[test]
fn a_soft_city_opens_the_window_and_iron_working_is_the_goal_once_iron_is_in_reach() {
    let (mut g, city) = fixture(&["mining", "bronze_working"]);
    with_iron(&mut g);
    let mut ai = gene_on();
    let journal = crate::reasoning::Journal::recording();
    ai.base.journal = journal.handle();
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert_eq!(w.opened, Some(40));
    assert_eq!(w.waiting, None);
    assert_eq!(w.unit, Some(legion()));
    assert_eq!(
        w.target.as_ref().map(|target| (target.player, target.city)),
        Some((1, city))
    );
    assert_eq!(w.goal, Some(crate::name!("iron_working")));
    assert_eq!(
        ai.edge_unit_research_goal(&g, 0),
        Some(crate::name!("iron_working"))
    );
    assert_eq!(counter(&g, "opened"), 1);
    let details: Vec<String> = journal
        .since(0)
        .thoughts
        .iter()
        .map(|thought| thought.detail.clone())
        .collect();
    assert!(
        details
            .iter()
            .any(|detail| detail.starts_with("edge-unit-takes-the-soft-city: opened: legion")),
        "{details:?}"
    );
    // Asked again, the window stays on the same city and counts one opening.
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(counter(&g, "opened"), 1);
    assert_eq!(window(&ai).target.map(|target| target.city), Some(city));
}

#[test]
fn before_bronze_working_the_goal_is_bronze_working() {
    let (mut g, _) = fixture(&["mining"]);
    with_iron(&mut g);
    let mut ai = gene_on();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(
        ai.edge_unit_research_goal(&g, 0),
        Some(crate::name!("bronze_working"))
    );
}

#[test]
fn no_iron_in_reach_at_bronze_working_spends_no_science() {
    let (mut g, _) = fixture(&["mining", "bronze_working"]);
    let mut ai = gene_on();
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert!(
        w.waiting
            .as_deref()
            .is_some_and(|why| why.starts_with("no iron source")),
        "{w:?}"
    );
    assert_eq!(w.opened, None);
    assert_eq!(ai.edge_unit_research_goal(&g, 0), None);
    assert_eq!(counter(&g, "no_source"), 1);
    // Iron held in stock is supply enough.
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 20.0);
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(
        ai.edge_unit_research_goal(&g, 0),
        Some(crate::name!("iron_working"))
    );
}

#[test]
fn the_window_needs_two_cities_and_closes_after_its_last_turn() {
    let (mut g, _) = fixture(&["mining", "bronze_working"]);
    with_iron(&mut g);
    let second = g.city_at(SECOND).unwrap();
    g.cities.get_mut(&second).unwrap().owner = 1;
    let mut ai = gene_on();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(window(&ai).opened, None);
    assert_eq!(ai.edge_unit_research_goal(&g, 0), None);

    let (mut g, city) = fixture(&["mining", "bronze_working"]);
    with_iron(&mut g);
    let mut ai = gene_on();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(window(&ai).opened.is_some());
    // Past the naming turn the named city, still soft, stays the target.
    g.turn = EDGE_LAST_TURN + 1;
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert!(!w.spent);
    assert_eq!(w.target.map(|target| target.city), Some(city));
    // Past the muster's last turn the window closes.
    g.turn = EDGE_MUSTER_LAST_TURN + 1;
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(window(&ai).spent);
    assert_eq!(counter(&g, "closed"), 1);
    assert_eq!(ai.edge_unit_research_goal(&g, 0), None);

    // A window that named nothing by the naming turn never opens.
    let (mut g, _) = fixture(&["mining", "bronze_working"]);
    with_iron(&mut g);
    g.turn = EDGE_LAST_TURN + 1;
    let mut ai = gene_on();
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert!(w.spent && w.opened.is_none());
}

#[test]
fn walls_without_a_breaker_we_can_train_are_not_soft() {
    let (mut g, _) = fixture(&["mining", "bronze_working", "iron_working"]);
    let rival = g.city_at(RIVAL).unwrap();
    g.cities.get_mut(&rival).unwrap().wall_hp = 100;
    let ai = gene_on();
    let ours = [CAPITAL, SECOND];
    assert_eq!(
        ai.edge_soft_city(&g, 0, 40.0, super::EDGE_MIN, &ours, 10, None),
        None
    );
    g.cities.get_mut(&rival).unwrap().wall_hp = 0;
    assert!(ai
        .edge_soft_city(&g, 0, 40.0, super::EDGE_MIN, &ours, 10, None)
        .is_some_and(|soft| soft.city == rival && soft.edge >= super::EDGE_MIN));
}

#[test]
fn the_capital_trains_the_legion_and_a_warrior_upgrades_into_one() {
    let (mut g, _) = fixture(&["mining", "bronze_working", "iron_working"]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    g.players[0].gold = 1_000.0;
    g.players[0].gold_per_turn = 0.0;
    let warrior = g.spawn_test_unit("warrior", 0, CAPITAL);
    let mut ai = gene_on();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(
        g.units[&warrior].kind,
        legion(),
        "the upgrade ahead of the floor"
    );
    assert_eq!(counter(&g, "upgraded"), 1);
    ai.claim_edge_unit_queue(&mut g, 0);
    let capital = g.city_at(CAPITAL).unwrap();
    assert_eq!(
        g.cities[&capital].queue.first(),
        Some(&Item::Unit { unit: legion() })
    );
    assert!(counter(&g, "trained") >= 1);

    // A negative income takes no upgrade.
    let (mut g, _) = fixture(&["mining", "bronze_working", "iron_working"]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    g.players[0].gold = 1_000.0;
    g.players[0].gold_per_turn = -1.0;
    let warrior = g.spawn_test_unit("warrior", 0, CAPITAL);
    let mut ai = gene_on();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(g.units[&warrior].kind, crate::name!("warrior"));
}

#[test]
fn the_force_walks_to_a_stand_and_holds_it() {
    let (mut g, _) = fixture(&["mining", "bronze_working", "iron_working"]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    let far = g.spawn_test_unit("legion", 0, (5, 9));
    let stood = g.spawn_test_unit("legion", 0, (9, 8));
    let warrior = g.spawn_test_unit("warrior", 0, CAPITAL);
    let before = g.wdist(g.units[&far].pos, RIVAL);
    let mut ai = gene_on();
    // The turn-start walk: the far Legion steps toward a stand with every
    // move, the one already on a stand holds it with no move left.
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(window(&ai).opened.is_some());
    assert!(ai.edge_unit_force_member(&g, far));
    assert!(ai.edge_unit_staging_rally(&g, 0).is_some_and(|rally| {
        (super::EDGE_STAND_MIN_TILES..=super::EDGE_MUSTER_TILES).contains(&g.wdist(rally, RIVAL))
    }));
    assert!(g.wdist(g.units[&far].pos, RIVAL) < before);
    assert_eq!(g.units[&stood].pos, (9, 8));
    assert!(g.units[&stood].fortified && g.units[&stood].moves_left <= 0.0);
    assert_eq!(g.units[&warrior].pos, CAPITAL);
    // The unit pass's staging hook takes the next step with a fresh move.
    g.units.get_mut(&far).unwrap().moves_left = 2.0;
    let walked = g.wdist(g.units[&far].pos, RIVAL);
    assert!(ai.edge_unit_staging_step(&mut g, 0, far).is_some());
    assert!(g.wdist(g.units[&far].pos, RIVAL) <= walked);
    assert_eq!(ai.edge_unit_staging_step(&mut g, 0, warrior), None);
}

#[test]
fn a_raised_force_denounces_so_the_formal_war_is_ready() {
    let (mut g, _) = fixture(&["mining", "bronze_working", "iron_working"]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    for pos in [(10, 8), (10, 9), (10, 7)] {
        g.spawn_test_unit("legion", 0, pos);
    }
    let mut ai = gene_on();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(!g.is_at_war(0, 1));
    assert!(g.players[0]
        .denounced_until
        .get(&1)
        .is_some_and(|until| *until > g.turn));
    assert_eq!(counter(&g, "denounced"), 1);
    assert_eq!(window(&ai).declared, None);
}

#[test]
fn three_legions_at_the_city_declare_on_the_forecast() {
    let (mut g, city) = fixture(&["mining", "bronze_working", "iron_working"]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    for pos in [(10, 8), (10, 9), (10, 7)] {
        g.spawn_test_unit("legion", 0, pos);
    }
    // Denounced ten turns ago: the Formal War is legal.
    g.players[0].denounced_until.insert(1, 70);
    g.players[0].denounced_since.insert(1, 30);
    let mut ai = gene_on();
    let journal = crate::reasoning::Journal::recording();
    ai.base.journal = journal.handle();
    ai.maintain_edge_unit_window(&mut g, 0);
    let details: Vec<String> = journal
        .since(0)
        .thoughts
        .iter()
        .map(|thought| thought.detail.clone())
        .collect();
    assert!(g.is_at_war(0, 1), "{details:?}");
    assert_eq!(window(&ai).declared, Some(40));
    assert_eq!(counter(&g, "declared"), 1);
    assert!(
        details
            .iter()
            .any(|detail| detail.starts_with("edge-unit-takes-the-soft-city: declared: 3 legion")),
        "{details:?}"
    );
    // At war, the plan's target is the city.
    let mut plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.apply_edge_unit_to_strategy(&g, 0, &mut plan);
    assert_eq!(
        (plan.target_player, plan.target_city),
        (Some(1), Some(city))
    );
    assert_eq!(plan.strategy, GrandStrategy::Science);
}

#[test]
fn two_legions_do_not_declare() {
    let (mut g, _) = fixture(&["mining", "bronze_working", "iron_working"]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    for pos in [(10, 8), (10, 9)] {
        g.spawn_test_unit("legion", 0, pos);
    }
    let mut ai = gene_on();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(!g.is_at_war(0, 1));
    assert_eq!(window(&ai).declared, None);
}

/// A declared window on the rival's city, at war, with `legions` Legions
/// beside it, and the rival's second city at `next` when given.
fn at_war(legions: usize, next: Option<Pos>) -> (Game, AdvancedAi, u32) {
    let (mut g, city) = fixture(&["mining", "bronze_working", "iron_working"]);
    if let Some(pos) = next {
        g.found_city_for(1, pos, None);
        g.players[0].explored.insert(pos);
    }
    for k in 0..legions {
        g.spawn_test_unit("legion", 0, (10, 7 + k as i32));
    }
    g.at_war.insert((0, 1));
    let mut ai = gene_on();
    ai.edge_unit_window = Some(EdgeWindow {
        unit: Some(legion()),
        target: Some(EdgeTarget {
            player: 1,
            city,
            pos: RIVAL,
            owner_civ: g.players[1].civ.clone(),
        }),
        opened: Some(30),
        declared: Some(35),
        ..EdgeWindow::default()
    });
    (g, ai, city)
}

#[test]
fn a_capture_goes_on_to_the_next_city_in_reach() {
    let (mut g, mut ai, city) = at_war(3, Some((15, 11)));
    g.cities.get_mut(&city).unwrap().owner = 0;
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert_eq!(counter(&g, "captured"), 1);
    assert_eq!(counter(&g, "continued"), 1);
    assert!(!w.spent);
    assert_eq!(w.taken, 1);
    assert_eq!(w.target.map(|target| target.pos), Some((15, 11)));
}

#[test]
fn a_capture_with_nothing_in_reach_asks_for_peace() {
    let (mut g, mut ai, city) = at_war(3, None);
    g.cities.get_mut(&city).unwrap().owner = 0;
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert_eq!(counter(&g, "captured"), 1);
    assert_eq!(counter(&g, "peace"), 1);
    assert!(w.spent);
    assert!(ai.peace_offers.contains(&1));
}

#[test]
fn a_long_war_or_a_spent_force_asks_for_peace() {
    let (mut g, mut ai, _) = at_war(3, None);
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(
        !window(&ai).spent,
        "five war turns, three Legions: the war goes on"
    );
    g.turn = 35 + EDGE_WAR_PATIENCE;
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(window(&ai).spent);
    assert_eq!(counter(&g, "peace"), 1);

    let (mut g, mut ai, _) = at_war(1, None);
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(window(&ai).spent);
    assert_eq!(counter(&g, "peace"), 1);
}

fn from_the_start() -> AdvancedAi {
    let mut ai = gene_on();
    ai.enable_edge_unit_opens_from_the_start();
    ai
}

/// One city of ours: the second city handed to the rival.
fn one_city(g: &mut Game) {
    let second = g.city_at(SECOND).unwrap();
    g.cities.get_mut(&second).unwrap().owner = 1;
}

#[test]
fn from_the_start_researches_the_path_with_one_city() {
    let (mut g, _) = fixture(&[]);
    one_city(&mut g);
    // The first gene alone waits for two cities and names no goal.
    let mut ai = gene_on();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(ai.edge_unit_research_goal(&g, 0), None);
    assert_eq!(window(&ai).start, Some(false));
    // With the second flag the path to the Legion is the goal from turn one:
    // the technology that reveals its Iron (research walks Mining first).
    let (mut g, _) = fixture(&[]);
    one_city(&mut g);
    let mut ai = from_the_start();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(window(&ai).start, Some(true));
    assert_eq!(
        ai.edge_unit_research_goal(&g, 0),
        Some(crate::name!("bronze_working"))
    );
    assert_eq!(counter(&g, "start"), 1);
    // Asked again it journals the goal once.
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(counter(&g, "goal"), 1);
}

#[test]
fn from_the_start_stands_down_without_iron_at_bronze_working() {
    let (mut g, _) = fixture(&["mining", "bronze_working"]);
    one_city(&mut g);
    let mut ai = from_the_start();
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert_eq!(w.start, Some(false));
    assert!(!w.start_supplied);
    assert_eq!(ai.edge_unit_research_goal(&g, 0), None);
    assert_eq!(counter(&g, "start_closed"), 1);
    assert_eq!(ai.edge_upgrade_gold_held(&g, 0), 0.0);
}

#[test]
fn from_the_start_with_iron_in_reach_trains_bodies_and_holds_the_gold() {
    let (mut g, _) = fixture(&["mining", "bronze_working"]);
    with_iron(&mut g);
    one_city(&mut g);
    let warrior = g.spawn_test_unit("warrior", 0, CAPITAL);
    let mut ai = from_the_start();
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert!(w.start_supplied && w.target.is_none());
    assert_eq!(
        ai.edge_unit_research_goal(&g, 0),
        Some(crate::name!("iron_working"))
    );
    // The Warrior's upgrade into a Legion is held back from other purchases.
    let held = ai.edge_upgrade_gold_held(&g, 0);
    let price = g
        .unit_upgrade_price_in_formation(0, crate::name!("warrior"), legion(), 0)
        .unwrap()
        .0;
    assert!(held > 0.0 && (held - price).abs() < 1e-6, "{held} {price}");
    assert!(ai.purchase_upkeep_floor(&g, 0, 0.0) >= held);
    // The capital trains a second body to upgrade, with no city named.
    ai.claim_edge_unit_queue(&mut g, 0);
    let capital = g.city_at(CAPITAL).unwrap();
    assert_eq!(
        g.cities[&capital].queue.first(),
        Some(&Item::Unit {
            unit: crate::name!("warrior")
        })
    );
    assert_eq!(g.units[&warrior].kind, crate::name!("warrior"));
    // Off, nothing is held.
    let (g, _) = fixture(&["mining", "bronze_working"]);
    let ai = gene_on();
    assert_eq!(ai.edge_upgrade_gold_held(&g, 0), 0.0);
}

#[test]
fn from_the_start_stands_down_when_no_soft_city_is_named_by_turn_45() {
    let (mut g, _) = fixture(&["mining", "bronze_working"]);
    with_iron(&mut g);
    // The rival's city is out of sight: nothing to name.
    g.players[0].explored.remove(&RIVAL);
    let mut ai = from_the_start();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(!window(&ai).spent);
    g.turn = super::EDGE_START_NAME_TURN + 1;
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(window(&ai).spent);
    assert_eq!(counter(&g, "closed"), 1);
    assert_eq!(ai.edge_unit_research_goal(&g, 0), None);
}

#[test]
fn at_war_the_force_strikes_the_city() {
    let (mut g, mut ai, city) = at_war(3, None);
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(counter(&g, "assault") >= 1);
    assert!(
        g.cities[&city].hp < 200 || g.cities[&city].owner == 0,
        "health {}",
        g.cities[&city].hp
    );
}

#[test]
fn a_declaration_that_never_reached_the_board_is_asked_again() {
    let (mut g, mut ai, _) = at_war(3, None);
    g.at_war.remove(&(0, 1));
    assert!(!g.is_at_war(0, 1));
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert!(!w.spent);
    assert_eq!(w.declared, None);
    assert_eq!(w.redeclared, 1);
    assert_eq!(counter(&g, "declare_lost"), 1);
}
