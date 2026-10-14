use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};

fn fixture() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 40, 24, 936301, 650, 0, false);
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
    g.found_city_for(0, (10, 12), None);
    let rival_city = g.found_city_for(1, (26, 12), None);
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.current = 0;
    g.turn = 180;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_war_kills_the_bands();
    (g, ai, rival_city)
}

fn plan_on(city: Option<u32>) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: city,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

#[test]
fn war_kills_the_bands_is_opt_in() {
    opt_in_off_in_both_controllers("war-kills-the-bands", |ai| ai.war_kills_the_bands);
}

/// G409's shape: a Spanish band three tiles from a cavalry of ours, at war.
#[test]
fn a_band_in_reach_is_run_down() {
    let (mut g, mut ai, _) = fixture();
    let band = g.spawn_test_unit("rock_band", 1, (16, 12));
    let hunter = g.spawn_test_unit("cavalry", 0, (13, 12));
    let hunters = ai.plan_band_hunt(&mut g, 0, &plan_on(None), &BTreeSet::new());
    assert_eq!(hunters, BTreeSet::from([hunter]));
    assert!(!g.units.contains_key(&band), "the band is gone");
    assert_eq!(g.units[&hunter].pos, (16, 12));
    assert_eq!(g.players[0].counters.get("band_hunt:killed"), Some(&1));
}

#[test]
fn no_kill_off_the_gene_at_peace_out_of_reach_reserved_escorted_or_on_the_ring() {
    for case in ["off", "peace", "far", "reserved", "escorted", "ring"] {
        let (mut g, mut ai, rival_city) = fixture();
        let at = match case {
            "far" => (24, 18),
            "ring" => (21, 12),
            _ => (16, 12),
        };
        let band = g.spawn_test_unit("rock_band", 1, at);
        let start = if case == "ring" { (24, 12) } else { (13, 12) };
        let hunter = g.spawn_test_unit("cavalry", 0, start);
        let mut reserved = BTreeSet::new();
        let mut plan = plan_on(None);
        match case {
            "off" => ai.disable_war_kills_the_bands(),
            "peace" => g.at_war.clear(),
            "reserved" => {
                reserved.insert(hunter);
            }
            "escorted" => {
                g.spawn_test_unit("warrior", 1, at);
            }
            "ring" => {
                // The hunter stands on the siege ring of the campaign's
                // objective, and the band is in its reach.
                plan = plan_on(Some(rival_city));
            }
            _ => {}
        }
        let hunters = ai.plan_band_hunt(&mut g, 0, &plan, &reserved);
        assert!(hunters.is_empty(), "{case}: no hunt");
        assert!(g.units.contains_key(&band), "{case}: the band stands");
        if case == "ring" {
            // The control: off the ring's campaign, the same walk is taken.
            let hunters = ai.plan_band_hunt(&mut g, 0, &plan_on(None), &reserved);
            assert_eq!(hunters, BTreeSet::from([hunter]), "ring control");
        }
    }
}

#[test]
fn war_kills_the_bands_2_is_opt_in() {
    opt_in_off_in_both_controllers("war-kills-the-bands-2", |ai| ai.war_kills_the_bands_2);
}

/// G424: the host let two Trebuchet `CAPTURE`s stand with the band alive.
/// Version one sends the nearest military body; version two only a
/// melee-capable one.
#[test]
fn version_two_hunts_with_melee_bodies_only() {
    let run = |v2: bool, with_cavalry: bool| {
        let (mut g, mut ai, _) = fixture();
        if v2 {
            ai.disable_war_kills_the_bands();
            ai.enable_war_kills_the_bands_2();
        }
        let band = g.spawn_test_unit("rock_band", 1, (15, 12));
        let trebuchet = g.spawn_test_unit("trebuchet", 0, (13, 12));
        let cavalry = with_cavalry.then(|| g.spawn_test_unit("cavalry", 0, (11, 12)));
        let hunters = ai.plan_band_hunt(&mut g, 0, &plan_on(None), &BTreeSet::new());
        (hunters, trebuchet, cavalry, g.units.contains_key(&band))
    };
    let (hunters, trebuchet, _, standing) = run(false, false);
    assert_eq!(
        hunters,
        BTreeSet::from([trebuchet]),
        "v1 sends the siege gun"
    );
    assert!(!standing);
    let (hunters, _, _, standing) = run(true, false);
    assert!(hunters.is_empty(), "v2: no melee body, no hunt");
    assert!(standing);
    let (hunters, _, cavalry, standing) = run(true, true);
    assert_eq!(
        hunters,
        BTreeSet::from([cavalry.unwrap()]),
        "v2 sends the cavalry"
    );
    assert!(!standing);
}

/// The live military pass runs the version-two hunt before the battle is
/// planned, so the body is still free when the hunt reads it.
#[test]
fn version_two_hunts_in_the_military_pass() {
    let run = |v2: bool| {
        let (mut g, mut ai, _) = fixture();
        ai.disable_war_kills_the_bands();
        if v2 {
            ai.enable_war_kills_the_bands_2();
        }
        let band = g.spawn_test_unit("rock_band", 1, (16, 12));
        g.spawn_test_unit("cavalry", 0, (13, 12));
        ai.advanced_units(&mut g, 0, &plan_on(None));
        (
            g.units.contains_key(&band),
            g.players[0].counters.get("band_hunt:killed").copied(),
        )
    };
    assert_eq!(run(false), (true, None), "off: the band stands");
    assert_eq!(run(true), (false, Some(1)), "v2: run down in the pass");
}
