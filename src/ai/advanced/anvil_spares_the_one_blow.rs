//! `anvil-spares-the-one-blow`: the Anvil around a threatened city never
//! leaves the City Center empty while it has a land soldier, and never posts
//! a soldier off the city where one blow at the top of its roll removes it.
//!
//! The Anvil (`anvil_orders_for` in `siege_train.rs`) gives the city tile to
//! a wounded member or a Shooter and puts melee on the front tiles facing the
//! enemy. A force with no Shooter leaves the city tile to nobody, and the
//! front is the most exposed ground there is. A garrison is not a combat
//! target (blows on a City Center land on the city), so the soldier inside
//! cannot be removed by a blow; on the ring it can.
//!
//! Live Lv 8 Deity Korea, 2026-11-05: across 89 games, 135 of 824 of our
//! units killed in an enemy phase had started that turn inside one of our
//! cities and stepped out; 42 of them under an Anvil at that city, 15 with
//! the journal reading "Anvil at <city>: 1 on the ring, nobody in the city".
//! civvis-20261105T033244Z t151: Sangju's only soldier, a full-health
//! Warrior, was posted from the City Center to the ring beside a Cavalry,
//! with a Tank and a Machine Gun three tiles off; it fortified there and a
//! Machine Gun killed it in the enemy phase, leaving the city bare.
//!
//! With the gene on, after the posts are drawn (and after
//! `anvil-garrisons-its-melee`):
//! 1. If no post holds the City Center and no soldier of ours outside the
//!    force stands in it, the force's land soldier standing in the city, or
//!    else the nearest one that can reach it this turn, takes the city post.
//! 2. A soldier posted off the city where the danger field's top-of-roll
//!    reading meets its health takes, instead, the tile it stands on or an
//!    open tile within two of the city that is not posted — the one with the
//!    least such danger, nearest the city — when that reading is lower.
//!
//! Off, the Anvil's posts are unchanged.

use super::battle_planner::DangerField;
use super::*;
use std::cmp::Reverse;

/// A land soldier the Anvil may post: military, on land and not embarked,
/// unlinked, and able to fight.
fn land_soldier(g: &Game, pid: usize, uid: u32) -> bool {
    let Some(unit) = g.units.get(&uid) else {
        return false;
    };
    let spec = &g.rules.units[unit.kind];
    unit.owner == pid
        && spec.class == "military"
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        && unit.linked_to.is_none()
        && !g.is_embarked(unit)
        && (spec.is_melee_capable() || spec.has_ranged_attack())
}

/// A tile a soldier of ours may be posted on: passable land holding nobody
/// but our own.
fn open_post(g: &Game, pid: usize, pos: Pos) -> bool {
    g.map
        .get(pos)
        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        && g.unit_ids_at(pos).iter().all(|id| g.units[id].owner == pid)
}

/// What the rewrite did, for the journal: the soldier given the empty city
/// tile, and each soldier moved off a one-blow post `(unit, from, to)`.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Spared {
    pub filled: Option<u32>,
    pub moved: Vec<(u32, Pos, Pos)>,
}

/// Rewrite `orders` (the Anvil's posts for `city`) as the module docs say.
pub(super) fn spare_the_one_blow(
    g: &Game,
    pid: usize,
    city: Pos,
    members: &[u32],
    orders: &mut BTreeMap<u32, Pos>,
    field: &mut DangerField,
) -> Spared {
    let mut spared = Spared::default();
    let soldiers: Vec<u32> = members
        .iter()
        .copied()
        .filter(|uid| land_soldier(g, pid, *uid))
        .collect();
    // 1. The City Center is never left to nobody.
    let held = orders.values().any(|post| *post == city);
    let kept_by_another = g
        .unit_ids_at(city)
        .iter()
        .any(|id| !members.contains(id) && land_soldier(g, pid, *id));
    if !held && !kept_by_another {
        let pick = soldiers
            .iter()
            .copied()
            .filter(|uid| g.units[uid].pos == city || g.reachable(*uid).contains(&city))
            .min_by_key(|uid| {
                let unit = &g.units[uid];
                (
                    g.wdist(unit.pos, city),
                    Reverse(
                        (crate::game::effective_strength(g.unit_strength(unit, true), unit.hp)
                            * 100.0) as i64,
                    ),
                    *uid,
                )
            });
        if let Some(pick) = pick {
            orders.insert(pick, city);
            spared.filled = Some(pick);
        }
    }
    // 2. No post off the city where one blow at the top of the roll removes
    //    the soldier.
    let posted: Vec<(u32, Pos)> = orders
        .iter()
        .filter(|(uid, post)| **post != city && soldiers.contains(uid))
        .map(|(uid, post)| (*uid, *post))
        .collect();
    for (uid, post) in posted {
        let hp = f64::from(g.units[&uid].hp);
        let at_post = field.upper_danger(post, uid);
        if at_post < hp {
            continue;
        }
        let here = g.units[&uid].pos;
        let taken: BTreeSet<Pos> = orders
            .iter()
            .filter(|(other, _)| **other != uid)
            .map(|(_, pos)| *pos)
            .collect();
        let mut options: Vec<Pos> = g
            .wdisk(city, 2)
            .into_iter()
            .filter(|pos| *pos != city && !taken.contains(pos) && open_post(g, pid, *pos))
            .collect();
        if here != city && !taken.contains(&here) && !options.contains(&here) {
            options.push(here);
        }
        let best = options
            .into_iter()
            .map(|pos| (field.upper_danger(pos, uid), pos))
            .min_by(|(left, a), (right, b)| {
                (left >= &hp)
                    .cmp(&(right >= &hp))
                    .then_with(|| left.total_cmp(right))
                    .then_with(|| (*a != here).cmp(&(*b != here)))
                    .then_with(|| g.wdist(*a, city).cmp(&g.wdist(*b, city)))
                    .then_with(|| a.cmp(b))
            });
        if let Some((danger, to)) = best {
            if to != post && danger < at_post {
                orders.insert(uid, to);
                spared.moved.push((uid, post, to));
            }
        }
    }
    spared
}

impl AdvancedAi {
    /// `anvil-spares-the-one-blow`: rewrite this turn's Anvil posts for city
    /// `cid`. Called right after the posts are drawn; nothing with the gene
    /// off.
    pub(super) fn anvil_spares_the_one_blow_step(
        &mut self,
        g: &Game,
        pid: usize,
        cid: u32,
        members: &[u32],
    ) {
        if !self.anvil_spares_the_one_blow {
            return;
        }
        let Some(city) = g.cities.get(&cid).map(|c| c.pos) else {
            return;
        };
        let mut field = DangerField::with_reach(g, pid, self.strike_reach);
        let mut orders = std::mem::take(&mut self.anvil_orders);
        let spared = spare_the_one_blow(g, pid, city, members, &mut orders, &mut field);
        self.anvil_orders = orders;
        if !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        let name = g.cities[&cid].name.clone();
        if let Some(uid) = spared.filled {
            let kind = g.units[&uid].kind.to_string();
            think!(self.journal(), Military, Decision,
                "Anvil at {name}: the {kind} keeps the city tile";
                "anvil-spares-the-one-blow: no member was posted inside, and a garrison takes no blow — the city does";
                city);
        }
        for (uid, from, to) in spared.moved {
            let kind = g.units[&uid].kind.to_string();
            think!(self.journal(), Military, Decision,
                "Anvil at {name}: the {kind} posted at {from:?} takes {to:?}";
                "anvil-spares-the-one-blow: one blow at the top of its roll removes it at the post";
                to);
        }
    }
}

#[cfg(test)]
mod tests;
