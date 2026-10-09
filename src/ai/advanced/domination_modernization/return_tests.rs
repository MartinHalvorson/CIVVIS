use super::*;
use crate::game::{HostUnitFacts, HostUnitUpgrade, HostUnitUpgradeResource};

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(2, 32, 20, 366_410, 250, 0, false);
    g.units.clear();
    g.barb_camps.clear();
    g.at_war.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.owner_city = None;
    }
    g.found_city_for(0, (4, 4), None);
    g.found_city_for(1, (20, 13), None);
    g.current = 0;
    g.turn = 100;
    g.players[0].techs.insert(crate::name!("machinery"));
    g.players[0].gold = 2000.0;
    g.players[0].gold_per_turn = 18.0;
    let uid = g.spawn_test_unit("archer", 0, (11, 4));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(g.player_city_ids(1)[0]),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (
        g,
        AdvancedAi::targeting(VictoryTarget::Domination),
        plan,
        uid,
    )
}

fn fresh_turn(g: &mut Game, uid: u32) {
    g.turn += 1;
    let moves = g.unit_max_moves(uid);
    let unit = g.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.acted = false;
    unit.moved = false;
    unit.attacks_left = 1;
}

#[test]
fn campaign_fallback_returns_then_funds_a_legal_upgrade() {
    let (mut g, mut ai, plan, uid) = fixture();
    let abroad = g.units[&uid].pos;
    ai.fund_domination_upgrades(&mut g, 0, &plan);
    assert_eq!(g.units[&uid].kind, "archer");
    assert!(ai.advanced_military_step(&mut g, 0, uid, &plan));
    assert_ne!(g.units[&uid].pos, abroad);
    assert_eq!(g.units[&uid].moves_left, 0.0);
    for _ in 0..8 {
        fresh_turn(&mut g, uid);
        ai.fund_domination_upgrades(&mut g, 0, &plan);
        if g.units[&uid].kind == "crossbowman" {
            assert!(g.players[0].gold >= 30.0);
            return;
        }
        assert_eq!(
            ai.domination_upgrade_return_step(&mut g, 0, uid, &plan),
            Some(true)
        );
    }
    panic!("the reachable upgrade did not complete within the bounded return");
}

#[test]
fn host_successor_and_resource_bill_survive_return_without_bypassing_refusal() {
    let (mut g, mut ai, plan, uid) = fixture();
    g.units.get_mut(&uid).unwrap().kind = crate::name!("warrior");
    g.players[0].techs.insert(crate::name!("iron_working"));
    g.players[0].techs.insert(crate::name!("apprenticeship"));
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 10.0);
    g.players[0].gold = 91.0;
    std::sync::Arc::make_mut(&mut g.host_unit_facts).insert(
        uid,
        HostUnitFacts {
            upgrade: Some(HostUnitUpgrade {
                to: Some(crate::name!("man_at_arms")),
                cost: Some(60.0),
                resources: Some(HostUnitUpgradeResource {
                    resource: Some(crate::name!("iron")),
                    cost: 10.0,
                }),
                blocked: Some("Must be in friendly territory.".into()),
            }),
            ..Default::default()
        },
    );
    assert_eq!(
        g.unit_upgrade_target(0, crate::name!("warrior")),
        Some(crate::name!("swordsman"))
    );
    for _ in 0..8 {
        assert_eq!(
            ai.domination_upgrade_return_step(&mut g, 0, uid, &plan),
            Some(true)
        );
        fresh_turn(&mut g, uid);
        let here = g.units[&uid].pos;
        if g.map.tiles[&here]
            .owner_city
            .is_some_and(|cid| g.cities[&cid].owner == 0)
        {
            assert_eq!(g.unit_gold_upgrade_detail(0, uid), Err("foreign territory"));
            assert!(g.apply(0, &Action::UpgradeUnit { unit: uid }).is_err());
            std::sync::Arc::make_mut(&mut g.host_unit_facts)
                .get_mut(&uid)
                .unwrap()
                .upgrade
                .as_mut()
                .unwrap()
                .blocked = None;
            assert_eq!(
                g.unit_gold_upgrade_offer(0, uid).unwrap(),
                (crate::name!("man_at_arms"), 60.0, 10.0)
            );
            g.apply(0, &Action::UpgradeUnit { unit: uid }).unwrap();
            assert_eq!(g.units[&uid].kind, "man_at_arms");
            assert_eq!(g.players[0].gold, 31.0);
            assert_eq!(g.strategic_stockpile(0, crate::name!("iron")), 0.0);
            return;
        }
    }
    panic!("the unit never reached owned upgrade ground");
}

#[test]
fn return_preserves_roles_cash_permissions_and_nearby_combat() {
    for case in [
        "science",
        "no_target",
        "peaceful",
        "no_tech",
        "cash",
        "deficit",
        "recon",
        "sea",
        "spent",
        "linked",
        "reserved",
        "enemy_unit",
        "enemy_city",
        "host_unknown",
        "host_gold",
        "host_material",
        "host_cash_quote",
    ] {
        let (mut g, mut ai, mut plan, uid) = fixture();
        match case {
            "science" => ai.victory_target = Some(VictoryTarget::Science),
            "no_target" => plan.target_player = None,
            "peaceful" => plan.strategy = GrandStrategy::Expansion,
            "no_tech" => {
                g.players[0].techs.clear();
            }
            "cash" => g.players[0].gold = 1.0,
            "deficit" => g.players[0].gold_per_turn = -2000.0,
            "recon" => {
                g.units.get_mut(&uid).unwrap().kind = crate::name!("scout");
                g.players[0].techs.insert(crate::name!("military_tactics"));
            }
            "sea" => {
                g.units.get_mut(&uid).unwrap().kind = crate::name!("galley");
                g.players[0].techs.insert(crate::name!("cartography"));
            }
            "spent" => g.units.get_mut(&uid).unwrap().moves_left = 0.0,
            "linked" => {
                let peer = g.spawn_test_unit("settler", 0, g.units[&uid].pos);
                g.units.get_mut(&uid).unwrap().linked_to = Some(peer);
            }
            "reserved" => {
                ai.reserved_units.insert(uid);
            }
            "enemy_unit" => {
                g.at_war.insert((0, 1));
                g.spawn_test_unit("warrior", 1, (13, 4));
            }
            "enemy_city" => {
                g.at_war.insert((0, 1));
                g.found_city_for(1, (13, 4), None);
            }
            "host_unknown" | "host_gold" | "host_material" | "host_cash_quote" => {
                let blocked = match case {
                    "host_gold" => "Not enough Gold in treasury.",
                    "host_material" => "Insufficient Resources.",
                    "host_cash_quote" => "Must be in friendly territory.",
                    _ => "No command available.",
                };
                std::sync::Arc::make_mut(&mut g.host_unit_facts).insert(
                    uid,
                    HostUnitFacts {
                        upgrade: Some(HostUnitUpgrade {
                            to: Some(crate::name!("crossbowman")),
                            cost: Some(2001.0),
                            blocked: Some(blocked.into()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                );
            }
            _ => unreachable!(),
        }
        let before = (
            g.units[&uid].pos,
            g.units[&uid].moves_left,
            g.players[0].gold,
            g.log.len(),
        );
        assert_eq!(
            ai.domination_upgrade_return_step(&mut g, 0, uid, &plan),
            None,
            "{case}"
        );
        assert_eq!(
            (
                g.units[&uid].pos,
                g.units[&uid].moves_left,
                g.players[0].gold,
                g.log.len()
            ),
            before,
            "{case}"
        );
    }
}

#[test]
fn territory_block_does_not_hide_missing_material() {
    let (mut g, mut ai, plan, uid) = fixture();
    g.units.get_mut(&uid).unwrap().kind = crate::name!("warrior");
    std::sync::Arc::make_mut(&mut g.host_unit_facts).insert(
        uid,
        HostUnitFacts {
            upgrade: Some(HostUnitUpgrade {
                to: Some(crate::name!("man_at_arms")),
                cost: Some(60.0),
                resources: Some(HostUnitUpgradeResource {
                    resource: Some(crate::name!("iron")),
                    cost: 10.0,
                }),
                blocked: Some("Must be in friendly territory.".into()),
            }),
            ..Default::default()
        },
    );
    assert_eq!(
        ai.domination_upgrade_return_step(&mut g, 0, uid, &plan),
        None
    );
}

#[test]
fn blocked_routes_do_not_consume_movement_or_fall_back_to_a_long_detour() {
    let (mut g, mut ai, plan, uid) = fixture();
    for pos in g.nbrs(g.units[&uid].pos) {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
    }
    let before = g.units[&uid].pos;
    assert_eq!(
        ai.domination_upgrade_return_step(&mut g, 0, uid, &plan),
        None
    );
    assert_eq!(g.units[&uid].pos, before);
    assert!(!g.units[&uid].acted);
}

#[test]
fn already_owned_units_keep_the_ordinary_upgrade_pass() {
    let (mut g, mut ai, plan, uid) = fixture();
    g.relocate(uid, (4, 4));
    assert!(g.unit_gold_upgrade_offer(0, uid).is_some());
    assert_eq!(
        ai.domination_upgrade_return_step(&mut g, 0, uid, &plan),
        None
    );
    ai.fund_domination_upgrades(&mut g, 0, &plan);
    assert_eq!(g.units[&uid].kind, "crossbowman");
}
