//! `disrupt-waits-for-the-sources`: a Spy quoted long odds on Disrupt
//! Rocketry in a space racer's city runs Gain Sources there first, on every
//! victory lane and at every level.
//!
//! ## Why (live Deity, 2026-11-18, 62 games reaching turn 150)
//!
//! Gain Sources never levels a Spy, but it lifts the host's quote for every
//! later operation of that Spy in that city by two of the host's odds steps
//! (`UnitManager.GetResultProbability`, exported per mission as
//! `spy_mission_targets`, read here as `HostUnitFacts::spy_targets`).
//! Across 215 of our Gain Sources the Disrupt Rocketry quote went 0.254 to
//! 0.496 (19 times), 0.156 to 0.371 (6) and 0.371 to 0.625 (3), and the
//! Steal Tech Boost quote 0.371 to 0.625 (102). The native model already
//! prices it the same way: `spy_effective_level` adds two levels while
//! `sources_city` holds. It takes about four turns at Online speed, and the
//! host offers it again only some twelve turns after it ends.
//!
//! The quote is honest. 301 of our 302 Disrupt Rocketry launches carried
//! one the frame before they started: quoted 0.254 or less they pillaged the
//! pad 29% of the time and lost the Spy 60% (84 launches); 0.371-0.496, 39%
//! and 49% (146); 0.625, 53% and 43% (30); 0.738 or more, 83% and 10% (41).
//! 217 of the 301 launched with a sure Gain Sources still on the host's
//! menu, 184 of them quoted below 0.7 (36% pillaged, 51% lost).
//!
//! `disrupt-before-the-sources` (armed live) skips the green Spy's training
//! step once the racer has landed a project, and only a Culture seat
//! prepares first (`native_culture_spy_prepares`, quotes up to one third).
//! Launches at one third or less with Gain Sources offered: 6 of 46 on the
//! Culture seats, 74 of 171 on the others (Religious 45 of 63, Diplomatic 18
//! of 28, Science 11 of 80), which pillaged 22-55% and lost the Spy 55-72%.
//! A Spy of host level 3 or more never reaches the training step at all, so
//! none of the gates above ever asked it to prepare.
//!
//! ## What the gene does
//!
//! In a city of a racer (`racer-keeps-the-spies`) or of a science threat
//! (`science-denial-every-pad`) that has not launched its Exoplanet
//! Expedition, a Spy offered Disrupt Rocketry (by the board or the host) and
//! a Gain Sources the host quotes sure runs Gain Sources first while its
//! Disrupt Rocketry quote is below [`SOURCES_FIRST_BELOW`]. On a native
//! board, with no host facts for the Spy, the native odds
//! (`Game::spy_success_chance`) stand in for the quote and Gain Sources is
//! sure by its spec. A live Spy whose host quotes did not cross is left to
//! the existing rule, and so is every Spy once the expedition is underway
//! (the urgent case `native_culture_spy_prepares` also keeps).
//!
//! Off: unchanged.

use super::*;

/// The Disrupt Rocketry quote at and above which a Spy disrupts without
/// preparing. Live launches quoted 0.625 lost the Spy 43% of the time (30
/// launches) and those quoted 0.738 or more 10% (41); two odds steps from
/// Gain Sources carry 0.496 to 0.738 and 0.625 to 0.836.
pub(crate) const SOURCES_FIRST_BELOW: f64 = 0.7;

/// A Gain Sources the host quotes at least this sure counts as safe.
const SOURCES_SURE: f64 = 0.999;

impl AdvancedAi {
    /// `disrupt-waits-for-the-sources`: the Gain Sources order `spy` runs
    /// before a long-odds Disrupt Rocketry where it stands, or `None`.
    /// Always `None` with the gene off.
    pub(super) fn sources_before_the_disrupt(
        &self,
        g: &Game,
        spy: u32,
        legal: &[Action],
        racers: &BTreeSet<usize>,
        threats: &BTreeSet<usize>,
    ) -> Option<Action> {
        if !self.disrupt_waits_for_the_sources {
            return None;
        }
        let city = g
            .spies
            .get(&spy)
            .and_then(|agent| agent.city)
            .and_then(|city| g.cities.get(&city))?;
        let owner = city.owner;
        if !(racers.contains(&owner) || threats.contains(&owner))
            || g.players[owner]
                .science_projects
                .contains("exoplanet_expedition")
            || !Self::racer_spy_disrupt_offered(g, spy, legal)
        {
            return None;
        }
        let sources = legal.iter().find(|action| {
            matches!(action, Action::SpyMission { spy: id, mission, .. }
                if *id == spy && mission == "gain_sources")
        })?;
        let (disrupt, sure) = Self::disrupt_and_sources_quotes(g, spy, city.id, legal)?;
        (sure && disrupt.is_finite() && disrupt < SOURCES_FIRST_BELOW).then(|| sources.clone())
    }

    /// The Disrupt Rocketry odds of `spy` where it stands and whether its
    /// Gain Sources is sure: the host's quotes when they crossed, the native
    /// model's when the host sent no facts for the Spy at all, else `None`.
    fn disrupt_and_sources_quotes(
        g: &Game,
        spy: u32,
        city: u32,
        legal: &[Action],
    ) -> Option<(f64, bool)> {
        let quote = |target: Option<&crate::game::HostSpyTarget>| {
            target
                .and_then(|target| target.success)
                .filter(|odds| odds.is_finite() && (0.0..=1.0).contains(odds))
        };
        match g.host_unit_facts.get(&spy) {
            Some(facts) => {
                let targets = facts.spy_targets.as_ref()?;
                let disrupt = quote(targets.get("disrupt_rocketry"))?;
                let sure =
                    quote(targets.get("gain_sources")).is_some_and(|odds| odds >= SOURCES_SURE);
                Some((disrupt, sure))
            }
            None => {
                let target = legal.iter().find_map(|action| match action {
                    Action::SpyMission {
                        spy: id,
                        mission,
                        target,
                    } if *id == spy && mission == "disrupt_rocketry" => Some(*target),
                    _ => None,
                })?;
                let mission = crate::game::SpyMission {
                    kind: "disrupt_rocketry".to_string(),
                    city,
                    target,
                    started: g.turn,
                    ends: g.turn,
                };
                Some((g.spy_success_chance(spy, &mission), true))
            }
        }
    }

    /// Journal a Spy that prepares before the disruption, with its level and
    /// quote, so a live read can tell a waiting Spy from an idle one.
    pub(super) fn journal_sources_before_the_disrupt(&self, g: &Game, spy: u32) {
        if !self.journal().wants(crate::reasoning::Level::Detail) {
            return;
        }
        let Some(agent) = g.spies.get(&spy) else {
            return;
        };
        let Some(city) = agent.city.and_then(|city| g.cities.get(&city)) else {
            return;
        };
        let level = agent.level + 1;
        let quote = g
            .host_unit_facts
            .get(&spy)
            .and_then(|facts| facts.spy_targets.as_ref())
            .and_then(|targets| targets.get("disrupt_rocketry"))
            .and_then(|target| target.success)
            .map_or_else(|| "native odds".to_string(), |odds| format!("{odds:.3}"));
        let name = city.name.clone();
        let civ = g.players[city.owner].civ.clone();
        think!(self.journal(), Military, Detail,
               "A level {level} Spy gains sources before it disrupts";
               "disrupt-waits-for-the-sources: Disrupt Rocketry in {civ}'s {name} is quoted {quote}, \
                below {SOURCES_FIRST_BELOW}; Gain Sources lifts it two steps");
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
    use crate::game::{Action, Game, HostSpyTarget};

    #[test]
    fn disrupt_waits_for_the_sources_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("disrupt-waits-for-the-sources", |ai| {
            ai.disrupt_waits_for_the_sources
        });
    }

    /// Two majors at peace and in contact; seat 1, the space racer, holds
    /// Rocketry, has launched its Earth Satellite and stands a Spaceport seat
    /// 0 has seen. A Spy of seat 0 at `level` stands in its capital with its
    /// sources not yet gained.
    fn racer_with_a_pad(level: i64) -> (Game, u32, u32) {
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
        g.players[1]
            .science_projects
            .insert("launch_earth_satellite".to_string());
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
                level,
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

    /// The host's quotes for `spy` in `city`, as the live mirror files them.
    fn quote(g: &mut Game, spy: u32, city: u32, disrupt: f64, sources: f64) {
        let pos = g.cities[&city].pos;
        let pad = *g.cities[&city]
            .districts
            .get(crate::name!("spaceport"))
            .unwrap();
        let targets = [
            (
                "gain_sources".to_string(),
                HostSpyTarget {
                    pos,
                    success: Some(sources),
                    fail: Some(0.0),
                },
            ),
            (
                "disrupt_rocketry".to_string(),
                HostSpyTarget {
                    pos: pad,
                    success: Some(disrupt),
                    fail: Some(0.335938),
                },
            ),
        ]
        .into_iter()
        .collect();
        std::sync::Arc::make_mut(&mut g.host_unit_facts)
            .entry(spy)
            .or_default()
            .spy_targets = Some(targets);
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

    /// The live arming this gene joins: the Rocketry-trigger network, the
    /// racer's Spies and the green Spy's skipped training step, on a
    /// Diplomatic seat (no Culture preparation).
    fn live() -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Diplomacy);
        ai.enable_space_denial_spies_from_rocketry();
        ai.enable_racer_keeps_the_spies();
        ai.enable_disrupt_before_the_sources();
        ai
    }

    fn on() -> AdvancedAi {
        let mut ai = live();
        ai.enable_disrupt_waits_for_the_sources();
        ai
    }

    fn mission(g: &Game, spy: u32) -> Option<String> {
        g.spies[&spy]
            .mission
            .as_ref()
            .map(|mission| mission.kind.clone())
    }

    fn played(ai: AdvancedAi, g: &Game) -> Game {
        let mut ai = ai;
        let mut g = g.clone();
        ai.advanced_spies(&mut g, 0, &conquest_on(1));
        g
    }

    #[test]
    fn a_long_odds_spy_gains_sources_before_it_disrupts_on_any_lane() {
        let (mut g, city, spy) = racer_with_a_pad(0);
        quote(&mut g, spy, city, 0.253906, 1.0);
        assert_eq!(
            mission(&played(live(), &g), spy).as_deref(),
            Some("disrupt_rocketry"),
            "off, the green Spy disrupts at a quarter"
        );
        assert_eq!(
            mission(&played(on(), &g), spy).as_deref(),
            Some("gain_sources")
        );
    }

    #[test]
    fn a_seasoned_spy_prepares_too_but_a_good_quote_disrupts_at_once() {
        // Level 3 on the host: past the training step, so only this gene
        // asks it to prepare.
        let (mut g, city, spy) = racer_with_a_pad(2);
        quote(&mut g, spy, city, 0.625, 1.0);
        assert_eq!(
            mission(&played(live(), &g), spy).as_deref(),
            Some("disrupt_rocketry")
        );
        assert_eq!(
            mission(&played(on(), &g), spy).as_deref(),
            Some("gain_sources")
        );
        quote(&mut g, spy, city, 0.738281, 1.0);
        assert_eq!(
            mission(&played(on(), &g), spy).as_deref(),
            Some("disrupt_rocketry")
        );
    }

    #[test]
    fn the_expedition_underway_disrupts_at_once() {
        let (mut g, city, spy) = racer_with_a_pad(0);
        quote(&mut g, spy, city, 0.253906, 1.0);
        g.players[1]
            .science_projects
            .insert("exoplanet_expedition".into());
        assert_eq!(
            mission(&played(on(), &g), spy).as_deref(),
            Some("disrupt_rocketry")
        );
    }

    #[test]
    fn preparation_needs_a_sure_quoted_source_in_a_racers_city() {
        let (mut g, city, spy) = racer_with_a_pad(0);
        let ai = on();
        let racers: BTreeSet<usize> = [1].into_iter().collect();
        let none = BTreeSet::new();
        quote(&mut g, spy, city, 0.253906, 1.0);
        let legal = g.legal_spy_actions(0, spy);
        let sources = |action: Option<Action>| matches!(action, Some(Action::SpyMission { mission, .. }) if mission == "gain_sources");
        assert!(sources(
            ai.sources_before_the_disrupt(&g, spy, &legal, &racers, &none)
        ));
        // A science threat's city counts as a racer's.
        assert!(sources(
            ai.sources_before_the_disrupt(&g, spy, &legal, &none, &racers)
        ));
        // Neither a racer nor a threat.
        assert!(ai
            .sources_before_the_disrupt(&g, spy, &legal, &none, &none)
            .is_none());
        // No Disrupt Rocketry offered, no Gain Sources offered.
        let without = |name: &str| -> Vec<Action> {
            legal
                .iter()
                .filter(|action| !matches!(action, Action::SpyMission { mission, .. } if mission == name))
                .cloned()
                .collect()
        };
        assert!(ai
            .sources_before_the_disrupt(&g, spy, &without("gain_sources"), &racers, &none)
            .is_none());
        let mut unoffered = g.clone();
        std::sync::Arc::make_mut(&mut unoffered.host_unit_facts)
            .get_mut(&spy)
            .unwrap()
            .spy_missions = Some(["gain_sources".to_string()].into_iter().collect());
        assert!(ai
            .sources_before_the_disrupt(
                &unoffered,
                spy,
                &without("disrupt_rocketry"),
                &racers,
                &none
            )
            .is_none());
        // A source the host does not quote sure, and unreadable quotes.
        for (disrupt, sources_odds) in [(0.253906, 0.9), (f64::NAN, 1.0), (-1.0, 1.0)] {
            quote(&mut g, spy, city, disrupt, sources_odds);
            assert!(ai
                .sources_before_the_disrupt(&g, spy, &legal, &racers, &none)
                .is_none());
        }
        // Host facts without quotes: the existing rule stands.
        std::sync::Arc::make_mut(&mut g.host_unit_facts)
            .get_mut(&spy)
            .unwrap()
            .spy_targets = None;
        assert!(ai
            .sources_before_the_disrupt(&g, spy, &legal, &racers, &none)
            .is_none());
        // Off: never.
        quote(&mut g, spy, city, 0.253906, 1.0);
        assert!(live()
            .sources_before_the_disrupt(&g, spy, &legal, &racers, &none)
            .is_none());
    }

    #[test]
    fn a_native_spy_reads_the_model_and_disrupts_once_its_sources_hold() {
        let (g, _, spy) = racer_with_a_pad(2);
        assert!(
            g.host_unit_facts.get(&spy).is_none(),
            "fixture: a native board"
        );
        let after = played(on(), &g);
        assert_eq!(mission(&after, spy).as_deref(), Some("gain_sources"));
        assert_eq!(
            mission(&played(live(), &g), spy).as_deref(),
            Some("disrupt_rocketry"),
            "off, the seasoned native Spy disrupts at once"
        );
        // Its sources gained, the board no longer offers them: it disrupts.
        let mut prepared = g.clone();
        let city = prepared.spies[&spy].city;
        let spy_row = prepared.spies.get_mut(&spy).unwrap();
        spy_row.sources_city = city;
        spy_row.sources_until = prepared.turn + 20;
        assert_eq!(
            mission(&played(on(), &prepared), spy).as_deref(),
            Some("disrupt_rocketry")
        );
    }
}
