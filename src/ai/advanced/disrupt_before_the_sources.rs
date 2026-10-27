//! `disrupt-before-the-sources`: a Spy standing in a space racer's city runs
//! Disrupt Rocketry when it is offered, before the training step that sends a
//! green Spy on Gain Sources.
//!
//! The spy pass starts every operation in a foreign city with "a Spy below
//! level 2 runs Gain Sources while it is offered" and only then reaches
//! `science-denial-every-pad`'s and `racer-keeps-the-spies`' Disrupt Rocketry.
//! The live seat reads every Spy at level 1, so whenever the host lists Gain
//! Sources it pre-empts the disruption. Disrupt Rocketry is the one operation
//! that stops a launch (`UNITOPERATION_SPY_DISRUPT_ROCKETRY`, TargetDistrict
//! `DISTRICT_SPACEPORT`, `Base/Assets/Gameplay/Data/UnitOperations.xml`).
//!
//! Retained live Immortal games to 2026-10-27: on the turn after an idle
//! Spy-turn with Disrupt Rocketry on the host's menu, the Spy ran it 91 times,
//! Gain Sources 71 times, Listening Post 16 times and nothing 73 times.
//! Technology was the most common played-out loss after the turn-80
//! production rule (10 of 27). In 12 of the 14 retained Science losses a Spy
//! of ours stood in a city of the winner with the disruption offered (first
//! offered at turns 139-183, the winner's projects from 168-210); 061046Z
//! kept one in Beijing from 168 to 196 with it offered on every idle turn and
//! ran it 0 times while China won at 200.
//!
//! Under the gene, in a city of a rival `racer-keeps-the-spies` names a
//! racer, or of a `science-denial-every-pad` threat, a Spy offered Disrupt
//! Rocketry (by the board or the host) skips the training step; the existing
//! disruption paths then take it.
//!
//! Off: unchanged.

use super::*;

impl AdvancedAi {
    /// `disrupt-before-the-sources`: whether `spy`'s Gain Sources training
    /// step yields to Disrupt Rocketry: the gene is on, the Spy stands in a
    /// city of one of `racers` or `threats`, and the disruption is offered
    /// there (`racer_spy_disrupt_offered`). Always `false` with the gene off.
    pub(super) fn training_yields_to_the_disrupt(
        &self,
        g: &Game,
        spy: u32,
        legal: &[Action],
        racers: &BTreeSet<usize>,
        threats: &BTreeSet<usize>,
    ) -> bool {
        if !self.disrupt_before_the_sources {
            return false;
        }
        let Some(owner) = g
            .spies
            .get(&spy)
            .and_then(|agent| agent.city)
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner)
        else {
            return false;
        };
        (racers.contains(&owner) || threats.contains(&owner))
            && Self::racer_spy_disrupt_offered(g, spy, legal)
    }

    /// Journal a green Spy that skips the training step for the disruption,
    /// with its level, so a live read can tell an attempt that failed from
    /// one that was never made.
    pub(super) fn journal_training_yields(&self, g: &Game, spy: u32) {
        if !self.journal().wants(crate::reasoning::Level::Detail) {
            return;
        }
        let Some(agent) = g.spies.get(&spy) else {
            return;
        };
        let Some(city) = agent.city.and_then(|city| g.cities.get(&city)) else {
            return;
        };
        let level = agent.level;
        let name = city.name.clone();
        let civ = g.players[city.owner].civ.clone();
        think!(self.journal(), Military, Detail,
               "A level {level} Spy disrupts before it gains sources";
               "disrupt-before-the-sources: Disrupt Rocketry is offered in {civ}'s {name}, the space racer's city");
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
    use crate::game::Game;

    #[test]
    fn disrupt_before_the_sources_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("disrupt-before-the-sources", |ai| {
            ai.disrupt_before_the_sources
        });
    }

    /// Two majors at peace and in contact; seat 1, the space racer, holds
    /// Rocketry and a Spaceport seat 0 has seen. A green Spy of seat 0 stands
    /// in its capital with its sources not yet gained, so the board offers it
    /// both Gain Sources and Disrupt Rocketry.
    fn racer_with_a_pad() -> (Game, u32, u32) {
        let mut g = Game::new_full(2, 30, 20, 41_830, 250, 0, false);
        for pid in 0..2 {
            let settler = g
                .player_unit_ids(pid)
                .into_iter()
                .find(|uid| g.units[uid].kind == "settler")
                .expect("each fixture major begins with a Settler");
            let position = g.units[&settler].pos;
            g.found_city_for(pid, position, None);
            for uid in g.player_unit_ids(pid) {
                g.remove_unit(uid);
            }
        }
        g.current = 0;
        g.turn = 150;
        g.at_war.clear();
        g.record_contact(0, 1);
        let capital = g.player_city_ids(1)[0];
        let centre = g.cities[&capital].pos;
        g.players[0].explored.insert(centre);
        g.players[1].techs.insert(crate::name!("rocketry"));
        let pad = g
            .wdisk(centre, 1)
            .into_iter()
            .find(|pos| {
                *pos != centre
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| tile.district.is_none() && !g.rules.is_water(tile))
            })
            .expect("the fixture city has a free land neighbour");
        let tile = g.map.tiles.get_mut(&pad).unwrap();
        tile.owner_city = Some(capital);
        tile.district = Some(crate::name!("spaceport"));
        tile.pillaged = false;
        tile.improvement = None;
        g.cities
            .get_mut(&capital)
            .unwrap()
            .districts
            .insert(crate::name!("spaceport"), pad);
        g.players[0].explored.insert(pad);
        let spy = g.next_id;
        g.next_id += 1;
        g.spies.insert(
            spy,
            crate::game::Spy {
                id: spy,
                owner: 0,
                level: 0,
                promotions: BTreeSet::new(),
                city: Some(capital),
                ready_turn: g.turn,
                mission: None,
                sources_city: None,
                sources_until: 0,
                captured_by: None,
            },
        );
        (g, capital, spy)
    }

    fn conquest_on(target: usize) -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(target),
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: 0,
            rush: false,
        }
    }

    /// The live arming the gene joins: the Rocketry-trigger network and the
    /// racer's Spies.
    fn live() -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_space_denial_spies_from_rocketry();
        ai.enable_racer_keeps_the_spies();
        ai
    }

    fn mission(g: &Game, spy: u32) -> Option<String> {
        g.spies[&spy]
            .mission
            .as_ref()
            .map(|mission| mission.kind.clone())
    }

    #[test]
    fn a_green_spy_in_the_racers_city_disrupts_before_it_gains_sources() {
        let (g, _, spy) = racer_with_a_pad();
        let legal = g.legal_spy_actions(0, spy);
        let offered = |name: &str| {
            legal.iter().any(|action| {
                matches!(action, crate::game::Action::SpyMission { mission, .. } if mission == name)
            })
        };
        assert!(
            offered("gain_sources") && offered("disrupt_rocketry"),
            "fixture: both operations are offered"
        );

        let mut off = g.clone();
        live().advanced_spies(&mut off, 0, &conquest_on(1));
        assert_eq!(
            mission(&off, spy).as_deref(),
            Some("gain_sources"),
            "off, the training step takes the green Spy"
        );

        let mut ai = live();
        ai.enable_disrupt_before_the_sources();
        let mut on = g.clone();
        ai.advanced_spies(&mut on, 0, &conquest_on(1));
        assert_eq!(mission(&on, spy).as_deref(), Some("disrupt_rocketry"));
    }

    #[test]
    fn the_training_step_keeps_a_spy_outside_the_racers_cities() {
        let (g, _, spy) = racer_with_a_pad();
        let legal = g.legal_spy_actions(0, spy);
        let mut ai = live();
        ai.enable_disrupt_before_the_sources();
        let racers: BTreeSet<usize> = [1].into_iter().collect();
        assert!(ai.training_yields_to_the_disrupt(&g, spy, &legal, &racers, &BTreeSet::new()));
        // Seat 1 is no racer and no threat: the training step keeps the Spy.
        assert!(!ai.training_yields_to_the_disrupt(
            &g,
            spy,
            &legal,
            &BTreeSet::new(),
            &BTreeSet::new()
        ));
        // Nothing offered: the training step keeps the Spy.
        assert!(!ai.training_yields_to_the_disrupt(&g, spy, &[], &racers, &BTreeSet::new()));
    }
}
