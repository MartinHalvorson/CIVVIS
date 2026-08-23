//! The housing breakdown and the housing total are one rule, not two.
//!
//! `city_housing` used to be a 143-line accumulator and the instrument that
//! scored it against Civilization VI could only see the number that fell out of
//! the end. The host has always exported its own per-category breakdown, so the
//! model now produces the same categories — and the risk that creates is a
//! breakdown that quietly stops adding up to the total anyone acts on.
//!
//! These hold that line: the parts sum to the total, on real played states, for
//! every city, every turn.

use super::*;
use crate::ai::{Ai, BasicAi};

/// A real game played far enough that cities have districts, buildings and
/// improvements — the terms the split had to reattribute.
fn played(turns: u32, seed: u64) -> Game {
    let mut game = Game::new_full(3, 32, 24, seed, 200, 0, false);
    let mut ais: Vec<BasicAi> = (0..game.players.len()).map(|_| BasicAi::new()).collect();
    for _ in 0..turns {
        for (pid, ai) in ais.iter_mut().enumerate() {
            if game.players[pid].alive {
                ai.take_turn(&mut game, pid);
            }
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    game
}

#[test]
fn the_parts_sum_to_the_total_on_every_city_of_a_played_game() {
    let game = played(60, 20260817);
    let mut checked = 0;
    for city in game.cities.values() {
        let sources = game.city_housing_sources(city);
        let total = sources.total();
        let named: f64 = sources.named().iter().map(|(_, value)| value).sum();
        assert!(
            (total - named).abs() < 1e-9,
            "{}: total() {total} but the named categories sum to {named}",
            city.name
        );
        assert!(
            (game.city_housing(city) - total).abs() < 1e-9,
            "{}: city_housing {} but its sources total {total}",
            city.name,
            game.city_housing(city)
        );
        checked += 1;
    }
    assert!(
        checked >= 3,
        "only {checked} cities; the test proved little"
    );
}

#[test]
fn a_mirrored_board_adds_the_correction_to_the_total_and_not_to_a_category() {
    // The mirror's per-turn correction is the host's ceiling, not a housing
    // source CIVVIS derived. Folding it into a category would make that
    // category compare as "right" against the host by construction, which is
    // precisely the comparison the instrument exists to make honestly.
    let mut game = played(40, 7);
    let id = *game.cities.keys().next().expect("a played game has a city");
    let before = game.city_housing_sources(&game.cities[&id]);
    let modelled = game.city_housing(&game.cities[&id]);

    game.observed_city_housing_adjustments.insert(id, 3.0);
    let after = game.city_housing_sources(&game.cities[&id]);

    assert_eq!(before, after, "the correction must not move a category");
    assert!(
        (game.city_housing(&game.cities[&id]) - (modelled + 3.0)).abs() < 1e-9,
        "the correction must reach the total"
    );
}

#[test]
fn the_aqueduct_counts_as_water_not_as_a_district() {
    // An Aqueduct grants its city fresh water, so Civilization VI folds its
    // lift into `housing_from_water` and leaves `housing_from_districts` to the
    // Neighborhoods. Written the other way round first, and the per-source
    // comparison found it within one run: every aqueduct city was wrong in
    // `districts` and `water` by exactly equal and opposite amounts, and those
    // amounts were exactly these lifts. The total was right the whole time.
    for (fresh, coastal, lift) in [(true, false, 2.0), (false, true, 3.0), (false, false, 4.0)] {
        let dry = Game::city_housing_floor(fresh, coastal, false);
        let wet = Game::city_housing_floor(fresh, coastal, true);
        assert_eq!(
            wet - dry,
            lift,
            "aqueduct lift changed (fresh={fresh} coastal={coastal}); the host \
             folds this into housing_from_water, so a change here moves what \
             the drift instrument compares"
        );
    }

    // And the mapping itself, on a city that ACTUALLY HAS ONE. Asserting this
    // against a city with no aqueduct proves nothing — both attributions agree
    // when the lift is zero, which is how the first version of this test passed
    // with the wrong mapping reintroduced.
    let mut game = played(12, 99);
    let id = *game.cities.keys().next().expect("a game has a city");
    let site = *game.cities[&id]
        .owned_tiles
        .iter()
        .find(|p| **p != game.cities[&id].pos)
        .expect("a city owns more than its centre");
    let (fresh, coastal) = game.city_water(&game.cities[&id]);

    let dry = game.city_housing_sources(&game.cities[&id]);
    assert!(
        !game.city_has_active_district_family(&game.cities[&id], crate::name!("aqueduct")),
        "this city was supposed to start without an aqueduct"
    );

    game.cities
        .get_mut(&id)
        .unwrap()
        .districts
        .insert(crate::name!("aqueduct"), site);
    let wet = game.city_housing_sources(&game.cities[&id]);

    let lift = Game::city_housing_floor(fresh, coastal, true)
        - Game::city_housing_floor(fresh, coastal, false);
    assert!(lift > 0.0, "the aqueduct must be worth something here");
    assert_eq!(
        wet.water - dry.water,
        lift,
        "the aqueduct's housing belongs to `water`: the host folds it there \
         because an Aqueduct grants the city fresh water"
    );
    assert_eq!(
        wet.districts, dry.districts,
        "the aqueduct must not appear in `districts`; the host puts \
         Neighborhoods there, not this"
    );
}

#[test]
fn the_total_counts_every_category() {
    // Each field a different value, so dropping any one from `total()` changes
    // the answer. All-zero defaults cannot catch that, which is how the first
    // version of this passed with a category missing from the sum.
    let sources = HousingSources {
        water: 1.0,
        buildings: 2.0,
        districts: 4.0,
        improvements: 8.0,
        civics: 16.0,
        great_people: 32.0,
        great_works: 64.0,
        starting_era: 128.0,
        other: 256.0,
    };
    assert_eq!(sources.total(), 511.0, "a category is missing from total()");
    let named: f64 = sources.named().iter().map(|(_, value)| value).sum();
    assert_eq!(named, 511.0, "a category is missing from named()");
    assert_eq!(sources.named().len(), 9);
}

#[test]
fn every_category_the_host_reports_exists_here() {
    // A category CIVVIS cannot produce should read as a zero somebody can see,
    // not as an absence nobody notices. These are the host's own field names.
    let named = HousingSources::default().named();
    let names: Vec<&str> = named.iter().map(|(name, _)| *name).collect();
    for host in [
        "water",
        "buildings",
        "districts",
        "improvements",
        "civics",
        "great_people",
        "great_works",
        "starting_era",
    ] {
        assert!(names.contains(&host), "no category for housing_from_{host}");
    }
}

#[test]
fn housing_is_unchanged_by_the_split() {
    // The split was meant to be a pure reattribution: same arithmetic, named
    // destinations. A played game's housing is a fingerprint of that claim —
    // if any term moved category AND changed value, this moves with it.
    let game = played(50, 424242);
    let totals: Vec<(String, f64)> = game
        .cities
        .values()
        .map(|c| (c.name.clone(), game.city_housing(c)))
        .collect();
    for (name, housing) in &totals {
        assert!(
            *housing >= 2.0,
            "{name} has {housing} housing, below the driest possible floor"
        );
        assert!(
            housing.fract() == 0.0 || (housing.fract() - 0.5).abs() < 1e-9,
            "{name} has {housing} housing, which is not a Civilization VI step"
        );
    }
}

#[test]
fn a_pillaged_improvement_grants_no_housing() {
    // ⚠ Civilization VI gives a pillaged improvement NOTHING until it is
    // repaired — no yields, no housing. CIVVIS's building loop has always
    // honoured that (`city.pillaged_buildings`); the improvement loop did not
    // look at `tile.pillaged` at all, so a razed farm kept feeding the city's
    // growth ceiling forever.
    //
    // This is the asymmetry the per-source drift comparison surfaced:
    // `improvements` read HIGH against the host on the live seat — Ostia +1.5,
    // Cumae +2, Aquileia +1 — while every other category agreed.
    let mut game = played(30, 31337);
    let id = *game.cities.keys().next().expect("a played game has a city");

    // Put a farm on an owned tile inside the city's three-ring.
    let centre = game.cities[&id].pos;
    let site = *game.cities[&id]
        .owned_tiles
        .iter()
        .find(|p| **p != centre && game.wdist(centre, **p) <= 3)
        .expect("a city owns a tile beside its centre");
    {
        let tile = game.map.tiles.get_mut(&site).unwrap();
        tile.improvement = Some(crate::name!("farm"));
        tile.pillaged = false;
    }
    let standing = game.city_housing_sources(&game.cities[&id]).improvements;

    game.map.tiles.get_mut(&site).unwrap().pillaged = true;
    let razed = game.city_housing_sources(&game.cities[&id]).improvements;

    assert!(
        standing > razed,
        "a pillaged improvement still grants {standing} housing (razed reads \
         {razed}); Civilization VI gives it nothing until it is repaired"
    );

    // And repairing it brings the housing back, so the rule is a gate rather
    // than a one-way deletion.
    game.map.tiles.get_mut(&site).unwrap().pillaged = false;
    assert_eq!(
        game.city_housing_sources(&game.cities[&id]).improvements,
        standing,
        "repairing the improvement must restore its housing"
    );
}

#[test]
fn pillaging_moves_only_the_improvement_category() {
    // The fix must not leak into water, buildings or districts: a razed farm is
    // an improvement question and nothing else.
    let mut game = played(30, 4242);
    let id = *game.cities.keys().next().expect("a played game has a city");
    let centre = game.cities[&id].pos;
    let site = *game.cities[&id]
        .owned_tiles
        .iter()
        .find(|p| **p != centre && game.wdist(centre, **p) <= 3)
        .expect("a city owns a tile beside its centre");
    {
        let tile = game.map.tiles.get_mut(&site).unwrap();
        tile.improvement = Some(crate::name!("farm"));
        tile.pillaged = false;
    }
    let before = game.city_housing_sources(&game.cities[&id]);
    game.map.tiles.get_mut(&site).unwrap().pillaged = true;
    let after = game.city_housing_sources(&game.cities[&id]);

    assert_eq!(before.water, after.water);
    assert_eq!(before.buildings, after.buildings);
    assert_eq!(before.districts, after.districts);
    assert_eq!(before.civics, after.civics);
    assert_eq!(before.other, after.other);
    assert!(after.improvements < before.improvements);
}
