//! `cold-war-before-conservation`: a Culture target on a six-slot government
//! researches only the road to Cold War until it is known.
//!
//! **Why (live Deity, 2026-11-08..10).** Rock Bands open at Cold War, and the
//! turn they open is the second-strongest driver of a culture seat's visitors
//! (Spearman 0.57 over 116 Pericles games; median visitors 45 with Cold War
//! before t140, 16.5 at t150-159, 6.5 at t160-174). Without this gene the
//! civic slot reaches Cold War last among four detours:
//! - `culture_civic_goal` puts Conservation (1,540 standard, off the road)
//!   right after Humanism. It came before Cold War in nearly every Pericles
//!   game, a median 37 turns before it, and at t132 in Kristina 140731Z.
//! - `government-ladder-2` chases the tier-3 gate. 140731Z's civic slot read
//!   "the most valuable step toward class struggle" from t84 to t148.
//! - After Ideology the culture goal list asks for Professional Sports and
//!   Cultural Heritage before Space Race, the first goal that needs Cold War.
//!
//! 140731Z lost to Mapuche Culture at t150 without Cold War, about 42% of the
//! road (6,725 standard culture) still to go.
//!
//! **What the gene does.** While a live Culture target plays a government with
//! six or more policy slots and does not know Cold War, the civic slot's goal
//! is Cold War, ahead of the government ladder, the culture goal list and the
//! existing `culture-cold-war-window`. The first government, Drama and Poetry,
//! Great Person civics and the museum unlock keep their place above it, and a
//! four-slot government still climbs to its second rung first. Once Cold War
//! is known the ordinary order resumes. Version one of the window keeps
//! Conservation first; this gene does not.

use super::*;

/// Policy slots of a tier-2 government; below this the ladder climbs first.
const COLD_WAR_FIRST_SLOTS: i64 = 6;

impl AdvancedAi {
    /// `Some("cold_war")` while the gene drives the civic slot. See the module
    /// docs. `None` with the gene off.
    pub(super) fn cold_war_first_goal(&self, g: &Game, pid: usize) -> Option<&'static str> {
        if !self.cold_war_before_conservation
            || self.lane_lost
            || !g.victory_conditions.culture
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || g.players[pid].civics.contains(&crate::name!("cold_war"))
        {
            return None;
        }
        let slots = g.gov_slots(pid);
        let capacity = slots.military + slots.economic + slots.diplomatic + slots.wildcard;
        (capacity >= COLD_WAR_FIRST_SLOTS).then_some("cold_war")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::advanced::StrategicPlan;

    /// A Culture empire on Monarchy that knows every civic except Cold War,
    /// the civics past it, and the off-road ones the culture list asks for.
    fn board() -> Game {
        let mut g = Game::new_full(2, 40, 24, 79_174, 250, 0, false);
        g.current = 0;
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.apply(0, &crate::game::Action::FoundCity { unit: settler })
            .unwrap();
        g.at_war.clear();
        g.turn = 120;
        let withheld = [
            "cold_war",
            "conservation",
            "professional_sports",
            "cultural_heritage",
            "space_race",
            "environmentalism",
            "social_media",
            "suffrage",
            "class_struggle",
            "totalitarianism",
            "corporate_libertarianism",
            "digital_democracy",
            "synthetic_technocracy",
        ];
        g.players[0].civics = g
            .rules
            .civics
            .keys()
            .copied()
            .filter(|civic| !withheld.contains(&civic.as_str()))
            .collect();
        g.players[0].government = Some("monarchy".into());
        g.players[0].civic = None;
        g
    }

    fn plan(g: &Game, strategy: GrandStrategy) -> StrategicPlan {
        StrategicPlan {
            strategy,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 1,
            assessed_turn: g.turn,
            rush: false,
        }
    }

    fn seat() -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.enable_cold_war_before_conservation();
        ai
    }

    fn picks(ai: &AdvancedAi, g: &Game, strategy: GrandStrategy) -> Option<String> {
        let mut g = g.clone();
        let plan = plan(&g, strategy);
        ai.advanced_research(&mut g, 0, &plan);
        g.players[0].civic.as_deref().map(str::to_string)
    }

    #[test]
    fn cold_war_comes_before_conservation() {
        let g = board();
        let off = AdvancedAi::targeting(VictoryTarget::Culture);
        assert_eq!(
            picks(&off, &g, GrandStrategy::Culture).as_deref(),
            Some("conservation")
        );
        for strategy in [GrandStrategy::Culture, GrandStrategy::Recovery] {
            assert_eq!(
                picks(&seat(), &g, strategy).as_deref(),
                Some("cold_war"),
                "{strategy:?}"
            );
        }
    }

    #[test]
    fn short_of_ideology_every_step_is_on_the_road() {
        let mut g = board();
        g.players[0].civics.remove(&crate::name!("ideology"));
        let pick = picks(&seat(), &g, GrandStrategy::Culture).unwrap();
        assert!(seat().civic_leads_to(&g, &pick, "cold_war"), "{pick}");
        assert_ne!(pick, "conservation");
    }

    #[test]
    fn the_gene_steps_aside_where_it_should() {
        let g = board();
        assert_eq!(
            AdvancedAi::targeting(VictoryTarget::Culture).cold_war_first_goal(&g, 0),
            None
        );
        assert_eq!(seat().cold_war_first_goal(&g, 0), Some("cold_war"));
        // A four-slot government climbs the ladder first.
        let mut early = g.clone();
        early.players[0].government = Some("classical_republic".into());
        assert_eq!(seat().cold_war_first_goal(&early, 0), None);
        // Cold War known, another target, or a released lane.
        let mut known = g.clone();
        known.players[0].civics.insert(crate::name!("cold_war"));
        assert_eq!(seat().cold_war_first_goal(&known, 0), None);
        let mut science = AdvancedAi::targeting(VictoryTarget::Science);
        science.enable_cold_war_before_conservation();
        assert_eq!(science.cold_war_first_goal(&g, 0), None);
        let mut released = seat();
        released.lane_lost = true;
        assert_eq!(released.cold_war_first_goal(&g, 0), None);
        let mut no_culture = g.clone();
        no_culture.victory_conditions.culture = false;
        assert_eq!(seat().cold_war_first_goal(&no_culture, 0), None);
    }
}
