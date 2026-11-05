//! Target-contract routing for Science, Domination, and victory denial.
//!
//! These are not a new discretionary-war policy.  The normal planner must
//! first choose Conquest as an actionable response; only then may this module
//! name the concrete city whose capture interrupts the rival's victory work.

use super::{AdvancedAi, GrandStrategy, VictoryFocus, VictoryTarget};
use crate::game::Game;
use std::collections::BTreeMap;

/// The science reading at which a Domination army answers a rival's launch
/// chain: the Moon Landing (`rival_victory_pressure`'s 45).
pub(super) const DOMINATION_SCIENCE_COUNTER: i32 = 45;

const SCIENCE_VICTORY_TECH_CHAIN: [&str; 5] = [
    "rocketry",
    "satellites",
    "nanotechnology",
    "smart_materials",
    "offworld_mission",
];

impl AdvancedAi {
    /// A Domination army is already the counterforce. Do not require opt-in
    /// lane switching to aim it at a Culture leader, a Science racer past
    /// its Moon Landing, or a religious match point. Culture and Science
    /// leave a preparation window; religion uses the existing strict
    /// majority tally and its whole-civilization match-point threshold.
    pub(super) fn domination_counter_pressure(&self, g: &Game, pressure: VictoryFocus) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !self.deny_leaders
            || !Self::victory_strategy_enabled(g, pressure.strategy)
        {
            return false;
        }
        match pressure.strategy {
            GrandStrategy::Culture => pressure.progress >= self.culture_threat_pressure(),
            // The launch chain is serial and public: a rival that has landed
            // on the Moon has two launches left and its Spaceport is the one
            // bottleneck an army can take. Waiting for the expedition (78)
            // left six turns on King `civvis-20260930T211803Z`, where
            // Portugal finished the Moon at turn 215, flew at turn 242 and
            // won at 248 while the Domination seat stayed at peace with it.
            GrandStrategy::Science => pressure.progress >= DOMINATION_SCIENCE_COUNTER,
            GrandStrategy::Religion => {
                let living = g
                    .players
                    .iter()
                    .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
                    .count() as i32;
                living > 1 && pressure.progress >= 100 * (living - 1) / living
            }
            // `dvp-leader-is-the-front`: a Diplomatic Victory leader one
            // session from the win. See `advanced/dvp_leader_front.rs`.
            GrandStrategy::Diplomacy => self.dvp_pressure_is_a_clock(pressure),
            _ => false,
        }
    }

    /// A Domination seat with no faith of its own cannot win its own cities
    /// back, so once a rival's religion holds our majority that rival already
    /// counts us and needs only the other holdouts — civilizations our army
    /// cannot keep unconverted. The match point then arrives on the rival's
    /// schedule, not ours: on King `civvis-20260929T143005Z` Islam held our
    /// majority from turn 80, the match-point counter first fired at turn
    /// 140, and Arabia won at 149, before any army could reach a city. Start
    /// the counter at the religion early-warning bar
    /// (`denial_response_for_pressure`'s two-holdout reading) instead. A
    /// majority that follows no living rival's founded faith defends nothing
    /// either, so then every faith at the bar is countered. This selects the
    /// campaign target only: it is deliberately not urgency, so the ordinary
    /// declaration, readiness and one-war gates still apply.
    pub(super) fn domination_faithless_conversion_counter(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        pressure: VictoryFocus,
    ) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !self.deny_leaders
            || !Self::victory_strategy_enabled(g, GrandStrategy::Religion)
            || g.players[pid].religion.is_some()
        {
            return false;
        }
        // The rival's religion clock, whether or not it is the rival's best
        // lane. A faith at the bar is a threat to a faithless seat even while
        // the tech floor or tourism reads higher. In King
        // civvis-20261001T022028Z Brazil's Catholicism held two of four
        // majors from its first contact, but Brazil's best lane read Culture,
        // so the religion counter never looked.
        let progress = if pressure.strategy == GrandStrategy::Religion {
            pressure.progress
        } else {
            self.lane_progress_table(g, rival)[2]
        };
        let Some(faith) = g.players[rival].religion.as_deref() else {
            return false;
        };
        let living = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
            .count() as i32;
        let match_point = 100 * (living - 1) / living.max(1);
        let early_warning = (100 * (living - 2) / living.max(1))
            .max(50)
            .min(match_point);
        if living <= 2 || progress < early_warning {
            return false;
        }
        // Our majority's own faith, if a living rival founded it, is the one
        // that already counts us. Otherwise nothing of ours resists any
        // faith at the bar: on King civvis-20261001T022028Z our cities held
        // Islam, whose founder Arabia had itself turned Catholic by turn 75.
        // Catholicism sat at the bar from turn 75, the counter waited for our
        // majority until turn 144, and Brazil won at 147.
        let held_by_a_rival_faith = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && other
                    .religion
                    .as_deref()
                    .is_some_and(|founded| g.civ_follows_religion(pid, founded))
        });
        g.civ_follows_religion(pid, faith) || !held_by_a_rival_faith
    }

    /// Whether `rival`'s victory clock is one the Domination army answers:
    /// the lane-independent counter above, or a faith that already holds a
    /// faithless seat's majority.
    pub(super) fn domination_counter_target(&self, g: &Game, pid: usize, rival: usize) -> bool {
        let pressure = self.rival_victory_pressure(g, rival);
        self.domination_counter_pressure(g, pressure)
            || self.domination_faithless_conversion_counter(g, pid, rival, pressure)
            // See `one_war::culture_lane_threat`.
            || self.culture_lane_threat(g, rival)
    }
    /// The next irreducible Science milestone. An explicit or adaptive
    /// Science plan can still honour a declared rush or a war breakthrough,
    /// but an unrelated live Great Person must not detour it away from the
    /// prerequisite chain that opens the victory projects. Keep this as the
    /// single source for the chain: research scoring and forced-goal routing
    /// must agree about which victory technology is next, even while its
    /// project is still being built.
    pub(super) fn science_victory_tech_goal(
        g: &Game,
        pid: usize,
        objective: GrandStrategy,
    ) -> Option<&'static str> {
        (objective == GrandStrategy::Science)
            .then(|| {
                SCIENCE_VICTORY_TECH_CHAIN
                    .into_iter()
                    .find(|tech| !g.players[pid].techs.contains(&crate::name::Name::new(tech)))
            })
            .flatten()
    }

    /// A coastal Science seat may take Harbor infrastructure while the next
    /// victory technology is still at least two world eras away. During the
    /// development half this is a soft opening: more than one Harbor may be
    /// worthwhile when the map and city count support it. Once specialization
    /// begins, the research exception is bounded to the first Harbor and then
    /// returns to the first unknown Science victory technology. Holy Sites are
    /// never part of this support package; Astrology is only the prerequisite
    /// needed by Harbor.
    pub(super) fn science_harbor_research_goal(
        &self,
        g: &Game,
        pid: usize,
        objective: GrandStrategy,
    ) -> Option<&'static str> {
        const MIN_WORLD_ERA: usize = 2;
        const MIN_VICTORY_TECH_ERA_GAP: usize = 2;

        if objective != GrandStrategy::Science
            || g.world_era < MIN_WORLD_ERA
            || !crate::ai::BasicAi::empire_is_coastal(g, pid)
            || (self.phase_specialization_active(g) && Self::science_harbor_reserved(g, pid))
            || g.players[pid]
                .techs
                .contains(&crate::name!("celestial_navigation"))
        {
            return None;
        }
        let victory_goal = Self::science_victory_tech_goal(g, pid, objective)?;
        let victory_era = g.rules.techs.get(victory_goal)?.era;
        (victory_era.saturating_sub(g.world_era) >= MIN_VICTORY_TECH_ERA_GAP)
            .then_some("celestial_navigation")
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
                player.id != pid
                    && !g.same_team(pid, player.id)
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
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
        if self.domination_counter_pressure(g, pressure)
            || self.domination_faithless_conversion_counter(g, pid, rival, pressure)
        {
            // Faith purchases and home religious defense still run; this only
            // keeps the campaign in its assigned military lane.
            return Some(GrandStrategy::Conquest);
        }
        // Congress awards Diplomatic Victory Points in jumps. One jump must
        // not be projected into an imminent win while a Domination army is
        // already at a required capital's walls: that diverted production
        // from the Munich siege at nine of twenty points in the live King
        // game civvis-20260929T011038Z. Keep the projected warning for an
        // empty front; the observed sixteen-point stock alarm still wins.
        if pressure.strategy == GrandStrategy::Diplomacy
            && pressure.progress < 78
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.cities.values().any(|city| {
                city.owner != pid
                    && city.is_capital
                    && city.wall_hp > 0
                    && g.is_at_war(pid, city.owner)
                    && Self::domination_siege_present(g, pid, city.id)
            })
        {
            return None;
        }
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
        } else if !urgent && (pressure.progress < 78 || pressure.progress < own_progress + 15) {
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
            // ⭐ CULTURE IS THE ONE LANE THAT CANNOT ANSWER A LEADER WITH WAR.
            //
            // Every other arm here reaches Conquest: Science does unless
            // `counter-in-lane` holds it in lane, a Religion threat does
            // whenever we have no faith of our own, and a score leader does on
            // the same terms as Science. Culture alone answers a rival about to
            // win by racing them, with no gene to choose otherwise — and it is
            // the lane the live ladder actually loses to: six rival culture
            // finishes between standard turns 155 and 208 in the recorded
            // Emperor games.
            //
            // Racing is not nothing — our own domestic tourists are the bar the
            // rival has to clear, so building culture raises it. But it is a
            // race against a leader who is already ahead, and it gives up the
            // one counter with a double effect: **capturing a city takes its
            // Great Works**, which removes that tourism from the rival and adds
            // it to us in the same action. No other lane's counter does that,
            // and a culture leader is the rival likeliest to have spent its
            // production on Theatre Squares rather than on an army.
            //
            // With the gene on, Culture reaches Conquest on the same urgency
            // bar as the other lanes, and `victory_suppression_city` aims the
            // campaign at the Great Works. The actionable pass still has to
            // agree the war is executable, exactly as it does for Science.
            GrandStrategy::Culture if self.counter_culture_by_conquest => GrandStrategy::Conquest,
            GrandStrategy::Culture => GrandStrategy::Culture,
            GrandStrategy::Religion if g.players[pid].religion.is_some() => GrandStrategy::Religion,
            GrandStrategy::Religion => GrandStrategy::Conquest,
            GrandStrategy::Diplomacy => GrandStrategy::Diplomacy,
            GrandStrategy::Conquest => GrandStrategy::Recovery,
            // A score deadline must not park an explicitly assigned Domination
            // army in Expansion. Keep the existing urgency, stand-down and
            // executable-campaign gates; this selects a plan, not a declaration.
            GrandStrategy::Expansion
                if urgent && self.active_victory_target(g) == Some(VictoryTarget::Domination) =>
            {
                GrandStrategy::Conquest
            }
            GrandStrategy::Expansion if self.counter_in_lane => GrandStrategy::Expansion,
            GrandStrategy::Expansion => GrandStrategy::Conquest,
            GrandStrategy::Recovery => GrandStrategy::Recovery,
        })
    }

    /// The action planner must choose the highest-priority threat it can
    /// actually address, not abandon all denial because the nominal leader is
    /// un-actionable.  Reporting continues to expose that raw leader through
    /// `victory_denial`; only military/campaign routing uses this fallthrough.
    ///
    /// `launcher-war-serves-every-lane`: off Domination, a counter that is
    /// not already a war (an in-lane race, or none) yields to the racer the
    /// launcher war would open on. `None` from the gene with it off. See
    /// `advanced/launcher_war_every_lane.rs`.
    pub(super) fn actionable_victory_denial_with_culture_pressures(
        &self,
        g: &Game,
        pid: usize,
        culture_pressures: &BTreeMap<usize, i32>,
    ) -> Option<(usize, GrandStrategy)> {
        let chosen = self.lane_victory_denial_with_culture_pressures(g, pid, culture_pressures);
        if chosen.is_none_or(|(_, counter)| counter != GrandStrategy::Conquest) {
            if let Some(counter) = self.launcher_lane_counter(g, pid) {
                return Some(counter);
            }
        }
        chosen
    }

    fn lane_victory_denial_with_culture_pressures(
        &self,
        g: &Game,
        pid: usize,
        culture_pressures: &BTreeMap<usize, i32>,
    ) -> Option<(usize, GrandStrategy)> {
        if !self.deny_leaders {
            return None;
        }
        // This seat's victory focus and every rival's pressure below read the
        // same city yields; one memo derives them once.
        let _memo = g.query_memo();
        let targeted = self.active_victory_target(g).is_some();
        let own_progress = self.victory_focus(g, pid).progress;
        let ranked = self.ranked_rival_victory_pressures(g, pid, culture_pressures);
        if let Some(counter) = self.domination_military_counter(g, pid, own_progress, &ranked) {
            return Some(counter);
        }
        let actionable = |rival: usize, pressure: VictoryFocus| -> Option<GrandStrategy> {
            let domination_counter = self.domination_counter_pressure(g, pressure)
                || self.domination_faithless_conversion_counter(g, pid, rival, pressure);
            if targeted
                && !domination_counter
                && (!self.deny_while_targeted
                    || !self.victory_pressure_is_urgent(g, rival, pressure))
            {
                return None;
            }
            let counter =
                self.denial_response_for_pressure(g, pid, own_progress, rival, pressure)?;
            (self.conquest_denial_actionable(g, pid, rival, counter)
                && self.culture_denial_actionable(g, pid, rival, counter))
            .then_some(counter)
        };
        let chosen = ranked.iter().find_map(|(rival, pressure)| {
            actionable(*rival, *pressure).map(|counter| (*rival, counter, pressure.progress))
        });
        // `denial-keeps-its-rival`: last turn's counter rival keeps the
        // counter while it is still actionable and the new leader does not
        // lead it by [`DENIAL_SWAP_MARGIN`]. Every swap moves the Domination
        // army: 58 live games of October 4-5 bounced the campaign A -> B -> A
        // within ten turns 89 times, 53 of them while countering a rival
        // close to winning. Live King civvis-20261004T150335Z offered
        // Vietnam peace at turn 123 "to counter a rival victory threat",
        // declared on Indonesia, and was staging against Vietnam again at
        // 132; civvis-20261004T114858Z-cont1 moved its front from Byzantium
        // to France, 21 tiles off, at 156 and back at 161.
        if self.denial_keeps_its_rival {
            if let (Some((rival, _, progress)), Some(incumbent)) = (chosen, self.denial_incumbent) {
                if rival != incumbent {
                    let kept = ranked
                        .iter()
                        .find(|(other, _)| *other == incumbent)
                        .filter(|(_, held)| progress < held.progress + super::DENIAL_SWAP_MARGIN)
                        .and_then(|(_, held)| actionable(incumbent, *held));
                    if let Some(counter) = kept {
                        return Some((incumbent, counter));
                    }
                }
            }
        }
        chosen.map(|(rival, counter, _)| (rival, counter))
    }

    /// A Domination army answers with war, so it serves the most advanced
    /// clock it can take a city against before any in-lane counter ranked
    /// above it: a Congress or culture race cannot stop a launch chain. A
    /// rival's launch ladder is read on its own, because its strongest lane
    /// can hide it. On King `civvis-20260930T211803Z` the Maori and Ottoman
    /// Diplomatic Victory points (80 and 75) held the plan on "diplomacy"
    /// from turn 222 while Portugal, past its Mars launch, flew at 242 and won
    /// at 248 without a war.
    fn domination_military_counter(
        &self,
        g: &Game,
        pid: usize,
        own_progress: i32,
        ranked: &[(usize, VictoryFocus)],
    ) -> Option<(usize, GrandStrategy)> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination) {
            return None;
        }
        // `science-suppression-hits-the-pads`: a rival past its Mars base and
        // short of the Exoplanet launch is the counter before any other
        // clock. `None` with the gene off. See
        // `advanced/science_suppression_pads.rs`.
        if let Some(counter) = self.mars_racer_counter(g, pid) {
            return Some(counter);
        }
        let mut clocks: Vec<(usize, VictoryFocus)> = Vec::new();
        for (rival, pressure) in ranked.iter().copied() {
            if self.domination_counter_pressure(g, pressure)
                || self.domination_faithless_conversion_counter(g, pid, rival, pressure)
            {
                clocks.push((rival, pressure));
            }
            let launches = VictoryFocus {
                strategy: GrandStrategy::Science,
                progress: Self::science_launch_progress(g, rival),
            };
            if pressure.strategy != GrandStrategy::Science
                && self.domination_counter_pressure(g, launches)
            {
                clocks.push((rival, launches));
            }
            // `runaway-expander-counter`: see `advanced/runaway_expander.rs`.
            if let Some(runaway) = self.runaway_expander_clock(g, pid, rival) {
                clocks.push((rival, runaway));
            }
            // `denial-nearest-finish`: a culture race near its finish, read
            // by how soon it ends. See `advanced/denial_nearest_finish.rs`.
            if let Some(culture) = self.nearest_finish_culture_clock(g, rival).filter(|clock| {
                (pressure.strategy != GrandStrategy::Culture || clock.progress > pressure.progress)
                    && self.domination_counter_pressure(g, *clock)
            }) {
                clocks.push((rival, culture));
            }
        }
        clocks.sort_by(|left, right| {
            right
                .1
                .progress
                .cmp(&left.1.progress)
                .then_with(|| left.0.cmp(&right.0))
        });
        clocks.into_iter().find_map(|(rival, pressure)| {
            let counter =
                self.denial_response_for_pressure(g, pid, own_progress, rival, pressure)?;
            (counter == GrandStrategy::Conquest
                && self.conquest_denial_actionable(g, pid, rival, counter)
                && self.culture_denial_actionable(g, pid, rival, counter))
            .then_some((rival, counter))
        })
    }

    /// A Domination contract is fulfilled by foreign *original* capitals.
    /// An eligible known capital supplies both the next opponent and its city
    /// objective. The ordinary war policy still gates the declaration.
    pub(super) fn domination_capital_target(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        self.domination_capital_target_for(g, pid, None)
    }

    /// If taking a rival's original capital ends the match immediately, that
    /// capture is also the answer to the rival's victory clock. Use the same
    /// completion predicate as occupation safety so an unseen or lost home
    /// capital cannot create a false "last capital" shortcut.
    pub(super) fn domination_finishing_capital_for(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<u32> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !self.campaign_target_legal(g, pid, rival)
        {
            return None;
        }
        g.cities
            .values()
            .find(|city| {
                city.owner == rival
                    && city.is_capital
                    && Self::capture_completes_domination(g, pid, city.id)
            })
            .map(|city| city.id)
    }

    /// The rival we are at war with, and its capital, when taking that
    /// capital completes Domination. `domination_finish` otherwise looks only
    /// at the rival the denial layer names. Live King
    /// civvis-20261004T160213Z (game 65) took Aachen at turn 218, leaving
    /// the Maori capital as the last one Domination needed. The Maori, at war
    /// with us, held 3 cities at 59 to 125 power against our 2,800, and 16
    /// Diplomatic Victory points. The denial layer still named Germany, so
    /// the campaign stayed on Frankfurt and Cologne.
    pub(super) fn domination_finish_at_war(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid && rival.alive && !rival.is_minor && !rival.is_barbarian
            })
            .filter(|rival| g.is_at_war(pid, rival.id))
            .find_map(|rival| {
                self.domination_finishing_capital_for(g, pid, rival.id)
                    .map(|city| (rival.id, city))
            })
    }

    /// `flipped-capital-finishes`: the Free Cities seat and the original
    /// capital it holds whose capture completes Domination. The Free Cities
    /// are always at war with us, but their seat is barbarian-flagged, so
    /// `domination_finish_at_war`, `campaign_target_legal` and the wartime
    /// front sets skip it and a capital that flipped to them was never the
    /// objective again. Live King civvis-20261005T193504Z (game 156) held
    /// Canberra and Washington from 174 to 181 with Mikisiw-Wacîhk, the
    /// last capital, a Free City since 172; then Canberra and Mikisiw from 188
    /// with Washington a Free City since 182. The campaign sieged Los Angeles,
    /// Pihtokahanapiwiyin and Mistawasis.
    pub(super) fn free_city_finish(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        if !self.flipped_capital_finishes
            || self.forced_target_player.is_some()
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        g.cities
            .values()
            .filter(|city| {
                city.is_capital
                    && city.owner != pid
                    && g.players
                        .get(city.owner)
                        .is_some_and(|owner| owner.is_free_city)
                    && g.is_at_war(pid, city.owner)
                    && Self::capture_completes_domination(g, pid, city.id)
            })
            .map(|city| (city.owner, city.id))
            .min()
    }

    /// Rank required capitals inside the selected front as well as globally.
    /// A different rival owning the cheapest capital must not erase this
    /// front's capital objective and send the army after an ordinary city.
    pub(super) fn domination_capital_target_for(
        &self,
        g: &Game,
        pid: usize,
        target: Option<usize>,
    ) -> Option<(usize, u32)> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination) {
            return None;
        }
        g.cities
            .values()
            .filter(|city| city.is_capital && city.owner != pid && !g.same_team(pid, city.owner))
            .filter(|city| target.is_none_or(|owner| city.owner == owner))
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
                    self.conversion_campaign_cost(g, pid, city)
                        .map(|cost| cost * 15.0)
                        .unwrap_or_else(|| {
                            self.campaign_city_value(g, pid, city, GrandStrategy::Conquest)
                        }),
                )
            })
            .min_by(|left, right| {
                // `domination-targets-land-first`: a capital we can march to
                // before one across water. See
                // `advanced/domination_land_first.rs`.
                self.domination_land_rank(g, pid, left.0)
                    .cmp(&self.domination_land_rank(g, pid, right.0))
                    .then_with(|| left.2.total_cmp(&right.2))
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
        // `science-suppression-hits-the-pads`: a decisive space racer is
        // suppressed at a standing pad, whatever lane it reads. `None` with
        // the gene off. See `advanced/science_suppression_pads.rs`.
        if let Some(city) = self.suppression_pad_city(g, pid, rival) {
            return Some(city);
        }
        if pressure.progress < 78
            && !self.domination_counter_pressure(g, pressure)
            && !self.domination_faithless_conversion_counter(g, pid, rival, pressure)
        {
            return None;
        }
        let district = match pressure.strategy {
            GrandStrategy::Science => crate::name!("spaceport"),
            GrandStrategy::Religion => crate::name!("holy_site"),
            // The Theatre Square is where a culture leader's Great Works are
            // slotted, so it is the same kind of concrete bottleneck the other
            // two arms name — and the only one whose capture moves the tourism
            // rather than merely stopping it. Domination uses this counter
            // directly; other lanes opt in with `counter-culture-by-conquest`.
            GrandStrategy::Culture
                if self.counter_culture_by_conquest
                    || self.domination_counter_pressure(g, pressure) =>
            {
                crate::name!("theater_square")
            }
            _ => return None,
        };
        g.cities
            .values()
            .filter(|city| city.owner == rival)
            .filter(|city| city.districts.contains_key(district))
            // The first objective must pass the declaration's own range
            // gate. A distant victory district otherwise shadows a usable
            // frontier and sends the army toward a war it cannot open.
            // Once at war, retain distant infrastructure as a valid target.
            .filter(|city| {
                g.is_at_war(pid, rival) || Self::city_within_declaration_range(g, pid, city.pos)
            })
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
#[path = "domination_score_counter_tests.rs"]
mod domination_score_counter_tests;

#[cfg(test)]
#[path = "domination_counters/tests.rs"]
mod domination_counter_tests;

#[cfg(test)]
#[path = "native_science_clock_tests.rs"]
mod native_science_clock_tests;

#[cfg(test)]
#[path = "denial_frontier_tests.rs"]
mod denial_frontier_tests;

#[cfg(test)]
mod tests;
