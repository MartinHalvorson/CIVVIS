use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;
use crate::ai::advanced::objective_board::ObjectiveKey;

fn linked_pair(g: &mut Game, pos: Pos, support: &str, support_first: bool) -> (u32, u32) {
    let (carrier, peer) = if support_first {
        let peer = g.spawn_test_unit(support, 0, pos);
        (g.spawn_test_unit("pike_and_shot", 0, pos), peer)
    } else {
        let carrier = g.spawn_test_unit("pike_and_shot", 0, pos);
        (carrier, g.spawn_test_unit(support, 0, pos))
    };
    g.apply(
        0,
        &Action::LinkUnits {
            unit: carrier,
            with: peer,
        },
    )
    .unwrap();
    (carrier, peer)
}

fn planner() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_objective_board();
    ai.enable_siege_train();
    ai
}

fn normal_walled_city() -> (Game, u32) {
    let mut g = Game::new_full(2, 36, 22, 384_600, 160, 0, false);
    for pid in 0..2 {
        g.current = pid;
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("starting settler");
        g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
    }
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.current = 0;
    g.turn = 60;
    g.at_war.insert((0, 1));
    let cid = g.player_city_ids(1)[0];
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    (g, cid)
}

#[test]
fn linked_support_carrier_joins_the_siege_and_moves_the_pair() {
    for support_first in [false, true] {
        let (mut g, cid) = walled_city();
        let pos = at_distance(&g, cid, 6)[0];
        let (carrier, support) = linked_pair(&mut g, pos, "siege_tower", support_first);
        let mut ai = planner();
        let plan = plan_against(&g, cid);
        ai.rebuild_force_groups(&g, 0, &plan);
        let group = ai
            .force_groups()
            .iter()
            .find(|group| group.units.contains(&carrier));
        assert!(
            group.is_some(),
            "the combat carrier must remain in the army"
        );
        assert!(
            !ai.force_groups()
                .iter()
                .any(|group| group.units.contains(&support)),
            "support is not a second attacker"
        );
        assert_eq!(arm_of(&g, carrier), Arm::Melee);
        assert_eq!(arm_of(&g, support), Arm::Other);
        assert_eq!(
            ai.siege_doctrine_step(&mut g, 0, carrier, &plan),
            Some(true)
        );
        assert!(g.wdist(g.units[&carrier].pos, g.cities[&cid].pos) < 6);
        assert_eq!(g.units[&carrier].pos, g.units[&support].pos);
        assert_eq!(g.units[&carrier].linked_to, Some(support));
        assert_eq!(g.units[&support].linked_to, Some(carrier));
    }
}

#[test]
fn linked_support_carrier_stays_with_the_siege_instead_of_escorting_a_builder() {
    let (mut g, cid) = normal_walled_city();
    let approach = at_distance(&g, cid, 6);
    let (carrier, support) = linked_pair(&mut g, approach[0], "siege_tower", false);
    g.spawn_test_unit("bombard", 0, approach[1]);
    for pos in approach.iter().skip(2).take(5) {
        g.spawn_test_unit("pike_and_shot", 0, *pos);
    }
    let defenders: Vec<u32> = ring_of(&g, cid)
        .into_iter()
        .map(|pos| g.spawn_test_unit("infantry", 1, pos))
        .collect();

    let mut ai = planner();
    let plan = plan_against(&g, cid);
    ai.rebuild_force_groups(&g, 0, &plan);
    assert!(
        ai.objective_board()
            .forces
            .iter()
            .find(|force| force.objective_key == ObjectiveKey::Siege(cid))
            .is_some_and(|force| force.units.contains(&carrier)),
        "the supported carrier first joins the high-bill siege"
    );

    for defender in defenders {
        g.remove_unit(defender);
    }
    let builder = g.spawn_test_unit("builder", 0, approach[0]);
    g.turn += 1;
    ai.rebuild_force_groups(&g, 0, &plan);

    let siege = ai
        .objective_board()
        .forces
        .iter()
        .find(|force| force.objective_key == ObjectiveKey::Siege(cid))
        .expect("the city remains an active siege objective");
    assert!(
        siege.units.contains(&carrier),
        "the breach-support carrier must remain with its city objective"
    );
    let escort = ai
        .objective_board()
        .forces
        .iter()
        .find(|force| force.objective_key == ObjectiveKey::Escort(builder))
        .expect("another military unit can still serve the builder row");
    assert!(
        !escort.units.contains(&carrier),
        "a civilian escort must not pull the linked formation off the siege"
    );
    assert_eq!(g.units[&carrier].linked_to, Some(support));
    assert_eq!(g.units[&support].linked_to, Some(carrier));
}

#[test]
fn linked_infantry_can_finish_a_depleted_city_without_abandoning_support() {
    let (mut g, cid) = walled_city();
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    g.cities.get_mut(&cid).unwrap().hp = 1;
    let pos = ring_of(&g, cid)[0];
    let (carrier, support) = linked_pair(&mut g, pos, "battering_ram", false);
    let mut ai = planner();
    let plan = plan_against(&g, cid);
    ai.rebuild_force_groups(&g, 0, &plan);
    assert_eq!(
        ai.siege_doctrine_step(&mut g, 0, carrier, &plan),
        Some(true)
    );
    assert_eq!(g.cities[&cid].owner, 0);
    assert_eq!(g.units[&carrier].pos, g.cities[&cid].pos);
    assert_eq!(g.units[&support].pos, g.units[&carrier].pos);
}

#[test]
fn civilian_and_religious_escorts_stay_out_of_the_siege_pool() {
    for civilian in ["settler", "builder", "missionary"] {
        let (mut g, cid) = walled_city();
        let pos = at_distance(&g, cid, 4)[0];
        let (carrier, _) = linked_pair(&mut g, pos, civilian, false);
        let mut ai = planner();
        let plan = plan_against(&g, cid);
        ai.rebuild_force_groups(&g, 0, &plan);
        assert!(
            !ai.force_groups()
                .iter()
                .any(|group| group.units.contains(&carrier)),
            "{civilian}"
        );
        assert_eq!(arm_of(&g, carrier), Arm::Other, "{civilian}");
    }
}

#[test]
fn stale_or_foreign_support_links_do_not_supply_siege_strength() {
    for broken in ["separated", "one_way", "foreign"] {
        let (mut g, cid) = walled_city();
        let positions = at_distance(&g, cid, 4);
        let (carrier, peer) = linked_pair(&mut g, positions[0], "siege_tower", false);
        match broken {
            "separated" => g.units.get_mut(&peer).unwrap().pos = positions[1],
            "one_way" => g.units.get_mut(&peer).unwrap().linked_to = None,
            "foreign" => g.units.get_mut(&peer).unwrap().owner = 1,
            _ => unreachable!(),
        }
        let mut ai = planner();
        let plan = plan_against(&g, cid);
        ai.rebuild_force_groups(&g, 0, &plan);
        assert!(
            !ai.force_groups()
                .iter()
                .any(|group| group.units.contains(&carrier)),
            "{broken}"
        );
        assert_eq!(arm_of(&g, carrier), Arm::Other, "{broken}");
    }
}
