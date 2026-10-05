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
    pub(super) fn actionable_victory_denial_with_culture_pressures(
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
                    && g.players.get(city.owner).is_some_and(|owner| owner.is_free_city)
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
mod tests {
    use super::super::StrategicPlan;
    use super::*;
    use crate::{
        ai::BasicAi,
        game::{Game, LiveGreatPersonActivationNeed, ObservedPublicEmpireStats},
    };

    pub(super) fn found_capitals(game: &mut Game) {
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

    pub(super) fn open_land_near(game: &Game, center: crate::Pos, radius: i32) -> crate::Pos {
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

    /// `beeline-orders-by-value`: the Science beeline walks Rocketry's
    /// prerequisites by `tech_value`, not by printed price with ties by name.
    /// Off, the three 50-cost Ancient steps go archery, sailing, writing —
    /// alphabetical — whatever they unlock; on, the step is the value argmax
    /// over the same chain steps, and the goal is unchanged.
    #[test]
    fn beeline_orders_by_value_walks_the_chain_by_value() {
        let mut game = Game::new_full(1, 24, 16, 91_007, 300, 0, false);
        found_capitals(&mut game);
        game.turn = 8;
        for tech in ["pottery", "animal_husbandry", "mining"] {
            game.players[0].techs.insert(crate::name::Name::new(tech));
        }
        let plan = science_plan(game.turn);
        let steps: Vec<crate::name::Name> = crate::ai::BasicAi::era_window_techs(&game, 0)
            .into_iter()
            .filter(|tech| AdvancedAi::new().tech_leads_to(&game, tech, "rocketry"))
            .collect();
        assert!(
            steps.len() >= 3,
            "the fixture offers several chain steps: {steps:?}"
        );

        let off = AdvancedAi::targeting(VictoryTarget::Science);
        assert!(!off.beeline_orders_by_value, "the gene ships off");
        let mut off_game = game.clone();
        off_game.players[0].research = None;
        off.advanced_research(&mut off_game, 0, &plan);
        let cheapest = steps
            .iter()
            .min_by(|a, b| {
                off_game.rules.techs[a.as_str()]
                    .cost
                    .total_cmp(&off_game.rules.techs[b.as_str()].cost)
                    .then(a.cmp(b))
            })
            .copied();
        assert_eq!(
            off_game.players[0].research.as_deref(),
            cheapest.as_ref().map(|tech| tech.as_str()),
            "off, the beeline takes the cheapest step, ties by name"
        );

        let mut on = AdvancedAi::targeting(VictoryTarget::Science);
        on.enable_beeline_orders_by_value();
        let best = steps
            .iter()
            .map(|tech| (on.tech_value(&game, 0, tech.as_str(), plan.strategy), *tech))
            .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
            .map(|(_, tech)| tech);
        let mut on_game = game.clone();
        on_game.players[0].research = None;
        on.advanced_research(&mut on_game, 0, &plan);
        assert_eq!(
            on_game.players[0].research.as_deref(),
            best.as_ref().map(|tech| tech.as_str()),
            "on, the best-valued chain step"
        );
        assert_ne!(
            best, cheapest,
            "the fixture must separate the two orders, or it proves nothing"
        );
        assert_eq!(
            AdvancedAi::science_victory_tech_goal(&on_game, 0, GrandStrategy::Science),
            AdvancedAi::science_victory_tech_goal(&off_game, 0, GrandStrategy::Science),
            "the goal itself is the same either way"
        );
        on.disable_beeline_orders_by_value();
        assert!(!on.beeline_orders_by_value);
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.tag == "beeline-orders-by-value" && gene.opt_in()));
    }

    #[test]
    fn science_target_backfills_an_unfinished_ancient_tech_before_rocketry() {
        let mut game = Game::new_full(1, 24, 16, 91_001, 300, 0, false);
        found_capitals(&mut game);
        game.turn = game.max_turns / 2;

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
                ..LiveGreatPersonActivationNeed::default()
            });
        assert_eq!(
            BasicAi::live_great_person_tech_goal(&game, 0).as_deref(),
            Some("bronze_working"),
            "the fixture needs an unrelated live Great Person technology goal"
        );

        let plan = science_plan(game.turn);
        ai.advanced_research(&mut game, 0, &plan);

        // Any unfinished Ancient node is the backfill. This used to name
        // Bronze Working, which won the scorer only because its lookahead
        // counted other civilizations' Iron Working uniques (the Kongo Shield
        // Bearer, Hypaspist and Toa); counted honestly, Irrigation edges it.
        let picked = game.players[0].research.clone().expect("a research pick");
        assert!(
            picked != "rocketry" && game.rules.techs[picked.as_str()].era == 0,
            "an explicit Science target must clear an unfinished Ancient era before its Rocketry beeline, not {picked}"
        );
    }

    #[test]
    fn domination_capital_focus_is_an_independently_reversible_opt_in() {
        assert!(!AdvancedAi::new().domination_capital_focus);
        assert!(!AdvancedAi::legacy().domination_capital_focus);
        let mut ai = AdvancedAi::new();
        ai.enable_domination_capital_focus();
        assert!(ai.domination_capital_focus);
        ai.disable_domination_capital_focus();
        assert!(!ai.domination_capital_focus);
        assert!(super::super::GENES
            .iter()
            .any(|gene| gene.tag == "domination-capital-focus" && gene.opt_in()));
    }

    #[test]
    fn domination_target_aims_at_an_uncontrolled_original_capital_before_a_convenient_city() {
        check_domination_front_capital(2);
    }

    #[test]
    fn domination_target_keeps_the_active_fronts_capital_when_another_is_cheaper() {
        check_domination_front_capital(3);
    }

    fn check_domination_front_capital(majors: usize) {
        let mut game = Game::new_full(majors, 48, 28, 91_002, 300, 0, false);
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
        if majors == 3 {
            // A nearby friendly population base makes the capital holdable;
            // the rich enemy outpost remains the generic scorer's bargain.
            let support = game.found_city_for(0, open_land_near(&game, capital_pos, 2), None);
            game.cities.get_mut(&support).unwrap().pop = 30;
            let home = game.cities[&game.player_city_ids(0)[0]].pos;
            for _ in 0..6 {
                game.spawn_test_unit("giant_death_robot", 0, home);
            }
        }
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

        if majors == 3 {
            game.record_contact(0, 2);
            game.at_war.insert((0, 1));
            assert_eq!(
                ai.domination_capital_target(&game, 0)
                    .map(|(owner, _)| owner),
                Some(2),
                "the unengaged rival must own the cheaper global capital"
            );
        }
        if majors == 3 {
            assert_eq!(
                ai.assess(&game, 0).target_city,
                Some(outpost),
                "the off arm keeps the existing generic-city fallback"
            );
            ai.enable_domination_capital_focus();
        }
        let plan = ai.assess(&game, 0);
        assert_eq!(
            plan.strategy,
            if majors == 3 {
                // The forward support city is exposed to the garrison.
                // Retain the defensive posture while naming this front.
                GrandStrategy::Recovery
            } else {
                GrandStrategy::Conquest
            }
        );
        assert_eq!(plan.target_player, Some(1));
        assert_eq!(plan.target_city, Some(capital));
    }

    #[test]
    fn domination_capital_routing_uses_current_ownership_and_includes_home_capital() {
        let mut game = Game::new_full(3, 48, 28, 91_006, 300, 0, false);
        found_capitals(&mut game);
        game.turn = 200;
        game.record_contact(0, 1);
        game.record_contact(0, 2);
        let taken = game.player_city_ids(1)[0];
        let outpost =
            game.found_city_for(1, open_land_near(&game, game.cities[&taken].pos, 4), None);
        game.cities.get_mut(&taken).unwrap().owner = 0;
        game.cities.get_mut(&outpost).unwrap().pop = 20;
        let remaining = game.player_city_ids(2)[0];
        let ai = AdvancedAi::targeting(VictoryTarget::Domination);
        assert_eq!(
            ai.domination_capital_target_for(&game, 0, Some(2)),
            Some((2, remaining))
        );
        assert_eq!(ai.domination_capital_target_for(&game, 0, Some(1)), None);

        // Another conqueror can hold multiple original capitals. Route to
        // their present owner, including when our own capital needs retaking.
        let home = game
            .cities
            .values()
            .find(|c| c.original_owner == 0)
            .unwrap()
            .id;
        game.cities.get_mut(&home).unwrap().owner = 2;
        game.cities.get_mut(&remaining).unwrap().owner = 0;
        assert_eq!(
            ai.domination_capital_target_for(&game, 0, Some(2)),
            Some((2, home)),
            "our own lost original capital is still required"
        );
        assert_eq!(ai.domination_capital_target_for(&game, 0, Some(1)), None);
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
        let spaceport_city = game.found_city_for(
            1,
            open_land_near(&game, game.cities[&game.player_city_ids(0)[0]].pos, 10),
            Some("Launch Complex".to_string()),
        );
        assert!(AdvancedAi::city_within_declaration_range(
            &game,
            0,
            game.cities[&spaceport_city].pos
        ));
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

    /// Four majors at turn 190: rival 1 leads Culture but cannot be reached,
    /// rival 3 has launched the Exoplanet Expedition. `stock_denial_lead_time`
    /// makes that Science clock an actionable denial against player 3.
    fn launched_science_threat_board() -> Game {
        let mut game = Game::new_full(4, 48, 28, 91_004, 300, 0, false);
        found_capitals(&mut game);
        game.turn = 190;
        for rival in 1..4 {
            game.record_contact(0, rival);
        }
        std::sync::Arc::make_mut(&mut game.observed_public_empire_stats).insert(
            0,
            ObservedPublicEmpireStats {
                domestic_tourists: Some(10),
                ..ObservedPublicEmpireStats::default()
            },
        );
        std::sync::Arc::make_mut(&mut game.observed_public_empire_stats).insert(
            1,
            ObservedPublicEmpireStats {
                foreign_tourists: Some(150),
                ..ObservedPublicEmpireStats::default()
            },
        );
        std::sync::Arc::make_mut(&mut game.observed_public_empire_stats).insert(
            2,
            ObservedPublicEmpireStats {
                domestic_tourists: Some(150),
                ..ObservedPublicEmpireStats::default()
            },
        );
        std::sync::Arc::make_mut(&mut game.observed_public_empire_stats).insert(
            3,
            ObservedPublicEmpireStats {
                domestic_tourists: Some(20),
                ..ObservedPublicEmpireStats::default()
            },
        );
        game.players[3]
            .science_projects
            .insert("exoplanet_expedition".to_string());

        game
    }

    #[test]
    fn an_unactionable_leader_does_not_mask_an_actionable_victory_threat() {
        let game = launched_science_threat_board();
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

    /// `denial-outranks-expansion`: the same board, but the seat has a lane.
    /// One city against a target of several keeps the lane's branch on
    /// "Expansion", and that branch sits above the denial in the chain — so
    /// with the gene off the launched rival is never answered. With it on the
    /// denial is asked first and the plan is the same Conquest against player
    /// 3 that a lane-less seat already chooses.
    #[test]
    fn a_rival_close_to_winning_outranks_the_lanes_expansion_only_with_the_gene() {
        let game = launched_science_threat_board();
        // A Culture lane, so the off-case cannot land on Conquest-against-3 by
        // way of the lane's own strategy; the live seat's Domination lane hit
        // the same Expansion branch for the same reason.
        let mut lane = AdvancedAi::targeting(VictoryTarget::Culture);
        lane.stock_denial_lead_time = true;
        // `deny-while-targeted` is what lets a seat with a lane see a denial at
        // all; this gene is about what happens to that denial once it exists.
        lane.deny_while_targeted = true;
        assert_eq!(
            lane.actionable_victory_denial(&game, 0),
            Some((3, GrandStrategy::Conquest)),
            "the denial is actionable regardless of the lane"
        );
        let plan = lane.assess(&game, 0);
        assert_ne!(
            (plan.strategy, plan.target_player),
            (GrandStrategy::Conquest, Some(3)),
            "with the gene off the lane's expansion rule shadows the denial: {plan:?}"
        );

        lane.enable_denial_outranks_expansion();
        let plan = lane.assess(&game, 0);
        assert_eq!(plan.strategy, GrandStrategy::Conquest);
        assert_eq!(
            plan.target_player,
            Some(3),
            "the denial is answered ahead of the lane"
        );
    }
    // -----------------------------------------------------------------
    // `counter-culture-by-conquest`
    // -----------------------------------------------------------------

    fn culture_threat(progress: i32) -> VictoryFocus {
        VictoryFocus {
            strategy: GrandStrategy::Culture,
            progress,
        }
    }

    /// Two majors with capitals, and a rival Theatre Square to aim at.
    fn culture_board() -> (Game, u32) {
        let mut game = Game::new_full(2, 24, 16, 91_777, 300, 0, false);
        found_capitals(&mut game);
        game.turn = game.max_turns / 2;
        let rival_city = game
            .player_city_ids(1)
            .into_iter()
            .next()
            .expect("the rival founded a capital");
        game.cities
            .get_mut(&rival_city)
            .unwrap()
            .districts
            .insert(crate::name!("theater_square"), Default::default());
        (game, rival_city)
    }

    #[test]
    fn domination_lane_hands_over_is_a_native_opt_in_off_in_both_controllers() {
        super::super::test_support::opt_in_off_in_both_controllers(
            "domination-lane-hands-over",
            |ai| ai.domination_lane_hands_over,
        );
    }

    /// `domination-lane-hands-over`: four cities on a board whose lane target is
    /// higher. Gene off, the lane keeps reading Expansion — the branch that held
    /// the live seat at 8 and 9 cities. Gene on, it follows its lane to war.
    #[test]
    fn a_domination_lane_with_the_opening_band_in_hand_goes_to_war_only_with_the_gene() {
        let mut game = Game::new_full(4, 48, 28, 91_004, 300, 0, false);
        found_capitals(&mut game);
        game.turn = 120;
        for rival in 1..4 {
            game.record_contact(0, rival);
        }
        let capital = game.player_city_ids(0)[0];
        let center = game.cities[&capital].pos;
        let mut radius = 4;
        while game.player_city_ids(0).len() < super::super::DOMINATION_HANDOVER_CITIES {
            let site = open_land_near(&game, center, radius);
            game.found_city_for(0, site, None);
            radius += 1;
            assert!(radius < 12, "could not place four cities near {center:?}");
        }
        assert_eq!(
            game.player_city_ids(0).len(),
            super::super::DOMINATION_HANDOVER_CITIES
        );

        let mut lane = AdvancedAi::targeting(VictoryTarget::Domination);
        // The live seat's forced pair, which is what grows the target past the
        // land: 6 wanted, then 10, on a map that holds five or six.
        lane.enable_rapid_city_expansion_2();
        lane.enable_expansion_scales_with_difficulty();
        let plan = lane.assess(&game, 0);
        assert!(
            plan.desired_cities > super::super::DOMINATION_HANDOVER_CITIES,
            "the fixture must sit under the lane's own target: {plan:?}"
        );
        assert_eq!(
            plan.strategy,
            GrandStrategy::Expansion,
            "gene off: the lane waits for a target the map may never meet: {plan:?}"
        );

        lane.enable_domination_lane_hands_over();
        // ⚠ Four cities alone are not enough: the hand-over also needs an army
        // that clears the elective-war bar against the weakest rival. Game 6
        // declared at power 198 vs 384 and was down to one city by t170.
        let mut outgunned = game.clone();
        for rival in 1..4 {
            let capital = outgunned.player_city_ids(rival)[0];
            let around = outgunned.cities[&capital].pos;
            for _ in 0..8 {
                let site = open_land_near(&outgunned, around, 2);
                outgunned.spawn_test_unit("swordsman", rival, site);
            }
        }
        let plan = lane.assess(&outgunned, 0);
        assert_eq!(
            plan.strategy,
            GrandStrategy::Expansion,
            "gene on but outgunned: no war at half the rivals' power: {plan:?}"
        );

        for _ in 0..8 {
            let site = open_land_near(&game, center, 2);
            game.spawn_test_unit("swordsman", 0, site);
        }
        let plan = lane.assess(&game, 0);
        assert_eq!(
            plan.strategy,
            GrandStrategy::Conquest,
            "gene on with the army to back it: four cities in hand, the lane goes to war: {plan:?}"
        );
    }

    #[test]
    fn denial_outranks_expansion_is_a_native_opt_in_off_in_both_controllers() {
        super::super::test_support::opt_in_off_in_both_controllers(
            "denial-outranks-expansion",
            |ai| ai.denial_outranks_expansion,
        );
    }

    #[test]
    fn counter_culture_by_conquest_is_a_native_opt_in_off_in_both_controllers() {
        super::super::test_support::opt_in_off_in_both_controllers(
            "counter-culture-by-conquest",
            |ai| ai.counter_culture_by_conquest,
        );
    }

    /// The premise: every other lane can answer a leader with war, and Culture
    /// cannot.
    #[test]
    fn culture_is_the_only_lane_that_never_reaches_conquest() {
        let (game, _) = culture_board();
        let ai = AdvancedAi::new();
        let urgent = 95;
        for (strategy, expected) in [
            (GrandStrategy::Science, GrandStrategy::Conquest),
            (GrandStrategy::Religion, GrandStrategy::Conquest),
            (GrandStrategy::Expansion, GrandStrategy::Conquest),
            (GrandStrategy::Culture, GrandStrategy::Culture),
        ] {
            let pressure = VictoryFocus {
                strategy,
                progress: urgent,
            };
            assert_eq!(
                ai.denial_response_for_pressure(&game, 0, 0, 1, pressure),
                Some(expected),
                "{strategy:?} answers a leader this way with the gene off"
            );
        }
    }

    #[test]
    fn the_gene_answers_a_culture_leader_with_war() {
        let (game, _) = culture_board();
        let mut ai = AdvancedAi::new();
        ai.enable_counter_culture_by_conquest();
        assert_eq!(
            ai.denial_response_for_pressure(&game, 0, 0, 1, culture_threat(95)),
            Some(GrandStrategy::Conquest)
        );
    }

    /// The urgency bar is the shipped one and the gene does not lower it.
    #[test]
    fn a_culture_rival_below_the_bar_is_still_no_threat_either_way() {
        let (game, _) = culture_board();
        let mut ai = AdvancedAi::new();
        assert_eq!(
            ai.denial_response_for_pressure(&game, 0, 0, 1, culture_threat(50)),
            None
        );
        ai.enable_counter_culture_by_conquest();
        assert_eq!(
            ai.denial_response_for_pressure(&game, 0, 0, 1, culture_threat(50)),
            None,
            "the gene changes the answer, never the bar"
        );
    }

    #[test]
    fn the_campaign_is_aimed_at_the_rivals_great_works() {
        let (game, rival_city) = culture_board();
        let mut ai = AdvancedAi::new();
        assert_eq!(
            ai.victory_suppression_city(&game, 0, 1, culture_threat(95)),
            None,
            "off, a Culture threat names no suppression city"
        );
        ai.enable_counter_culture_by_conquest();
        assert_eq!(
            ai.victory_suppression_city(&game, 0, 1, culture_threat(95)),
            Some(rival_city),
            "on, the Theatre Square city is the campaign's first target"
        );
    }

    /// The other two arms are untouched, gene or no gene.
    #[test]
    fn the_science_and_religion_suppression_arms_are_unchanged() {
        let (mut game, rival_city) = culture_board();
        game.cities
            .get_mut(&rival_city)
            .unwrap()
            .districts
            .insert(crate::name!("spaceport"), Default::default());
        let mut ai = AdvancedAi::new();
        let science = VictoryFocus {
            strategy: GrandStrategy::Science,
            progress: 95,
        };
        assert_eq!(
            ai.victory_suppression_city(&game, 0, 1, science),
            Some(rival_city)
        );
        ai.enable_counter_culture_by_conquest();
        assert_eq!(
            ai.victory_suppression_city(&game, 0, 1, science),
            Some(rival_city)
        );
    }
}
