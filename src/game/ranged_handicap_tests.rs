use super::*;

fn fixture() -> Game {
    let mut g = Game::new_full(2, 20, 14, 591107, 250, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    g.difficulty = "prince".into();
    g.human_seats.insert(0);
    g.handicap_exempt.clear();
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
    }
    for p in g.players.iter_mut() {
        p.civ = "Rome".into();
        p.government = None;
        p.policies.clear();
        p.techs.clear();
        p.civics.clear();
        p.religion = None;
    }
    g
}

#[test]
fn ranged_handicap_deity_reaches_every_existing_weapon_without_creating_one() {
    let mut g = fixture();
    for kind in [
        "warrior",
        "archer",
        "field_cannon",
        "bombard",
        "frigate",
        "fighter",
        "bomber",
    ] {
        let id = g.spawn_test_unit(kind, 1, (4, 4));
        let mut deity = g.clone();
        deity.difficulty = "deity".into();
        let unit = &g.units[&id];
        let spec = &g.rules.units[kind];
        assert_eq!(
            deity.unit_ranged_strength(unit) - g.unit_ranged_strength(unit),
            if spec.ranged_strength > 0.0 { 4.0 } else { 0.0 },
            "{kind} ranged"
        );
        assert_eq!(
            deity.unit_bombard_strength(unit) - g.unit_bombard_strength(unit),
            if spec.bombard_strength > 0.0 {
                4.0
            } else {
                0.0
            },
            "{kind} bombard"
        );
        assert_eq!(
            deity.unit_ranged_attack_strength(unit) - g.unit_ranged_attack_strength(unit),
            if spec.has_ranged_attack() { 4.0 } else { 0.0 },
            "{kind} chosen weapon"
        );
        g.remove_unit(id);
    }
}

#[test]
fn ranged_handicap_respects_humans_exemptions_and_nonmajor_owners() {
    let mut g = fixture();
    for kind in ["archer", "bombard"] {
        let ours = g.spawn_test_unit(kind, 0, (4, 4));
        let ai = g.spawn_test_unit(kind, 1, (5, 4));
        let base = g.unit_ranged_attack_strength(&g.units[&ai]);
        let human = g.unit_ranged_attack_strength(&g.units[&ours]);
        let mut deity = g.clone();
        deity.difficulty = "deity".into();
        assert_eq!(
            deity.unit_ranged_attack_strength(&deity.units[&ours]),
            human
        );
        assert_eq!(
            deity.unit_ranged_attack_strength(&deity.units[&ai]),
            base + 4.0
        );
        for owner_case in 0..3 {
            let mut excluded = deity.clone();
            match owner_case {
                0 => {
                    excluded.handicap_exempt.insert(1);
                }
                1 => excluded.players[1].is_minor = true,
                _ => excluded.players[1].is_barbarian = true,
            }
            assert_eq!(
                excluded.unit_ranged_attack_strength(&excluded.units[&ai]),
                base
            );
        }
        // Preserve the existing below-Prince human handicap too.
        let mut settler = g.clone();
        settler.difficulty = "settler".into();
        assert_eq!(
            settler.unit_ranged_attack_strength(&settler.units[&ours]),
            human + 3.0
        );
        g.remove_unit(ours);
        g.remove_unit(ai);
    }
}

#[test]
fn ranged_handicap_exchange_adds_once_to_the_correct_side() {
    let mut g = fixture();
    let ours = g.spawn_test_unit("archer", 0, (4, 4));
    let ai = g.spawn_test_unit("archer", 1, (5, 4));
    let outgoing = g.ranged_strike_strengths(ours, ai, (5, 4)).unwrap();
    let incoming = g.ranged_strike_strengths(ai, ours, (4, 4)).unwrap();
    let mut deity = g.clone();
    deity.difficulty = "deity".into();
    assert_eq!(
        deity.ranged_strike_strengths(ours, ai, (5, 4)).unwrap(),
        (outgoing.0, outgoing.1 + 4.0)
    );
    assert_eq!(
        deity.ranged_strike_strengths(ai, ours, (4, 4)).unwrap(),
        (incoming.0 + 4.0, incoming.1)
    );
}

#[test]
fn ranged_handicap_city_and_air_forecasts_use_the_same_bonus() {
    let mut g = fixture();
    let city = g.found_city_for(0, (8, 4), None);
    let defender = g.spawn_test_unit("warrior", 0, (8, 4));
    for kind in ["archer", "bombard", "bomber"] {
        let id = g.spawn_test_unit(kind, 1, (6, 4));
        let city_pair = g.ranged_city_strengths(id, city).unwrap();
        let mut deity = g.clone();
        deity.difficulty = "deity".into();
        assert_eq!(
            deity.ranged_city_strengths(id, city).unwrap(),
            (city_pair.0 + 4.0, city_pair.1),
            "{kind} city shot"
        );
        if kind == "bomber" {
            assert_eq!(
                deity.air_strike_unit_strength(&deity.units[&id], &deity.units[&defender]),
                g.air_strike_unit_strength(&g.units[&id], &g.units[&defender]) + 4.0
            );
        }
        g.remove_unit(id);
    }
}
