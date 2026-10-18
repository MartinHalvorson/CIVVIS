use super::tests::walled_city;
use super::*;

/// `walled_city` on open grassland with a healthy Catapult of ours seven
/// tiles out: past the staging read, within the muster's.
fn gun_at_the_muster_line() -> (Game, u32, u32) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != city {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    let at = (city.0 - 7, city.1);
    assert_eq!(g.wdist(at, city), 7, "fixture: the gun's tile");
    let gun = g.spawn_unit("catapult", 0, at);
    (g, cid, gun)
}

#[test]
fn a_closed_muster_stages_with_the_guns_it_mustered() {
    let (g, cid, gun) = gun_at_the_muster_line();
    let city = CityView::of(&g, cid).unwrap();
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_stage_musters_out_of_reach();
    ai.stage_muster_ready.insert(cid, true);
    assert_eq!(ai.stage_breach_radius(cid), STAGING_FAR, "off by default");
    let off = ai.breach_reading_within(&g, 0, &city, &[gun], ai.stage_breach_radius(cid));
    assert!(off.leaves_walls_shut(&city), "today the gun is not read");

    ai.enable_stage_reads_the_muster_breakers();
    assert_eq!(ai.stage_breach_radius(cid), MUSTER_FAR);
    let on = ai.breach_reading_within(&g, 0, &city, &[gun], ai.stage_breach_radius(cid));
    assert_eq!(on.guns, 1, "the mustered gun is a breaker");
    assert!(!on.leaves_walls_shut(&city));
}

#[test]
fn an_open_muster_or_the_parent_gene_off_keeps_the_staging_read() {
    let (_, cid, _) = gun_at_the_muster_line();
    let mut ai = AdvancedAi::new();
    ai.enable_stage_reads_the_muster_breakers();
    ai.stage_muster_ready.insert(cid, true);
    assert_eq!(
        ai.stage_breach_radius(cid),
        STAGING_FAR,
        "needs stage-musters-out-of-reach"
    );
    ai.enable_stage_musters_out_of_reach();
    ai.stage_muster_ready.insert(cid, false);
    assert_eq!(
        ai.stage_breach_radius(cid),
        STAGING_FAR,
        "the muster is open"
    );
}
