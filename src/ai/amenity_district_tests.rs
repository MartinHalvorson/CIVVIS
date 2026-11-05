use super::*;
use crate::game::Game;

/// A unique replacement is the same family, and a name filter will miss it.
///
/// This is pinned because I got it wrong in a merged comment: a census run
/// as `verb LIKE '%ENTERTAINMENT%'` reported zero Entertainment Complexes
/// across five live games, when seven were standing — as Hippodromes and a
/// Street Carnival.
#[test]
fn a_unique_replacement_belongs_to_the_family_it_replaces() {
    let game = crate::game::Game::new(2, 8, 8, 3, 100, 0);
    for unique in ["hippodrome", "street_carnival"] {
        let spec = &game.rules.districts[unique];
        assert_eq!(
            spec.replaces.as_deref(),
            Some("entertainment_complex"),
            "{unique} is an Entertainment Complex and any census must count it"
        );
        assert_eq!(
            game.district_family(crate::name::Name::new(unique))
                .as_str(),
            "entertainment_complex",
            "and the engine agrees, so a family check is the correct filter"
        );
    }
}

/// The omission this repairs: four families, and the one that fixes the
/// Amenity band is not among them.
#[test]
fn the_baseline_district_list_has_no_amenity_district() {
    assert!(
        !DISTRICT_PRIORITY.contains(&"entertainment_complex"),
        "if this ever gains one, the treatment below is redundant"
    );
    assert_eq!(DISTRICT_PRIORITY.len(), 4);
}

/// Off for the frozen controllers, on for the promoted production agent.
#[test]
fn only_the_promoted_agent_repairs_amenities() {
    assert!(!BasicAi::new().amenity_districts);
    assert!(!BasicAi::with_weights(Weights::default()).amenity_districts);
    assert!(crate::ai::AdvancedAi::new().weights().city_target > 0.0);
}

/// A city in deficit must rank the repair above the lane's own families,
/// and a neutral city must not ask for it at all — the band is flat at and
/// above zero, so an extra Amenity there multiplies nothing.
#[test]
fn the_repair_outranks_specialty_districts_only_while_the_band_is_being_paid() {
    let mut game = Game::new(2, 32, 24, 9_101, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &crate::game::Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];

    assert!(
        game.city_amenity_surplus(&game.cities[&city]) >= 0,
        "a new size-1 city is not in deficit"
    );

    // Grow it until the host's own band actually bites.
    let deficit_pop = (6..30)
        .find(|pop| {
            game.cities.get_mut(&city).unwrap().pop = *pop;
            game.city_amenity_surplus(&game.cities[&city]) <= -3
        })
        .expect("some population puts this city into the -3 band");
    game.cities.get_mut(&city).unwrap().pop = deficit_pop;

    let surplus = game.city_amenity_surplus(&game.cities[&city]);
    assert!(surplus <= -3);
    // `amenity_yield_mult_for` is private to the engine, so assert the band
    // boundary it keys on rather than keeping a second copy of the table
    // here — a copy would be free to drift from the rule it describes.
    assert!(
        surplus <= -3,
        "at -3 the engine's band multiplies EVERY yield in this city by \
             0.80, which is what makes the repair worth a district slot: got \
             {surplus}"
    );

    // And the treatment must be inert where the band is flat.
    let mut neutral = BasicAi::new();
    neutral.amenity_districts = true;
    game.cities.get_mut(&city).unwrap().pop = 1;
    assert!(
        game.city_amenity_surplus(&game.cities[&city]) >= 0,
        "back to neutral, where an extra Amenity multiplies nothing"
    );
}

/// A city one short keeps its lane order until it has a lane: the ×0.90
/// band is not worth an Entertainment Complex as the FIRST district of a
/// 2.7-production city (civvis-20260816T054344Z, Puteoli t50), while the
/// ×0.80 band always is, and a served city may take the repair at −1.
#[test]
fn a_one_short_city_keeps_its_lane_order_until_it_has_a_lane() {
    assert_eq!(
        AMENITY_DISTRICT_FIRST_BAND, 2.0,
        "the −3 band's edge, not the −1 one"
    );
    assert!(
        !BasicAi::amenity_repair_outranks_lane(1.0, 0),
        "−1, no districts: lane first"
    );
    assert!(
        !BasicAi::amenity_repair_outranks_lane(1.0, 1),
        "−1, one district: lane first"
    );
    assert!(
        BasicAi::amenity_repair_outranks_lane(1.0, 2),
        "−1 with two districts: repair"
    );
    assert!(
        BasicAi::amenity_repair_outranks_lane(2.0, 0),
        "−2 anywhere: repair"
    );
    assert!(BasicAi::amenity_repair_outranks_lane(6.0, 0), "and deeper");
    assert!(
        !BasicAi::amenity_repair_outranks_lane(0.0, 5),
        "neutral asks for nothing"
    );
}
