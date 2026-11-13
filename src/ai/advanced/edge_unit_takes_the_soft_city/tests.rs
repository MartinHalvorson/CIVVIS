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
    // Version 2: the check repeats every turn for ten Online turns first,
    // spending no Science and holding no Gold meanwhile.
    let (mut g, _) = fixture(&["mining", "bronze_working"]);
    one_city(&mut g);
    g.turn = 19;
    let mut ai = from_the_start();
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert_eq!(w.start, Some(true));
    assert_eq!(w.recheck_since, Some(19));
    assert!(!w.start_supplied);
    assert_eq!(ai.edge_unit_research_goal(&g, 0), None);
    assert_eq!(counter(&g, "start_recheck"), 1);
    assert_eq!(counter(&g, "start_closed"), 0);
    assert_eq!(ai.edge_upgrade_gold_held(&g, 0), 0.0);
    // Still none nine turns on: still checking, journalled once.
    g.turn = 28;
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(window(&ai).start, Some(true));
    assert_eq!(counter(&g, "start_recheck"), 1);
    // Ten turns on, the opening stands down.
    g.turn = 19 + super::EDGE_START_RECHECK_TURNS;
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert_eq!(w.start, Some(false));
    assert!(!w.start_supplied);
    assert_eq!(ai.edge_unit_research_goal(&g, 0), None);
    assert_eq!(counter(&g, "start_closed"), 1);
    assert_eq!(ai.edge_upgrade_gold_held(&g, 0), 0.0);
}

#[test]
fn a_source_found_on_the_recheck_commits_the_opening() {
    let (mut g, _) = fixture(&["mining", "bronze_working"]);
    one_city(&mut g);
    g.turn = 19;
    let mut ai = from_the_start();
    let journal = crate::reasoning::Journal::recording();
    ai.base.journal = journal.handle();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(window(&ai).recheck_since, Some(19));
    // Iron comes into reach three turns later (a border grows over it).
    g.turn = 22;
    with_iron(&mut g);
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert_eq!(w.start, Some(true));
    assert!(w.start_supplied);
    assert_eq!(
        ai.edge_unit_research_goal(&g, 0),
        Some(crate::name!("iron_working"))
    );
    assert_eq!(counter(&g, "start_recheck_found"), 1);
    assert_eq!(counter(&g, "start_closed"), 0);
    let details: Vec<String> = journal
        .since(0)
        .thoughts
        .iter()
        .map(|thought| thought.detail.clone())
        .collect();
    assert!(
        details.iter().any(|detail| detail
            .starts_with("edge-unit-takes-the-soft-city: start: recheck: no iron source")),
        "{details:?}"
    );
    assert!(
        details
            .iter()
            .any(|detail| detail.contains("into the recheck")),
        "{details:?}"
    );
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

// ---- Version 2, from the first live game (civvis-20261113T205538Z) ----

fn details(journal: &crate::reasoning::Journal) -> Vec<String> {
    journal
        .since(0)
        .thoughts
        .iter()
        .map(|thought| thought.detail.clone())
        .collect()
}

/// The rival's city defends at `defence`: its best unit ever, less 10, plus
/// what the city adds (the Palace's 3), read off a Warrior's 20 first.
fn rival_defends_at(g: &mut Game, defence: i64) {
    let city = g.city_at(RIVAL).unwrap();
    let counters = &mut g.players[1].counters;
    counters.insert("strongest_unit_built".to_string(), 20);
    let base = g.city_strength(city) as i64;
    g.players[1]
        .counters
        .insert("strongest_unit_built".to_string(), defence - base + 20);
    assert_eq!(g.city_strength(city), defence as f64);
}

/// The window named on the rival's city at turn 40, with `legions` full
/// Legions of ours beside our capital.
fn named_with(legions: usize) -> (Game, AdvancedAi, u32, Vec<u32>) {
    let (mut g, city) = fixture(&["mining", "bronze_working", "iron_working"]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    let bodies: Vec<u32> = [(5, 9), (5, 7), (3, 8), (3, 9)]
        .into_iter()
        .take(legions)
        .map(|pos| g.spawn_test_unit("legion", 0, pos))
        .collect();
    let mut ai = gene_on();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(window(&ai).target.map(|target| target.city), Some(city));
    (g, ai, city, bodies)
}

/// A new turn for the force: every move back.
fn fresh_moves(g: &mut Game, bodies: &[u32]) {
    for uid in bodies {
        if let Some(body) = g.units.get_mut(uid) {
            body.moves_left = 2.0;
            body.acted = false;
        }
    }
}

#[test]
fn an_unwalled_city_at_edge_plus_six_stays_the_target_with_three_legions() {
    // Live 205538Z: Mbamba, never walled, went from defence 17 to 34 while
    // the force was raised, and the window closed at edge +6.
    let (mut g, mut ai, city, bodies) = named_with(3);
    let journal = crate::reasoning::Journal::recording();
    ai.base.journal = journal.handle();
    rival_defends_at(&mut g, 34);
    g.turn = EDGE_LAST_TURN + 1;
    fresh_moves(&mut g, &bodies);
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    let lines = details(&journal);
    assert!(!w.spent, "{lines:?}");
    assert_eq!(w.target.map(|target| target.city), Some(city));
    assert_eq!(w.kept, Some("forecast"), "{lines:?}");
    assert_eq!(counter(&g, "kept"), 1);
    assert_eq!(counter(&g, "closed"), 0);
    assert!(
        lines.iter().any(
            |line| line.starts_with("edge-unit-takes-the-soft-city: kept: ")
                && line.contains("edge +6")
                && line.contains("the forecast with the 3 legion owned beside it")
        ),
        "{lines:?}"
    );
}

#[test]
fn an_unwalled_city_keeps_to_the_floor_before_three_legions_are_owned() {
    // One Legion owned: kept at edge +1, closed below -4.
    let (mut g, mut ai, city, _) = named_with(1);
    rival_defends_at(&mut g, 39);
    g.turn = EDGE_LAST_TURN + 1;
    ai.maintain_edge_unit_window(&mut g, 0);
    let w = window(&ai);
    assert!(!w.spent);
    assert_eq!(w.target.map(|target| target.city), Some(city));
    assert_eq!(w.kept, Some("edge"));
    rival_defends_at(&mut g, 45);
    g.turn += 1;
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(window(&ai).spent);
    assert_eq!(counter(&g, "closed"), 1);
}

#[test]
fn a_walled_city_without_a_breaker_closes_past_the_naming_turn() {
    // Behind walls the naming rule holds: no Masonry, no Ram, not the target.
    let (mut g, mut ai, city, bodies) = named_with(3);
    let journal = crate::reasoning::Journal::recording();
    ai.base.journal = journal.handle();
    rival_defends_at(&mut g, 34);
    g.cities.get_mut(&city).unwrap().wall_hp = 100;
    g.turn = EDGE_LAST_TURN + 1;
    fresh_moves(&mut g, &bodies);
    ai.maintain_edge_unit_window(&mut g, 0);
    let lines = details(&journal);
    assert!(window(&ai).spent, "{lines:?}");
    assert_eq!(counter(&g, "closed"), 1);
    assert_eq!(counter(&g, "kept"), 0);
    assert!(
        lines.iter().any(|line| line.starts_with(
            "edge-unit-takes-the-soft-city: closed: past the naming turn the named city is no longer the target"
        ) && line.contains("no breaker")),
        "{lines:?}"
    );
}

#[test]
fn the_first_legion_denounces() {
    let (g, _, _, _) = named_with(1);
    assert!(!g.is_at_war(0, 1));
    assert!(g.players[0]
        .denounced_until
        .get(&1)
        .is_some_and(|until| *until > g.turn));
    assert_eq!(counter(&g, "denounced"), 1);
    // Never before a unit exists.
    let (g, _, _, _) = named_with(0);
    assert!(!g.players[0].denounced_until.contains_key(&1));
    assert_eq!(counter(&g, "denounced"), 0);
}

#[test]
fn gold_is_held_for_the_upgrades_while_the_window_is_open() {
    // The first gene alone (no from-the-start flag): a Warrior that upgrades
    // into a Legion holds its bill back from other purchases.
    let (mut g, _) = fixture(&["mining", "bronze_working", "iron_working"]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    g.spawn_test_unit("warrior", 0, CAPITAL);
    let mut ai = gene_on();
    assert_eq!(ai.edge_upgrade_gold_held(&g, 0), 0.0, "no window yet");
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(window(&ai).live());
    let price = g
        .unit_upgrade_price_in_formation(0, crate::name!("warrior"), legion(), 0)
        .unwrap()
        .0;
    let held = ai.edge_upgrade_gold_held(&g, 0);
    assert!(held > 0.0 && (held - price).abs() < 1e-6, "{held} {price}");
    assert!(ai.purchase_upkeep_floor(&g, 0, 0.0) >= held);
    assert_eq!(ai.edge_unit_held_for(&g, 0), Some(legion()));
    // No body to upgrade, nothing held.
    let (mut g, _) = fixture(&["mining", "bronze_working", "iron_working"]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    let mut ai = gene_on();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(window(&ai).live());
    assert_eq!(ai.edge_upgrade_gold_held(&g, 0), 0.0);
}

#[test]
fn a_wounded_legion_heals_before_it_marches() {
    let (mut g, _) = fixture(&["mining", "bronze_working", "iron_working"]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    // Far from any city of ours: heals in place. Two tiles from the
    // capital: heals in it. At full health: walks.
    let afield = g.spawn_test_unit("legion", 0, (8, 2));
    let near_home = g.spawn_test_unit("legion", 0, (6, 8));
    let fit = g.spawn_test_unit("legion", 0, (5, 9));
    for uid in [afield, near_home] {
        g.units.get_mut(&uid).unwrap().hp = 50;
    }
    assert!(g
        .player_city_ids(0)
        .iter()
        .all(|cid| g.wdist(g.cities[cid].pos, (8, 2)) > super::EDGE_HEAL_CITY_TILES));
    assert!(g.wdist((6, 8), CAPITAL) <= super::EDGE_HEAL_CITY_TILES);
    let fit_before = g.wdist(g.units[&fit].pos, RIVAL);
    let mut ai = gene_on();
    let journal = crate::reasoning::Journal::recording();
    ai.base.journal = journal.handle();
    ai.maintain_edge_unit_window(&mut g, 0);
    let lines = details(&journal);
    assert_eq!(g.units[&afield].pos, (8, 2), "{lines:?}");
    assert!(g.units[&afield].moves_left <= 0.0);
    assert!(g.wdist(g.units[&near_home].pos, CAPITAL) < 2, "{lines:?}");
    assert!(g.wdist(g.units[&fit].pos, RIVAL) < fit_before, "{lines:?}");
    assert_eq!(counter(&g, "heal"), 2, "{lines:?}");
    assert!(
        lines.iter().any(
            |line| line.starts_with("edge-unit-takes-the-soft-city: heal: legion")
                && line.contains("heals in place")
        ),
        "{lines:?}"
    );
    // The unit pass's staging hook holds the wounded too.
    g.units.get_mut(&afield).unwrap().moves_left = 2.0;
    assert_eq!(ai.edge_unit_staging_step(&mut g, 0, afield), Some(false));
    assert_eq!(g.units[&afield].pos, (8, 2));
    // Asked again the same episode, journalled once.
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(counter(&g, "heal"), 2);
}

/// Our third city, nearest the rival, and a fourth far from it.
const THIRD: Pos = (9, 13);
const FAR: Pos = (1, 18);

#[test]
fn three_cities_train_the_legion_at_once() {
    let (mut g, _) = fixture(&["mining", "bronze_working", "iron_working", "pottery"]);
    let third = g.found_city_for(0, THIRD, None);
    let far = g.found_city_for(0, FAR, None);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    let capital = g.city_at(CAPITAL).unwrap();
    let second = g.city_at(SECOND).unwrap();
    let mut ai = gene_on();
    // The opening book is done: the claim chain runs, not the turn start.
    ai.base.book_pos = 4;
    let journal = crate::reasoning::Journal::recording();
    ai.base.journal = journal.handle();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert!(window(&ai).live());
    let granary = Item::Building {
        building: crate::name!("granary"),
    };
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    for city in g.cities.values_mut() {
        city.queue.clear();
    }
    g.cities.get_mut(&second).unwrap().queue = vec![granary.clone()];
    g.cities.get_mut(&third).unwrap().queue = vec![builder.clone()];
    ai.claim_edge_unit_queue(&mut g, 0);
    let body = Item::Unit { unit: legion() };
    for cid in [capital, second, third] {
        assert_eq!(
            g.cities[&cid].queue.first(),
            Some(&body),
            "{}: {:?}",
            g.cities[&cid].name,
            details(&journal)
        );
    }
    assert!(
        g.cities[&far].queue.is_empty(),
        "the fourth city is left alone"
    );
    assert_eq!(AdvancedAi::edge_force(&g, 0, legion()), (0, 3));
    let lines = details(&journal);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("ahead of granary (it keeps its progress)")),
        "{lines:?}"
    );
    // The Granary claim and its kin leave the queued Legion in place.
    assert!(ai.first_capital_training_holds(&g, 0, second, &body));
    assert!(!ai.first_capital_training_holds(&g, 0, far, &body));

    // A city's first Settler is never deferred: the capital's went to the
    // second city, so only the capital's Settler gives way.
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    for city in g.cities.values_mut() {
        city.queue.clear();
    }
    g.cities.get_mut(&capital).unwrap().queue = vec![settler.clone()];
    g.cities.get_mut(&second).unwrap().queue = vec![settler.clone()];
    g.cities.get_mut(&third).unwrap().queue = vec![settler.clone()];
    ai.claim_edge_unit_queue(&mut g, 0);
    assert_eq!(g.cities[&capital].queue.first(), Some(&body));
    assert_eq!(g.cities[&second].queue.first(), Some(&settler));
    assert_eq!(g.cities[&third].queue.first(), Some(&settler));
}

#[test]
fn during_the_opening_book_the_turn_start_fills_idle_queues_only() {
    // Live 205538Z: the claim chain waits for the opening book, which held
    // the capital to turn 37.
    let (mut g, _) = fixture(&["mining", "bronze_working", "iron_working"]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    let capital = g.city_at(CAPITAL).unwrap();
    let second = g.city_at(SECOND).unwrap();
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    g.cities.get_mut(&second).unwrap().queue = vec![builder.clone()];
    let mut ai = gene_on();
    assert!(ai.base.book_pos < 4);
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(
        g.cities[&capital].queue.first(),
        Some(&Item::Unit { unit: legion() })
    );
    assert_eq!(g.cities[&second].queue.first(), Some(&builder));
}

/// A Warrior in our capital and a Slinger in our second city; Archery and
/// Iron Working known, Iron in stock, the counters reading only these two.
fn two_upgrades_on_offer(gold: f64) -> (Game, AdvancedAi, u32, u32) {
    let (mut g, city) = fixture(&["mining", "bronze_working", "iron_working", "archery"]);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    let warrior = g.spawn_test_unit("warrior", 0, CAPITAL);
    let slinger = g.spawn_test_unit("slinger", 0, SECOND);
    g.players[0].counters.remove("strongest_unit_built");
    g.players[0].counters.remove("strongest_ranged_built");
    g.players[0].gold = gold;
    g.players[0].gold_per_turn = 5.0;
    let mut ai = gene_on();
    ai.enable_one_upgrade_lifts_every_city();
    ai.edge_unit_window = Some(EdgeWindow {
        unit: Some(legion()),
        target: Some(EdgeTarget {
            player: 1,
            city,
            pos: RIVAL,
            owner_civ: g.players[1].civ.clone(),
        }),
        opened: Some(30),
        ..EdgeWindow::default()
    });
    (g, ai, warrior, slinger)
}

fn price(g: &Game, from: &str, to: &str) -> f64 {
    g.unit_upgrade_price_in_formation(0, Name::new(from), Name::new(to), 0)
        .unwrap()
        .0
}

#[test]
fn with_gold_for_one_upgrade_the_legion_is_the_one_taken() {
    // Gold for one of the two. The Archer lifts every city's strike by 10
    // for less Gold than the Legion's 20 to defence, so the pass takes it
    // first when nothing is held.
    let (g, _, _, _) = two_upgrades_on_offer(0.0);
    let (legion_bill, archer_bill) = (
        price(&g, "warrior", "legion"),
        price(&g, "slinger", "archer"),
    );
    assert!(10.0 / archer_bill > 20.0 / legion_bill);
    let gold = legion_bill.max(archer_bill) + 1.0;
    assert!(gold < legion_bill + archer_bill);

    let (mut g, mut ai, warrior, slinger) = two_upgrades_on_offer(gold);
    ai.edge_unit_window = None;
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 1);
    assert_eq!(g.units[&slinger].kind, crate::name!("archer"));
    assert_eq!(g.units[&warrior].kind, crate::name!("warrior"));

    // The window open and holding the Legion's bill: the Legion.
    let (mut g, ai, warrior, slinger) = two_upgrades_on_offer(gold);
    assert!((ai.edge_upgrade_gold_held(&g, 0) - legion_bill).abs() < 1e-6);
    assert_eq!(ai.upgrades_that_lift_every_city(&mut g, 0), 1);
    assert_eq!(g.units[&warrior].kind, legion());
    assert_eq!(g.units[&slinger].kind, crate::name!("slinger"));
    // Nothing is held once the Warrior is a Legion.
    assert_eq!(ai.edge_upgrade_gold_held(&g, 0), 0.0);
}

#[test]
fn the_warrior_to_legion_upgrade_is_bought_once_and_counted_in_the_force() {
    // The one-upgrade pass buys it: the edge gene buys no second one and
    // counts the Legion.
    let (mut g, mut ai, warrior, _) = two_upgrades_on_offer(1_000.0);
    let journal = crate::reasoning::Journal::recording();
    ai.base.journal = journal.handle();
    assert!(ai.upgrades_that_lift_every_city(&mut g, 0) >= 1);
    assert_eq!(g.units[&warrior].kind, legion());
    assert_eq!(AdvancedAi::edge_force(&g, 0, legion()).0, 1);
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(counter(&g, "upgraded"), 0);
    let lines = details(&journal);
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("warrior to legion"))
            .count(),
        1,
        "{lines:?}"
    );
    assert!(
        lines
            .iter()
            .any(|line| line.starts_with("edge-unit-takes-the-soft-city: force: 1 legion owned")),
        "{lines:?}"
    );

    // The edge gene's pass runs first in the turn: it buys it, and the
    // one-upgrade pass after it buys no second one.
    let (mut g, mut ai, warrior, _) = two_upgrades_on_offer(1_000.0);
    ai.edge_unit_window = None;
    let journal = crate::reasoning::Journal::recording();
    ai.base.journal = journal.handle();
    ai.maintain_edge_unit_window(&mut g, 0);
    assert_eq!(g.units[&warrior].kind, legion());
    assert_eq!(counter(&g, "upgraded"), 1);
    ai.upgrades_that_lift_every_city(&mut g, 0);
    let lines = details(&journal);
    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("warrior to legion"))
            .count(),
        1,
        "{lines:?}"
    );
}
