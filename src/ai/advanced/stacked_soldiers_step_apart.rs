//! `stacked-soldiers-step-apart`: when our unit pass is over, every tile
//! where two or more of our soldiers of one stacking layer still stand keeps
//! one of them and steps each other one that still has movement onto the
//! best free tile beside it. A stacked unit is one the host will not let us
//! hold, so the seat's turn otherwise ends by force with the stack in place.
//!
//! **The host rule.** Civilization VI puts a unit trained or bought in a
//! city whose centre is already held onto the centre itself, and refuses a
//! unit that shares its tile with another of its owner's units of the same
//! layer every operation that keeps it there: Fortify, Alert, Skip Turn and
//! Sleep. The shipped Action Panel names the blocker
//! (`Base/Assets/UI/ActionPanel.lua:100`,
//! `ENDTURN_BLOCKING_STACKED_UNITS` -> `LOC_ACTION_PANEL_STACKED_UNIT`) and
//! answers it only with `UI.SelectNextReadyUnit()` (`:575-578`): a person
//! moves one of them. The engine models the end-of-move rule
//! ([`Game::can_stop`]) but not a stack it is handed, so a plan that leaves
//! one in place says nothing to either unit.
//!
//! **Why (hotseat `hotseat-20261128T031456Z`, seat p2, Poland, 2026-11-27).**
//! Kraków (12,36) built a Warrior on turn 15 while another stood on its
//! centre. The turn's battle plan sent the third Warrior after a barbarian
//! Scout ("1 unit(s) placed, 0 paced") and the per-unit ladder gave the two
//! on the centre nothing; `decisions.jsonl` names neither on turns 15 and 16.
//! The bridge's hold pass tried Fortify, Alert and Skip Turn on both, the
//! end-turn parking pass Skip Turn, Fortify, Alert and Sleep: 11
//! `operation_refused` on each turn, `orders`
//! `unmentioned_held: 2, unmentioned_held_applied: 0` both turns, and both
//! turns ended by force (`dismissed {"forced": true}`). Two of p2's three
//! soldiers stood idle for two turns beside five barbarians. Every other
//! unmentioned unit of that game from turn 13 on (six civilians and one
//! soldier, each alone on its tile) took the bridge's hold.
//!
//! `stacked-shooter-steps-out` already breaks a stack that holds a shooter
//! with an enemy soldier near, before the battle is planned. This is the
//! rest: any layer-mates, any time, after every other pass has had them.
//!
//! ## What the gene does
//!
//! After the per-unit ladder, for each tile holding two or more of our combat
//! units (land and sea are separate layers, as in [`Game::can_stop`]):
//!
//! 1. **Who keeps the tile.** A unit with no movement left cannot step and
//!    keeps it; then a soldier guarding a civilian (in a formation, a
//!    Settler's bound escort, a Builder's reserved guard); then a shooter;
//!    then the strongest in melee, then the most wounded (it heals on the
//!    tile it holds, and the healthier goes out), then the lowest id.
//! 2. **Who steps.** Every other one with movement left that is no guard and
//!    not embarked steps once onto a tile beside it, chosen and scored as
//!    `stacked-shooter-steps-out` chooses (no City Center, no unit of another
//!    player, no soldier of ours; enemy melee beside or near the tile costs,
//!    cover and our own ground pay). The enemies read are every soldier at
//!    war with us in sight within [`STEP_OUT_THREAT_TILES`] + 2 tiles. A step
//!    the engine refuses tries the next tile.
//! 3. **Nothing else.** The step is the gene's only order. With no tile it
//!    can take, the stack stands and a Detail line says so.
//!
//! Stateless: every frame is read from that frame's board. Off, nothing here
//! runs. Grep the journal for `stacked-soldiers-step-apart:`.

use super::last_orders::OrderStep;
use super::stacked_shooter_steps_out::STEP_OUT_THREAT_TILES;
use super::*;

impl AdvancedAi {
    /// The post-pass: see the module. The number of soldiers stepped off a
    /// shared tile. Nothing with the gene off.
    pub(super) fn step_stacked_soldiers_apart(&mut self, g: &mut Game, pid: usize) -> usize {
        if !self.stacked_soldiers_step_apart || self.base.minor || self.base.barb {
            return 0;
        }
        let mut tiles: BTreeMap<Pos, Vec<u32>> = BTreeMap::new();
        for uid in g.player_unit_ids(pid) {
            if Self::step_out_soldier(g, pid, uid) {
                tiles.entry(g.units[&uid].pos).or_default().push(uid);
            }
        }
        let mut stepped = 0;
        for (tile, mut soldiers) in tiles {
            if soldiers.len() < 2 {
                continue;
            }
            soldiers.sort_unstable();
            // One stacking layer at a time: a ship in port and a soldier on
            // the centre do not contend.
            let mut layers: Vec<Vec<u32>> = Vec::new();
            for uid in soldiers {
                match layers
                    .iter_mut()
                    .find(|layer| g.units_share_stacking_layer(uid, layer[0]))
                {
                    Some(layer) => layer.push(uid),
                    None => layers.push(vec![uid]),
                }
            }
            for layer in layers.into_iter().filter(|layer| layer.len() >= 2) {
                stepped += self.step_one_layer_apart(g, pid, tile, &layer);
            }
        }
        if stepped > 0 {
            self.force_groups_dirty = true;
            self.credit_orders(g, OrderStep::StandDown);
        }
        stepped
    }

    /// Whether `uid` can take a step this frame at all.
    fn step_apart_can_step(g: &Game, uid: u32) -> bool {
        g.units
            .get(&uid)
            .is_some_and(|unit| unit.moves_left > 0.0 && !g.is_embarked(unit))
    }

    /// The unit of `layer` that keeps `tile`: see the module, rule 1.
    fn step_apart_keeper(&self, g: &Game, layer: &[u32]) -> u32 {
        layer
            .iter()
            .copied()
            .max_by_key(|uid| {
                let unit = &g.units[uid];
                (
                    !Self::step_apart_can_step(g, *uid),
                    self.step_out_guard(g, *uid),
                    Self::step_out_shooter(g, *uid),
                    (g.rules.units[unit.kind].strength * 100.0).round() as i64,
                    std::cmp::Reverse(unit.hp),
                    std::cmp::Reverse(*uid),
                )
            })
            .expect("a layer holds two units")
    }

    /// Step every unit of `layer` but its keeper off `tile`. The number that
    /// stepped.
    fn step_one_layer_apart(
        &mut self,
        g: &mut Game,
        pid: usize,
        tile: Pos,
        layer: &[u32],
    ) -> usize {
        let keeper = self.step_apart_keeper(g, layer);
        let movers: Vec<u32> = layer
            .iter()
            .copied()
            .filter(|uid| {
                *uid != keeper
                    && Self::step_apart_can_step(g, *uid)
                    && !self.step_out_guard(g, *uid)
            })
            .collect();
        let kept = plain(g.units[&keeper].kind.as_str());
        let enemies = Self::step_out_enemies(g, pid, tile, STEP_OUT_THREAT_TILES + 2);
        let mut stepped = 0;
        for uid in movers {
            let kind = g.units[&uid].kind;
            let options = Self::step_out_tiles(g, pid, uid, tile, &enemies);
            let open = options.len();
            let mut moved = false;
            for (score, to, _) in options {
                if g.apply(pid, &Action::Move { unit: uid, to }).is_err() {
                    continue;
                }
                stepped += 1;
                moved = true;
                think!(self.journal(), Military, Decision,
                    "The {} steps off the tile it shared with our {kept}", plain(kind.as_str());
                    "stacked-soldiers-step-apart: {} of our soldiers of one layer ended the unit \
                     pass on {tile:?}, and the host refuses every hold to a stacked unit; \
                     {tile:?} -> {to:?}, score {score}",
                    layer.len());
                break;
            }
            if !moved {
                think!(self.journal(), Military, Detail,
                    "The {} stays stacked with our {kept}", plain(kind.as_str());
                    "stacked-soldiers-step-apart: {} of our soldiers of one layer share {tile:?} and \
                     {open} tile(s) beside it are open, none of them a step it can take",
                    layer.len());
            }
        }
        // Layer-mates that cannot step (no movement, embarked, or a guard
        // other than the keeper) are named, so a stack left in place is never
        // silent.
        let stuck = layer
            .iter()
            .filter(|uid| {
                **uid != keeper
                    && (!Self::step_apart_can_step(g, **uid) || self.step_out_guard(g, **uid))
                    && g.units.get(uid).is_some_and(|unit| unit.pos == tile)
            })
            .count();
        if stuck > 0 {
            think!(self.journal(), Military, Detail,
                "{stuck} soldier(s) stay stacked with our {kept}";
                "stacked-soldiers-step-apart: on {tile:?} {stuck} layer-mate(s) have no step to \
                 take (no movement left, embarked, or guarding a civilian)");
        }
        stepped
    }
}

#[cfg(test)]
mod tests;
