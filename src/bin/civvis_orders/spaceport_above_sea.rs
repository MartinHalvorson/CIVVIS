//! Moves a queued live Spaceport order to the site
//! `AdvancedAi::native_spaceport_above_sea_position` picks.

use super::*;

pub(super) fn retarget(
    ai: &civvis::ai::AdvancedAi,
    mirror: &mirror::LiveMirror,
    orders: &mut [Order],
) -> usize {
    let mut changed = 0;
    for order in orders {
        if !matches!(order.kind, "produce" | "produce_next")
            || order.verb.as_deref() != Some("DISTRICT_SPACEPORT")
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
        if let Some(pos) = ai.native_spaceport_above_sea_position(&mirror.game, 0, cid, old) {
            order.pos = Some(civvis::hex::axial_to_offset(pos.0, pos.1));
            changed += 1;
        }
    }
    changed
}
