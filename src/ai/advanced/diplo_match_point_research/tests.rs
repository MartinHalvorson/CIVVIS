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
