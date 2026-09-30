//! The surge keeps its own war: the declaring frame, and the stalled-war
//! peace offer. Live King 20260930T211803Z lost the appointment both ways.
use super::*;

fn fixture() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 40, 24, 936077, 650, 0, false);
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
    }
    g.found_city_for(0, (12, 12), None);
    g.found_city_for(0, (12, 18), None);
    let target = g.found_city_for(1, (20, 12), None);
    g.found_city_for(1, (26, 16), None);
    g.record_contact(0, 1);
    g.players[0].techs.extend(
        g.rules.tech_ancestors[AIR_SURGE_GOAL_TECH]
            .iter()
            .map(|tech| Name::new(tech)),
    );
    g.players[0].techs.insert(Name::new(AIR_SURGE_GOAL_TECH));
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 400.0);
    g.at_war.clear();
    g.current = 0;
    g.turn = 170;
    for pos in [(12, 12), (11, 12)] {
        g.spawn_test_unit("bomber", 0, pos);
    }
    for pos in [(13, 12), (13, 13)] {
        g.spawn_test_unit("cuirassier", 0, pos);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: target,
        objective_pos: g.cities[&target].pos,
        body_unit: crate::name!("cuirassier"),
        body_is_cavalry: true,
        opened_at_war: false,
        phase: AirSurgePhase::Exploit,
        appointed_turn: 150,
        tech_turn: Some(160),
        declared_turn: Some(170),
        last_reviewed_turn: 170,
        recovery_assessments: 0,
    });
    (g, ai, target)
}

/// Turn 183 of the live game: frame one declared, frame two's fresh board had
/// not exported the war yet, and the wing stood down with its cooldown.
#[test]
fn the_declaring_frame_does_not_read_as_peace() {
    for (declared, survives) in [(170, true), (169, true), (168, false)] {
        let (g, mut ai, _) = fixture();
        ai.air_surge_plan.as_mut().unwrap().declared_turn = Some(declared);
        assert!(!g.is_at_war(0, 1));
        ai.maintain_air_surge(&g, 0);
        assert_eq!(ai.air_surge_plan.is_some(), survives, "declared {declared}");
        if survives {
            assert_eq!(
                ai.air_surge_plan.as_ref().unwrap().phase,
                AirSurgePhase::Exploit,
                "an in-flight declaration must not re-open Strike and declare again"
            );
            assert!(ai.air_surge_census.aborts.is_empty());
        } else {
            assert_eq!(ai.air_surge_census.aborts.get("peace closed the war"), Some(&1));
        }
    }
}

#[test]
fn the_declaring_frame_grace_is_version_two_only() {
    let (g, mut ai, _) = fixture();
    ai.disable_air_surge_2();
    ai.air_surge = true;
    ai.maintain_air_surge(&g, 0);
    assert!(ai.air_surge_plan.is_none());
}

fn at_war(g: &mut Game) {
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
}

#[test]
fn the_surge_front_is_held_once_the_wing_is_near() {
    let (mut g, mut ai, _) = fixture();
    at_war(&mut g);
    ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
    assert!(ai.air_surge_status.bombers >= 2);
    assert!(ai.air_surge_holds_front(&g, 0, 1));

    // Two techs out still holds; three does not.
    for uid in g.player_unit_ids(0) {
        if AdvancedAi::air_surge_is_bomber(&g, g.units[&uid].kind) {
            g.remove_unit(uid);
        }
    }
    ai.air_surge_status = AirSurgeStatus::default();
    for tech in ["advanced_flight", "radio"] {
        g.players[0].techs.remove(&Name::new(tech));
    }
    assert_eq!(AdvancedAi::air_surge_missing_techs(&g, 0), 2);
    assert!(ai.air_surge_holds_front(&g, 0, 1));
    g.players[0].techs.remove(&crate::name!("flight"));
    assert!(!ai.air_surge_holds_front(&g, 0, 1));
}

#[test]
fn the_front_hold_needs_the_domination_surge_on_that_rival() {
    for case in ["peace", "legacy", "lane", "no_plan", "other_target"] {
        let (mut g, mut ai, _) = fixture();
        at_war(&mut g);
        ai.air_surge_status = ai.air_surge_status(&g, 0, ai.air_surge_plan.as_ref().unwrap());
        match case {
            "peace" => g.at_war.clear(),
            "legacy" => {
                ai.disable_air_surge_2();
                ai.air_surge = true;
            }
            "lane" => ai.victory_target = Some(VictoryTarget::Science),
            "no_plan" => ai.air_surge_plan = None,
            "other_target" => ai.air_surge_plan.as_mut().unwrap().target_player = 0,
            _ => unreachable!(),
        }
        assert!(!ai.air_surge_holds_front(&g, 0, 1), "{case}");
    }
}

/// Turns 132 and 168 of the live game: "the war has stalled" at 2.7x and
/// 1.7x the target's power, against the civilization the wing was built for.
#[test]
fn a_stalled_war_against_the_armed_surge_target_is_not_offered_peace() {
    let offers = |hold: bool| {
        let (mut g, mut ai, target) = fixture();
        at_war(&mut g);
        g.players[0].gold = 0.0;
        g.players[0].gold_per_turn = -11.0;
        ai.enable_peace_when_war_does_not_pay();
        ai.major_war_since = Some(g.turn - 40);
        ai.last_campaign_progress = g.turn - 40;
        if !hold {
            ai.air_surge_plan = None;
        }
        ai.air_surge_status = ai
            .air_surge_plan
            .as_ref()
            .map(|plan| ai.air_surge_status(&g, 0, plan))
            .unwrap_or_default();
        let plan = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        ai.advanced_diplomacy(&mut g, 0, &plan);
        ai.peace_offers.contains(&1)
    };
    assert!(offers(false), "the control: this war is stalled and unpaid");
    assert!(!offers(true), "the surge's own front keeps its war");
}
