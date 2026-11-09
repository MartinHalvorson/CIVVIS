//! Do not spend a planned Builder turn on clearing the output guard withholds.
//!
//! Native 014048Z kept Builder 3080216 on (44, 8), with three charges and
//! full movement, through t95-104. It repeatedly planned HARVEST_RESOURCE;
//! the output guard withheld the failed operation, after it had already
//! displaced other work. Apply that same, expiring guard before planning.

use super::{HostOrderRefusals, Order};
use civvis::{game::Game, name::Name};
use std::collections::BTreeMap;

/// These names are the reverse of `translate(Action::Improve)`. REMOVE_FEATURE
/// cannot distinguish the three clearings, so all three share its cooldown.
pub(super) fn apply_cooldowns(
    game: &mut Game,
    mapped: &BTreeMap<u32, i64>,
    turn: u32,
    refusals: &HostOrderRefusals,
) -> usize {
    let mut blocked = Vec::new();
    for (uid, host) in mapped {
        let Some(unit) = game
            .units
            .get(uid)
            .filter(|unit| unit.owner == 0 && unit.kind == "builder" && unit.charges > 0)
        else {
            continue;
        };
        for (verb, names) in [
            ("HARVEST_RESOURCE", &["harvest_resource"][..]),
            (
                "REMOVE_FEATURE",
                &["chop_woods", "chop_rainforest", "clear_marsh"][..],
            ),
            ("PLANT_FOREST", &["plant_woods"][..]),
        ] {
            let order = Order {
                kind: "unit",
                subject: Some(*host),
                verb: Some(verb.into()),
                pos: None,
            };
            if refusals.withheld(&order, turn).is_some() {
                blocked.extend(names.iter().map(|name| (unit.pos, Name::new(name))));
            }
        }
    }
    // Only the disposable decision board changes. The authoritative mirror,
    // other tiles, and other operations remain available; a fresh board after
    // the existing cooldown expires can offer this operation again.
    if blocked.is_empty() {
        return 0;
    }
    let names = std::sync::Arc::make_mut(&mut game.blocked_improvements);
    blocked
        .into_iter()
        .filter(|(pos, name)| names.entry(*pos).or_default().insert(*name))
        .count()
}

#[cfg(test)]
#[path = "builder_operation_refusals/tests.rs"]
mod tests;
