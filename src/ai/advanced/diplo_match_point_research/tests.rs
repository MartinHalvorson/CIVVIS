use super::*;

#[test]
fn a_diplomatic_seat_at_match_point_names_both_points() {
    let mut g = Game::new(2, 24, 16, 72, 200, 0);
    let mut seat = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    g.players[0].dvp = DIPLO_MATCH_POINT_DVP;
    assert_eq!(seat.diplo_match_point_tech(&g, 0), None, "off");
    seat.enable_diplo_match_point_beelines_the_points();
    assert_eq!(seat.diplo_match_point_tech(&g, 0), Some("seasteads"));
    assert_eq!(
        seat.diplo_match_point_civic(&g, 0),
        Some("global_warming_mitigation")
    );

    // Below the bar: nothing.
    g.players[0].dvp = DIPLO_MATCH_POINT_DVP - 1;
    assert_eq!(seat.diplo_match_point_tech(&g, 0), None);
    assert_eq!(seat.diplo_match_point_civic(&g, 0), None);
    g.players[0].dvp = 18;

    // Each goal stops once its point is in.
    g.players[0].techs.insert(crate::name!("seasteads"));
    assert_eq!(seat.diplo_match_point_tech(&g, 0), None);
    assert_eq!(
        seat.diplo_match_point_civic(&g, 0),
        Some("global_warming_mitigation")
    );
    g.players[0]
        .civics
        .insert(crate::name!("global_warming_mitigation"));
    assert_eq!(seat.diplo_match_point_civic(&g, 0), None);
    g.players[0].techs.remove(&crate::name!("seasteads"));

    // A lost lane, or another lane, never.
    let mut lost = seat.clone();
    lost.lane_lost = true;
    assert_eq!(lost.diplo_match_point_tech(&g, 0), None);
    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_diplo_match_point_beelines_the_points();
    assert_eq!(culture.diplo_match_point_tech(&g, 0), None);
}

/// The goals reach the research slot: under a Recovery plan -- the plan live
/// civvis-20261030T111154Z read from t197 -- a Diplomatic seat at match point
/// with the Future era open researches Seasteads itself and adopts Global
/// Warming Mitigation itself, among the other Future-era nodes on offer.
#[test]
fn a_recovery_plan_still_researches_the_points_at_match_point() {
    use crate::ai::advanced::tests::wonder_plan;
    let mut g = Game::new_full(1, 20, 14, 764, 300, 0, false);
    g.turn = g.max_turns * 3 / 4;
    let before_future: Vec<Name> = g
        .rules
        .techs
        .iter()
        .filter(|(_, spec)| spec.era < 8)
        .map(|(tech, _)| Name::new(tech))
        .collect();
    g.players[0].techs.extend(before_future);
    let civics_before: Vec<Name> = g
        .rules
        .civics
        .iter()
        .filter(|(_, spec)| spec.era < 8)
        .map(|(civic, _)| Name::new(civic))
        .collect();
    g.players[0].civics.extend(civics_before);
    g.players[0].dvp = DIPLO_MATCH_POINT_DVP;
    let recovery = wonder_plan(GrandStrategy::Recovery, g.turn);

    let mut seat = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    seat.enable_diplo_match_point_beelines_the_points();
    let mut on = g.clone();
    seat.advanced_research(&mut on, 0, &recovery);
    assert_eq!(on.players[0].research.as_deref(), Some("seasteads"));
    assert_eq!(
        on.players[0].civic.as_deref(),
        Some("global_warming_mitigation")
    );

    // Below the bar the same seat leaves the slot to the plan.
    let mut below = g.clone();
    below.players[0].dvp = DIPLO_MATCH_POINT_DVP - 1;
    seat.advanced_research(&mut below, 0, &recovery);
    assert!(below.players[0].research.is_some());
}

/// The civic point comes before the government ladder: live Immortal
/// civvis-20261030T190332Z stood on 18 of 20 from t172 with its civic slot on
/// `government_ladder_goal`'s Corporate Libertarianism, and lost to
/// Technology at t175.
#[test]
fn the_civic_point_comes_before_the_government_ladder() {
    use crate::ai::advanced::tests::wonder_plan;
    let mut g = Game::new_full(1, 20, 14, 764, 300, 0, false);
    // Inside the ladder's window (half the clock while not behind on slots).
    g.turn = g.max_turns * 2 / 5;
    let techs: Vec<Name> = g
        .rules
        .techs
        .iter()
        .filter(|(_, spec)| spec.era < 8)
        .map(|(tech, _)| Name::new(tech))
        .collect();
    g.players[0].techs.extend(techs);
    // Every tier-three government stays unadopted, so the ladder has a rung
    // to name; the civic point's prerequisites are cleared, so this seed's
    // Future layout cannot make the ladder's rung a step toward it.
    let tier_three = [
        "corporate_libertarianism",
        "digital_democracy",
        "synthetic_technocracy",
    ];
    {
        let rules = std::sync::Arc::make_mut(&mut g.rules);
        rules
            .civics
            .get_mut("global_warming_mitigation")
            .expect("the civic point exists")
            .requires
            .clear();
        if let Some(ancestors) = rules.civic_ancestors.get_mut("global_warming_mitigation") {
            ancestors.clear();
        }
    }
    let civics: Vec<Name> = g
        .rules
        .civics
        .iter()
        .filter(|(civic, spec)| spec.era < 8 && !tier_three.contains(&civic.as_str()))
        .map(|(civic, _)| Name::new(civic))
        .collect();
    g.players[0].civics.extend(civics);
    g.players[0].dvp = DIPLO_MATCH_POINT_DVP + 2;
    let plan = wonder_plan(GrandStrategy::Expansion, g.turn);

    let mut seat = AdvancedAi::targeting(VictoryTarget::Diplomacy);
    seat.enable_government_ladder_2();
    assert!(
        seat.government_ladder_goal(&g, 0, GrandStrategy::Expansion)
            .is_some(),
        "fixture: the ladder names a tier-three government"
    );
    let mut off = g.clone();
    seat.advanced_research(&mut off, 0, &plan);
    assert_ne!(
        off.players[0].civic.as_deref(),
        Some("global_warming_mitigation"),
        "off: the ladder takes the slot"
    );
    seat.enable_diplo_match_point_beelines_the_points();
    let mut on = g.clone();
    seat.advanced_research(&mut on, 0, &plan);
    assert_eq!(
        on.players[0].civic.as_deref(),
        Some("global_warming_mitigation"),
        "on: the point first"
    );
}
