//! Equipment shared by production and the support-unit escort mover.
use crate::{game::Game, rules::UnitSpec};

pub(super) fn eligible_attacker(spec: &UnitSpec) -> bool {
    spec.class == "military" && matches!(spec.promotion_class.as_str(), "melee" | "anti_cavalry")
}

/// A ram cannot equip a mounted-only army. Include the current production
/// commitments so support can finish alongside the infantry that will use it.
pub(super) fn has_attackers(g: &Game, pid: usize) -> bool {
    g.units
        .values()
        .any(|unit| unit.owner == pid && eligible_attacker(&g.rules.units[unit.kind]))
        || g.cities
            .values()
            .filter(|city| city.owner == pid)
            .any(|city| {
                matches!(city.queue.first(), Some(crate::game::Item::Unit { unit })
                if eligible_attacker(&g.rules.units[*unit]))
            })
}
