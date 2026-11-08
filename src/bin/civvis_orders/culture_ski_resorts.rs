//! Translate the AI's adjacent Ski Resort jobs into Native unit operations.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn append(
    ai: &civvis::ai::AdvancedAi,
    mirror: &mirror::LiveMirror,
    blocked_host: &BTreeSet<(i32, i32)>,
    orders: &mut Vec<Order>,
) -> usize {
    let busy = orders
        .iter()
        .filter(|o| o.kind == "unit")
        .filter_map(|o| o.subject.and_then(|id| mirror.uid_of.get(&id).copied()))
        .collect();
    let blocked = blocked_host
        .iter()
        .map(|&(x, y)| civvis::hex::offset_to_axial(x, y))
        .collect();
    append_jobs(
        &mirror.civ6_of,
        // Simulated combat and movement can differ from what the host has
        // executed. New unit operations must start on the observed board.
        ai.native_culture_ski_resort_jobs(&mirror.game, 0, &busy, &blocked),
        orders,
    )
}

fn append_jobs(
    host_ids: &BTreeMap<u32, i64>,
    jobs: Vec<(u32, civvis::Pos, Option<civvis::Pos>)>,
    orders: &mut Vec<Order>,
) -> usize {
    let mut added = 0;
    for (uid, site, next) in jobs {
        let Some(&host) = host_ids.get(&uid) else {
            continue;
        };
        let pos = next.unwrap_or(site);
        orders.push(Order {
            kind: "unit",
            subject: Some(host),
            verb: Some(
                if next.is_some() {
                    "MOVE_TO"
                } else {
                    "IMPROVE:IMPROVEMENT_SKI_RESORT"
                }
                .into(),
            ),
            pos: Some(civvis::hex::axial_to_offset(pos.0, pos.1)),
        });
        added += 1;
    }
    added
}

#[cfg(test)]
#[path = "culture_ski_resorts/tests.rs"]
mod tests;
