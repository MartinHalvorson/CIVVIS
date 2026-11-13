use super::*;
use crate::setup::GameSpeed;

#[test]
fn score_seat_holds_its_capital_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "score-seat-holds-its-capital",
        |ai| ai.score_seat_holds_its_capital,
    );
}

/// Two majors on open grassland at Online turn 35, every unit removed: our
/// capital at (4, 10), our second city at (10, 10), six tiles from the
/// rival's city at (16, 10), so the capital is held for the clock and the
/// second city for the frontier. Masonry known, every queue empty. Returns
/// the board and `[capital, frontier]`.
fn board() -> (Game, Vec<u32>) {
    let mut g = Game::new_full(2, 40, 24, 4_177, 300, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    g.players[0].civ = "Gran Colombia".to_string();
    let capital = g.found_city_for(0, (4, 10), None);
    let frontier = g.found_city_for(0, (10, 10), None);
    g.found_city_for(1, (16, 10), None);
    g.record_contact(0, 1);
    g.players[0].techs.insert(crate::name!("masonry"));
    g.game_speed = GameSpeed::Online;
    g.turn = 35;
    g.current = 0;
    for cid in [capital, frontier] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 3;
        city.queue.clear();
    }
    assert!(g.cities[&capital].is_capital, "fixture: the capital");
    (g, vec![capital, frontier])
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_score_seat_holds_its_capital();
    }
    ai
}

fn walls() -> Item {
    Item::Building {
        building: crate::name!("walls"),
    }
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: crate::name::Name::new(name),
    }
}

fn plan(g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: g.turn,
        rush: false,
    }
}

fn claim_over(ai: &AdvancedAi, g: &Game, heads: [Item; 2]) -> Game {
    let mut board = g.clone();
    for (cid, head) in g.player_city_ids(0).into_iter().zip(heads) {
        board.cities.get_mut(&cid).unwrap().queue = vec![head];
    }
    let plan = plan(&board);
    ai.claim_score_capital_defences(&mut board, 0, &plan);
    board
}

/// The capital is held from Online turn 30 and the frontier city whenever a
/// rival city stands within eight tiles; neither on another target or with
/// the gene off.
#[test]
fn the_capital_from_turn_thirty_and_the_frontier_are_held() {
    let (mut g, cities) = board();
    let ai = seat(VictoryTarget::Score, true);
    assert!(ai.score_held_city(&g, 0, cities[0]));
    assert!(ai.score_held_city(&g, 0, cities[1]));
    g.turn = 20;
    assert!(!ai.score_held_city(&g, 0, cities[0]), "before turn 30");
    assert!(
        ai.score_held_city(&g, 0, cities[1]),
        "the frontier at any turn"
    );
    g.turn = 35;
    for off in [
        seat(VictoryTarget::Score, false),
        seat(VictoryTarget::Domination, true),
        seat(VictoryTarget::Culture, true),
    ] {
        assert!(!off.score_held_city(&g, 0, cities[0]));
        assert!(!off.score_held_city(&g, 0, cities[1]));
    }
}

/// Walls go ahead of a wonder and a district and hold; off, or for another
/// target, both keep their queue.
#[test]
fn walls_go_ahead_of_a_wonder_and_a_district() {
    let (g, cities) = board();
    let wonder = Item::Wonder {
        wonder: crate::name!("pyramids"),
        pos: (5, 11),
    };
    let campus = Item::District {
        district: crate::name!("campus"),
        pos: (11, 11),
    };
    let heads = [wonder, campus];
    for ai in [
        seat(VictoryTarget::Score, false),
        seat(VictoryTarget::Science, true),
    ] {
        let kept = claim_over(&ai, &g, heads.clone());
        for (cid, head) in cities.iter().zip(&heads) {
            assert_eq!(kept.cities[cid].queue.first(), Some(head));
        }
    }
    let ai = seat(VictoryTarget::Score, true);
    let on = claim_over(&ai, &g, heads);
    for cid in &cities {
        assert_eq!(on.cities[cid].queue.first(), Some(&walls()));
        assert!(ai.score_capital_holds(&on, 0, *cid, &walls()));
    }
}

/// Once Ancient Walls stand at full health and Castles is known, the next
/// tier is the Medieval Walls.
#[test]
fn the_medieval_walls_follow_the_ancient_ones() {
    let (mut g, cities) = board();
    g.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    let full = g.city_max_wall_hp(&g.cities[&cities[0]]);
    g.cities.get_mut(&cities[0]).unwrap().wall_hp = full;
    g.players[0].techs.insert(crate::name!("construction"));
    g.players[0].techs.insert(crate::name!("castles"));
    let next = AdvancedAi::score_next_walls(&g, 0, cities[0]);
    assert_eq!(
        next,
        Some(Item::Building {
            building: crate::name!("medieval_walls")
        })
    );
}

/// The first two Settlers, a Builder, a soldier and anything finishing this
/// turn keep their queue.
#[test]
fn settlers_builders_soldiers_and_finishing_work_keep_their_queue() {
    let (mut g, cities) = board();
    let ai = seat(VictoryTarget::Score, true);
    let kept = claim_over(&ai, &g, [unit("settler"), unit("builder")]);
    assert_eq!(
        kept.cities[&cities[0]].queue.first(),
        Some(&unit("settler"))
    );
    assert_eq!(
        kept.cities[&cities[1]].queue.first(),
        Some(&unit("builder"))
    );
    let warrior = unit("warrior");
    let cost = g.item_cost_for_city(0, cities[1], &warrior);
    g.cities
        .get_mut(&cities[1])
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&warrior), cost - 0.01);
    let library = Item::Building {
        building: crate::name!("library"),
    };
    let kept = claim_over(&ai, &g, [unit("slinger"), library.clone()]);
    assert_eq!(
        kept.cities[&cities[0]].queue.first(),
        Some(&unit("slinger"))
    );
    assert_eq!(kept.cities[&cities[1]].queue.first(), Some(&walls()));
    let kept = claim_over(&ai, &g, [library, warrior.clone()]);
    assert_eq!(
        kept.cities[&cities[1]].queue.first(),
        Some(&warrior),
        "finishing this turn"
    );
}

/// With its Walls standing (no Castles), a held city trains a shooter; a
/// shooter standing in its centre ends the claim and keeps the post at
/// peace, which the Monument claim then leaves alone.
#[test]
fn a_shooter_follows_the_walls_and_keeps_the_centre() {
    let (mut g, cities) = board();
    for cid in &cities {
        g.cities
            .get_mut(cid)
            .unwrap()
            .buildings
            .push(crate::name!("walls"));
    }
    g.players[0].techs.insert(crate::name!("archery"));
    let ai = seat(VictoryTarget::Score, true);
    let (item, _) = ai
        .score_capital_item(&g, 0, cities[0])
        .expect("a shooter for the capital");
    let Item::Unit { unit: kind } = &item else {
        panic!("{item:?}");
    };
    assert_eq!(g.rules.units[kind].promotion_class, "ranged");
    let mut culture = ai.clone();
    culture.enable_domination_seat_raises_its_culture();
    let plan = plan(&g);
    assert_eq!(
        culture.domination_monument_passes(&g, 0, cities[0], &plan, &item, true),
        None,
        "the Monument claim leaves the held shooter alone"
    );
    let at = g.cities[&cities[0]].pos;
    let archer = g.spawn_test_unit("archer", 0, at);
    assert!(ai.score_capital_item(&g, 0, cities[0]).is_none());
    assert!(ai.score_holds_the_centre(&g, 0, archer));
    assert!(ai.keeps_the_centre(&g, 0, archer), "kept at peace");
    let off = seat(VictoryTarget::Score, false);
    assert!(!off.keeps_the_centre(&g, 0, archer));
}
