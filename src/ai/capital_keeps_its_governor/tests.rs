use super::*;

fn fixture() -> (Game, u32, u32) {
    let mut g = Game::new_full(1, 36, 24, 950_108, 500, 0, false);
    let pos = g.units[&g.player_unit_ids(0)[0]].pos;
    let capital = g.found_city_for(0, pos, None);
    let next = g
        .map
        .tiles
        .values()
        .filter(|t| {
            g.rules.is_passable(t)
                && !g.rules.is_water(t)
                && t.owner_city.is_none()
                && t.district.is_none()
                && g.wdist(pos, t.pos) >= 5
        })
        .map(|t| t.pos)
        .min()
        .unwrap();
    let colony = g.found_city_for(0, next, None);
    g.turn = 69;
    g.players[0]
        .counters
        .insert("district_governor_titles".into(), 1);
    g.apply(
        0,
        &Action::AppointGovernor {
            governor: crate::name!("magnus"),
            city: capital,
        },
    )
    .unwrap();
    g.cities.get_mut(&capital).unwrap().loyalty = 96.3125;
    g.cities.get_mut(&colony).unwrap().loyalty = 66.7188;
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(capital, -0.515625);
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(colony, -4.21875);
    (g, capital, colony)
}

fn guarded() -> BasicAi {
    let mut ai = BasicAi::new();
    ai.capital_keeps_its_governor = true;
    ai
}

#[test]
fn the_native_donor_shape_keeps_its_anchor_only_with_the_treatment() {
    let (g, capital, colony) = fixture();
    let mut off = g.clone();
    assert!(BasicAi::new().reassign_governor_for_loyalty(&mut off, 0));
    assert_eq!(off.players[0].governor_roster["magnus"].city, Some(colony));
    let mut on = g;
    let log_length = on.log.len();
    assert!(!guarded().reassign_governor_for_loyalty(&mut on, 0));
    assert_eq!(on.players[0].governor_roster["magnus"].city, Some(capital));
    assert_eq!(on.log.len(), log_length);
}

#[test]
fn a_positive_rate_that_depends_on_the_governor_is_not_a_safe_donor() {
    let (mut g, capital, _) = fixture();
    for (rate, allowed) in [(7.5, false), (8.0, true), (8.5, true)] {
        Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(capital, rate);
        assert_eq!(
            guarded().capital_governor_may_depart(&g, 0, "magnus"),
            allowed
        );
    }
}

#[test]
fn governor_policy_support_is_included_in_the_departure_estimate() {
    let (mut g, capital, _) = fixture();
    g.players[0].policies.insert(crate::name!("praetorium"));
    for (rate, allowed) in [(9.5, false), (10.0, true)] {
        Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(capital, rate);
        assert_eq!(
            guarded().capital_governor_may_depart(&g, 0, "magnus"),
            allowed
        );
    }
}

#[test]
fn a_safe_capital_still_supplies_ordinary_loyalty_relief() {
    let (mut g, capital, colony) = fixture();
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).insert(capital, 12.0);
    assert!(guarded().reassign_governor_for_loyalty(&mut g, 0));
    assert_eq!(g.players[0].governor_roster["magnus"].city, Some(colony));
}

#[test]
fn noncapital_foreign_founder_and_unassigned_governors_keep_the_old_permission() {
    let (g, capital, _) = fixture();
    let mut ordinary = g.clone();
    ordinary.cities.get_mut(&capital).unwrap().is_capital = false;
    assert!(guarded().capital_governor_may_depart(&ordinary, 0, "magnus"));
    let mut foreign = g.clone();
    foreign.cities.get_mut(&capital).unwrap().original_owner = 1;
    assert!(guarded().capital_governor_may_depart(&foreign, 0, "magnus"));
    let mut unassigned = g;
    unassigned.players[0]
        .governor_roster
        .get_mut("magnus")
        .unwrap()
        .city = None;
    assert!(guarded().capital_governor_may_depart(&unassigned, 0, "magnus"));
}

#[test]
fn analysis_preserves_the_observed_rate_and_live_roster_even_inside_a_memo() {
    let (g, capital, _) = fixture();
    let _memo = g.query_memo();
    let before = serde_json::to_string(&g).unwrap();
    assert!(!guarded().capital_governor_may_depart(&g, 0, "magnus"));
    assert_eq!(g.city_loyalty_per_turn(&g.cities[&capital]), -0.515625);
    assert_eq!(serde_json::to_string(&g).unwrap(), before);
}
