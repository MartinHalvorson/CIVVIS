//! Target-contract routing for Science, Domination, and victory denial.
//!
//! These are not a new discretionary-war policy.  The normal planner must
//! first choose Conquest as an actionable response; only then may this module
//! name the concrete city whose capture interrupts the rival's victory work.

use super::{AdvancedAi, GrandStrategy, VictoryFocus, VictoryTarget};
use crate::game::Game;
use std::collections::BTreeMap;

impl AdvancedAi {
    /// The next irreducible Science milestone.  An explicit or adaptive
    /// Science plan can still honour a declared rush or a war breakthrough,
    /// but an unrelated live Great Person must not detour it away from the
    /// prerequisite chain that opens the victory projects.
    pub(super) fn science_victory_tech_goal(
        g: &Game,
        pid: usize,
        objective: GrandStrategy,
    ) -> Option<&'static str> {
        (objective == GrandStrategy::Science)
            .then(|| {
                [
                    "rocketry",
                    "satellites",
                    "nanotechnology",
                    "smart_materials",
                    "offworld_mission",
                ]
                .into_iter()
                .find(|tech| !g.players[pid].techs.contains(&crate::name::Name::new(tech)))
            })
            .flatten()
    }

    /// Rank every living major's strongest victory clock, rather than asking
    /// only the leader to pass every operational feasibility test.  A culture
    /// leader whose denominator we cannot affect should not mask a different
    /// rival whose launched expedition an army can still stop.
    pub(super) fn ranked_rival_victory_pressures(
        &self,
        g: &Game,
        pid: usize,
        culture_pressures: &BTreeMap<usize, i32>,
    ) -> Vec<(usize, VictoryFocus)> {
        let mut pressures: Vec<_> = g
            .players
            .iter()
            .filter(|player| {
                player.id != pid && player.alive && !player.is_minor && !player.is_barbarian
            })
            .map(|player| {
                (
                    player.id,
                    self.rival_victory_pressure_with_culture(
                        g,
                        player.id,
                        culture_pressures.get(&player.id).copied(),
                    ),
                )
            })
            .collect();
        pressures.sort_by(|left, right| {
            right
                .1
                .progress
                .cmp(&left.1.progress)
                .then_with(|| left.0.cmp(&right.0))
        });
        pressures
    }

    /// Turn one observed victory clock into the response that can counter it.
    /// This preserves the raw denial policy; the actionable pass below merely
    /// falls through to the next rival when this particular answer cannot be
    /// executed by the current seat.
    pub(super) fn denial_response_for_pressure(
        &self,
        g: &Game,
        pid: usize,
        own_progress: i32,
        rival: usize,
        pressure: VictoryFocus,
    ) -> Option<GrandStrategy> {
        let urgent = self.victory_pressure_is_urgent(g, rival, pressure);
        // Religious progress advances in whole-civilization jumps, and a
        // defender needs time to produce and route religious counters. Start
        // reacting with two holdouts left when the rival also leads our own
        // race, then treat one remaining holdout as an unconditional match
        // point: a slower "close" victory must not suppress that interrupt.
        if pressure.strategy == GrandStrategy::Religion {
            let living = g
                .players
                .iter()
                .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
                .count()
                .max(1) as i32;
            let match_point = 100 * living.saturating_sub(1) / living;
            let early_warning = (100 * living.saturating_sub(2) / living)
                .max(50)
                .min(match_point);
            if pressure.progress < early_warning
                || (pressure.progress < match_point
                    && !urgent
                    && pressure.progress < own_progress + 15)
            {
                return None;
            }
        } else if pressure.progress < 78 || (!urgent && pressure.progress < own_progress + 15) {
            return None;
        }

        // Racing a Science or score leader in-lane remains available when the
        // configured policy asks for it.  This module does not broaden that
        // choice into an unconditional war.
        if self.counter_stand_down
            && matches!(
                pressure.strategy,
                GrandStrategy::Science | GrandStrategy::Expansion
            )
        {
            return None;
        }
        Some(match pressure.strategy {
            GrandStrategy::Science if self.counter_in_lane => GrandStrategy::Science,
            GrandStrategy::Science => GrandStrategy::Conquest,
            GrandStrategy::Culture => GrandStrategy::Culture,
            GrandStrategy::Religion if g.players[pid].religion.is_some() => GrandStrategy::Religion,
            GrandStrategy::Religion => GrandStrategy::Conquest,
            GrandStrategy::Diplomacy => GrandStrategy::Diplomacy,
            GrandStrategy::Conquest => GrandStrategy::Recovery,
            GrandStrategy::Expansion if self.counter_in_lane => GrandStrategy::Expansion,
            GrandStrategy::Expansion => GrandStrategy::Conquest,
            GrandStrategy::Recovery => GrandStrategy::Recovery,
        })
    }

    /// The action planner must choose the highest-priority threat it can
    /// actually address, not abandon all denial because the nominal leader is
    /// un-actionable.  Reporting continues to expose that raw leader through
    /// `victory_denial`; only military/campaign routing uses this fallthrough.
    pub(super) fn actionable_victory_denial_with_culture_pressures(
        &self,
        g: &Game,
        pid: usize,
        culture_pressures: &BTreeMap<usize, i32>,
    ) -> Option<(usize, GrandStrategy)> {
        if !self.deny_leaders {
            return None;
        }
        let targeted = self.active_victory_target(g).is_some();
        if targeted && !self.deny_while_targeted {
            return None;
        }
        let own_progress = self.victory_focus(g, pid).progress;
        for (rival, pressure) in self.ranked_rival_victory_pressures(g, pid, culture_pressures) {
            if targeted && !self.victory_pressure_is_urgent(g, rival, pressure) {
                continue;
            }
            let Some(counter) =
                self.denial_response_for_pressure(g, pid, own_progress, rival, pressure)
            else {
                continue;
            };
            if self.conquest_denial_actionable(g, pid, rival, counter)
                && self.culture_denial_actionable(g, pid, rival, counter)
            {
                return Some((rival, counter));
            }
        }
        None
    }

    /// A Domination contract is fulfilled by foreign *original* capitals. An
    /// exposed city-state can still be a useful staging target, but once the
    /// campaign names a major rival, its first city must advance the victory.
    pub(super) fn domination_capital_target(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination) {
            return None;
        }
        g.cities
            .values()
            .filter(|city| city.is_capital && city.owner != pid && !g.same_team(pid, city.owner))
            .filter(|city| {
                g.players
                    .get(city.original_owner)
                    .is_some_and(|owner| !owner.is_minor && !owner.is_barbarian)
            })
            .filter(|city| {
                !g.same_team(pid, city.original_owner) || city.owner != city.original_owner
            })
            .filter(|city| self.campaign_target_legal(g, pid, city.owner))
            .filter(|city| !Self::should_defer_city_capture(g, pid, city.id))
            .map(|city| {
                (
                    city.owner,
                    city.id,
                    self.campaign_city_value(g, pid, city, GrandStrategy::Conquest),
                )
            })
            .min_by(|left, right| {
                left.2
                    .total_cmp(&right.2)
                    .then_with(|| left.0.cmp(&right.0))
                    .then_with(|| left.1.cmp(&right.1))
            })
            .map(|(owner, city, _)| (owner, city))
    }

    /// Once a normal denial response has selected Conquest, aim the first
    /// campaign city at the rival's victory infrastructure instead of a
    /// merely convenient settlement.  Science's Spaceport and a no-religion
    /// response's Holy Site are concrete bottlenecks that taking can disrupt.
    pub(super) fn victory_suppression_city(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        pressure: VictoryFocus,
    ) -> Option<u32> {
        if pressure.progress < 78 {
            return None;
        }
        let district = match pressure.strategy {
            GrandStrategy::Science => crate::name!("spaceport"),
            GrandStrategy::Religion => crate::name!("holy_site"),
            _ => return None,
        };
        g.cities
            .values()
            .filter(|city| city.owner == rival)
            .filter(|city| city.districts.contains_key(district))
            .filter(|city| !Self::should_defer_city_capture(g, pid, city.id))
            .min_by(|left, right| {
                self.campaign_city_value(g, pid, left, GrandStrategy::Conquest)
                    .total_cmp(&self.campaign_city_value(g, pid, right, GrandStrategy::Conquest))
                    .then_with(|| left.id.cmp(&right.id))
            })
            .map(|city| city.id)
    }
}

#[cfg(test)]
mod tests {
    use super::super::StrategicPlan;
    use super::*;
    use crate::{
        ai::BasicAi,
        game::{Game, LiveGreatPersonActivationNeed, ObservedPublicEmpireStats},
    };

    fn found_capitals(game: &mut Game) {
        let majors: Vec<_> = game
            .players
            .iter()
            .filter(|player| !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        for pid in majors {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .expect("every major starts with a settler");
            game.found_city_for(pid, game.units[&settler].pos, None);
        }
        game.current = 0;
    }

    fn science_plan(turn: u32) -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: turn,
            rush: false,
        }
    }

    fn open_land_near(game: &Game, center: crate::Pos, radius: i32) -> crate::Pos {
        game.wdisk(center, radius)
            .into_iter()
            .find(|position| {
                *position != center
                    && game.city_at(*position).is_none()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("the fixture has a nearby open land tile")
    }

    #[test]
    fn science_target_keeps_rocketry_ahead_of_an_unrelated_live_great_person_detour() {
        let mut game = Game::new_full(1, 24, 16, 91_001, 300, 0, false);
        found_capitals(&mut game);
        game.turn = 140;

        let ai = AdvancedAi::targeting(VictoryTarget::Science);
        let techs: Vec<_> = game
            .rules
            .techs
            .keys()
            .filter(|tech| ai.tech_leads_to(&game, tech, "rocketry"))
            .copied()
            .collect();
        game.players[0]
            .techs
            .extend(techs.into_iter().filter(|tech| tech.as_str() != "rocketry"));
        game.players[0].techs.insert(crate::name!("mining"));
        game.players[0]
            .live_great_person_activation_needs
            .push(LiveGreatPersonActivationNeed {
                kind: "general".to_string(),
                individual: None,
                required_district: None,
            });
        assert_eq!(
            BasicAi::live_great_person_tech_goal(&game, 0).as_deref(),
            Some("bronze_working"),
            "the fixture needs an unrelated live Great Person technology goal"
        );

        let plan = science_plan(game.turn);
        ai.advanced_research(&mut game, 0, &plan);

        assert_eq!(game.players[0].research.as_deref(), Some("rocketry"));
    }

    #[test]
    fn domination_target_aims_at_an_uncontrolled_original_capital_before_a_convenient_city() {
        let mut game = Game::new_full(2, 48, 28, 91_002, 300, 0, false);
        found_capitals(&mut game);
        game.turn = 200;
        game.record_contact(0, 1);
        let capital = game.player_city_ids(1)[0];
        let capital_pos = game.cities[&capital].pos;
        let outpost = game.found_city_for(1, open_land_near(&game, capital_pos, 4), None);
        game.cities.get_mut(&capital).unwrap().hp = 200;
        game.cities.get_mut(&capital).unwrap().wall_hp = 400;
        game.cities.get_mut(&capital).unwrap().buildings.extend([
            crate::name!("walls"),
            crate::name!("medieval_walls"),
            crate::name!("renaissance_walls"),
        ]);
        for _ in 0..3 {
            game.spawn_test_unit("giant_death_robot", 1, capital_pos);
        }
        game.cities.get_mut(&outpost).unwrap().hp = 25;
        game.cities.get_mut(&outpost).unwrap().wall_hp = 0;
        game.cities.get_mut(&outpost).unwrap().pop = 14;
        let _capital_observer = game.spawn_test_unit("scout", 0, capital_pos);
        let _outpost_observer = game.spawn_test_unit("scout", 0, game.cities[&outpost].pos);

        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.belief.observe(&game, 0);
        let outpost_value =
            ai.campaign_city_value(&game, 0, &game.cities[&outpost], GrandStrategy::Conquest);
        let capital_value =
            ai.campaign_city_value(&game, 0, &game.cities[&capital], GrandStrategy::Conquest);
        assert!(
            outpost_value < capital_value,
            "the fixture needs the generic scorer to prefer the convenient city: \
             outpost={outpost_value}, capital={capital_value}"
        );
        assert!(
            !AdvancedAi::should_defer_city_capture(&game, 0, capital),
            "the required capital must be an operationally valid target"
        );

        let plan = ai.assess(&game, 0);
        assert_eq!(plan.strategy, GrandStrategy::Conquest);
        assert_eq!(plan.target_player, Some(1));
        assert_eq!(plan.target_city, Some(capital));
    }

    #[test]
    fn domination_target_ignores_capitals_already_held_by_its_team() {
        let mut game = Game::new_full(3, 36, 22, 91_005, 300, 0, false);
        found_capitals(&mut game);
        game.players[0].team = Some(1);
        game.players[1].team = Some(1);
        game.turn = 200;
        game.record_contact(0, 2);

        let ai = AdvancedAi::targeting(VictoryTarget::Domination);
        assert_eq!(
            ai.domination_capital_target(&game, 0),
            Some((2, game.player_city_ids(2)[0])),
            "a teammate retaining its own capital is already sufficient for a team victory"
        );
    }

    #[test]
    fn science_denial_aims_at_the_rival_spaceport_before_a_convenient_city() {
        let mut game = Game::new_full(2, 48, 28, 91_003, 300, 0, false);
        found_capitals(&mut game);
        game.turn = 190;
        game.record_contact(0, 1);
        let rival_capital = game.player_city_ids(1)[0];
        let spaceport_city = game.found_city_for(
            1,
            open_land_near(&game, game.cities[&rival_capital].pos, 4),
            Some("Launch Complex".to_string()),
        );
        let district = game.cities[&spaceport_city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&spaceport_city].pos)
            .expect("a founded city owns a district tile");
        game.map.tiles.get_mut(&district).unwrap().district = Some(crate::name!("spaceport"));
        game.cities
            .get_mut(&spaceport_city)
            .unwrap()
            .districts
            .insert(crate::name!("spaceport"), district);
        game.players[1]
            .science_projects
            .insert("exoplanet_expedition".to_string());

        let ai = AdvancedAi::new();
        assert_eq!(
            ai.victory_denial(&game, 0),
            Some((1, GrandStrategy::Conquest))
        );
        let plan = ai.assess(&game, 0);
        assert_eq!(plan.strategy, GrandStrategy::Conquest);
        assert_eq!(plan.target_player, Some(1));
        assert_eq!(plan.target_city, Some(spaceport_city));
    }

    #[test]
    fn an_unactionable_leader_does_not_mask_an_actionable_victory_threat() {
        let mut game = Game::new_full(4, 48, 28, 91_004, 300, 0, false);
        found_capitals(&mut game);
        game.turn = 190;
        for rival in 1..4 {
            game.record_contact(0, rival);
        }
        game.observed_public_empire_stats.insert(
            0,
            ObservedPublicEmpireStats {
                domestic_tourists: Some(10),
                ..ObservedPublicEmpireStats::default()
            },
        );
        game.observed_public_empire_stats.insert(
            1,
            ObservedPublicEmpireStats {
                foreign_tourists: Some(150),
                ..ObservedPublicEmpireStats::default()
            },
        );
        game.observed_public_empire_stats.insert(
            2,
            ObservedPublicEmpireStats {
                domestic_tourists: Some(150),
                ..ObservedPublicEmpireStats::default()
            },
        );
        game.observed_public_empire_stats.insert(
            3,
            ObservedPublicEmpireStats {
                domestic_tourists: Some(20),
                ..ObservedPublicEmpireStats::default()
            },
        );
        game.players[3]
            .science_projects
            .insert("exoplanet_expedition".to_string());

        let mut ai = AdvancedAi::new();
        ai.stock_denial_lead_time = true;
        assert_eq!(
            ai.victory_denial(&game, 0),
            Some((1, GrandStrategy::Culture)),
            "the public signal keeps reporting the leading Culture clock"
        );
        assert_eq!(
            ai.actionable_victory_denial(&game, 0),
            Some((3, GrandStrategy::Conquest)),
            "the army planner falls through to the launched Science threat"
        );
        let plan = ai.assess(&game, 0);
        assert_eq!(plan.strategy, GrandStrategy::Conquest);
        assert_eq!(plan.target_player, Some(3));
    }
}
