use super::*;
use civvis::{
    ai::VictoryTarget,
    game::{Action, HostUnitFacts, TradeRoute},
    name::Name,
    rules::Yields,
};
use std::sync::Arc;

type Fixture = (
    AdvancedAi,
    Game,
    BTreeMap<i64, u32>,
    Vec<Order>,
    u32,
    u32,
    u32,
    u32,
);

fn fixture() -> Fixture {
    let mut g = Game::new_full(4, 24, 14, 475_900, 250, 0, false);
    for pid in 0..4 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|u| g.units[u].kind == "settler")
            .unwrap();
        g.current = pid;
        g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
    }
    g.current = 0;
    for pid in 1..4 {
        g.record_contact(0, pid);
    }
    let origin = g.player_city_ids(0)[0];
    let covered = g.player_city_ids(1)[0];
    let missing = g.player_city_ids(2)[0];
    let alternative = g.player_city_ids(3)[0];
    g.cities.get_mut(&alternative).unwrap().owner = 0;
    let uid = g.player_unit_ids(0)[0];
    let u = g.units.get_mut(&uid).unwrap();
    u.kind = Name::new("trader");
    u.pos = g.cities[&origin].pos;
    u.moves_left = 2.0;
    Arc::make_mut(&mut g.host_unit_facts).insert(
        uid,
        HostUnitFacts {
            civ6_id: Some(1000),
            ..Default::default()
        },
    );
    Arc::make_mut(&mut g.observed_tourism_per_turn).insert(0, 60.0);
    Arc::make_mut(&mut g.observed_trade_capacity).insert(0, 4);
    Arc::make_mut(&mut g.observed_route_options).insert(
        (alternative, missing),
        Yields {
            gold: 6.0,
            ..Default::default()
        },
    );
    g.routes.push(TradeRoute {
        origin,
        dest: covered,
        owner: 0,
        ends: 30,
    });
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_culture_routes_reach_every_rival();
    let orders = vec![Order {
        kind: "unit",
        subject: Some(1000),
        verb: Some("TRADE_ROUTE".into()),
        pos: Some(axial_to_offset(
            g.cities[&covered].pos.0,
            g.cities[&covered].pos.1,
        )),
    }];
    (
        ai,
        g,
        BTreeMap::from([(1000, uid)]),
        orders,
        uid,
        origin,
        alternative,
        missing,
    )
}

#[test]
fn native_positive_other_origin_replaces_a_duplicate_market_route() {
    let (ai, g, units, mut orders, _, _, alternative, _) = fixture();
    assert_eq!(apply(&ai, &g, &units, &BTreeSet::new(), &mut orders), 1);
    assert_eq!(orders[0].verb.as_deref(), Some("TELEPORT_TO_CITY"));
    assert_eq!(
        orders[0].pos,
        Some(axial_to_offset(
            g.cities[&alternative].pos.0,
            g.cities[&alternative].pos.1
        ))
    );
}

#[test]
fn current_origin_or_existing_new_market_plan_does_not_need_a_transfer() {
    let (ai, mut g, units, orders, _, origin, _, missing) = fixture();
    let mut direct = orders.clone();
    direct[0].pos = Some(axial_to_offset(
        g.cities[&missing].pos.0,
        g.cities[&missing].pos.1,
    ));
    assert_eq!(apply(&ai, &g, &units, &BTreeSet::new(), &mut direct), 0);
    Arc::make_mut(&mut g.observed_route_options).insert((origin, missing), Yields::default());
    let mut local = orders;
    assert_eq!(apply(&ai, &g, &units, &BTreeSet::new(), &mut local), 0);
}

#[test]
fn missing_native_observation_busy_trader_and_disabled_lane_preserve_orders() {
    let (ai, g, units, original, uid, _, _, _) = fixture();
    let mut orders = original.clone();
    assert_eq!(
        apply(&ai, &g, &units, &BTreeSet::from([1000]), &mut orders),
        0
    );
    for mode in 0..5 {
        let mut game = g.clone();
        let mut orders = original.clone();
        let mut policy = AdvancedAi::targeting(if mode == 3 {
            VictoryTarget::Science
        } else {
            VictoryTarget::Culture
        });
        if mode != 4 {
            policy.enable_culture_routes_reach_every_rival();
        }
        if mode == 0 {
            Arc::make_mut(&mut game.observed_route_options).clear();
        }
        if mode == 1 {
            Arc::make_mut(&mut game.host_unit_facts).remove(&uid);
        }
        if mode == 2 {
            Arc::make_mut(&mut game.observed_tourism_per_turn).insert(0, 0.0);
        }
        assert_eq!(
            apply(&policy, &game, &units, &BTreeSet::new(), &mut orders),
            0
        );
        assert_eq!(orders[0].verb.as_deref(), Some("TRADE_ROUTE"));
    }
}

#[test]
fn only_one_idle_trader_reserves_each_missing_market() {
    let (ai, mut g, mut units, mut orders, uid, _, _, _) = fixture();
    let mut other = g.units[&uid].clone();
    other.id = uid + 1000;
    let other_id = other.id;
    g.units.insert(other_id, other);
    Arc::make_mut(&mut g.host_unit_facts).insert(
        other_id,
        HostUnitFacts {
            civ6_id: Some(1001),
            ..Default::default()
        },
    );
    units.insert(1001, other_id);
    let mut second = orders[0].clone();
    second.subject = Some(1001);
    orders.push(second);
    assert_eq!(apply(&ai, &g, &units, &BTreeSet::new(), &mut orders), 1);
    assert_eq!(orders[1].verb.as_deref(), Some("TRADE_ROUTE"));
}

#[test]
fn hostile_or_damaged_origin_does_not_receive_a_trader() {
    let (ai, mut g, units, mut orders, _, _, alternative, _) = fixture();
    g.cities.get_mut(&alternative).unwrap().hp = 150;
    assert_eq!(apply(&ai, &g, &units, &BTreeSet::new(), &mut orders), 0);
    g.cities.get_mut(&alternative).unwrap().hp = 200;
    g.at_war.insert((0, 2));
    assert_eq!(apply(&ai, &g, &units, &BTreeSet::new(), &mut orders), 0);
}

#[test]
fn transfer_verification_needs_the_trader_at_an_owned_city() {
    use super::super::{verify_unit_order, IssuedOrder, LaterFrames, Verdict};
    use civvis::mirror::{Snapshot, StateCity, StateSnapshot, StateUnit};
    let before = StateSnapshot {
        turn: 175,
        units: vec![StateUnit {
            id: 1000,
            kind: "UNIT_TRADER".into(),
            x: 1,
            y: 1,
            ..Default::default()
        }],
        cities: vec![StateCity {
            id: 3,
            x: 10,
            y: 10,
            ..Default::default()
        }],
        ..Default::default()
    };
    let order = IssuedOrder {
        kind: "unit".into(),
        subject: Some(1000),
        verb: Some("TELEPORT_TO_CITY".into()),
        pos: Some((10, 10)),
    };
    let check = |after: &StateSnapshot| {
        verify_unit_order(
            &order,
            175,
            &before,
            after,
            &Snapshot::default(),
            &[],
            LaterFrames::default(),
        )
    };
    assert!(matches!(check(&before), Verdict::Failed(_)));
    let mut after = before.clone();
    (after.units[0].x, after.units[0].y) = (10, 10);
    assert!(matches!(check(&after), Verdict::Verified));
    after.cities.clear();
    assert!(matches!(check(&after), Verdict::Failed(_)));
    after.units.clear();
    assert!(matches!(check(&after), Verdict::Failed(_)));
}
