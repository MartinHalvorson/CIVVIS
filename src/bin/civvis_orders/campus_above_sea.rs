//! Moves a queued live Campus order to the site
//! `AdvancedAi::native_campus_above_sea_position` picks.

use super::*;

pub(super) fn retarget(
    ai: &civvis::ai::AdvancedAi,
    mirror: &mirror::LiveMirror,
    orders: &mut [Order],
) -> usize {
    let campus = mirror.game.civ_district_variant(0, "campus");
    let native_campus = civ6_district_type(&campus);
    let mut changed = 0;
    for order in orders {
        if !matches!(order.kind, "produce" | "produce_next")
            || order.verb.as_deref() != Some(native_campus.as_str())
        {
            continue;
        }
        let (Some(host_city), Some((x, y))) = (order.subject, order.pos) else {
            continue;
        };
        let Some(cid) = mirror.cid_of.get(&host_city).copied() else {
            continue;
        };
        let old = civvis::hex::offset_to_axial(x, y);
        if let Some(pos) = ai.native_campus_above_sea_position(&mirror.game, 0, cid, old) {
            order.pos = Some(civvis::hex::axial_to_offset(pos.0, pos.1));
            changed += 1;
        }
    }
    changed
}
