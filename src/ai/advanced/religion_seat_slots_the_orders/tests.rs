use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn board() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new(2, 30, 20, 84_1935, 250, 0);
    let city = g.found_city_for(0, (6, 8), None);
    g.cities.get_mut(&city).unwrap().pop = 5;
    let pos = g.cities[&city].pos;
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), pos);
    g.turn = 84;
    g.players[0].religion = Some("Buddhism".into());
    g.players[0].government = Some("theocracy".into());
    g.players[0].gold = 42.0;
    g.players[0].gold_per_turn = -5.0;
    g.players[0].civics.extend(
        [
            "code_of_laws",
            "craftsmanship",
            "foreign_trade",
            "early_empire",
            "political_philosophy",
            "recorded_history",
            "theology",
            "divine_right",
            "civil_service",
            "reformed_church",
        ]
        .into_iter()
        .map(Name::new),
    );
    g.players[0].policies = [
        "retainers",
        "wars_of_religion",
        "scripture",
        "natural_philosophy",
        "charismatic_leader",
        "urban_planning",
    ]
    .into_iter()
    .map(Name::new)
    .collect();
    let apostle = g.spawn_unit("apostle", 0, pos);
    g.units.get_mut(&apostle).unwrap().religion = Some("Buddhism".into());
    g.host_policy_windows.insert(
        0,
        crate::game::HostPolicyWindow {
            turn: g.turn,
            open: true,
        },
    );
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    ai.research_economy = true;
    ai.urban_planning_fills_the_slot = true;
    ai.solvent_or_hold = true;
    (g, ai, apostle)
}

fn has(g: &Game, card: &str) -> bool {
    g.players[0].policies.contains(&Name::new(card))
}

#[test]
fn the_gene_is_opt_in_in_both_controllers() {
    opt_in_off_in_both_controllers("religion-seat-slots-the-orders", |ai| {
        ai.religion_seat_slots_the_orders
    });
}

#[test]
fn a_full_wanted_deck_uses_the_window_for_religious_orders() {
    let (g, mut ai, _) = board();
    let before = g.players[0].policies.clone();
    let mut off = g.clone();
    ai.strategic_policies(&mut off, 0, GrandStrategy::Religion);
    assert_eq!(
        off.players[0].policies, before,
        "control really locks out the card"
    );
    ai.enable_religion_seat_slots_the_orders();
    let mut on = g;
    ai.strategic_policies(&mut on, 0, GrandStrategy::Religion);
    let mut expected = before;
    expected.remove(&crate::name!("urban_planning"));
    expected.insert(crate::name!("religious_orders"));
    assert_eq!(on.players[0].policies, expected);
    ai.strategic_policies(&mut on, 0, GrandStrategy::Religion);
    assert_eq!(
        on.players[0].policies, expected,
        "the repair stays slotted on a replan"
    );
}

#[test]
fn the_repair_requires_a_founder_with_an_apostle_of_its_faith_and_religious_target() {
    for case in [
        "no founding",
        "no Apostle",
        "foreign faith",
        "other target",
        "disabled victory",
    ] {
        let (mut g, mut ai, apostle) = board();
        match case {
            "no founding" => g.players[0].religion = None,
            "no Apostle" => {
                g.remove_unit(apostle);
            }
            "foreign faith" => {
                g.units.get_mut(&apostle).unwrap().religion = Some("Hinduism".into())
            }
            "other target" => ai.victory_target = Some(VictoryTarget::Science),
            "disabled victory" => g.victory_conditions.religious = false,
            _ => unreachable!(),
        }
        let mut off = g.clone();
        ai.strategic_policies(&mut off, 0, GrandStrategy::Religion);
        ai.enable_religion_seat_slots_the_orders();
        ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
        assert_eq!(g.players[0].policies, off.players[0].policies, "{case}");
    }
}

#[test]
fn a_closed_host_window_keeps_the_deck_and_treasury() {
    let (mut g, mut ai, _) = board();
    ai.enable_religion_seat_slots_the_orders();
    g.host_policy_windows.get_mut(&0).unwrap().open = false;
    let before = g.players[0].policies.clone();
    let gold = g.players[0].gold;
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert_eq!(g.players[0].policies, before);
    assert_eq!(g.players[0].gold, gold);
}

#[test]
fn the_card_must_be_on_the_menu() {
    let (mut g, mut ai, _) = board();
    ai.enable_religion_seat_slots_the_orders();
    g.players[0].civics.remove(&crate::name!("reformed_church"));
    assert!(!g
        .available_policies(0)
        .contains(&crate::name!("religious_orders")));
    let before = g.players[0].policies.clone();
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert_eq!(g.players[0].policies, before);
}

#[test]
fn a_timed_expansion_and_builder_portfolio_keeps_its_earlier_cards() {
    let (mut g, mut ai, _) = board();
    ai.enable_religion_seat_slots_the_orders();
    ai.wide_map_capacity = true;
    g.players[0].civics.insert(crate::name!("feudalism"));
    let c1 = g.player_city_ids(0)[0];
    let c2 = g.found_city_for(0, (12, 8), None);
    g.cities.get_mut(&c1).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    g.cities.get_mut(&c2).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("builder"),
    }];
    g.players[0].policies.remove(&crate::name!("scripture"));
    g.players[0]
        .policies
        .remove(&crate::name!("urban_planning"));
    g.players[0]
        .policies
        .extend([crate::name!("colonization"), crate::name!("serfdom")]);
    let before = g.players[0].policies.clone();
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert_eq!(g.players[0].policies, before);
    assert!(!has(&g, "religious_orders"));
}

#[test]
fn insolvency_still_protects_the_amenity_card() {
    let (mut g, mut ai, _) = board();
    ai.enable_religion_seat_slots_the_orders();
    g.players[0].government = Some("chiefdom".into());
    g.players[0].policies = ["retainers", "civil_prestige"]
        .into_iter()
        .map(Name::new)
        .collect();
    let before = g.players[0].policies.clone();
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert_eq!(g.players[0].policies, before);
}

#[test]
fn a_wanted_other_type_cannot_donate_its_wildcard_capacity() {
    let (mut g, mut ai, _) = board();
    ai.enable_religion_seat_slots_the_orders();
    g.players[0].government = Some("oligarchy".into());
    g.players[0].civics.extend([
        crate::name!("mobilization"),
        crate::name!("diplomatic_service"),
    ]);
    g.players[0].policies = [
        "wars_of_religion",
        "levee_en_masse",
        "civil_prestige",
        "wisselbanken",
    ]
    .into_iter()
    .map(Name::new)
    .collect();
    // Demonstrate that the wanted military card really occupies capacity
    // Orders could borrow; the planner must still refuse this donor.
    let mut borrowed = g.clone();
    borrowed
        .apply(
            0,
            &Action::UnslotPolicy {
                policy: crate::name!("levee_en_masse"),
            },
        )
        .unwrap();
    borrowed
        .apply(
            0,
            &Action::SlotPolicy {
                policy: crate::name!("religious_orders"),
            },
        )
        .unwrap();
    let before = g.players[0].policies.clone();
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert_eq!(g.players[0].policies, before);
    assert!(!has(&g, "religious_orders"));
}

#[test]
fn work_ethic_keeps_scriptures_faith_and_production() {
    let (mut g, mut ai, _) = board();
    ai.enable_religion_seat_slots_the_orders();
    g.players[0].government = Some("chiefdom".into());
    g.players[0].policies = ["retainers", "scripture"]
        .into_iter()
        .map(Name::new)
        .collect();
    g.players[0].religion_beliefs = vec!["work_ethic".into()];
    let city = g.player_city_ids(0)[0];
    g.cities
        .get_mut(&city)
        .unwrap()
        .pressure
        .insert("Buddhism".into(), 500.0);
    let site = crate::game::install_test_district(&mut g, city, "holy_site");
    let mountain = g
        .nbrs(site)
        .into_iter()
        .find(|pos| *pos != g.cities[&city].pos)
        .unwrap();
    g.map.tiles.get_mut(&mountain).unwrap().terrain = crate::name!("mountain");
    let with_scripture = g.city_yields(city);
    let mut without_scripture = g.clone();
    without_scripture
        .apply(
            0,
            &Action::UnslotPolicy {
                policy: crate::name!("scripture"),
            },
        )
        .unwrap();
    let lost = without_scripture.city_yields(city);
    assert!(
        lost.faith < with_scripture.faith,
        "Scripture is paying Faith"
    );
    assert!(
        lost.production < with_scripture.production,
        "Work Ethic also pays Production"
    );
    let before = g.players[0].policies.clone();
    let mut without_work_ethic = g.clone();
    without_work_ethic.players[0].religion_beliefs.clear();
    ai.strategic_policies(&mut without_work_ethic, 0, GrandStrategy::Religion);
    assert!(
        has(&without_work_ethic, "religious_orders"),
        "the ordinary donor would fit"
    );
    assert!(!has(&without_work_ethic, "scripture"));
    ai.strategic_policies(&mut g, 0, GrandStrategy::Religion);
    assert_eq!(
        g.players[0].policies, before,
        "Work Ethic's production card stays"
    );
}
