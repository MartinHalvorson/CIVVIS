use super::*;
use crate::{game::GovernorState, setup::GameSpeed};

fn board() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new(3, 32, 22, 71, 250, 0);
    g.game_speed = GameSpeed::Online;
    // The rule under test is the governor's. Where this seed's Barbarians
    // happen to land must not decide whether either city counts as safe.
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
    for (cid, pop) in [(old, 2), (new, 20)] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = pop;
        city.queue.clear();
    }
    for (governor, city, promotions) in [
        (
            "pingala",
            old,
            vec!["researcher", "connoisseur", "space_initiative"],
        ),
        ("magnus", new, vec!["industrialist", "surplus_logistics"]),
    ] {
        g.players[0].governor_roster.insert(
            governor.into(),
            GovernorState {
                city: Some(city),
                assigned_turn: 0,
                disabled_until: 0,
                promotions: promotions.into_iter().map(str::to_string).collect(),
            },
        );
    }
    g.players[0].governors = vec![old, new];
    g.turn = 100;
    let pos = g.cities[&new].pos;
    g.cities
        .get_mut(&new)
        .unwrap()
        .districts
        .insert(Name::new("spaceport"), pos);
    let item = Item::Project {
        project: Name::new("launch_earth_satellite"),
    };
    g.cities.get_mut(&new).unwrap().queue = vec![item.clone()];
    let production = g.item_remaining_cost_for_city(0, new, &item)
        / (20.0 * g.item_prod_mult(0, new, Some(&item)));
    let before = g.city_yields(new).production;
    std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments)
        .entry(new)
        .or_default()
        .production += production - before;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_pingala_earns_the_launch();
    ai.live_governor_assignment_adapter = true;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 5,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, old, new)
}

#[test]
fn registered_default_off() {
    super::super::super::test_support::opt_in_off_in_both_controllers(
        "pingala-earns-the-launch",
        |ai| ai.pingala_earns_the_launch,
    );
}

#[test]
fn paid_bonus_moves_before_titles_and_prices_the_magnus_exchange() {
    let (mut g, mut ai, plan, old, new) = board();
    let before = g.log.len();
    let proposal = ai
        .earned_launch_move(&g, 0, &plan)
        .expect("long launch earns its move");
    assert_eq!(proposal.city, new);
    assert_eq!(proposal.replacement, Some(Name::new("magnus")));
    assert!(proposal.turns_saved >= 1.0);
    assert!(ai.relocate_pingala_for_launch(&mut g, 0, &plan));
    let actions: Vec<_> = g.log.since(before).map(|(_, action)| action).collect();
    assert!(
        matches!(actions.as_slice(), [Action::ReassignGovernor { governor, city },
        Action::ReassignGovernor { governor: second, city: returning }]
        if governor == "pingala" && *city == new && second == "magnus" && *returning == old)
    );
    assert_eq!(g.players[0].governor_roster["magnus"].city, Some(old));
    assert_eq!(
        g.governor_effect(0, new, "space_project_production_pct"),
        0.0
    );
    g.turn += 5;
    assert_eq!(
        g.governor_effect(0, new, "space_project_production_pct"),
        30.0
    );
}

#[test]
fn science_assignment_survives_temporary_posture_and_remote_war() {
    let (mut g, mut ai, mut plan, _, new) = board();
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    plan.strategy = GrandStrategy::Conquest;
    assert!(ai.relocate_pingala_for_launch(&mut g, 0, &plan));
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(new));
}

#[test]
fn local_threat_recovery_and_other_victory_target_keep_posts() {
    for control in 0..4 {
        let (mut g, mut ai, mut plan, old, new) = board();
        match control {
            0 => plan.threatened_city = Some(new),
            1 => plan.strategy = GrandStrategy::Recovery,
            2 => g.cities.get_mut(&old).unwrap().last_attacked = g.turn - 1,
            _ => {
                ai = AdvancedAi::targeting(VictoryTarget::Culture);
                ai.enable_pingala_earns_the_launch();
            }
        }
        assert!(
            !ai.relocate_pingala_for_launch(&mut g, 0, &plan),
            "control {control}"
        );
        assert_eq!(g.players[0].governor_roster["pingala"].city, Some(old));
    }
}

#[test]
fn construction_short_projects_and_existing_pad_keep_pingala() {
    for control in 0..3 {
        let (mut g, mut ai, plan, old, new) = board();
        match control {
            0 => {
                g.cities.get_mut(&new).unwrap().queue = vec![Item::District {
                    district: Name::new("spaceport"),
                    pos: (11, 10),
                }]
            }
            1 => {
                let item = g.cities[&new].queue[0].clone();
                let cost = g.item_cost_for_city(0, new, &item);
                g.cities.get_mut(&new).unwrap().production = cost - 1.0;
            }
            _ => {
                let pos = g.cities[&old].pos;
                g.cities
                    .get_mut(&old)
                    .unwrap()
                    .districts
                    .insert(Name::new("spaceport"), pos);
            }
        }
        assert!(
            !ai.relocate_pingala_for_launch(&mut g, 0, &plan),
            "control {control}"
        );
    }
}

#[test]
fn unavailable_promotion_and_defensive_occupant_keep_posts() {
    for control in 0..3 {
        let (mut g, mut ai, plan, old, _) = board();
        match control {
            0 => {
                g.players[0]
                    .governor_roster
                    .get_mut("pingala")
                    .unwrap()
                    .promotions
                    .remove("space_initiative");
            }
            1 => {
                g.players[0]
                    .governor_roster
                    .get_mut("pingala")
                    .unwrap()
                    .disabled_until = g.turn + 1
            }
            _ => {
                let occupant = g.players[0].governor_roster.remove("magnus").unwrap();
                g.players[0]
                    .governor_roster
                    .insert("victor".into(), occupant);
            }
        }
        assert!(
            !ai.relocate_pingala_for_launch(&mut g, 0, &plan),
            "control {control}"
        );
        assert_eq!(g.players[0].governor_roster["pingala"].city, Some(old));
    }
}

#[test]
fn lost_research_dividends_can_outweigh_the_launch() {
    let (mut g, mut ai, plan, old, new) = board();
    g.cities.get_mut(&old).unwrap().pop = 40;
    g.cities.get_mut(&new).unwrap().pop = 2;
    assert!(!ai.relocate_pingala_for_launch(&mut g, 0, &plan));
}

#[test]
fn lagged_native_frame_does_not_repeat_exchange() {
    let (mut g, mut ai, plan, _, _) = board();
    let mut stale = g.clone();
    assert!(ai.relocate_pingala_for_launch(&mut g, 0, &plan));
    assert!(!ai.relocate_pingala_for_launch(&mut stale, 0, &plan));
}

fn native_quote(g: &mut Game, cid: u32, turns: f64) {
    let key = Game::production_block_key(&g.cities[&cid].queue[0]);
    std::sync::Arc::make_mut(&mut g.host_buildable)
        .entry(cid)
        .or_default()
        .insert(
            key,
            crate::game::HostMenuEntry {
                cost: None,
                turns: Some(turns),
            },
        );
}

#[test]
fn native_quote_can_refuse_a_move_the_simulated_cost_would_admit() {
    let (mut g, mut ai, plan, _, new) = board();
    assert!(ai.earned_launch_move(&g, 0, &plan).is_some());
    native_quote(&mut g, new, 6.0);
    assert!(!ai.relocate_pingala_for_launch(&mut g, 0, &plan));
}

#[test]
fn native_post_clock_requires_history_and_survives_the_ordinary_pass() {
    let (mut g, mut ai, plan, old, new) = board();
    for name in ["pingala", "magnus"] {
        g.players[0]
            .governor_roster
            .get_mut(name)
            .unwrap()
            .assigned_turn = g.turn - 5;
    }
    assert!(!ai.relocate_pingala_for_launch(&mut g, 0, &plan));
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert!(ai.remembered_governor_posts.contains_key("pingala"));
    g.turn += 20;
    for name in ["pingala", "magnus"] {
        g.players[0]
            .governor_roster
            .get_mut(name)
            .unwrap()
            .assigned_turn = g.turn - 5;
    }
    assert!(ai.relocate_pingala_for_launch(&mut g, 0, &plan));
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(new));
    assert_eq!(g.players[0].governor_roster["magnus"].city, Some(old));
}

#[test]
fn magnus_plant_output_can_make_the_thirty_percent_bonus_a_loss() {
    let (mut g, mut ai, plan, _, new) = board();
    let pos = g.cities[&new].pos;
    g.cities
        .get_mut(&new)
        .unwrap()
        .districts
        .insert(Name::new("industrial_zone"), pos);
    g.cities
        .get_mut(&new)
        .unwrap()
        .buildings
        .push(Name::new("coal_power_plant"));
    let absent = AdvancedAi::governor_dividend_world(&g, 0, "magnus", None);
    assert_eq!(g.governor_effect(0, new, "power_plant_production"), 2.0);
    let lost = g.city_yields(new).production - absent.city_yields(new).production;
    assert!(
        lost > 0.0,
        "an active plant must actually deliver production"
    );
    // City yield multipliers also apply to the base two production. Make
    // the native baseline depend mostly on that measured, real plant output.
    let production = g.city_yields(new).production;
    std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments)
        .entry(new)
        .or_default()
        .production += lost * 1.25 - production;
    native_quote(&mut g, new, 20.0);
    // Without pricing Magnus, a flat 30% applied after five turns appears to
    // turn twenty quoted turns into seventeen. His two plant production matter.
    assert!(!ai.relocate_pingala_for_launch(&mut g, 0, &plan));
}
