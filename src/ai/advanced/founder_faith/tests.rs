use super::*;
use crate::game::install_test_district;

/// Live Emperor G186 (civvis-20261006T025742Z) at turn 48: Bogotá, our only
/// Holy Site with a Shrine and a Temple, follows Indonesia's Hinduism, and
/// our Great Prophet is pending.
fn g186_turn_48() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(4, 40, 24, 372_101, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (6, 8), None);
    let bogota = g.player_city_ids(0)[0];
    install_test_district(&mut g, bogota, "holy_site");
    g.cities.get_mut(&bogota).unwrap().buildings =
        vec![crate::name!("shrine"), crate::name!("temple")];
    g.players[0].civics.insert(crate::name!("theology"));
    g.players[1].religion = Some("Hinduism".into());
    g.cities
        .get_mut(&bogota)
        .unwrap()
        .pressure
        .insert("Hinduism".into(), 600.0);
    g.players[0].prophet_pending = true;
    g.players[0].faith = 1_000.0;
    g.current = 0;
    g.turn = 48;
    assert_eq!(g.city_religion(&g.cities[&bogota]), Some("Hinduism"));
    (g, AdvancedAi::targeting(VictoryTarget::Domination), bogota)
}

/// `BasicAi`'s ancillary pass founding the religion on our board, as it does
/// the moment a Prophet is pending. The engine hands the Holy City our faith.
fn found_on_the_board(g: &mut Game) -> String {
    g.apply(
        0,
        &Action::FoundReligion {
            follower: crate::name!("work_ethic"),
            founder: crate::name!("tithe"),
        },
    )
    .unwrap();
    g.players[0].religion.clone().unwrap()
}

fn religious_units(g: &Game) -> usize {
    g.player_unit_ids(0)
        .into_iter()
        .filter(|uid| g.rules.units[g.units[uid].kind].class == "religious")
        .count()
}

#[test]
fn a_founding_pending_on_this_turn_buys_no_religious_unit_under_the_gene() {
    for gene in [false, true] {
        let (mut g, mut ai, bogota) = g186_turn_48();
        if gene {
            ai.enable_founder_spreads_only_its_faith();
        }
        // The turn begins with no religion on the host's board.
        ai.record_turn_start_faith(&g, 0);
        let ours = found_on_the_board(&mut g);
        assert_eq!(
            g.city_religion(&g.cities[&bogota]),
            Some(ours.as_str()),
            "our board hands Bogotá our faith at once; the host still counts it Hindu"
        );
        assert!(AdvancedAi::city_needs_religious_support(
            &g,
            0,
            &g.cities[&bogota],
            &ours
        ));
        assert_eq!(ai.founding_pending(&g, 0), gene);
        let before = religious_units(&g);
        ai.religious_spending(&mut g, 0, false);
        if gene {
            assert_eq!(
                religious_units(&g),
                before,
                "no purchase while the founding is pending"
            );
            assert_eq!(g.players[0].faith, 1_000.0);
        } else {
            assert_eq!(
                religious_units(&g),
                before + 1,
                "G186 turn 48: the shipped spender buys in Bogotá, which the host still counts Hindu"
            );
        }
    }
}

#[test]
fn a_founder_buys_only_where_its_faith_held_when_the_turn_began() {
    for gene in [false, true] {
        let (mut g, mut ai, bogota) = g186_turn_48();
        let ours = found_on_the_board(&mut g);
        if gene {
            ai.enable_founder_spreads_only_its_faith();
        }
        // Founded on an earlier turn, but Hinduism took Bogotá back.
        g.cities
            .get_mut(&bogota)
            .unwrap()
            .pressure
            .insert("Hinduism".into(), 1_200.0);
        assert_eq!(g.city_religion(&g.cities[&bogota]), Some("Hinduism"));
        ai.record_turn_start_faith(&g, 0);
        assert!(!ai.founding_pending(&g, 0));
        // A charge of ours spent on this turn's board turns it back; the host
        // has not seen that yet.
        g.cities
            .get_mut(&bogota)
            .unwrap()
            .pressure
            .insert(ours.clone(), 2_000.0);
        assert_eq!(g.city_religion(&g.cities[&bogota]), Some(ours.as_str()));
        assert_eq!(ai.founder_purchase_withheld(&g, 0, bogota), gene);
        let before = religious_units(&g);
        ai.religious_spending(&mut g, 0, false);
        if gene {
            assert_eq!(religious_units(&g), before);
        } else {
            assert_eq!(religious_units(&g), before + 1);
        }
    }
}

#[test]
fn a_founder_holds_a_spreader_of_another_faith_under_the_gene() {
    for gene in [false, true] {
        let (mut g, mut ai, bogota) = g186_turn_48();
        found_on_the_board(&mut g);
        if gene {
            ai.enable_founder_spreads_only_its_faith();
        }
        // G186 turns 49-51: the Hindu Missionary bought at 48 stands on
        // Bogotá, now our Holy City.
        let hindu = g.spawn_test_unit("missionary", 0, g.cities[&bogota].pos);
        g.units.get_mut(&hindu).unwrap().religion = Some("Hinduism".into());
        let charges = g.units[&hindu].charges;
        assert!(charges > 0);
        let acted = ai.advanced_missionary_step(&mut g, 0, hindu, false);
        if gene {
            assert!(!acted, "the foreign spreader holds");
            assert_eq!(g.units[&hindu].charges, charges);
        } else {
            assert!(acted);
            assert_eq!(
                g.units[&hindu].charges,
                charges - 1,
                "G186: Hinduism spread into our new Holy City"
            );
        }
    }
}

#[test]
fn a_seat_with_no_religion_keeps_its_adopted_faith_and_the_gene_off_records_nothing() {
    let (g, mut ai, _) = g186_turn_48();
    ai.record_turn_start_faith(&g, 0);
    assert_eq!(ai.turn_start_faith, None, "off: nothing recorded");
    ai.enable_founder_spreads_only_its_faith();
    ai.record_turn_start_faith(&g, 0);
    assert!(ai.turn_start_faith.is_some());
    assert!(!ai.founder_holds_a_foreign_spreader(&g, 0, "Hinduism"));
    assert!(!ai.founding_pending(&g, 0));
    assert!(!ai.founder_purchase_withheld(&g, 0, g.player_city_ids(0)[0]));
}

#[test]
fn founder_spreads_only_its_faith_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "founder-spreads-only-its-faith",
        |ai| ai.founder_spreads_only_its_faith,
    );
}
