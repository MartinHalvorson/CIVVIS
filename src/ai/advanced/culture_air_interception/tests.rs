use super::*;
use crate::ai::Ai;
use std::sync::Arc;

fn install_airfield(g: &mut Game, city: u32, pos: Pos) {
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("aerodrome"), pos);
    let tile = g.map.tiles.get_mut(&pos).unwrap();
    tile.owner_city = Some(city);
    tile.district = Some(crate::name!("aerodrome"));
    tile.pillaged = false;
    assert!(g.air_capacity_at(0, pos) >= 2);
}

fn fixture() -> (Game, AdvancedAi, StrategicPlan, [u32; 2], [Pos; 2]) {
    let mut g = Game::new_full(4, 36, 24, 59002899, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
        tile.improvement_owner = None;
        tile.pillaged = false;
    }
    let base = (6, 8);
    let own = g.found_city_for(0, (5, 8), None);
    install_airfield(&mut g, own, base);
    g.found_city_for(0, (2, 14), None);
    g.found_city_for(1, (25, 16), None);
    let town = g.found_city_for(1, (11, 16), None);
    g.cities.get_mut(&town).unwrap().is_capital = false;
    let cities = [
        g.found_city_for(2, (15, 8), None),
        g.found_city_for(2, (16, 4), None),
    ];
    g.found_city_for(3, (22, 20), None);
    let theaters = [(14, 8), (15, 4)];
    for (city, pos) in cities.into_iter().zip(theaters) {
        g.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(crate::name!("theater_square"), pos);
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.owner_city = Some(city);
        tile.district = Some(crate::name!("theater_square"));
        tile.pillaged = false;
    }
    for rival in [1, 2, 3] {
        g.record_contact(0, rival);
    }
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.players[1].dvp = 16;
    g.current = 0;
    g.turn = 185;
    g.players[0].gold = 10000.0;
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 100.0);
    let bombers = [
        g.spawn_test_unit("bomber", 0, base),
        g.spawn_test_unit("bomber", 0, base),
    ];
    for pos in [(13, 8), (14, 4)] {
        g.spawn_test_unit("scout", 0, pos);
    }
    let powers = Arc::make_mut(&mut g.observed_military_power);
    powers.insert(0, 1307.0);
    powers.insert(1, 200.0);
    powers.insert(2, 2506.0);
    let stats = Arc::make_mut(&mut g.observed_public_empire_stats);
    stats.entry(0).or_default().domestic_tourists = Some(100);
    let racer = stats.entry(2).or_default();
    racer.foreign_tourists = Some(85);
    racer.culture_turns_to_victory = Some(10.0);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_one_war_at_a_time();
    ai.enable_diplomatic_contender_eliminated();
    ai.enable_elimination_yields_to_a_launch();
    ai.one_war_observe(&g, 0);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(town),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, bombers, theaters)
}

#[test]
fn baseline_keeps_the_elimination_war_beside_a_declarable_outgunning_culture_racer() {
    let (mut g, mut ai, plan, bombers, theaters) = fixture();
    assert!(g.military_power(0) < g.military_power(2));
    assert!(ai.urgent_victory_threat(&g, 2));
    assert!(ai.preferred_war_opening(&g, 0, 2).is_some_and(|a| matches!(
        a,
        Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }
    )));
    assert_eq!(ai.diplomatic_contender_to_eliminate(&g, 0), Some(1));
    assert!(!ai.elimination_yields_to_a_launch(&g, 0, 1));
    assert_ne!(ai.one_war_second_front(&g, 0), Some(2));
    let opening = ai.preferred_war_opening(&g, 0, 2).unwrap();
    let mut physical = g.speculative_clone();
    physical.apply(0, &opening).unwrap();
    for (uid, target) in bombers.into_iter().zip(theaters) {
        physical
            .apply(0, &Action::AirPillage { unit: uid, target })
            .unwrap();
        assert!(physical.units[&uid].hp >= 60);
        assert!(physical.map.get(target).unwrap().pillaged);
    }
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1));
    assert!(!g.is_at_war(0, 2));
}

fn version_two(ai: &mut AdvancedAi) {
    ai.disable_elimination_yields_to_a_launch();
    ai.enable_elimination_yields_to_a_launch_2();
}

#[test]
fn two_current_selected_sorties_redirect_the_plan_and_open_the_second_war() {
    let (mut g, mut ai, mut plan, bombers, theaters) = fixture();
    version_two(&mut ai);
    ai.enable_declaration_needs_production_parity();
    assert!(
        ai.one_war_second_front(&g, 0) != Some(2),
        "fixture: the ordinary second-front selector refuses the outgunned rival"
    );
    assert!(ai.culture_air_package(&g, 0, 2, &plan).is_some());
    let before = serde_json::to_vec(&g).unwrap();
    ai.apply_war_plan_to_strategy(&mut plan);
    ai.apply_air_surge_to_strategy(&g, 0, &mut plan);
    ai.apply_culture_air_interception_to_strategy(&g, 0, &mut plan);
    assert_eq!(plan.target_player, Some(2));
    assert_eq!(g.cities[&plan.target_city.unwrap()].owner, 2);
    assert_eq!(
        serde_json::to_vec(&g).unwrap(),
        before,
        "preflight does not change the board"
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        g.is_at_war(0, 2),
        "the fresh package opens beside the ordinary-city war"
    );
    for uid in bombers {
        let action = ai
            .advanced_air_action(&g, 0, uid, &plan)
            .expect("selected real mission");
        assert!(matches!(action, Action::AirPillage { .. }));
        g.apply(0, &action).unwrap();
        assert!(g.units[&uid].hp >= 60);
    }
    assert!(theaters
        .into_iter()
        .all(|pos| g.map.get(pos).unwrap().pillaged));
    assert_eq!(
        g.players[0].counters["elimination-yields-to-a-launch-2:opened"],
        1
    );
}

#[test]
fn the_fresh_package_interrupts_an_otherwise_current_plan() {
    let (mut g, mut ai, plan, _, theaters) = fixture();
    version_two(&mut ai);
    ai.deny_leaders = false;
    let journal = crate::reasoning::Journal::recording();
    ai.attach_journal(journal.handle());
    ai.plan = Some(plan);
    assert!(
        !ai.plan_stale(&g, 0),
        "fixture keeps the normal five-turn plan"
    );
    ai.take_turn_inner(&mut g, 0);
    assert!(
        g.is_at_war(0, 2),
        "plan {:?}; target {:?}; clock {:?}; package {:?}; thoughts {:?}",
        ai.current_plan(),
        ai.active_victory_target(&g),
        g.culture_turns_to_victory(2),
        ai.culture_air_package(&g, 0, 2, ai.current_plan().unwrap()),
        journal
            .since(0)
            .thoughts
            .iter()
            .map(|t| (&t.headline, &t.detail))
            .collect::<Vec<_>>()
    );
    assert_eq!(ai.current_plan().unwrap().target_player, Some(2));
    assert!(
        theaters
            .into_iter()
            .any(|pos| g.map.get(pos).unwrap().pillaged),
        "the real serial unit path performs a useful sortie"
    );
}

#[test]
fn physical_and_policy_failures_preserve_the_old_target() {
    for case in [
        "off",
        "one-plane",
        "spent",
        "hurt",
        "one-theater",
        "no-route",
        "strong-aa",
        "fog",
        "peace",
        "friend",
        "treasury",
        "home",
        "war-exposes-home",
        "forced",
        "recovery",
        "lane",
        "long-clock",
        "unbacked",
        "dead",
        "minor",
    ] {
        let (mut g, mut ai, mut plan, bombers, theaters) = fixture();
        version_two(&mut ai);
        match case {
            "off" => ai.disable_elimination_yields_to_a_launch_2(),
            "one-plane" => {
                g.remove_unit(bombers[1]);
            }
            "spent" => g.units.get_mut(&bombers[1]).unwrap().attacks_left = 0,
            "hurt" => g.units.get_mut(&bombers[1]).unwrap().hp = 79,
            "one-theater" => g.map.tiles.get_mut(&theaters[1]).unwrap().pillaged = true,
            "no-route" => {
                Arc::make_mut(&mut g.rules)
                    .units
                    .get_mut("bomber")
                    .unwrap()
                    .range = 1;
            }
            "strong-aa" => {
                Arc::make_mut(&mut g.rules)
                    .units
                    .get_mut("mobile_sam")
                    .unwrap()
                    .anti_air_strength = 300.0;
                g.spawn_test_unit("mobile_sam", 2, (14, 7));
                g.spawn_test_unit("mobile_sam", 2, (15, 3));
            }
            "fog" => {
                Arc::make_mut(&mut g.host_observed).insert((5, 8));
            }
            "peace" => ai.peace_until = g.turn + 1,
            "friend" => {
                g.players[0].friends_until.insert(2, g.turn + 20);
                g.players[2].friends_until.insert(0, g.turn + 20);
            }
            "treasury" => {
                g.players[0].gold = 0.0;
                g.players[0].gold_per_turn = -50.0;
            }
            "home" => plan.threatened_city = g.player_city_ids(0).first().copied(),
            "war-exposes-home" => {
                let ring: Vec<_> = g
                    .wring((5, 8), 2)
                    .into_iter()
                    .filter(|pos| {
                        g.map
                            .get(*pos)
                            .is_some_and(|tile| tile.owner_city.is_none())
                    })
                    .take(6)
                    .collect();
                assert_eq!(ring.len(), 6);
                for pos in ring {
                    g.spawn_test_unit("modern_armor", 2, pos);
                }
                assert!(ai.threatened_city(&g, 0).is_none());
                let mut war = g.speculative_clone();
                war.apply(0, &ai.preferred_war_opening(&g, 0, 2).unwrap())
                    .unwrap();
                assert!(
                    ai.threatened_city(&war, 0).is_some(),
                    "fixture: the new war exposes home"
                );
            }
            "forced" => ai.forced_target_player = Some(1),
            "recovery" => plan.strategy = GrandStrategy::Recovery,
            "lane" => ai.retarget(VictoryTarget::Science),
            "long-clock" => {
                Arc::make_mut(&mut g.observed_public_empire_stats)
                    .entry(2)
                    .or_default()
                    .culture_turns_to_victory = Some(100.0)
            }
            "unbacked" => {
                Arc::make_mut(&mut g.observed_public_empire_stats)
                    .entry(2)
                    .or_default()
                    .foreign_tourists = Some(0)
            }
            "dead" => g.players[2].alive = false,
            "minor" => g.players[2].is_minor = true,
            _ => unreachable!(),
        }
        assert!(ai.culture_air_package(&g, 0, 2, &plan).is_none(), "{case}");
        ai.apply_culture_air_interception_to_strategy(&g, 0, &mut plan);
        assert_eq!(plan.target_player, Some(1), "{case}");
        assert!(
            !ai.open_culture_air_interception(&mut g, 0, &plan),
            "{case}"
        );
        assert!(!g.is_at_war(0, 2), "{case}");
    }
}

fn next_own_turn(g: &mut Game) {
    let turn = g.turn;
    while g.turn == turn || g.current != 0 {
        assert!(g.winner.is_none());
        g.apply(g.current, &Action::EndTurn).unwrap();
    }
}

#[test]
fn staging_moves_one_plane_per_turn_and_waits_for_real_readiness() {
    let (mut g, mut ai, mut plan, old, theaters) = fixture();
    version_two(&mut ai);
    let rear_city = g.found_city_for(0, (0, 0), None);
    install_airfield(&mut g, rear_city, (1, 0));
    for uid in old {
        g.remove_unit(uid);
    }
    let bombers = [
        g.spawn_test_unit("bomber", 0, (1, 0)),
        g.spawn_test_unit("bomber", 0, (1, 0)),
    ];
    assert!(ai.culture_air_package(&g, 0, 2, &plan).is_none());
    ai.stage_culture_air_interception(&mut g, 0, &plan);
    assert_eq!(
        bombers
            .iter()
            .filter(|uid| g.units[uid].pos != (1, 0))
            .count(),
        1
    );
    assert!(!g.is_at_war(0, 2));
    assert!(
        ai.culture_air_package(&g, 0, 2, &plan).is_none(),
        "a forecast is not fresh readiness"
    );
    let positions = bombers.map(|uid| g.units[&uid].pos);
    ai.stage_culture_air_interception(&mut g, 0, &plan);
    assert_eq!(bombers.map(|uid| g.units[&uid].pos), positions);
    next_own_turn(&mut g);
    ai.stage_culture_air_interception(&mut g, 0, &plan);
    assert_eq!(
        bombers
            .iter()
            .filter(|uid| g.units[uid].pos != (1, 0))
            .count(),
        2
    );
    assert!(!g.is_at_war(0, 2));
    assert!(ai.culture_air_package(&g, 0, 2, &plan).is_none());
    next_own_turn(&mut g);
    assert!(ai.culture_air_package(&g, 0, 2, &plan).is_some());
    ai.apply_culture_air_interception_to_strategy(&g, 0, &mut plan);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 2));
    for uid in bombers {
        let action = ai.advanced_air_action(&g, 0, uid, &plan).unwrap();
        assert!(matches!(action, Action::AirPillage { .. }));
        g.apply(0, &action).unwrap();
    }
    assert!(theaters
        .into_iter()
        .all(|pos| g.map.get(pos).unwrap().pillaged));
}

#[test]
fn a_changed_board_invalidates_a_previously_ready_opening() {
    for change in ["friend", "fog", "spent", "pillaged"] {
        let (mut g, mut ai, mut plan, bombers, theaters) = fixture();
        version_two(&mut ai);
        assert!(ai.culture_air_package(&g, 0, 2, &plan).is_some());
        ai.apply_culture_air_interception_to_strategy(&g, 0, &mut plan);
        assert_eq!(plan.target_player, Some(2));
        match change {
            "friend" => {
                g.players[0].friends_until.insert(2, g.turn + 20);
                g.players[2].friends_until.insert(0, g.turn + 20);
            }
            "fog" => {
                Arc::make_mut(&mut g.host_observed).insert(theaters[0]);
            }
            "spent" => g.units.get_mut(&bombers[1]).unwrap().attacks_left = 0,
            "pillaged" => g.map.tiles.get_mut(&theaters[1]).unwrap().pillaged = true,
            _ => unreachable!(),
        }
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert!(!g.is_at_war(0, 2), "{change}");
        assert!(!g.players[0]
            .counters
            .contains_key("elimination-yields-to-a-launch-2:opened"));
    }
}

fn capture_capital(g: &mut Game, owner: usize) -> u32 {
    let id = g
        .cities
        .values()
        .find(|city| city.owner == owner && city.is_capital)
        .unwrap()
        .id;
    g.cities.get_mut(&id).unwrap().owner = 0;
    id
}

#[test]
fn a_final_capital_front_is_not_diverted_to_a_different_culture_rival() {
    let (mut g, mut ai, mut plan, _, _) = fixture();
    version_two(&mut ai);
    let captured = capture_capital(&mut g, 2);
    capture_capital(&mut g, 3);
    g.cities
        .get_mut(&captured)
        .unwrap()
        .districts
        .remove(crate::name!("theater_square"));
    // Its captured capital's Theater returns to the rival's ordinary town,
    // preserving two useful targets while the last original capital is France's.
    let town = g.found_city_for(2, (11, 8), None);
    let target = (14, 8);
    g.map.tiles.get_mut(&target).unwrap().owner_city = Some(town);
    g.cities
        .get_mut(&town)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), target);
    let final_capital = g
        .cities
        .values()
        .find(|city| city.owner == 1 && city.is_capital)
        .unwrap()
        .id;
    plan.target_city = Some(final_capital);
    ai.enable_domination_finish_holds_the_front();
    assert!(AdvancedAi::capture_completes_domination(
        &g,
        0,
        final_capital
    ));
    assert_eq!(ai.domination_finish_front(&g, 0), Some((1, final_capital)));
    let mut physical = g.speculative_clone();
    physical
        .apply(0, &ai.preferred_war_opening(&g, 0, 2).unwrap())
        .unwrap();
    for uid in physical.player_unit_ids(0) {
        if let Some(action) = ai.culture_air_sortie(&physical, &physical, 0, uid, 2) {
            physical.apply(0, &action).unwrap();
        }
    }
    assert!(
        physical.map.get((14, 8)).unwrap().pillaged && physical.map.get((15, 4)).unwrap().pillaged
    );
    assert!(ai.culture_air_package(&g, 0, 2, &plan).is_none());
    ai.apply_culture_air_interception_to_strategy(&g, 0, &mut plan);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(final_capital));
    ai.stage_culture_air_interception(&mut g, 0, &plan);
    assert!(!g.players[0]
        .counters
        .contains_key("elimination-yields-to-a-launch-2:staged"));
    assert!(!ai.open_culture_air_interception(&mut g, 0, &plan));
}

#[test]
fn the_culture_racer_can_itself_hold_the_final_capital() {
    let (mut g, mut ai, mut plan, _, _) = fixture();
    version_two(&mut ai);
    capture_capital(&mut g, 1);
    capture_capital(&mut g, 3);
    ai.enable_domination_finish_holds_the_front();
    assert!(ai.culture_air_package(&g, 0, 2, &plan).is_some());
    ai.apply_culture_air_interception_to_strategy(&g, 0, &mut plan);
    assert_eq!(plan.target_player, Some(2));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 2));
}

#[test]
fn serial_staging_keeps_the_first_plane_in_position_until_its_partner_is_ready() {
    let (mut g, mut ai, plan, old, theaters) = fixture();
    version_two(&mut ai);
    ai.deny_leaders = false;
    let rear = g.found_city_for(0, (0, 0), None);
    install_airfield(&mut g, rear, (1, 0));
    for uid in old {
        g.remove_unit(uid);
    }
    let planes = [
        g.spawn_test_unit("bomber", 0, (1, 0)),
        g.spawn_test_unit("bomber", 0, (1, 0)),
    ];
    // Controlled successive observations keep the targets visible; the test
    // advances actual readiness instead of resetting it in a forecast.
    Arc::make_mut(&mut g.host_observed).extend(theaters);
    let journal = crate::reasoning::Journal::recording();
    ai.attach_journal(journal.handle());
    ai.plan = Some(plan);
    for step in 0..3 {
        ai.take_turn_inner(&mut g, 0);
        eprintln!(
            "step {step} turn {} plan {:?}; lane {:?}; clock {:?}; planes {:?}; counters {:?}",
            g.turn,
            ai.current_plan(),
            ai.active_victory_target(&g),
            g.culture_turns_to_victory(2),
            planes.map(|uid| g
                .units
                .get(&uid)
                .map(|u| (u.pos, u.moves_left, u.attacks_left))),
            g.players[0].counters
        );
        if g.is_at_war(0, 2) {
            break;
        }
        assert!(
            step < 2,
            "the real serial path never assembled its staged package: {:?}",
            journal
                .since(0)
                .thoughts
                .iter()
                .map(|t| (&t.headline, &t.detail))
                .collect::<Vec<_>>()
        );
        next_own_turn(&mut g);
    }
    assert!(g.is_at_war(0, 2));
    assert!(theaters
        .into_iter()
        .all(|pos| g.map.get(pos).unwrap().pillaged));
    assert!(planes.into_iter().all(|uid| g.units.contains_key(&uid)));
    assert!(
        g.players[0]
            .counters
            .get("elimination-yields-to-a-launch-2:held")
            .copied()
            .unwrap_or(0)
            > 0
    );
}
