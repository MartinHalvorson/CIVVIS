//! Two small, separately screenable settlement-information heuristics.
//!
//! `opening-warrior-recon` runs only before a player's first city: the nearby
//! Warrior acts before the Settler, then all first-city targets are refreshed.
//! A candidate is eligible only once its whole radius-two city footprint has
//! been observed, so native full-map state cannot silently decide the opening.
//!
//! `settler-second-look` is broader. A Settler's first real movement leg drops
//! its disposable destination cache when movement remains; its next serial
//! step evaluates the current board before spending the second leg. Durable
//! delay, threat, and retreat histories intentionally survive that refresh.

use super::AdvancedAi;
use crate::game::Game;
use crate::Pos;

impl AdvancedAi {
    /// The narrow opening phase where a Warrior can earn information for a
    /// live Settler. If the Warrior has died, ordinary settlement logic stays
    /// available instead of waiting forever for information that cannot come.
    pub(super) fn opening_settlement_recon_active(&self, g: &Game, pid: usize) -> bool {
        self.opening_warrior_recon
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
    /// specific scout-before-settle decision, not a free army turn.
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
                unit.kind == "warrior" && unit.moves_left > 0.0
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

    /// A first-city score reads the center and its workable radius-two disk.
    /// While the recon gene is active, require that exact disk to be in the
    /// player's explored memory. Once the city is founded, this is an exact
    /// no-op and ordinary settlement scoring is restored.
    pub(super) fn opening_settlement_footprint_known(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
    ) -> bool {
        !self.opening_settlement_recon_active(g, pid)
            || g.wdisk(pos, 2)
                .into_iter()
                .all(|tile| g.players[pid].explored.contains(&tile))
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

    /// Refresh exactly once after the first actual Settler movement leg. A
    /// hill, river, or zone of control can consume the full turn, in which case
    /// there is no remaining leg to reconsider.
    pub(super) fn reassess_settler_after_first_leg(
        &mut self,
        g: &Game,
        uid: u32,
        before: Pos,
    ) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        if !self.settler_second_look
            || unit.kind != "settler"
            || unit.pos == before
            || unit.moves_left <= 0.0
        {
            return false;
        }
        self.refresh_settler_target_after_information(uid);
        true
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
    fn both_information_genes_are_opt_ins_with_independent_toggles() {
        let mut ai = AdvancedAi::new();
        assert!(!ai.opening_warrior_recon);
        assert!(!ai.settler_second_look);
        assert!(!AdvancedAi::legacy().opening_warrior_recon);
        assert!(!AdvancedAi::legacy().settler_second_look);
        assert_eq!(gene("opening-warrior-recon").unwrap().kind, Kind::OptIn);
        assert_eq!(gene("settler-second-look").unwrap().kind, Kind::OptIn);

        ai.enable_opening_warrior_recon();
        assert!(ai.opening_warrior_recon);
        assert!(!ai.settler_second_look);
        ai.enable_settler_second_look();
        assert!(ai.settler_second_look);
        ai.disable_opening_warrior_recon();
        assert!(!ai.opening_warrior_recon);
        assert!(ai.settler_second_look);
        ai.disable_settler_second_look();
        assert!(!ai.settler_second_look);
    }

    #[test]
    fn opening_recon_selects_the_warrior_and_stops_after_the_capital() {
        let (mut game, pid, settler, warrior) = opening_board();
        let mut ai = AdvancedAi::new();
        ai.enable_opening_warrior_recon();
        assert_eq!(ai.opening_recon_warrior(&game, pid), Some(warrior));

        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit: settler })
            .expect("the initial Settler can found the capital");
        assert_eq!(game.player_city_ids(pid).len(), 1);
        assert!(!ai.opening_settlement_recon_active(&game, pid));
        assert_eq!(ai.opening_recon_warrior(&game, pid), None);
    }

    #[test]
    fn opening_recon_moves_the_warrior_before_the_settler_turn() {
        let (mut game, pid, settler, warrior) = opening_board();
        game.current = pid;
        let plan = opening_plan(&game);
        let log_start = game.log.len();
        let mut ai = AdvancedAi::new();
        ai.enable_opening_warrior_recon();

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
        ai.enable_opening_warrior_recon();
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
    fn opening_site_scoring_requires_an_observed_radius_two_footprint() {
        let (mut game, pid, settler, _) = opening_board();
        let start = game.units[&settler].pos;
        let unknown = game
            .wdisk(start, 4)
            .into_iter()
            .find(|pos| game.wdist(start, *pos) == 4)
            .expect("a 24-by-16 map has a fourth ring");
        game.players[pid].explored.clear();
        for pos in game.wdisk(start, 2) {
            game.players[pid].explored.insert(pos);
        }

        let mut ai = AdvancedAi::new();
        ai.enable_opening_warrior_recon();
        assert!(ai.opening_settlement_footprint_known(&game, pid, start));
        assert!(
            !ai.opening_settlement_footprint_known(&game, pid, unknown),
            "the fourth-ring site still has unseen workable ground"
        );
    }

    #[test]
    fn second_look_refreshes_once_after_a_real_first_leg() {
        let (mut game, pid, settler, _) = opening_board();
        let from = game.units[&settler].pos;
        let to = game
            .nbrs(from)
            .into_iter()
            .find(|pos| game.can_move(settler, *pos) && game.step_cost(from, *pos) < 2.0)
            .expect("the opening Settler has a one-point legal step");
        game.current = pid;
        game.units.get_mut(&settler).unwrap().moves_left = 2.0;
        game.apply(pid, &Action::Move { unit: settler, to })
            .expect("the chosen first leg is legal");
        assert!(game.units[&settler].moves_left > 0.0);

        let mut ai = AdvancedAi::new();
        ai.settler_targets.insert(settler, from);
        assert!(!ai.reassess_settler_after_first_leg(&game, settler, from));
        assert_eq!(ai.settler_targets.get(&settler), Some(&from));

        ai.enable_settler_second_look();
        assert!(ai.reassess_settler_after_first_leg(&game, settler, from));
        assert!(!ai.settler_targets.contains_key(&settler));
    }
}
