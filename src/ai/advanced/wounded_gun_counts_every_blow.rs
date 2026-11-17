//! `wounded-gun-counts-every-blow`: a damaged siege gun or land ranged unit
//! does not end a step where every blow that reaches the tile adds up to its
//! health, and one already standing on such a tile steps off it.
//!
//! **Why (live Emperor Domination, 2026-11-15/16).** Four games of the
//! Domination seat (civvis-20261115T230117Z, 20261116T035719Z,
//! 20261116T054247Z and 20261116T095002Z, Gran Colombia's 75-turn war on
//! Vietnam that took no city). Read from `events.jsonl` (`unit_lost`, the
//! `combat` rows, `state` units, `orders.sqlite`), one row per host unit id:
//! 75 Bombards, Artillery, Rocket Artillery, Field Cannons and Trebuchets left
//! the board. 20 were upgraded, 16 went into a corps or army and 12 vanished
//! with no blow on them; **27 died in combat**. 18 of those 27 began their
//! last turn damaged (2 to 98 health) and ended it within reach of the city
//! they besieged or of a hostile: twelve had walked there that turn, four
//! fired and two fortified where they stood. The blows that took them came in
//! twos: a City Center and an Encampment strike (t134 Field Cannon at 72: 54
//! and 18), a city strike and a Warrior Monk (t112 Trebuchet at 93: 69 and
//! 24), a Field Cannon and an Encampment (t122 Bombard: 49 and 51).
//!
//! The rules that read danger left that case open. `guns-stay-out-of-reach`
//! (armed live) refuses a gun's step only where ONE hostile unit's blow kills
//! it, and leaves City and Encampment strikes out on purpose, so that
//! `guns-enter-together` can walk two or more fresh guns into one city's
//! strike to share it. The battle planner's rotation reads the whole field
//! but leaves the members of an active siege to the train, and the train's
//! post holds (`siege_post_holds`) skip the recovery at 50 health or more.
//!
//! **What the gene does.** For each of our land units whose force role is
//! Siege or Ranged, with movement left and under full health, every tile it
//! can reach this turn (its own included) where the roll-top total of the
//! danger field (`DangerField::contributions`: every visible hostile's blow,
//! a walled City Center's and a standing Encampment's strike within two
//! tiles, each at the top of its roll as `wounded-out-of-reach` reads it,
//! and the tile's movement hazard) meets its health is marked in
//! `BasicAi::gun_lethal_tiles`, beside `guns-stay-out-of-reach`'s marks (the
//! larger reading kept). `path_step_allowed` then refuses a step onto a
//! marked tile unless the step lowers the danger the unit stands under, and
//! `guns_step_out_of_reach` walks a unit marked on its own tile to the
//! nearest unmarked tile it can reach. A full-health gun is left alone, so a
//! fresh gun still enters a city's strike ring with the others; a garrison
//! tile reads no strike, so a damaged gun can always come home.
//!
//! Off, nothing is read: `wounded_guns` returns before the board.
use super::battle_planner::DangerField;
use super::{AdvancedAi, ForceRole};
use crate::game::Game;
use crate::Pos;
use std::collections::HashMap;

/// A unit at this health is whole: every unit's maximum.
pub(super) const FULL_HEALTH: i32 = 100;

impl AdvancedAi {
    /// The units the gene reads this frame: our damaged land siege guns and
    /// ranged units with movement left, not embarked and not read by
    /// `risk-is-priced`. Empty with the gene off.
    pub(super) fn wounded_guns(&self, g: &Game, pid: usize) -> Vec<u32> {
        if !self.wounded_gun_counts_every_blow {
            return Vec::new();
        }
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && matches!(
                        Self::force_role(g, *uid),
                        ForceRole::Siege | ForceRole::Ranged
                    )
                    && unit.hp < FULL_HEALTH
                    && unit.moves_left > 0.0
                    && !g.is_embarked(unit)
                    && !self.base.priced_risk.contains_key(uid)
            })
            .collect()
    }

    /// For each of `guns`, the tiles it can reach this turn (its own
    /// included) where the roll-top total of every blow meets its health
    /// ([`roll_top_total`](Self::roll_top_total)), merged into
    /// `BasicAi::gun_lethal_tiles` keeping the larger reading per tile.
    /// Journals each unit it marks.
    pub(super) fn mark_wounded_gun_tiles(
        &mut self,
        g: &Game,
        field: &mut DangerField,
        guns: &[u32],
    ) {
        for &uid in guns {
            let unit = &g.units[&uid];
            let hp = f64::from(unit.hp);
            let mut tiles = vec![unit.pos];
            tiles.extend(g.reachable(uid));
            let summed: HashMap<Pos, f64> = tiles
                .into_iter()
                .filter_map(|tile| {
                    let danger = Self::roll_top_total(field, tile, uid);
                    (danger >= hp).then_some((tile, danger))
                })
                .collect();
            if summed.is_empty() {
                continue;
            }
            let here = summed.get(&unit.pos).copied();
            crate::think!(self.journal(), Military, Detail,
                "wounded-gun-counts-every-blow: the {} {} at {} health keeps off {} tile(s)",
                unit.kind, uid, unit.hp, summed.len();
                "every blow that reaches them, the city's and Encampment's strikes included, adds up to its health{}",
                here.map_or(String::new(), |danger| format!("; its own tile reads {danger:.0}")));
            let marks = self.base.gun_lethal_tiles.entry(uid).or_default();
            for (tile, danger) in summed {
                let mark = marks.entry(tile).or_insert(danger);
                *mark = mark.max(danger);
            }
        }
    }

    /// Every blow that lands on `uid` standing on `tile` next turn, each
    /// strike at the top of its roll (`COMBAT_ROLL_MAX`), plus the tile's
    /// movement hazard as the field reads it.
    pub(super) fn roll_top_total(field: &mut DangerField, tile: Pos, uid: u32) -> f64 {
        field
            .contributions(tile, uid)
            .iter()
            .map(|(source, blow)| match source {
                Some(_) => blow * crate::ai::COMBAT_ROLL_MAX,
                None => *blow,
            })
            .sum()
    }

    /// Whether the gene marked `uid` this frame: a damaged unit it reads.
    pub(super) fn wounded_gun_marked(&self, g: &Game, uid: u32) -> bool {
        self.wounded_gun_counts_every_blow
            && self.base.gun_lethal_tiles.contains_key(&uid)
            && g.units.get(&uid).is_some_and(|unit| {
                unit.hp < FULL_HEALTH
                    && matches!(
                        Self::force_role(g, uid),
                        ForceRole::Siege | ForceRole::Ranged
                    )
            })
    }
}

#[cfg(test)]
mod tests;
