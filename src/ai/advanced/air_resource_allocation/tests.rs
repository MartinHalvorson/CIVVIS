use super::super::*;
use crate::ai::Ai;
use crate::setup::GameSpeed;

fn fixture(stock: f64) -> (Game, AdvancedAi, Vec<u32>, [u32; 2]) {
    let mut g = Game::new_full(2, 40, 24, 380_200, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let first = g.found_city_for(0, (6, 12), None);
    let second = g.found_city_for(0, (12, 12), None);
    let objective = g.found_city_for(1, (24, 12), None);
    for goal in ["advanced_flight", "synthetic_materials"] {
        g.players[0].techs.extend(
            g.rules.tech_ancestors[goal]
                .iter()
                .map(|tech| Name::new(tech)),
        );
        g.players[0].techs.insert(Name::new(goal));
    }
    g.map.tiles.get_mut(&(6, 12)).unwrap().resource = Some(crate::name!("aluminum"));
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), stock);
    g.players[0].gold = 2000.0;
    g.players[0].gold_per_turn = 50.0;
    g.game_speed = GameSpeed::Online;
    g.at_war.clear();
    g.record_contact(0, 1);
    g.current = 0;
    g.turn = 150;
    for cid in [first, second] {
        crate::game::install_test_district(&mut g, cid, "aerodrome");
        g.cities.get_mut(&cid).unwrap().pop = 8;
        for pos in g.cities[&cid].owned_tiles.clone() {
            if pos != g.cities[&cid].pos {
                let tile = g.map.tiles.get_mut(&pos).unwrap();
                tile.hills = true;
                tile.improvement = Some(crate::name!("mine"));
            }
        }
    }
    let cavalry = [(6, 12), (6, 13), (12, 12), (12, 13)]
        .into_iter()
        .map(|pos| g.spawn_test_unit("cavalry", 0, pos))
        .collect();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.base.book_pos = 4;
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(objective),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    });
    assert_eq!(g.strategic_resource_rate(0, "aluminum"), 2.0);
    assert!(ai.threatened_city(&g, 0).is_none());
    assert!(ai.domination_bomber_queue_needed(&g, 0, first));
    (g, ai, cavalry, [first, second])
}

fn next_owned_turn(g: &mut Game) {
    loop {
        g.apply(g.current, &Action::EndTurn).unwrap();
        if g.current == 0 {
            break;
        }
    }
}

#[test]
fn stockpile_grace_pays_existing_ground_deficit_for_real_resource_ticks() {
    let (mut g, _, cavalry, cities) = fixture(34.0);
    for uid in cavalry {
        g.apply(0, &Action::UpgradeUnit { unit: uid }).unwrap();
    }
    assert_eq!(g.strategic_stockpile(0, &crate::name!("aluminum")), 30.0);
    // A two-plane wing plus four Helicopters burns four Aluminum per turn.
    // Prove the short bank fails the actual upkeep processor before asking
    // the AI to reject its apparent speed-scaled grace-period bridge.
    let mut launched = g.clone();
    for cid in cities {
        launched.spawn_test_unit("bomber", 0, launched.cities[&cid].pos);
    }
    let grace = g.standard_duration(air_surge::AIR_SURGE_ALUMINUM_GRACE);
    for _ in 0..grace {
        next_owned_turn(&mut launched);
    }
    assert!(launched.players[0].strategic_resource_shortages[&crate::name!("aluminum")] > 0);
    assert!(
        AdvancedAi::air_surge_bomber_goal(&g, 0) < 2,
        "the launch bank must cover existing ground fuel deficits too"
    );
}

#[test]
fn actual_turn_keeps_the_aluminum_source_for_two_bomber_queues() {
    assert_dispatch_wing(false);
}

#[test]
fn observed_dispatch_keeps_the_aluminum_source_for_two_bomber_queues() {
    assert_dispatch_wing(true);
}

fn assert_dispatch_wing(observed: bool) {
    let (mut g, mut ai, cavalry, _) = fixture(8.0);
    ai.observed_player = observed;
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
    let log_start = g.log.len();
    ai.take_turn(&mut g, 0);
    let bombers = g
        .units
        .values()
        .filter(|unit| unit.owner == 0 && g.rules.units[unit.kind].promotion_class == "air_bomber")
        .count();
    let queued = g.player_city_ids(0).iter().flat_map(|cid| &g.cities[cid].queue)
        .filter(|item| matches!(item, Item::Unit { unit } if g.rules.units[unit].promotion_class == "air_bomber"))
        .count();
    eprintln!(
        "applied={:?}; wing={bombers}+{queued}; cavalry={:?}; stock={}",
        g.log.since(log_start).collect::<Vec<_>>(),
        cavalry
            .iter()
            .map(|uid| g.units.get(uid).map(|unit| unit.kind))
            .collect::<Vec<_>>(),
        g.strategic_stockpile(0, &crate::name!("aluminum"))
    );
    assert!(
        bombers + queued >= 2,
        "the canonical dispatcher must commit the launch wing"
    );
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 2);
}
