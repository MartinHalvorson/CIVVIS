//! Do not spend a planned Builder turn on clearing the output guard withholds.
//!
//! Native 014048Z kept Builder 3080216 on (44, 8), with three charges and
//! full movement, through t95-104. It repeatedly planned HARVEST_RESOURCE;
//! the output guard withheld the failed operation, after it had already
//! displaced other work. Apply that same, expiring guard before planning.

use super::{HostOrderRefusals, Order};
use civvis::{game::Game, name::Name};
use std::collections::{BTreeMap, BTreeSet};

/// These names are the reverse of `translate(Action::Improve)`. REMOVE_FEATURE
/// cannot distinguish the three clearings, so all three share its cooldown.
pub(super) fn refused_operations(
    game: &Game,
    mapped: &BTreeMap<u32, i64>,
    turn: u32,
    refusals: &HostOrderRefusals,
) -> BTreeMap<u32, BTreeSet<Name>> {
    let mut blocked: BTreeMap<u32, BTreeSet<Name>> = BTreeMap::new();
    for (uid, host) in mapped {
        let Some(_unit) = game
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
                blocked
                    .entry(*uid)
                    .or_default()
                    .extend(names.iter().map(|name| Name::new(name)));
            }
        }
    }
    blocked
}

#[cfg(test)]
#[path = "builder_operation_refusals/tests.rs"]
mod tests;
