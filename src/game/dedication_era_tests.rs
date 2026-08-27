use super::*;

/// A Golden Age multiplies nothing by itself; Heartbeat of Steam pays
/// Production equal to each Campus's Science adjacency; Reform the Coinage
/// pays +3 Gold per specialty district in the destination of an
/// INTERNATIONAL route. All three read off `CommemorationModifiers`
/// (`COMMEMORATION_INUDSTRIAL_GA_CAMPUS_MODIFIER`,
/// `COMMEMORATION_ECONOMIC_GA_TRADE_ROUTE_YIELDS`) and off the absence of
/// any yield modifier keyed on PLAYER_HAS_GOLDEN_AGE / a Dark Age; the
/// ×1.10 / ×0.95 this engine used to apply, and Sky and Stars' ×1.10, were
/// its own invention.
#[test]
fn a_golden_age_pays_only_what_its_dedication_says() {
    let mut g = Game::new_full(2, 26, 16, 4_217, 300, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .expect("a settler to found with");
    let cid = g.found_city_for(0, g.units[&settler].pos, None);
    g.players[0].age = "normal".to_string();
    let normal = g.city_yields(cid);
    g.players[0].age = "golden".to_string();
    let golden = g.city_yields(cid);
    assert!(
        (normal.production - golden.production).abs() < 1e-9,
        "no age multiplier: {normal:?} vs {golden:?}"
    );
    assert!((normal.science - golden.science).abs() < 1e-9);
    g.players[0].age = "dark".to_string();
    let dark = g.city_yields(cid);
    assert!((normal.production - dark.production).abs() < 1e-9);

    // Heartbeat of Steam: place a Campus, note its adjacency, dedicate.
    g.players[0].age = "golden".to_string();
    let city_pos = g.cities[&cid].pos;
    let site = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| {
            *pos != city_pos
                && g.map.get(*pos).is_some_and(|tile| {
                    !g.rules.is_water(tile) && tile.district.is_none() && tile.wonder.is_none()
                })
        })
        .expect("a land tile to hold the Campus");
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), site);
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("campus"));
    let before = g.city_yields(cid);
    g.players[0]
        .dedications
        .insert("heartbeat_of_steam".to_string());
    let after = g.city_yields(cid);
    let adjacency = g.district_yields(crate::name!("campus"), site).science
        - g.rules.districts["campus"].yields.science;
    assert!(
        (after.production - before.production - adjacency.max(0.0)).abs() < 1e-9,
        "Campus Science adjacency {adjacency} must arrive as Production: {before:?} -> {after:?}"
    );
    assert!(
        (after.science - before.science).abs() < 1e-9,
        "and nothing as Science"
    );
}

#[test]
fn every_dedication_opens_in_the_eras_its_commemoration_ships_for() {
    // CommemorationTypes carries MinimumGameEra/MaximumGameEra per
    // category, and Policies_XP1 carries a window again for each
    // same-named Golden Age card. They agree almost everywhere -- Free
    // Enquiry and SCIENTIFIC both Classical-Medieval, To Arms and MILITARY
    // both Industrial-Atomic, and four more -- which is what makes these
    // windows trustworthy rather than inferred. Sky and Stars is the one
    // exception; see the note on its row.
    //
    // Eras.ChronologyIndex is ONE-based (ERA_ANCIENT is 1), so every index
    // here is that column minus one.
    let expected: &[(&str, usize, usize)] = &[
        ("free_inquiry", 1, 2),              // SCIENTIFIC / POLICY_FREE_ENQUIRY
        ("pen_brush_and_voice", 1, 2),       // CULTURAL
        ("monumentality", 1, 3),             // INFRASTRUCTURE / POLICY_MONUMENTALITY
        ("exodus_of_the_evangelists", 1, 3), // RELIGIOUS / same-named card
        ("hic_sunt_dracones", 3, 5),         // EXPLORATION
        ("reform_the_coinage", 3, 5),        // ECONOMIC / same-named card
        ("heartbeat_of_steam", 4, 6),        // INDUSTRIAL / same-named card
        ("to_arms", 4, 6),                   // MILITARY / POLICY_TO_ARMS
        ("wish_you_were_here", 6, 8),        // TOURISM / same-named card
        ("bodyguard_of_lies", 6, 8),         // ESPIONAGE
        // The ONE place the two sources disagree, and the only one where it
        // matters which is authoritative. COMMEMORATION_AERONAUTICAL opens
        // at ERA_INFORMATION; the leftover POLICY_SKY_AND_STARS card says
        // ERA_ATOMIC. The Commemoration governs -- it is the table the age
        // transition reads, and CommemorationModifiers already carries the
        // Golden-Age half directly, which is what makes the same-named
        // RequiresGoldenAge cards dead data rather than a second opinion.
        ("sky_and_stars", 7, 8),     // COMMEMORATION_AERONAUTICAL
        ("automaton_warfare", 7, 8), // AUTOMATON
    ];
    let mut game = Game::new_full(1, 24, 16, 22_508, 120, 0, false);
    assert_eq!(game.rules.dedications.len(), expected.len());
    game.players[0].dedication_choices = 1;

    for &(name, first, last) in expected {
        let spec = &game.rules.dedications[name];
        assert_eq!((spec.eras.0, spec.eras.1), (first, last), "{name}");
    }

    // And the windows are what the chooser actually offers: no dedication
    // appears before its first era or after its last.
    for era in 0..=8 {
        game.world_era = era;
        let offered = game.available_dedications(0);
        for &(name, first, last) in expected {
            let inside = era >= first && era <= last;
            assert_eq!(
                offered.contains(&Name::new(name)),
                inside,
                "{name} in era {era}"
            );
        }
    }

    // The Ancient era offers nothing at all -- every category starts at
    // Classical or later, so the first Age is never a dedication choice.
    game.world_era = 0;
    assert!(game.available_dedications(0).is_empty());
}
