//! One small, screenable settlement-information heuristic.
//!
//! `opening-warrior-recon-2` runs only before a player's first city: a
//! Warrior directly escorting the initial Settler acts before it, then all
//! first-city targets are refreshed; normal candidate filtering stays in
//! force. Version one (any nearby Warrior first, and a candidate eligible only
//! once its whole radius-two footprint had been observed) and the companion
//! `settler-second-look` (drop a Settler's cached target after its first
//! movement leg) left the code on 2026-09-08 — the first by operator
//! directive (rank 266, P(>0) 0.0%, Diff -1.61 pp), the second under the
//! batch rule (-11/-27, Diff -0.53 pp).

use super::AdvancedAi;
use crate::game::Game;
use crate::Pos;

const OPENING_RECON_V2_ESCORT_DISTANCE: i32 = 1;

impl AdvancedAi {
    /// The narrow opening phase where a Warrior can earn information for a
    /// live Settler. If the Warrior has died, ordinary settlement logic stays
    /// available instead of waiting forever for information that cannot come.
    pub(super) fn opening_settlement_recon_active(&self, g: &Game, pid: usize) -> bool {
        self.opening_warrior_recon_2
            && g.player_city_ids(pid).is_empty()
            && g.player_unit_ids(pid)
                .into_iter()
                .any(|uid| g.units[&uid].kind == "settler")
            && g.player_unit_ids(pid)
                .into_iter()
                .any(|uid| g.units[&uid].kind == "warrior")
    }

    /// The Warrior nearest to any current first-city Settler, with unit ID as
    /// a deterministic tie-break. Only one gets priority: the gene buys a
    /// specific scout-before-settle decision, not a free army turn, and only
    /// a direct Settler escort earns it.
    pub(super) fn opening_recon_warrior(&self, g: &Game, pid: usize) -> Option<u32> {
        if !self.opening_settlement_recon_active(g, pid) {
            return None;
        }
        let settlers = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.units[uid].kind == "settler")
            .map(|uid| g.units[&uid].pos)
            .collect::<Vec<_>>();
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                unit.kind == "warrior"
                    && unit.moves_left > 0.0
                    && settlers.iter().any(|settler| {
                        g.wdist(unit.pos, *settler) <= OPENING_RECON_V2_ESCORT_DISTANCE
                    })
            })
            .min_by_key(|uid| {
                let pos = g.units[uid].pos;
                let distance = settlers
                    .iter()
                    .map(|settler| g.wdist(pos, *settler))
                    .min()
                    .unwrap_or(i32::MAX);
                (distance, *uid)
            })
    }

    /// Drop the cache that prevents the next settlement step from consulting
    /// new information. Do not reset counters that deliberately survive a
    /// retarget: otherwise a stranded Settler could evade its recovery path by
    /// repeatedly rediscovering the same dead end.
    fn refresh_settler_target_after_information(&mut self, uid: u32) {
        self.settler_targets.remove(&uid);
        self.settler_stalls.remove(&uid);
    }

    /// Apply a Warrior's newly earned sight to every still-live first-city
    /// Settler. A moved Warrior may reveal no new tile behind a ridge, while a
    /// successful action can reveal terrain without changing position, so
    /// either fact is enough to make the cached choice stale.
    pub(super) fn refresh_opening_settler_targets_after_recon(
        &mut self,
        g: &Game,
        pid: usize,
        warrior: u32,
        before: Pos,
        explored_before: usize,
    ) {
        if !self.opening_settlement_recon_active(g, pid) {
            return;
        }
        let learned = g.units.get(&warrior).is_some_and(|unit| unit.pos != before)
            || g.players[pid].explored.len() > explored_before;
        if !learned {
            return;
        }
        for uid in g.player_unit_ids(pid) {
            if g.units[&uid].kind == "settler" {
                self.refresh_settler_target_after_information(uid);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::advanced::{gene, GrandStrategy, Kind, StrategicPlan};
    use crate::game::{Action, Game};

    fn opening_board() -> (Game, usize, u32, u32) {
        let game = Game::new_full(2, 24, 16, 4_821, 120, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .expect("a standard opening has a Settler");
        let warrior = game
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| game.units[uid].kind == "warrior")
            .expect("a standard opening has a Warrior");
        (game, 0, settler, warrior)
    }

    fn opening_plan(game: &Game) -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        }
    }

    #[test]
    fn opening_recon_is_a_reversible_opt_in() {
        let mut ai = AdvancedAi::new();
        assert!(!ai.opening_warrior_recon_2);
        assert!(!AdvancedAi::legacy().opening_warrior_recon_2);
        assert_eq!(gene("opening-warrior-recon-2").unwrap().kind, Kind::OptIn);

        ai.enable_opening_warrior_recon_2();
        assert!(ai.opening_warrior_recon_2);
        ai.disable_opening_warrior_recon_2();
        assert!(!ai.opening_warrior_recon_2);
    }

    #[test]
    fn opening_recon_selects_the_escort_and_stops_after_the_capital() {
        let (mut game, pid, settler, warrior) = opening_board();
        let mut ai = AdvancedAi::new();
        ai.enable_opening_warrior_recon_2();
        assert_eq!(ai.opening_recon_warrior(&game, pid), Some(warrior));

        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .expect("the initial Settler can found the capital");
        assert_eq!(game.player_city_ids(pid).len(), 1);
        assert!(!ai.opening_settlement_recon_active(&game, pid));
        assert_eq!(ai.opening_recon_warrior(&game, pid), None);
    }

    #[test]
    fn opening_recon_v2_moves_the_escort_before_the_settler_turn() {
        let (mut game, pid, settler, warrior) = opening_board();
        game.current = pid;
        let plan = opening_plan(&game);
        let log_start = game.log.len();
        let mut ai = AdvancedAi::new();
        ai.enable_opening_warrior_recon_2();

        ai.advanced_units(&mut game, pid, &plan);

        let unit_actions = game
            .log
            .since(log_start)
            .enumerate()
            .filter_map(|(index, (actor, action))| match action {
                Action::Move { unit, .. }
                | Action::MoveTo { unit, .. }
                | Action::FoundCity { unit }
                    if *actor == pid && (*unit == settler || *unit == warrior) =>
                {
                    Some((index, *unit))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let warrior_move = unit_actions
            .iter()
            .position(|(_, unit)| *unit == warrior)
            .expect("the opening Warrior should spend its reconnaissance turn");
        let settler_turn = unit_actions
            .iter()
            .position(|(_, unit)| *unit == settler)
            .expect("the Settler should act after reconnaissance");
        assert!(
            warrior_move < settler_turn,
            "the Warrior must reveal before the Settler decides: {unit_actions:?}"
        );
    }

    #[test]
    fn warrior_information_invalidates_only_the_opening_settler_cache() {
        let (mut game, pid, settler, warrior) = opening_board();
        let mut ai = AdvancedAi::new();
        ai.enable_opening_warrior_recon_2();
        let target = game.units[&settler].pos;
        let before = game.units[&warrior].pos;
        let explored_before = game.players[pid].explored.len();
        ai.settler_targets.insert(settler, target);
        ai.settler_blocked_turns.insert(settler, 2);

        let newly_known = game
            .wdisk(before, 4)
            .into_iter()
            .find(|pos| !game.players[pid].explored.contains(pos))
            .expect("the opening has fog beyond its initial sight");
        game.players[pid].explored.insert(newly_known);
        ai.refresh_opening_settler_targets_after_recon(
            &game,
            pid,
            warrior,
            before,
            explored_before,
        );
        assert!(!ai.settler_targets.contains_key(&settler));
        assert_eq!(ai.settler_blocked_turns.get(&settler), Some(&2));
    }

    #[test]
    fn opening_recon_v2_only_prioritizes_a_real_settler_escort() {
        let (mut game, pid, settler, warrior) = opening_board();
        let settler_pos = game.units[&settler].pos;
        let mut ai = AdvancedAi::new();
        ai.enable_opening_warrior_recon_2();
        assert_eq!(ai.opening_recon_warrior(&game, pid), Some(warrior));

        let far = game
            .wdisk(settler_pos, 3)
            .into_iter()
            .find(|pos| game.wdist(settler_pos, *pos) == 3)
            .expect("a standard opening has a third ring");
        game.units.get_mut(&warrior).unwrap().pos = far;
        assert_eq!(ai.opening_recon_warrior(&game, pid), None);
    }
}
