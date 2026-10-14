use super::*;
use crate::ai::advanced::StrategicPlan;
use std::sync::Arc;

/// Where the fixture's cities stand: ours on the west edge, the target's
/// front city seven tiles east of them (the objective), a city 11 tiles
/// past the objective (on the front only through it), and a back-country
/// city over 12 tiles from both.
const OUR_CITIES: [Pos; 2] = [(2, 8), (2, 14)];
const FRONT_CITY: Pos = (9, 8);
const BEHIND_THE_OBJECTIVE: Pos = (20, 8);
const BACK_COUNTRY: Pos = (40, 8);

/// The culture-counter seat of `culture_counter_tests::fixture` on a map
/// wide enough for a back country: a Domination seat beside an urgent,
/// walled culture rival whose front city is the objective.
fn fixture() -> (Game, AdvancedAi, StrategicPlan, [u32; 3]) {
    let mut g = Game::new_full(2, 64, 24, 370_001, 400, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    for pos in OUR_CITIES {
        g.found_city_for(0, pos, None);
    }
    let front = g.found_city_for(1, FRONT_CITY, None);
    let behind = g.found_city_for(1, BEHIND_THE_OBJECTIVE, None);
    let back = g.found_city_for(1, BACK_COUNTRY, None);
    g.cities.get_mut(&front).unwrap().wall_hp = 400;
    for (city, pop) in [(front, 2), (behind, 3), (back, 10)] {
        g.cities.get_mut(&city).unwrap().pop = pop;
    }
    for ours in OUR_CITIES {
        assert!(g.wdist(ours, FRONT_CITY) <= PARITY_FRONT_RADIUS);
        assert!(g.wdist(ours, BEHIND_THE_OBJECTIVE) > PARITY_FRONT_RADIUS);
        assert!(g.wdist(ours, BACK_COUNTRY) > PARITY_FRONT_RADIUS);
    }
    assert!(g.wdist(FRONT_CITY, BEHIND_THE_OBJECTIVE) <= PARITY_FRONT_RADIUS);
    assert!(g.wdist(FRONT_CITY, BACK_COUNTRY) > PARITY_FRONT_RADIUS);
    g.record_contact(0, 1);
    g.at_war.clear();
    g.current = 0;
    g.turn = 170;
    g.players[0].gold = 10000.0;
    for y in 0..4 {
        g.spawn_test_unit("musketman", 0, (1, 2 + y));
    }
    g.spawn_test_unit("musketman", 1, (12, 8));
    let observed = Arc::make_mut(&mut g.observed_public_empire_stats);
    observed.entry(0).or_default().domestic_tourists = Some(100);
    let theirs = observed.entry(1).or_default();
    theirs.foreign_tourists = Some(85);
    // The host's public population: the three cities on the board and the
    // ones the seat has not seen.
    theirs.population = Some(30);
    theirs.city_count = Some(5);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.coalition_before_war = false;
    ai.coalition_before_war_2 = false;
    ai.coalition_before_war_3 = false;
    ai.enable_culture_counter_declares();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(front),
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(ai.urgent_victory_threat(&g, 1));
    assert!(
        ai.culture_counter_due(&g, 0, 1),
        "fixture: the counter is due"
    );
    (g, ai, plan, [front, behind, back])
}

/// The host's public Production for the target at `factor` times ours, as
/// the mirror carries it: the board's derived yields plus a correction.
fn public_production(g: &mut Game, factor: f64) -> f64 {
    let ours = crate::ai::BasicAi::seat_production_per_turn(g, 0);
    let derived = crate::ai::BasicAi::seat_production_per_turn(g, 1)
        - g.observed_yield_adjustments
            .get(&1)
            .map_or(0.0, |adjustment| adjustment.production);
    let wanted = factor * ours;
    let correction = if wanted == derived {
        0.25
    } else {
        wanted - derived
    };
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        1,
        crate::rules::Yields {
            production: correction,
            ..Default::default()
        },
    );
    let theirs = AdvancedAi::rival_production_reading(g, 1).expect("a reading");
    assert!(
        (theirs - wanted).abs() < 0.5,
        "fixture: {theirs} against {wanted}"
    );
    theirs
}

/// See `rival_front_production`: on the live board the host's public total
/// is shared out by the front cities' population over the public
/// population; the objective brings the cities within 12 tiles of it; the
/// city count stands in when the population did not cross; a native board
/// sums the front cities' own yields; and no city on the front is no
/// reading.
#[test]
fn the_front_reading_shares_the_public_total_by_population() {
    let (mut g, _, _, [front, behind, _]) = fixture();
    let whole = public_production(&mut g, 2.0);
    let objective = Some(FRONT_CITY);

    let reading = AdvancedAi::rival_front_production(&g, 0, 1, None, whole).unwrap();
    assert_eq!(reading.cities, 1, "only the city within 12 tiles of ours");
    assert!((reading.production - whole * 2.0 / 30.0).abs() < 1e-9);

    let reading = AdvancedAi::rival_front_production(&g, 0, 1, objective, whole).unwrap();
    assert_eq!(reading.cities, 2, "the objective brings the city behind it");
    assert!((reading.production - whole * 5.0 / 30.0).abs() < 1e-9);

    Arc::make_mut(&mut g.observed_public_empire_stats)
        .get_mut(&1)
        .unwrap()
        .population = None;
    let reading = AdvancedAi::rival_front_production(&g, 0, 1, objective, whole).unwrap();
    assert!(
        (reading.production - whole * 2.0 / 5.0).abs() < 1e-9,
        "two of the five public cities"
    );
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .get_mut(&1)
        .unwrap()
        .city_count = None;
    assert_eq!(
        AdvancedAi::rival_front_production(&g, 0, 1, objective, whole),
        None,
        "nothing to share the total out by"
    );

    Arc::make_mut(&mut g.observed_public_empire_stats).remove(&1);
    Arc::make_mut(&mut g.observed_yield_adjustments).remove(&1);
    let native = AdvancedAi::rival_production_reading(&g, 1).unwrap();
    let reading = AdvancedAi::rival_front_production(&g, 0, 1, objective, native).unwrap();
    let cities = g.city_yields(front).production + g.city_yields(behind).production;
    assert!(
        (reading.production - cities).abs() < 1e-9,
        "a native board sums its cities"
    );
    assert!(reading.production < native);

    for city in [front, behind] {
        g.cities.get_mut(&city).unwrap().owner = 0;
    }
    assert_eq!(
        AdvancedAi::rival_front_production(&g, 0, 1, objective, native),
        None,
        "no city of theirs on the front"
    );
}

/// See `declaration_has_production_parity`: the urgent culture counter
/// against a rival making twice our Production. Its cities within 12 tiles
/// of ours and of the objective hold 5 of its 30 public population, a
/// third of ours at the front: `declaration-needs-production-parity` holds
/// the war and `parity-reads-the-front` opens it. The front gene alone is
/// the ungated war.
#[test]
fn a_rival_out_producing_us_only_behind_its_front_is_declared_on_under_the_gene() {
    // (parity, front; declares)
    for (parity, front, declares) in [
        (false, false, true),
        (false, true, true),
        (true, false, false),
        (true, true, true),
    ] {
        let (mut g, mut ai, plan, _) = fixture();
        public_production(&mut g, 2.0);
        if parity {
            ai.enable_declaration_needs_production_parity();
        }
        if front {
            ai.enable_parity_reads_the_front();
        }
        assert_eq!(
            ai.declaration_has_production_parity(&g, 0, 1, Some(FRONT_CITY)),
            declares,
            "parity {parity} front {front}"
        );
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert_eq!(g.is_at_war(0, 1), declares, "parity {parity} front {front}");
    }
}

/// See `declaration_has_production_parity`: a rival whose Production stands
/// at its front -- the front cities hold 25 of its 30 public population --
/// holds the war under both genes, and so does one with no city on the
/// front, where the whole-empire reading stands.
#[test]
fn a_front_that_out_produces_us_holds_the_war_under_the_gene() {
    let (mut g, mut ai, plan, [front, behind, back]) = fixture();
    for (city, pop) in [(front, 15), (behind, 10), (back, 2)] {
        g.cities.get_mut(&city).unwrap().pop = pop;
    }
    let whole = public_production(&mut g, 2.0);
    ai.enable_declaration_needs_production_parity();
    ai.enable_parity_reads_the_front();
    let reading = AdvancedAi::rival_front_production(&g, 0, 1, Some(FRONT_CITY), whole).unwrap();
    let ours = crate::ai::BasicAi::seat_production_per_turn(&g, 0);
    assert!(ours < DECLARATION_PRODUCTION_PARITY * reading.production);
    assert!(!ai.declaration_has_production_parity(&g, 0, 1, Some(FRONT_CITY)));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1));

    let (mut g, mut ai, _, [front, behind, _]) = fixture();
    for city in [front, behind] {
        g.cities.get_mut(&city).unwrap().owner = 0;
    }
    let whole = public_production(&mut g, 2.0);
    ai.enable_declaration_needs_production_parity();
    ai.enable_parity_reads_the_front();
    assert_eq!(
        AdvancedAi::rival_front_production(&g, 0, 1, None, whole),
        None
    );
    assert!(!ai.declaration_has_production_parity(&g, 0, 1, None));
}

/// See `declaration_has_production_parity`: with no reading of the target's
/// Production (public figures without a Production term) the war opens
/// under either gene, as it did before them.
#[test]
fn no_production_reading_leaves_the_war_to_the_other_gates() {
    for front in [false, true] {
        let (mut g, mut ai, plan, _) = fixture();
        assert_eq!(AdvancedAi::rival_production_reading(&g, 1), None);
        ai.enable_declaration_needs_production_parity();
        if front {
            ai.enable_parity_reads_the_front();
        }
        assert!(ai.declaration_has_production_parity(&g, 0, 1, Some(FRONT_CITY)));
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert!(g.is_at_war(0, 1), "front {front}");
    }
}

#[test]
fn parity_reads_the_front_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers("parity-reads-the-front", |ai| {
        ai.parity_reads_the_front
    });
}
