//! `culture-seat-drama-first` and `culture-seat-drama-before-government`: a
//! seat aiming at a Culture victory takes Drama and Poetry on its way out of
//! the opening instead of after whatever the civic score prefers.
//!
//! **Why (live Deity Pericles, 2026-11-08, 39 games).** Drama and Poetry
//! unlocks the Theater Square, Greece's Acropolis and the Amphitheater. It
//! landed at a median of t54 (t47-71) in one fixed order: Code of Laws, Foreign
//! Trade, Early Empire, Craftsmanship, State Workforce, Political Philosophy,
//! Games and Recreation, Defensive Tactics, Drama and Poetry. That is 372
//! Culture on the Online clock, and the civic's own closure is 120, which the
//! seat had banked by a median of t36.
//!
//! The Acropolis count at t60 tracks the Culture rate at t60 (r = 0.81, 67
//! games); only 124 of 311 cities had one. The t60 rate tracks the Cold War
//! turn (r = -0.58), and Cold War is what opens Rock Bands. Cold War landed
//! at a median of t153, and rivals finish Science at a median of t187.
//!
//! **What the genes do.** While Drama and Poetry is unresearched on a live
//! Culture target, it becomes the forced civic goal. The `first government`
//! goal sends every seat to Political Philosophy first, so the two genes
//! differ only in where they sit around it:
//! - `culture-seat-drama-first` goes straight after that goal, which removes
//!   Games and Recreation and Defensive Tactics from in front of the civic;
//! - `culture-seat-drama-before-government` goes ahead of it.
//!
//! Both yield to a Great Person goal and to a released lane. Neither changes
//! anything once the civic is known.

use super::*;

impl AdvancedAi {
    /// The shared gate: a live Culture target that still lacks the civic.
    fn culture_drama_goal_open(&self, g: &Game, pid: usize) -> bool {
        g.victory_conditions.culture
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
            && !g.players[pid]
                .civics
                .contains(&crate::name!("drama_poetry"))
    }

    /// `culture-seat-drama-before-government`, read above the first
    /// government goal.
    pub(super) fn culture_drama_before_government(&self, g: &Game, pid: usize) -> bool {
        self.culture_seat_drama_before_government && self.culture_drama_goal_open(g, pid)
    }

    /// `culture-seat-drama-first`, read directly below the first government
    /// goal.
    pub(super) fn culture_drama_after_government(&self, g: &Game, pid: usize) -> bool {
        self.culture_seat_drama_first && self.culture_drama_goal_open(g, pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::advanced::StrategicPlan;
    use crate::game::Action;

    fn board(known: &[&str]) -> Game {
        let mut g = Game::new_full(2, 30, 20, 79_208, 250, 0, false);
        for pid in 0..2 {
            g.current = pid;
            let unit = g
                .player_unit_ids(pid)
                .into_iter()
                .find(|uid| g.units[uid].kind == "settler")
                .unwrap();
            g.apply(pid, &Action::FoundCity { unit }).unwrap();
        }
        g.current = 0;
        g.turn = 20;
        g.players[0].civics = known.iter().map(|civic| Name::new(civic)).collect();
        g.players[0].civic = None;
        g
    }

    fn plan(g: &Game) -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        }
    }

    fn culture_seat() -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.wide_map_capacity = true;
        ai
    }

    const OPENED: [&str; 3] = ["code_of_laws", "foreign_trade", "early_empire"];
    const GOVERNED: [&str; 6] = [
        "code_of_laws",
        "foreign_trade",
        "early_empire",
        "craftsmanship",
        "state_workforce",
        "political_philosophy",
    ];

    #[test]
    fn before_government_takes_drama_ahead_of_political_philosophy() {
        let mut g = board(&OPENED);
        let mut ai = culture_seat();
        ai.enable_culture_seat_drama_before_government();
        let plan = plan(&g);
        ai.advanced_research(&mut g, 0, &plan);
        assert_eq!(g.players[0].civic.as_deref(), Some("drama_poetry"));
    }

    #[test]
    fn after_government_still_secures_the_first_government() {
        let mut g = board(&OPENED);
        let mut ai = culture_seat();
        ai.enable_culture_seat_drama_first();
        let plan = plan(&g);
        ai.advanced_research(&mut g, 0, &plan);
        assert_ne!(g.players[0].civic.as_deref(), Some("drama_poetry"));
    }

    #[test]
    fn after_government_takes_drama_next() {
        let mut g = board(&GOVERNED);
        let mut ai = culture_seat();
        ai.enable_culture_seat_drama_first();
        let plan = plan(&g);
        ai.advanced_research(&mut g, 0, &plan);
        assert_eq!(g.players[0].civic.as_deref(), Some("drama_poetry"));
    }

    #[test]
    fn the_goal_closes_once_the_civic_is_known() {
        let mut known = GOVERNED.to_vec();
        known.push("drama_poetry");
        let g = board(&known);
        let mut ai = culture_seat();
        ai.enable_culture_seat_drama_first();
        ai.enable_culture_seat_drama_before_government();
        assert!(!ai.culture_drama_after_government(&g, 0));
        assert!(!ai.culture_drama_before_government(&g, 0));
    }

    #[test]
    fn only_a_culture_target_reads_the_goal() {
        let g = board(&GOVERNED);
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.enable_culture_seat_drama_first();
        ai.enable_culture_seat_drama_before_government();
        assert!(!ai.culture_drama_after_government(&g, 0));
        assert!(!ai.culture_drama_before_government(&g, 0));
    }

    #[test]
    fn both_genes_off_leave_the_goal_closed() {
        let g = board(&GOVERNED);
        let ai = culture_seat();
        assert!(!ai.culture_drama_after_government(&g, 0));
        assert!(!ai.culture_drama_before_government(&g, 0));
    }
}
