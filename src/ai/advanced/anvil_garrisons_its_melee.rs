//! `anvil-garrisons-its-melee`: the Anvil around a threatened city with no
//! standing walls garrisons its strongest melee soldier, not a Shooter.
//!
//! The Anvil (`anvil_orders_for` in `siege_train.rs`) gives the city tile to
//! a Shooter and puts melee on the ring. Behind walls that is right: the
//! city shoots and the Shooter is safe. Without walls the city's defence is
//! its garrison's melee strength (`Game::city_strength`: the strongest
//! garrisoned soldier, over a floor of the strongest unit built less ten):
//! 20 with a Warrior inside, 10 with a Slinger (melee 5) or nobody.
//!
//! Live Lv 7, 2026-10-30, two frontier Corinths, both founded t15-16 at 1-4
//! production, neither with walls:
//! - civvis-20261030T200156Z: "Anvil at Corinth: 1 on the ring, slinger in
//!   the city" t22-26. The Slinger never reached the city (it was 4+ tiles
//!   off), the Warrior stood on the ring, and Civ VI read the city's
//!   defence at 10 every turn but t23 (20, the Warrior inside). It fell at
//!   t27.
//! - civvis-20261030T201608Z: a Warrior stood inside at t26 (defence 18.6).
//!   At t27 the Anvil posted the Slinger to the city and the Warrior to the
//!   ring; defence 10, siege t27, and the city fell at t28.
//!
//! With the gene on and the city's walls down, the Anvil's strongest melee
//! soldier standing in the city or beside it takes the city post when its
//! melee strength would lift the city's defence over the post holder's and
//! the floor. One beside the city swaps in at once. The Shooter it displaces
//! steps to the open tile within two farthest from the hostiles. That
//! garrison makes no blow, since a melee kill advances out of the city.
//! Walls standing, no melee near, or the gene off: the Anvil is unchanged.

use super::*;
use std::cmp::Reverse;

/// The garrison's melee strength must beat the post holder's by at least
/// this much to take the post (a Scout for a Slinger is not worth a swap).
const GARRISON_GAIN: f64 = 5.0;

/// A land soldier whose garrison lends a city its melee strength: military,
/// on land and not embarked, no ranged attack, and able to fight in melee.
pub(super) fn holds_a_city(g: &Game, uid: u32) -> bool {
    let Some(unit) = g.units.get(&uid) else {
        return false;
    };
    let spec = &g.rules.units[unit.kind];
    spec.class == "military"
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
        && unit.linked_to.is_none()
        && !g.is_embarked(unit)
        && !spec.has_ranged_attack()
        && spec.is_melee_capable()
}

/// The defence an unwalled city of `pid` holds with nobody inside: the
/// strongest unit built less ten, never under ten (`Game::city_strength`).
fn ungarrisoned_floor(g: &Game, pid: usize) -> f64 {
    let current_best = g
        .units
        .values()
        .filter(|unit| unit.owner == pid)
        .map(|unit| g.rules.units[unit.kind].strength)
        .fold(20.0, f64::max);
    let built = g.players[pid]
        .counters
        .get("strongest_unit_built")
        .map(|value| *value as f64)
        .unwrap_or(current_best);
    (built - 10.0).max(10.0)
}

/// The melee strength `uid` lends a city it garrisons; nothing for a
/// Shooter's or a civilian's.
fn garrison_strength(g: &Game, uid: u32) -> f64 {
    g.units
        .get(&uid)
        .map(|unit| g.rules.units[unit.kind].strength)
        .unwrap_or(0.0)
}

/// The member of `members` that holds the unwalled city at `city`: the
/// strongest melee soldier of `pid` standing in it or beside it, when it
/// lifts the city's defence past both the floor and `holder`'s by
/// [`GARRISON_GAIN`]. `None` keeps the Anvil's own post.
pub(super) fn melee_garrison(
    g: &Game,
    pid: usize,
    city: Pos,
    members: &[u32],
    holder: Option<u32>,
) -> Option<u32> {
    if holder.is_some_and(|uid| holds_a_city(g, uid)) {
        return None;
    }
    let bar = ungarrisoned_floor(g, pid).max(holder.map_or(0.0, |uid| garrison_strength(g, uid)));
    members
        .iter()
        .copied()
        .filter(|uid| {
            g.units
                .get(uid)
                .is_some_and(|unit| unit.owner == pid && g.wdist(unit.pos, city) <= 1)
                && holds_a_city(g, *uid)
                && garrison_strength(g, *uid) >= bar + GARRISON_GAIN
        })
        .max_by_key(|uid| {
            let unit = &g.units[uid];
            (
                (garrison_strength(g, *uid) * 10.0).round() as i64,
                unit.pos == city,
                unit.hp,
                Reverse(*uid),
            )
        })
}

/// Where a Shooter displaced from the city post stands: the open land tile
/// within two of the city, not posted, farthest from the nearest hostile,
/// then best ground. `None` when every tile is taken.
fn behind_the_garrison(
    g: &Game,
    pid: usize,
    city: Pos,
    hostiles: &[Pos],
    taken: &BTreeSet<Pos>,
) -> Option<Pos> {
    let hostile_distance = |pos: Pos| {
        hostiles
            .iter()
            .map(|h| g.wdist(*h, pos))
            .min()
            .unwrap_or(i32::MAX)
    };
    g.wdisk(city, 2)
        .into_iter()
        .filter(|pos| {
            *pos != city
                && !taken.contains(pos)
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                && g.unit_ids_at(*pos)
                    .iter()
                    .all(|id| g.units[id].owner == pid)
        })
        .max_by_key(|pos| {
            (
                hostile_distance(*pos),
                (g.tile_defense_bonus(*pos) * 10.0).round() as i64,
                Reverse(*pos),
            )
        })
}

impl AdvancedAi {
    /// Rewrite this turn's Anvil posts so the unwalled city `cid` holds its
    /// strongest melee soldier: see the module docs. Called right after the
    /// posts are drawn; nothing with the gene off, the walls up, or no melee
    /// soldier that would lift the city's defence.
    pub(super) fn anvil_garrisons_melee(
        &mut self,
        g: &mut Game,
        pid: usize,
        cid: u32,
        members: &[u32],
        hostiles: &[Pos],
    ) {
        let Some((city, wall_hp)) = g.cities.get(&cid).map(|c| (c.pos, c.wall_hp)) else {
            return;
        };
        if !self.anvil_garrisons_its_melee || wall_hp > 0 {
            return;
        }
        let holder = self
            .anvil_orders
            .iter()
            .find(|(_, post)| **post == city)
            .map(|(uid, _)| *uid);
        // Our melee soldier already inside is the garrison when it is the
        // Anvil's (posted to the ring, it would walk out); one that is not
        // the Anvil's holds the city without our help.
        let inside = g
            .unit_ids_at(city)
            .iter()
            .copied()
            .find(|id| g.units[id].owner == pid && holds_a_city(g, *id));
        let candidates: Vec<u32> = match inside {
            Some(uid) if members.contains(&uid) => vec![uid],
            Some(_) => return,
            None => members.to_vec(),
        };
        let Some(pick) = melee_garrison(g, pid, city, &candidates, holder) else {
            return;
        };
        let started_inside = g.units[&pick].pos == city;
        let vacated = self.anvil_orders.insert(pick, city);
        let mut moved_out = None;
        if let Some(shooter) = holder {
            self.anvil_orders.remove(&shooter);
            let taken: BTreeSet<Pos> = self.anvil_orders.values().copied().collect();
            if let Some(tile) = behind_the_garrison(g, pid, city, hostiles, &taken) {
                self.anvil_orders.insert(shooter, tile);
                moved_out = Some(shooter);
            }
        }
        // A garrison beside the city trades places with whoever stands in
        // it now, before either spends its movement: waiting for the
        // occupant's own step leaves the city bare for a turn.
        if !started_inside {
            let occupant = g
                .unit_ids_at(city)
                .iter()
                .copied()
                .find(|id| *id != pick && members.contains(id) && g.units[id].owner == pid);
            if let Some(other) = occupant {
                if g.apply(pid, &Action::Swap { unit: pick, other }).is_ok() {
                    self.force_groups_dirty = true;
                }
            }
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let pick_kind = g.units[&pick].kind.to_string();
            let displaced = moved_out
                .or(holder)
                .map(|uid| g.units[&uid].kind.to_string())
                .unwrap_or_else(|| "nobody".to_string());
            let strength = garrison_strength(g, pick);
            let was = if started_inside {
                "and stays inside"
            } else if vacated.is_some_and(|pos| g.wdist(pos, city) == 1) {
                "from the ring"
            } else {
                "from beside it"
            };
            let name = g.cities[&cid].name.clone();
            think!(self.journal(), Military, Decision,
                "Anvil at {name}: the {pick_kind} holds the unwalled city {was}";
                "anvil-garrisons-its-melee: melee {strength:.0} inside against the {displaced}'s {:.0}; no walls stand",
                holder.map_or(0.0, |uid| garrison_strength(g, uid));
                city);
        }
    }

    /// `anvil-garrisons-its-melee`: whether `uid`, posted to `post`, is the
    /// melee garrison of the unwalled city at `city`, which holds rather
    /// than attacks out.
    pub(super) fn anvil_garrison_holds(
        &self,
        g: &Game,
        uid: u32,
        post: Pos,
        city: Pos,
        wall_hp: i32,
    ) -> bool {
        self.anvil_garrisons_its_melee && wall_hp <= 0 && post == city && holds_a_city(g, uid)
    }
}

#[cfg(test)]
mod tests;
