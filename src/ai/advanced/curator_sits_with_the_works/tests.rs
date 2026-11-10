use super::*;
use crate::game::GovernorState;
use crate::setup::GameSpeed;
use std::sync::Arc;

/// Two cities of ours, Pingala sitting in the first since turn 0 with
/// `promotions`, a Recovery plan, and the given Writing/Art/Music works housed
/// in each city (`works[0]` in Pingala's, `works[1]` in the other).
fn board(promotions: &[&str], works: [usize; 2]) -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new(3, 32, 22, 71, 250, 0);
    g.game_speed = GameSpeed::Online;
    let barbarians: Vec<u32> = g
        .units
        .iter()
        .filter(|(_, unit)| g.players[unit.owner].is_barbarian)
        .map(|(id, _)| *id)
        .collect();
    for id in barbarians {
        g.remove_unit(id);
    }
    g.barb_pid = None;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let old = g.found_city_for(0, g.units[&settler].pos, None);
    g.remove_unit(settler);
    let new = g.found_city_for(0, (10, 10), None);
    for cid in [old, new] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.queue.clear();
        city.loyalty = 100.0;
    }
    g.players[0].governor_roster.insert(
        "pingala".to_string(),
        GovernorState {
            city: Some(old),
            assigned_turn: 0,
            disabled_until: 0,
            promotions: promotions.iter().map(|p| p.to_string()).collect(),
        },
    );
    g.players[0].governors = vec![old];
    g.turn = g.standard_duration(16) * 8;
    house(&mut g, old, works[0]);
    house(&mut g, new, works[1]);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 5,
        assessed_turn: g.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_curator_sits_with_the_works();
    (g, ai, plan, old, new)
}

/// House `count` Writing works in `city` as the host reports them.
fn house(g: &mut Game, city: u32, count: usize) {
    let mut housing = g
        .observed_great_work_housing
        .as_deref()
        .cloned()
        .unwrap_or_default();
    housing.insert(city, [("writing".to_string(), count)].into_iter().collect());
    g.observed_great_work_housing = Some(Arc::new(housing));
}

const TREE: [&str; 4] = ["connoisseur", "researcher", "grants", "curator"];

#[test]
fn curator_sits_with_the_works_is_reversible_and_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "curator-sits-with-the-works",
        |ai| ai.curator_sits_with_the_works,
    );
}

#[test]
fn the_curator_moves_to_the_works_under_the_recovery_plan() {
    let (mut g, mut ai, plan, old, new) = board(&TREE, [0, 3]);
    assert_eq!(ai.curator_destination(&g, 0, &plan), Some((new, 3, 0)));
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(new));
    assert_ne!(Some(old), g.players[0].governor_roster["pingala"].city);
}

#[test]
fn the_curator_stays_without_a_clear_margin_or_inside_the_stay() {
    let (g, ai, plan, _, _) = board(&TREE, [2, 3]);
    assert_eq!(
        ai.curator_destination(&g, 0, &plan),
        None,
        "one more is no margin"
    );
    let (g, ai, plan, _, new) = board(&TREE, [2, 5]);
    assert_eq!(ai.curator_destination(&g, 0, &plan), Some((new, 5, 2)));
    let (mut g, ai, plan, _, _) = board(&TREE, [2, 5]);
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .assigned_turn = g.turn - 1;
    assert_eq!(
        ai.curator_destination(&g, 0, &plan),
        None,
        "inside the stay"
    );
    // The stay counts from his last assignment by anyone, even where he
    // holds no works: a seat just given is held, not traded back.
    let (mut g, ai, plan, _, new) = board(&TREE, [0, 3]);
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .assigned_turn = g.turn - 1;
    assert_eq!(ai.curator_destination(&g, 0, &plan), None);
    assert!(matches!(
        ai.curator_post(&g, 0, &plan),
        Some(CuratorPost::Hold { city, works: 3, .. }) if city == new
    ));
}

#[test]
fn no_curator_another_governor_or_another_target_stays_put() {
    let (g, ai, plan, _, _) = board(&["connoisseur", "researcher", "grants"], [0, 5]);
    assert_eq!(ai.curator_destination(&g, 0, &plan), None, "no Curator");

    let (mut g, ai, plan, _, new) = board(&TREE, [0, 5]);
    let mut victor = g.players[0].governor_roster["pingala"].clone();
    victor.city = Some(new);
    g.players[0].governor_roster.insert("victor".into(), victor);
    assert_eq!(ai.curator_destination(&g, 0, &plan), None, "occupied");

    let (mut g, _, plan, old, _) = board(&TREE, [0, 5]);
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_curator_sits_with_the_works();
    assert_eq!(
        science.curator_destination(&g, 0, &plan),
        None,
        "science seat"
    );
    let mut off = AdvancedAi::targeting(VictoryTarget::Culture);
    off.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(
        g.players[0].governor_roster["pingala"].city,
        Some(old),
        "gene off"
    );
}

#[test]
fn curator_comes_next_only_once_four_works_stand() {
    // Three works: the stock Recovery order stands (Space Initiative before
    // Curator), and no early Curator is taken.
    let (mut g, ai, plan, old, new) = board(&["connoisseur", "researcher", "grants"], [1, 2]);
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 10);
    assert!(!ai.curator_comes_next(&g, 0));
    assert_eq!(
        ai.preferred_governor_promotion(&g, 0, plan.strategy, "pingala")
            .as_deref(),
        Some("space_initiative"),
        "three works: the stock order"
    );
    assert!(!ai.promote_the_curator(&mut g, 0));

    // Four works: Curator is next, and a title in hand takes it first.
    house(&mut g, old, 2);
    house(&mut g, new, 2);
    assert!(ai.curator_comes_next(&g, 0));
    assert_eq!(
        ai.preferred_governor_promotion(&g, 0, plan.strategy, "pingala")
            .as_deref(),
        Some("curator")
    );
    assert_eq!(
        ai.preferred_governor_promotion(&g, 0, plan.strategy, "magnus"),
        AdvancedAi::targeting(VictoryTarget::Culture).preferred_governor_promotion(
            &g,
            0,
            plan.strategy,
            "magnus"
        ),
        "another governor keeps the stock order"
    );
    assert!(ai.promote_the_curator(&mut g, 0));
    assert!(g.players[0].governor_roster["pingala"]
        .promotions
        .contains("curator"));

    // Off, the Recovery plan's order stands with the works too.
    let (mut g, _, plan, _, _) = board(&["connoisseur", "researcher", "grants"], [3, 3]);
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 10);
    let off = AdvancedAi::targeting(VictoryTarget::Culture);
    assert_eq!(
        off.preferred_governor_promotion(&g, 0, plan.strategy, "pingala")
            .as_deref(),
        Some("space_initiative")
    );
}

/// Live civvis-20261110T193814Z, rebuilt: Stockholm (6 works, Loyalty 100)
/// holds a Curator Pingala, newly settled Malmö (Loyalty 60) has no
/// Governor, and with `reyna` Reyna sits in a loyal Göteborg. Returns the
/// game, the gene's seat, the plan and the three cities.
fn loyalty_board(reyna: bool) -> (Game, AdvancedAi, StrategicPlan, [u32; 3]) {
    let (mut g, ai, plan, stockholm, malmo) = board(&TREE, [6, 0]);
    let goteborg_site = g
        .map
        .tiles
        .values()
        .filter(|tile| {
            g.rules.is_passable(tile)
                && !g.rules.is_water(tile)
                && tile.owner_city.is_none()
                && g.cities
                    .values()
                    .all(|city| g.wdist(city.pos, tile.pos) >= 5)
        })
        .map(|tile| tile.pos)
        .min()
        .expect("room for a third city");
    let goteborg = g.found_city_for(0, goteborg_site, None);
    for cid in [stockholm, goteborg] {
        g.cities.get_mut(&cid).unwrap().loyalty = 100.0;
    }
    g.cities.get_mut(&malmo).unwrap().loyalty = 60.0;
    if reyna {
        g.players[0].governor_roster.insert(
            "reyna".to_string(),
            GovernorState {
                city: Some(goteborg),
                assigned_turn: 0,
                disabled_until: 0,
                promotions: Default::default(),
            },
        );
        g.players[0].governors = vec![stockholm, goteborg];
    }
    (g, ai, plan, [stockholm, malmo, goteborg])
}

/// Both governor passes, in the advanced turn's order, over `frames` turns;
/// returns how many times Pingala's post changed.
fn play_frames(g: &mut Game, ai: &mut AdvancedAi, plan: &StrategicPlan, frames: u32) -> usize {
    let mut moves = 0;
    let mut post = g.players[0].governor_roster["pingala"].city;
    for _ in 0..frames {
        ai.relocate_governors_for_dividends(g, 0, plan);
        ai.base.curator_keeps_his_seat = ai.curator_seat_active(g);
        ai.base.reassign_governor_for_loyalty(g, 0);
        let now = g.players[0].governor_roster["pingala"].city;
        if now != post {
            moves += 1;
            post = now;
        }
        g.turn += 1;
    }
    moves
}

#[test]
fn the_loyalty_backstop_sends_another_governor_before_the_curator() {
    let (mut g, mut ai, plan, [stockholm, malmo, _]) = loyalty_board(true);
    assert_eq!(
        play_frames(&mut g, &mut ai, &plan, 8),
        0,
        "Pingala never moves"
    );
    assert_eq!(
        g.players[0].governor_roster["pingala"].city,
        Some(stockholm)
    );
    assert_eq!(g.players[0].governor_roster["reyna"].city, Some(malmo));

    // Off, the backstop takes Pingala (the alphabet breaks his tie with Reyna).
    let (mut g, _, plan, [_, malmo, _]) = loyalty_board(true);
    let mut off = AdvancedAi::targeting(VictoryTarget::Culture);
    play_frames(&mut g, &mut off, &plan, 1);
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(malmo));
}

#[test]
fn a_lone_curator_answers_the_loyalty_call_once_and_holds_the_seat() {
    let (mut g, mut ai, plan, [stockholm, malmo, _]) = loyalty_board(false);
    assert_eq!(
        play_frames(&mut g, &mut ai, &plan, 8),
        1,
        "one reassignment, never traded back"
    );
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(malmo));
    assert!(matches!(
        ai.curator_post(&g, 0, &plan),
        Some(CuratorPost::Hold { city, works: 6, .. }) if city == stockholm
    ));

    // Once Malmö is loyal and the stay has run, he goes back to the works.
    g.cities.get_mut(&malmo).unwrap().loyalty = 100.0;
    g.turn += g.standard_duration(CURATOR_MIN_STAY_STANDARD);
    assert_eq!(
        ai.curator_destination(&g, 0, &plan),
        Some((stockholm, 6, 0))
    );
}
