//! The diplomatic turn: how `AdvancedAi` deals with other civilizations.
//!
//! `advanced_diplomacy` runs it: deals in and out (`incoming_deal_value`,
//! `strategic_bilateral_trade`), alliances (`propose_strategic_alliance`),
//! and when to open a war (`timed_war_opening`, `preferred_war_opening`,
//! `strike_when_staged` and the staging checks behind them).
//! `congress_choice` votes in the World Congress, `advanced_envoys` places
//! envoys with city-states, and `advanced_spies` runs the spy corps.
//!
//! Moved verbatim out of the one `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-04). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged.

use super::*;

impl AdvancedAi {
    fn timed_war_launch_ready(
        &self,
        g: &Game,
        pid: usize,
        plan: &WarPlan,
        status: &WarPackageStatus,
    ) -> bool {
        g.players[pid].techs.contains(&plan.breakthrough_tech)
            && status.ready_bodies >= TIMED_WAR_BODIES
            && status.breach_ready
            && self.timed_war_staging_ready(status)
            && self.threatened_city(g, pid).is_none()
    }

    /// `denial-nearest-finish`: a rival whose victory is urgent, that our
    /// military outweighs [`DENIAL_FAR_REACH_RATIO`] times over, is worth a
    /// march past the ordinary 18-tile declaration range, out to
    /// [`DENIAL_FAR_REACH_TILES`]. A G24 replay (civvis-20261003T060034Z)
    /// retargeted Sweden's culture race at turn 194 at 1,567 power against 81
    /// and stopped on "no city of theirs is within 18 tiles of one of mine";
    /// Sweden won at turn 205.
    pub(super) fn denial_reaches_far(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        objective: Pos,
    ) -> bool {
        self.denial_nearest_finish
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && self.urgent_victory_threat(g, target)
            && g.military_power(pid) >= DENIAL_FAR_REACH_RATIO * g.military_power(target).max(1.0)
            && g.player_city_ids(pid)
                .iter()
                .any(|city| g.wdist(g.cities[city].pos, objective) <= DENIAL_FAR_REACH_TILES)
    }

    /// How many votes to put behind this turn's Congress choice.
    ///
    /// The shipped rule reads the posture of the turn the session sits on.
    /// See `lane_votes_its_favor` for why an assigned diplomatic lane is
    /// usually in some other posture when that happens.
    pub(super) fn congress_ballot_size(
        &self,
        g: &Game,
        pid: usize,
        strategy: GrandStrategy,
    ) -> u32 {
        let lane_votes =
            self.lane_votes_its_favor && self.victory_target == Some(VictoryTarget::Diplomacy);
        if strategy == GrandStrategy::Diplomacy || lane_votes {
            g.congress_affordable_votes(pid)
        } else {
            1
        }
    }

    pub(super) fn incoming_deal_value(
        &self,
        g: &Game,
        pid: usize,
        deal: &DiplomaticDeal,
        plan: &StrategicPlan,
    ) -> f64 {
        let partner = deal.from;
        let my_power = g.military_power(pid);
        let partner_power = g.military_power(partner);
        let grievance = g.players[pid]
            .grievances
            .get(&partner)
            .copied()
            .unwrap_or(0.0);
        let fatigued = self.major_war_since.is_some_and(|started| {
            g.turn.saturating_sub(started) >= 24
                && g.turn.saturating_sub(self.last_campaign_progress) >= 12
        });
        // `one_war_at_a_time`: a Joint War while any war burns is a second
        // front by treaty; the fatigue acceptance stands down while the
        // front still has something to take; the peace the gene wants is a
        // white peace. See `advanced/one_war.rs`.
        if self.one_war_refuses_joint_war(g, pid, deal) {
            return -1_000.0;
        }
        let fatigued = fatigued
            && !self.one_war_presses(g, pid, partner)
            && !self.domination_front_crushed(g, pid, partner)
            && !self.domination_siege_is_progressing(g, pid, partner, plan)
            && !self.domination_siege_train_mobilizing(g, pid, partner, plan);
        let one_war_peace = self.one_war_peace(g, pid, partner).is_some();
        let denied_partner = plan.target_player == Some(partner)
            && (plan.strategy == GrandStrategy::Conquest
                || g.is_at_war(pid, partner)
                || self.rival_victory_pressure(g, partner).progress >= 78);

        let mut value = deal.give_gold - deal.request_gold;
        if deal.peace {
            value += if my_power < partner_power * 0.85 || fatigued || one_war_peace {
                320.0
            } else if denied_partner {
                // Recovery is a temporary battlefield posture, not an order
                // to abandon the campaign. A locally threatened city can put
                // an overwhelmingly stronger attacker into Recovery for one
                // assessment window; keep refusing its active target's white
                // peace until the army is actually outmatched or fatigued.
                -260.0
            } else if plan.strategy == GrandStrategy::Recovery {
                320.0
            } else {
                35.0
            };
        } else if denied_partner {
            return -1_000.0;
        }
        // `culture-threat-early`: passage is the one thing a proposal can
        // ask of us that finances a rival's culture finish; a threat's
        // offer of it is refused outright. See `advanced/culture_strategy.rs`.
        if deal.open_borders
            && !deal.peace
            && self.culture_threat_early
            && self.culture_trade_threats(g, pid).contains(&partner)
        {
            return -1_000.0;
        }
        if deal.open_borders {
            value += match plan.strategy {
                GrandStrategy::Culture => 70.0,
                GrandStrategy::Conquest => 45.0,
                _ => 25.0,
            };
        }
        // `friendship-refuses-the-racer`: a friendship forbids the war that
        // counters a racer for its whole term, so a racer's offer of one is
        // refused, as our own offers to it already are. See
        // `advanced/protective_friendship.rs`.
        if deal.friendship && self.friendship_refuses_the_racer && self.friend_is_racing(g, partner)
        {
            return -1_000.0;
        }
        if deal.friendship {
            value += if plan.strategy == GrandStrategy::Diplomacy {
                80.0
            } else {
                40.0
            };
        }
        if let Some(alliance) = deal.alliance.as_deref() {
            value += match (plan.strategy, alliance) {
                (GrandStrategy::Science, "research")
                | (GrandStrategy::Culture, "cultural")
                | (GrandStrategy::Religion, "religious")
                | (GrandStrategy::Conquest | GrandStrategy::Recovery, "military")
                | (GrandStrategy::Expansion | GrandStrategy::Diplomacy, "economic") => 150.0,
                (GrandStrategy::Diplomacy, _) => 110.0,
                _ => 55.0,
            };
        }
        if deal.defensive_pact {
            // The engine only offers a pact to an existing Ally. Its value is
            // protection against a surprise declaration, but it is never
            // worth protecting the campaign target (handled above).
            value += if partner_power > my_power * 0.8 {
                95.0
            } else {
                55.0
            };
        }
        if let Some(target) = deal.joint_war_target {
            let strategic_target = plan.target_player == Some(target)
                || self.rival_victory_pressure(g, target).progress >= 78;
            value += if strategic_target {
                300.0
            } else {
                // A Joint War is a 30-turn commitment, not a free diplomatic
                // favour. Do not accept a human/AI proposal that pulls this
                // plan off its own objective.
                -300.0
            };
            // `enemy_of_my_enemy`: a rival's war on its own far side is the
            // war we least want to join. See `advanced/enemy_of_my_enemy.rs`.
            value += self.enemy_of_my_enemy_joint_war_penalty(
                g,
                pid,
                deal,
                (plan.strategy == GrandStrategy::Conquest)
                    .then_some(plan.target_player)
                    .flatten(),
            );
        }
        if let Some(promise) = deal.promise.as_deref() {
            let conflicts_with_plan = matches!(
                (plan.strategy, promise),
                (GrandStrategy::Expansion, "no_settling")
                    | (GrandStrategy::Religion, "no_conversion")
                    | (
                        GrandStrategy::Conquest | GrandStrategy::Recovery,
                        "no_city_state_attack"
                    )
            );
            value += if conflicts_with_plan { -130.0 } else { 20.0 };
        }
        value - grievance * 0.8
    }

    pub(super) fn strategic_bilateral_trade(
        &self,
        g: &mut Game,
        pid: usize,
        excluded_partner: Option<usize>,
        strategy: GrandStrategy,
    ) {
        let objective = self
            .victory_target
            .map(VictoryTarget::strategy)
            .unwrap_or(strategy);
        let culture_threats = self.culture_trade_threats(g, pid);
        // `science-threat-denial`: passage and Great Works are the two
        // sales that pay a rival's space race. Empty when the gene is off.
        // See `advanced/science_threat_denial.rs`.
        let science_threats = self.science_threat_seats(g, pid);
        // `culture-keeps-every-border-open`: every rival's passage, every
        // turn, once our tourism is worth multiplying. Exact no-op while the
        // gene is off. See `advanced/culture_open_borders.rs`.
        if objective == GrandStrategy::Culture
            && self.keep_every_border_open(
                g,
                pid,
                excluded_partner,
                &culture_threats,
                &science_threats,
            ) > 0
        {
            return;
        }
        if objective == GrandStrategy::Culture && g.turn % 6 == pid as u32 % 6 {
            let best = g
                .quick_deals(pid)
                .into_iter()
                .filter(|deal| Some(deal.partner) != excluded_partner)
                .filter(|deal| {
                    self.culture_deal_allowed(deal, &culture_threats)
                        && self.science_denial_deal_allowed(deal, &science_threats)
                })
                .filter(|deal| {
                    deal.item == "open_borders"
                        && deal.direction == "buy"
                        && deal.my_value >= 2.0
                        && deal.partner_value >= 2.0
                })
                .max_by(|left, right| {
                    g.domestic_tourists(left.partner)
                        .cmp(&g.domestic_tourists(right.partner))
                        .then_with(|| {
                            self.base
                                .deal_objective(left)
                                .partial_cmp(&self.base.deal_objective(right))
                                .unwrap()
                        })
                        .then_with(|| right.partner.cmp(&left.partner))
                });
            if let Some(deal) = best {
                if self.base.close_quick_deal(g, pid, deal) {
                    return;
                }
            }

            let best = g
                .quick_deals(pid)
                .into_iter()
                .filter(|deal| Some(deal.partner) != excluded_partner)
                .filter(|deal| {
                    self.culture_deal_allowed(deal, &culture_threats)
                        && self.science_denial_deal_allowed(deal, &science_threats)
                })
                .filter(|deal| {
                    deal.category == "great_work"
                        && deal.direction == "buy"
                        && deal.my_value >= 2.0
                        && deal.partner_value >= 2.0
                })
                .max_by(|left, right| {
                    (self.base.deal_objective(left) + self.completion_deal_bonus(g, pid, left))
                        .partial_cmp(
                            &(self.base.deal_objective(right)
                                + self.completion_deal_bonus(g, pid, right)),
                        )
                        .unwrap()
                        .then_with(|| right.partner.cmp(&left.partner))
                        .then_with(|| right.item.cmp(&left.item))
                });
            if let Some(deal) = best {
                if self.base.close_quick_deal(g, pid, deal) {
                    return;
                }
            }

            // A Culture objective preserves its own Great Works. If neither
            // the strategically useful Open Borders direction nor a housed
            // purchase is available, it may still take the best ordinary
            // mutually beneficial quote.
            let best = g
                .quick_deals(pid)
                .into_iter()
                .filter(|deal| Some(deal.partner) != excluded_partner)
                .filter(|deal| {
                    self.culture_deal_allowed(deal, &culture_threats)
                        && self.science_denial_deal_allowed(deal, &science_threats)
                })
                .filter(|deal| {
                    !(deal.category == "great_work" && deal.direction == "sell")
                        && deal.my_value >= 2.0
                        && deal.partner_value >= 2.0
                })
                .max_by(|left, right| {
                    self.base
                        .deal_objective(left)
                        .partial_cmp(&self.base.deal_objective(right))
                        .unwrap()
                });
            if let Some(deal) = best {
                self.base.close_quick_deal(g, pid, deal);
            }
            return;
        }
        if culture_threats.is_empty() && science_threats.is_empty() {
            self.base
                .bilateral_trade_excluding(g, pid, excluded_partner);
        } else if !self.base.minor && !self.base.barb && g.turn % 6 == pid as u32 % 6 {
            let best = g
                .quick_deals(pid)
                .into_iter()
                .filter(|deal| Some(deal.partner) != excluded_partner)
                .filter(|deal| {
                    self.culture_deal_allowed(deal, &culture_threats)
                        && self.science_denial_deal_allowed(deal, &science_threats)
                })
                .max_by(|left, right| {
                    self.base
                        .deal_objective(left)
                        .total_cmp(&self.base.deal_objective(right))
                });
            if let Some(deal) =
                best.filter(|deal| deal.my_value >= 2.0 && deal.partner_value >= 2.0)
            {
                self.base.close_quick_deal(g, pid, deal);
            }
        }
    }

    pub(super) fn propose_strategic_alliance(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        denied_partner: Option<usize>,
    ) {
        // A level-one alliance generates one Favor on the next turn.  Nobel
        // Peace scores that stream while its host clock is live, so a fixed
        // twelve-turn proposal cadence can throw away up to eleven scoring
        // turns despite a legal ally already being known.  Keep the cadence
        // for ordinary games and even an expiring Peace contest: only bypass
        // it when the next-turn Favor can still reach the host score.
        let nobel_peace_alliance_pays =
            self.nobel_peace_favor_score_value(g, pid, 1.0, 1.0) > f64::EPSILON;
        // `research-alliance-first`: the kind is Research and the cadence is
        // not waited for while a Research Alliance is worth waiting for; no
        // other kind is proposed while it is not yet legal on our tree. Off,
        // `Stock`. See `advanced/research_alliance.rs`.
        let research_lane = self.research_alliance_lane(g, pid);
        // `culture-seat-allies`: a Culture seat does not wait for Scientific
        // Theory; it asks for the stock kind meanwhile.
        // `science-seat-allies-2`: before Diplomatic Service a Science seat
        // does not wait either; it asks for a Military Alliance. See
        // `advanced/culture_seat_allies.rs`.
        let early_military = research_lane == research_alliance::ResearchAllianceLane::Wait
            && self.science_seat_allies_early(g, pid);
        if research_lane == research_alliance::ResearchAllianceLane::Wait
            && !self.culture_seat_allies_on()
            && !early_military
        {
            return;
        }
        let research_first = research_lane == research_alliance::ResearchAllianceLane::Research;
        if (!nobel_peace_alliance_pays && !research_first && g.turn % 12 != pid as u32 % 12)
            || !g.players[pid]
                .civics
                .contains(&crate::name!("civil_service"))
        {
            return;
        }
        let kind = if research_first {
            research_alliance::ALLY_RESEARCH_KIND
        } else if early_military {
            "military"
        } else {
            match plan.strategy {
                GrandStrategy::Science => "research",
                GrandStrategy::Culture => "cultural",
                GrandStrategy::Religion => "religious",
                GrandStrategy::Conquest | GrandStrategy::Recovery => "military",
                GrandStrategy::Expansion | GrandStrategy::Diplomacy => "economic",
            }
        };
        if kind == "research" && g.tree_effect(pid, "research_agreements") <= 0.0 {
            return;
        }
        // Empty when `science-threat-denial` is off, so the partner filter
        // below is unchanged.
        let science_threats = self.science_threat_seats(g, pid);
        if g.players[pid]
            .alliances
            .values()
            .any(|alliance| alliance.ends > g.turn && alliance.kind == kind)
        {
            return;
        }
        let pending_with = |partner: usize| {
            g.pending_deals.iter().any(|deal| {
                deal.expires >= g.turn
                    && ((deal.from == pid && deal.to == partner)
                        || (deal.from == partner && deal.to == pid))
            })
        };
        let partner = g
            .players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.has_met(pid, other.id)
                    && Some(other.id) != denied_partner
                    && !g.is_at_war(pid, other.id)
                    && other.civics.contains(&crate::name!("civil_service"))
                    && g.alliance_with(pid, other.id).is_none()
                    && !pending_with(other.id)
                    && (kind != "research" || g.tree_effect(other.id, "research_agreements") > 0.0)
                    && !other
                        .alliances
                        .values()
                        .any(|alliance| alliance.ends > g.turn && alliance.kind == kind)
                    && g.players[pid]
                        .grievances
                        .get(&other.id)
                        .copied()
                        .unwrap_or(0.0)
                        < 75.0
                    && self.rival_victory_pressure(g, other.id).progress < 82
                    // A rival whose clock the Domination army answers is no
                    // ally: the friendship bundled here makes the denial war
                    // illegal for its term. Live King
                    // civvis-20261001T022028Z asked Brazil for a Research
                    // Alliance at turns 125 and 135 while its Catholicism held
                    // two of four majors, and Brazil won at 147. Nor is a
                    // rival still holding the capital Domination needs that
                    // we outgun twice over (`domination_capital_prey`).
                    && !self.domination_counter_target(g, pid, other.id)
                    && !self.domination_capital_prey(g, pid, other.id)
                    // `denial-nearest-finish`: nor a rival whose culture race
                    // is projected to finish within the horizon.
                    && self.nearest_finish_culture_clock(g, other.id).is_none()
                    // `science-threat-denial`: a research agreement hands a
                    // science threat the yield it is winning with, and any
                    // alliance makes the denial war illegal for its whole
                    // term. See `advanced/science_threat_denial.rs`.
                    && !self.science_denial_refuses_alliance(&science_threats, other.id)
                    // `research-alliance-first`: a culture threat, or a
                    // partner inside the cool-down on our last ask. Never off.
                    && !self.research_alliance_barred(g, pid, other.id)
                    // `culture-seat-allies`: nor the science racer, nor the
                    // rival that sets our culture bar. Never off.
                    && !self.culture_seat_ally_barred(g, pid, other.id)
                    // `alliance-remembers-the-refusal`: nor a partner asked
                    // within thirty turns that did not ally. Never off.
                    && !self.alliance_refusal_barred(g, pid, other.id)
            })
            .max_by(|left, right| {
                let score = |other: usize| {
                    let friendship = if g.are_friends(pid, other) {
                        180.0
                    } else {
                        0.0
                    };
                    // `research-alliance-first`: the partner's science as a
                    // share of ours, the number level 3 pays 10 percent of.
                    // Zero off.
                    let science = self.research_alliance_science_term(g, pid, other, kind);
                    let connected = if g.routes.iter().any(|route| {
                        route.ends > g.turn
                            && ((route.owner == pid
                                && g.cities
                                    .get(&route.dest)
                                    .is_some_and(|destination| destination.owner == other))
                                || (route.owner == other
                                    && g.cities
                                        .get(&route.dest)
                                        .is_some_and(|destination| destination.owner == pid)))
                    }) {
                        70.0
                    } else {
                        0.0
                    };
                    let complement = match kind {
                        "research" => {
                            g.players[other]
                                .techs
                                .difference(&g.players[pid].techs)
                                .count() as f64
                                * 4.0
                        }
                        "cultural" => g.tourism_per_turn(other).min(300.0) * 0.15,
                        "economic" => {
                            g.players
                                .iter()
                                .filter(|minor| minor.alive && minor.is_minor)
                                .filter(|minor| g.suzerain_of(minor.id) == Some(other))
                                .count() as f64
                                * 35.0
                        }
                        "military" => g.military_power(other).min(250.0) * 0.25,
                        "religious" => g.players[other].religion.is_some() as usize as f64 * 45.0,
                        _ => 0.0,
                    };
                    // `enemy_of_my_enemy`: a partner with a city on a
                    // rival's far side. See `advanced/enemy_of_my_enemy.rs`.
                    let across = self.enemy_of_my_enemy_partner_bonus(
                        g,
                        pid,
                        other,
                        (plan.strategy == GrandStrategy::Conquest)
                            .then_some(plan.target_player)
                            .flatten(),
                    );
                    // `culture-seat-allies`: the nearest partner first. Zero off.
                    let near = self.culture_seat_ally_nearness(g, pid, other);
                    friendship + connected + complement + across + science + near
                        - g.players[pid]
                            .grievances
                            .get(&other)
                            .copied()
                            .unwrap_or(0.0)
                };
                score(left.id)
                    .partial_cmp(&score(right.id))
                    .unwrap()
                    .then_with(|| right.id.cmp(&left.id))
            })
            .map(|other| other.id);
        if let Some(partner) = partner {
            let proposed = g
                .apply(
                    pid,
                    &Action::ProposeDeal {
                        player: partner,
                        give_gold: 0.0,
                        request_gold: 0.0,
                        // `no_free_passage`: passage is sold, not bundled.
                        open_borders: !self.base.no_free_passage
                            && g.players[pid]
                                .civics
                                .contains(&crate::name!("early_empire")),
                        friendship: true,
                        peace: false,
                        alliance: Some(kind.to_string()),
                    },
                )
                .is_ok();
            let across = self.enemy_of_my_enemy_partner_bonus(
                g,
                pid,
                partner,
                (plan.strategy == GrandStrategy::Conquest)
                    .then_some(plan.target_player)
                    .flatten(),
            );
            if proposed && across > 0.0 {
                *g.players[pid]
                    .counters
                    .entry("eoe:partners".to_string())
                    .or_insert(0) += 1;
            }
            if proposed {
                self.research_alliance_note_asked(g, pid, partner, kind);
                self.alliance_note_asked(g, pid, partner);
                if research_first {
                    think!(self.journal(), Diplomacy, Decision,
                           "Proposing a Research Alliance to {}", g.players[partner].civ;
                           "their science is worth allying for, and the handicap does not \
                            touch a shared tech boost or the level-three science share");
                }
            }
        }
    }

    /// The empire the denial layer would point a targeted congress penalty at,
    /// or `None` when it is not pointing anywhere.
    fn congress_denial_target(&self, g: &Game, pid: usize) -> Option<usize> {
        self.congress_counter_leader
            .then(|| self.victory_denial(g, pid).map(|(rival, _)| rival))
            .flatten()
    }

    /// The empire a targeted congress penalty is actually pointed at.
    fn congress_counter_target(&self, g: &Game, pid: usize) -> Option<usize> {
        self.congress_denial_target(g, pid)
            .or_else(|| Self::congress_diplomatic_leader(g))
    }

    /// The living major holding the most Diplomatic Victory Points.
    ///
    /// Its own concept, not a fallback: the `world_leader` ballot aims here
    /// whatever the denial layer thinks, because that resolution's ±2 moves
    /// Diplomatic Victory Points and nothing else.
    fn congress_diplomatic_leader(g: &Game) -> Option<usize> {
        g.players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .max_by_key(|player| (player.dvp, std::cmp::Reverse(player.id)))
            .map(|player| player.id)
    }

    pub(super) fn congress_choice(
        &self,
        g: &Game,
        pid: usize,
        resolution: &CongressResolution,
        strategy: GrandStrategy,
    ) -> Option<String> {
        if let Some(proposal) = g.emergency_proposal_for_resolution(&resolution.id) {
            if proposal.target == pid {
                return Some("B:oppose".to_string());
            }
            if !proposal.eligible.contains(&pid) {
                return None;
            }
            let grievance = g.players[pid]
                .grievances
                .get(&proposal.target)
                .copied()
                .unwrap_or(0.0);
            let threat = self.rival_victory_pressure(g, proposal.target).progress;
            let affordable_war =
                g.military_power(pid) * 1.75 + 20.0 >= g.military_power(proposal.target);
            let support = proposal.kind == "city_state"
                || strategy == GrandStrategy::Diplomacy
                || grievance >= 25.0
                || threat >= 55
                || affordable_war;
            return Some(if support { "A:support" } else { "B:oppose" }.to_string());
        }
        // Legacy saves encoded only a target. Preserve their old strategic
        // behavior while new sessions use explicit `A:target`/`B:target`
        // ballots.
        if resolution
            .choices
            .iter()
            .all(|choice| !choice.contains(':'))
        {
            let own = pid.to_string();
            return match resolution.id.as_str() {
                "world_leader" | "international_aid" if strategy == GrandStrategy::Diplomacy => {
                    resolution
                        .choices
                        .iter()
                        .find(|choice| **choice == own)
                        .cloned()
                }
                "world_leader" | "international_aid" => resolution
                    .choices
                    .iter()
                    .filter_map(|choice| {
                        choice.parse::<usize>().ok().map(|target| (choice, target))
                    })
                    .min_by_key(|(_, target)| (g.players[*target].dvp, *target))
                    .map(|(choice, _)| choice.clone()),
                "world_fair" if strategy == GrandStrategy::Culture => resolution
                    .choices
                    .iter()
                    .find(|choice| **choice == own)
                    .cloned(),
                "world_fair" => resolution
                    .choices
                    .iter()
                    .filter_map(|choice| {
                        choice.parse::<usize>().ok().map(|target| (choice, target))
                    })
                    .max_by(|left, right| {
                        g.players[left.1]
                            .culture_lifetime
                            .partial_cmp(&g.players[right.1].culture_lifetime)
                            .unwrap_or(std::cmp::Ordering::Equal)
                            .then_with(|| right.1.cmp(&left.1))
                    })
                    .map(|(choice, _)| choice.clone()),
                _ => resolution.choices.first().cloned(),
            };
        }

        // Who a targeted penalty is pointed at. `world_leader` keeps aiming at
        // the diplomatic leader because its +/-2 moves Diplomatic Victory
        // Points and nothing else; the resolutions that cost an empire real
        // yields aim at whoever is actually about to win. See
        // [`Self::congress_counter_leader`] for what the census measured, and
        // [`Self::congress_counter_target`] for why this is a shared function
        // and not two.
        let diplomatic_leader = Self::congress_diplomatic_leader(g);
        let denied = self.congress_denial_target(g, pid);
        let counter_target = self.congress_counter_target(g, pid);
        let preferred_district = match strategy {
            GrandStrategy::Science => "campus",
            GrandStrategy::Culture => "theater_square",
            GrandStrategy::Religion => "holy_site",
            GrandStrategy::Conquest | GrandStrategy::Recovery => "encampment",
            GrandStrategy::Diplomacy => "diplomatic_quarter",
            GrandStrategy::Expansion => "commercial_hub",
        };
        let preferred_person = match strategy {
            GrandStrategy::Science => "scientist",
            GrandStrategy::Culture => "artist",
            GrandStrategy::Religion => "prophet",
            GrandStrategy::Conquest | GrandStrategy::Recovery => "general",
            GrandStrategy::Diplomacy | GrandStrategy::Expansion => "merchant",
        };
        let preferred_work = match strategy {
            GrandStrategy::Culture => "art",
            GrandStrategy::Religion => "relic",
            _ => "writing",
        };
        let observed = |choice: &str| {
            resolution
                .ballots
                .values()
                .filter(|(cast, _)| cast == choice)
                .map(|(_, votes)| *votes as f64)
                .sum::<f64>()
        };

        resolution.choices.iter().cloned().max_by(|left, right| {
            let score = |choice: &str| {
                let (outcome, target) = Game::congress_choice_parts(choice);
                let target_player = target.parse::<usize>().ok();
                let base = match resolution.id.as_str() {
                    "world_leader" => match (outcome, target_player) {
                        ("A", Some(target))
                            if target == pid && strategy == GrandStrategy::Diplomacy =>
                        {
                            1_000.0
                        }
                        ("B", Some(target))
                            if Some(target) == diplomatic_leader && target != pid =>
                        {
                            900.0
                        }
                        ("A", Some(target)) => 100.0 - 12.0 * g.players[target].dvp as f64,
                        ("B", Some(target)) => 20.0 + 18.0 * g.players[target].dvp as f64,
                        _ => 0.0,
                    },
                    "mercenary_companies" => match (outcome, target) {
                        ("B", "production") => 340.0,
                        ("B", "gold") => 180.0,
                        ("B", "faith") if strategy == GrandStrategy::Religion => 230.0,
                        ("B", "faith") => 90.0,
                        ("A", _) => -120.0,
                        _ => 0.0,
                    },
                    "luxury_policy" => {
                        let own = g.resource_access_count(pid, target) as f64;
                        let rival = g
                            .players
                            .iter()
                            .filter(|player| player.id != pid)
                            .map(|player| g.resource_access_count(player.id, target))
                            .max()
                            .unwrap_or(0) as f64;
                        if outcome == "A" {
                            own * 75.0
                        } else {
                            rival * 55.0 - own * 85.0
                        }
                    }
                    "trade_policy" => match target_player {
                        // Outcome B embargoes every inter-civ trade route the
                        // target has. Against an empire about to win that is
                        // worth more than a trade slot of our own, which is all
                        // outcome A on ourselves buys.
                        Some(target)
                            if outcome == "B"
                                && Some(target) == denied
                                && target != pid =>
                        {
                            420.0
                        }
                        Some(target) if outcome == "A" && target == pid => 260.0,
                        Some(target)
                            if outcome == "B"
                                && Some(target) == counter_target
                                && target != pid =>
                        {
                            150.0
                        }
                        Some(target)
                            if outcome == "A" && g.alliance_with(pid, target).is_some() =>
                        {
                            120.0
                        }
                        _ => 10.0,
                    },
                    "world_religion" => {
                        let mine = g.players[pid].religion.as_deref() == Some(target);
                        if outcome == "A" && mine {
                            320.0
                        } else if outcome == "B" && !mine {
                            150.0
                        } else {
                            0.0
                        }
                    }
                    "urban_development_treaty" => {
                        if outcome == "A" && target == preferred_district {
                            280.0
                        } else if outcome == "A" {
                            80.0
                        } else {
                            -80.0
                        }
                    }
                    "patronage" => {
                        if outcome == "A" && target == preferred_person {
                            280.0
                        } else if outcome == "A" {
                            70.0
                        } else {
                            -100.0
                        }
                    }
                    "military_advisory" => {
                        let own = g
                            .units
                            .values()
                            .filter(|unit| {
                                unit.owner == pid
                                    && g.rules.units[unit.kind].promotion_class == target
                            })
                            .count() as f64;
                        let rival = g
                            .units
                            .values()
                            .filter(|unit| {
                                unit.owner != pid
                                    && g.rules.units[unit.kind].promotion_class == target
                            })
                            .count() as f64;
                        if outcome == "A" {
                            own * 45.0 - rival * 10.0
                        } else {
                            rival * 35.0 - own * 50.0
                        }
                    }
                    "migration_treaty" => match (outcome, target_player) {
                        // Outcome B costs its target 20% growth and pushes its
                        // loyalty. Shipped, it scores 0.0 against every rival,
                        // so the penalty can never be aimed at anybody.
                        ("B", Some(target)) if Some(target) == denied && target != pid => 300.0,
                        ("A", Some(target))
                            if target == pid && strategy == GrandStrategy::Expansion =>
                        {
                            220.0
                        }
                        ("B", Some(target)) if target == pid => 140.0,
                        ("A", Some(target)) if target != pid => 35.0,
                        _ => 0.0,
                    },
                    "public_relations" => match (outcome, target_player) {
                        ("B", Some(target)) if target == pid => 230.0,
                        ("A", Some(target))
                            if Some(target) == counter_target && target != pid =>
                        {
                            150.0
                        }
                        _ => 0.0,
                    },
                    "heritage_organization" => {
                        if outcome == "A" && target == preferred_work {
                            300.0
                        } else if outcome == "A" {
                            90.0
                        } else {
                            -120.0
                        }
                    }
                    "arms_control" => match target_player {
                        Some(target) => {
                            let inventory = |player: usize| {
                                [
                                    "project_effect:nuclear_devices",
                                    "project_effect:thermonuclear_devices",
                                ]
                                .into_iter()
                                .map(|key| {
                                    g.players[player]
                                        .counters
                                        .get(key)
                                        .copied()
                                        .unwrap_or(0)
                                        .max(0)
                                })
                                .sum::<i64>() as f64
                            };
                            let mine = inventory(pid);
                            let theirs = inventory(target);
                            let major_inventories: Vec<f64> = g
                                .players
                                .iter()
                                .filter(|player| {
                                    player.alive && !player.is_minor && !player.is_barbarian
                                })
                                .map(|player| inventory(player.id))
                                .collect();
                            let world_total = major_inventories.iter().sum::<f64>();
                            let equalized_total =
                                theirs * major_inventories.len().max(1) as f64;
                            let disarmament = world_total - equalized_total;
                            match outcome {
                                // Outcome A copies the target's stockpile to every
                                // major. Peaceful strategies therefore nominate the
                                // smallest arsenal, not the nuclear leader.
                                "A" if strategy != GrandStrategy::Conquest => {
                                    180.0 + 55.0 * disarmament
                                        - 75.0 * (-disarmament).max(0.0)
                                }
                                "A" => 20.0 + 35.0 * (theirs - mine)
                                    - 45.0 * (equalized_total - world_total).max(0.0),
                                "B" if target != pid => {
                                    let aggression = if matches!(
                                        strategy,
                                        GrandStrategy::Conquest | GrandStrategy::Recovery
                                    ) {
                                        100.0
                                    } else {
                                        55.0
                                    };
                                    90.0 + aggression * theirs
                                }
                                "B" => -500.0,
                                _ => 0.0,
                            }
                        }
                        None => 0.0,
                    },
                    "world_ideology" => {
                        let mine = g.players[pid].government.as_deref() == Some(target);
                        let rival_users = g
                            .players
                            .iter()
                            .filter(|player| {
                                player.id != pid
                                    && player.alive
                                    && !player.is_minor
                                    && !player.is_barbarian
                                    && player.government.as_deref() == Some(target)
                            })
                            .count() as f64;
                        match outcome {
                            "A" if mine => 320.0,
                            "A" => 30.0,
                            "B" if mine => -260.0,
                            "B" => 90.0 + 70.0 * rival_users,
                            _ => 0.0,
                        }
                    }
                    "border_control_treaty" => match (outcome, target_player) {
                        // Outcome B stops the target annexing tiles by border
                        // growth. Shipped, it is aimed by raw territory -- the
                        // one leader-targeting term in this table that already
                        // uses something other than Diplomatic Victory Points,
                        // and still not the empire about to win.
                        ("B", Some(target)) if Some(target) == denied && target != pid => 400.0,
                        ("A", Some(target)) if target == pid => 300.0,
                        ("B", Some(target)) if target == pid => -240.0,
                        ("B", Some(target)) => {
                            let territory = g
                                .player_city_ids(target)
                                .into_iter()
                                .map(|city| g.cities[&city].owned_tiles.len())
                                .sum::<usize>() as f64;
                            80.0 + territory
                        }
                        _ => 20.0,
                    },
                    "public_works_program" => {
                        let queued = |owner: usize| {
                            g.cities
                                .values()
                                .filter(|city| city.owner == owner)
                                .filter(|city| {
                                    city.queue.iter().any(|item| {
                                        matches!(item, Item::Project { project } if project == target)
                                    })
                                })
                                .count() as f64
                        };
                        let own_queued = queued(pid);
                        let rival_queued = g
                            .players
                            .iter()
                            .filter(|player| {
                                player.id != pid
                                    && player.alive
                                    && !player.is_minor
                                    && !player.is_barbarian
                            })
                            .map(|player| queued(player.id))
                            .sum::<f64>();
                        let aligned = match strategy {
                            GrandStrategy::Science => {
                                target.contains("launch_")
                                    || target.contains("laser_station")
                                    || target == "exoplanet_expedition"
                            }
                            GrandStrategy::Conquest | GrandStrategy::Recovery => {
                                target.contains("nuclear")
                                    || matches!(target, "manhattan_project" | "operation_ivy")
                            }
                            GrandStrategy::Diplomacy => target == "carbon_recapture",
                            _ => false,
                        };
                        match (outcome, aligned) {
                            ("A", true) => 300.0 + 160.0 * own_queued,
                            ("A", false) => 65.0 + 180.0 * own_queued,
                            ("B", true) => {
                                -220.0 + 140.0 * rival_queued - 180.0 * own_queued
                            }
                            ("B", false) => {
                                25.0 + 140.0 * rival_queued - 180.0 * own_queued
                            }
                            _ => 0.0,
                        }
                    }
                    "global_energy_treaty" => {
                        let queued = |owner: usize| {
                            g.cities
                                .values()
                                .filter(|city| city.owner == owner)
                                .filter(|city| {
                                    city.queue.iter().any(|item| {
                                        matches!(item, Item::Building { building } if building == target)
                                    })
                                })
                                .count() as f64
                        };
                        let own_queued = queued(pid);
                        let rival_queued = g
                            .players
                            .iter()
                            .filter(|player| {
                                player.id != pid
                                    && player.alive
                                    && !player.is_minor
                                    && !player.is_barbarian
                            })
                            .map(|player| queued(player.id))
                            .sum::<f64>();
                        let preferred = match strategy {
                            GrandStrategy::Science | GrandStrategy::Diplomacy => {
                                "nuclear_power_plant"
                            }
                            GrandStrategy::Conquest | GrandStrategy::Recovery => "coal_power_plant",
                            _ => "oil_power_plant",
                        };
                        match (outcome, target) {
                            ("A", candidate) if candidate == preferred => {
                                270.0 + 160.0 * own_queued
                            }
                            ("A", _) => 90.0 + 160.0 * own_queued,
                            ("B", "coal_power_plant") if strategy == GrandStrategy::Diplomacy => {
                                180.0 + 120.0 * rival_queued - 180.0 * own_queued
                            }
                            ("B", candidate) if candidate == preferred => {
                                -180.0 + 120.0 * rival_queued - 180.0 * own_queued
                            }
                            ("B", _) => 35.0 + 120.0 * rival_queued - 180.0 * own_queued,
                            _ => 0.0,
                        }
                    }
                    "deforestation_treaty" => {
                        let owned_copies = |owner: usize| {
                            g.cities
                                .values()
                                .filter(|city| city.owner == owner)
                                .flat_map(|city| city.owned_tiles.iter())
                                .filter(|position| {
                                    g.map.tiles[*position].feature.as_deref() == Some(target)
                                })
                                .count() as f64
                        };
                        let own_copies = owned_copies(pid);
                        let rival_copies = g
                            .players
                            .iter()
                            .filter(|player| {
                                player.id != pid
                                    && player.alive
                                    && !player.is_minor
                                    && !player.is_barbarian
                            })
                            .map(|player| owned_copies(player.id))
                            .sum::<f64>();
                        match outcome {
                            "A" => {
                                65.0
                                    + 35.0 * own_copies
                                    + if strategy == GrandStrategy::Expansion {
                                        90.0
                                    } else {
                                        0.0
                                    }
                            }
                            "B" if strategy == GrandStrategy::Culture && target == "forest" => {
                                165.0 + 5.0 * (own_copies + rival_copies)
                            }
                            "B" => 20.0 + 5.0 * rival_copies - 20.0 * own_copies,
                            _ => 0.0,
                        }
                    }
                    _ => 0.0,
                };
                base + observed(choice) * 35.0
            };
            score(left)
                .partial_cmp(&score(right))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.cmp(left))
        })
    }

    /// Prefer an available low-Grievance casus belli. If none is ready, a
    /// major rival is denounced and the campaign waits for Formal War rather
    /// than opening with a Surprise War. The sole exception is a rival already
    /// on the brink of victory, where five setup turns can lose the game.
    /// City-states cannot be denounced and therefore remain direct targets.
    /// A rival whose faith the Domination army counters, with its spreaders
    /// already working our land, is answered by a Surprise War: only war
    /// lets our soldiers condemn them, and a Formal War waits five turns
    /// after the denouncement. Live King civvis-20261001T022028Z had ten
    /// Brazilian Apostles and three Missionaries around our cities from turn
    /// 130 while we were at peace with Brazil until turn 144, and Brazil won
    /// on religion at 147.
    pub(super) fn faith_counter_spreaders_at_home(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
    ) -> bool {
        g.players[target].religion.is_some()
            && self.domination_counter_target(g, pid, target)
            && g.units.values().any(|unit| {
                unit.owner == target
                    && g.rules.units[unit.kind].class == "religious"
                    && (g.map.get(unit.pos).is_some_and(|tile| {
                        tile.owner_city
                            .and_then(|cid| g.cities.get(&cid))
                            .is_some_and(|city| city.owner == pid)
                    }) || g
                        .player_city_ids(pid)
                        .iter()
                        .any(|cid| g.wdist(g.cities[cid].pos, unit.pos) <= 3))
            })
    }

    pub(super) fn preferred_war_opening(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
    ) -> Option<Action> {
        if !self.conversion_upgrade_launch_ready(g, pid) {
            return None;
        }
        let legal = g.legal_actions_within(pid, ActionFamilies::DIPLOMACY);
        let casus_belli = legal
            .iter()
            .filter_map(|action| match action {
                Action::DeclareWarWithCasusBelli {
                    player,
                    casus_belli,
                } if *player == target => {
                    let grievance_cost = match casus_belli.as_str() {
                        "reconquest_war" | "protectorate_war" | "liberation_war" => 0,
                        "golden_age_war" => 25,
                        "holy_war" | "colonial_war" | "retribution_war" | "ideological_war" => 50,
                        "territorial_war" => 75,
                        // Formal War is the baseline. This branch is also a
                        // conservative fallback if another legal profile is
                        // added without teaching the planner its cost.
                        _ => 100,
                    };
                    Some((grievance_cost, casus_belli, action))
                }
                _ => None,
            })
            .min_by_key(|(cost, name, _)| (*cost, *name))
            .map(|(_, _, action)| action.clone());
        if casus_belli.is_some() {
            return casus_belli;
        }

        let surprise = legal.iter().find_map(|action| match action {
            Action::DeclareWar { player } if *player == target => Some(action.clone()),
            _ => None,
        });
        if g.players[target].is_minor {
            return surprise;
        }

        // A final exoplanet launch and a religious match point are both
        // irreversible victory clocks. They already interrupt strategic
        // planning before 90%, so waiting five turns for a Formal War here
        // would make the counter-campaign start after the game can end.
        let urgent = self.urgent_victory_threat(g, target)
            || self.faith_counter_spreaders_at_home(g, pid, target)
            // See `match_point_spreaders_at_home` (`religious-match-point-defence`).
            || self.match_point_spreaders_at_home(g, pid, target);
        let denounced = g.players[pid]
            .denounced_until
            .get(&target)
            .is_some_and(|until| *until > g.turn);
        if !urgent && !denounced {
            return legal.iter().find_map(|action| match action {
                Action::Denounce { player } if *player == target => Some(action.clone()),
                _ => None,
            });
        }
        // See `favor_spares_surprise`: with Favor at stake the urgent war is
        // a Formal War too, unless its clock is nearly out.
        let spared = urgent && self.favor_spares_surprise(g, pid, target);
        if urgent && !spared {
            surprise
        } else if spared && !denounced {
            legal.iter().find_map(|action| match action {
                Action::Denounce { player } if *player == target => Some(action.clone()),
                _ => None,
            })
        } else {
            // The denouncement is active but its five-turn preparation period
            // has not elapsed, so preserve the army and wait for Formal War.
            None
        }
    }

    /// `domination-strikes-when-staged`: the surprise war to open with in
    /// place of `opening`, when `opening` is a denouncement, the army is
    /// `staged`, the seat targets Domination and holds
    /// [`STRIKE_WHEN_STAGED_RATIO`] times the target's power. Live King games
    /// on 2026-10-04: 19 of 29 "Declaring war" lines sent only a
    /// denouncement, and the Formal War followed 3 to 66 turns later (5 to
    /// 16 typically). A staged army sat idle while the target reinforced.
    /// The surprise war costs grievances and favor (game 63 drained its favor
    /// to zero in ten turns of war), which is what the gene measures.
    pub(super) fn strike_when_staged(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        opening: &Action,
        staged: bool,
    ) -> Option<Action> {
        if !self.domination_strikes_when_staged
            || !staged
            || !matches!(opening, Action::Denounce { .. })
            // See `favor_spares_surprise`.
            || self.favor_spares_surprise(g, pid, target)
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || g.military_power(pid) < STRIKE_WHEN_STAGED_RATIO * g.military_power(target).max(1.0)
        {
            return None;
        }
        g.legal_actions_within(pid, ActionFamilies::DIPLOMACY)
            .into_iter()
            .find(|action| matches!(action, Action::DeclareWar { player } if *player == target))
    }

    pub(super) fn staged_campaign_units(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        objective: Pos,
    ) -> Vec<u32> {
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && (spec.is_melee_capable() || spec.has_ranged_attack())
                    && unit.hp as f64 > self.base.w.withdraw_hp
                    && self.campaign_staging_position(g, pid, target, *uid, objective, unit.pos)
            })
            .collect()
    }

    /// Whether the stack standing off the victim's capital is the size the
    /// engine's own combat math says takes it.
    ///
    /// A Monte Carlo over `damage`, `effective_strength` and `city_strength`
    /// against the measured turn-50 capital (strength 17.2, mean garrison 0.7,
    /// no walls) puts two melee units at 100% and one at 0%. Four is the
    /// figure that still reads 100% when the defender pulls its entire field
    /// army home — the case this lane cannot rule out, since it declares
    /// three tiles from the victim's capital.
    pub(super) fn early_rush_stack_ready(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        cid: u32,
    ) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        let objective = city.pos;
        let units = self.staged_campaign_units(g, pid, target, objective);
        let takers: Vec<u32> = units
            .into_iter()
            .filter(|uid| g.rules.units[g.units[uid].kind].is_melee_capable())
            .collect();
        if takers.len() < RUSH_STACK {
            return false;
        }
        // Counting takers is not the same as being able to finish, and the
        // difference cost this lane twenty turns. War was declared at turn 30
        // on two warriors and the first city did not fall until turn 54: a
        // warrior against the measured capital takes 28 damage a blow and dies
        // on its fourth, having dealt 134 of the 200 needed. The stack traded
        // itself and the next one had to be built and marched all over again.
        //
        // So ask the engine's own combat math whether this force can take the
        // city *before it dies*, using the same `damage` curve the fight will
        // use. `30 * exp((att - def) / 25)` per blow each way, `100 / incoming`
        // blows survived.
        let defense = g.city_strength(cid);
        let deliverable: f64 = takers
            .iter()
            .filter_map(|uid| g.units.get(uid))
            .map(|unit| {
                let attack = crate::game::effective_strength(g.unit_strength(unit, false), unit.hp);
                let out = 30.0 * ((attack - defense) / 25.0).exp();
                let incoming = (30.0 * ((defense - attack) / 25.0).exp()).max(1.0);
                let blows = (unit.hp as f64 / incoming).floor().max(1.0);
                out * blows
            })
            .sum();
        // The city's pool, plus a turn of the 20 HP/turn it regenerates
        // whenever the ring is not fully sealed.
        deliverable >= city.hp as f64 + 20.0
    }

    pub(super) fn campaign_staged_for_war(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        objective: Pos,
        committed_domination: bool,
    ) -> bool {
        let units = self.staged_campaign_units(g, pid, target, objective);
        let has_capturer = units
            .iter()
            .any(|uid| g.rules.units[g.units[uid].kind].is_melee_capable());
        let ratio = self.local_strength_ratio(g, pid, &units, &[target], objective);
        // The 3–5 ring is a useful launch check, but closed borders can keep
        // the last taker just outside it while shooters are already waiting.
        // Hami stayed unwalled for over thirty turns in a live King game:
        // three shooters stood on the ring and a full-health Man-at-Arms was
        // seven tiles away, yet the declaration waited until Mongolia opened
        // the war for us. Allow that short final march only against an open
        // city and only when the staged firepower dominates locally.
        let capturer_close_behind = committed_domination
            && !has_capturer
            && units.len() >= 3
            && ratio >= 1.60
            && g.city_at(objective)
                .is_some_and(|cid| g.cities[&cid].owner == target && g.cities[&cid].wall_hp <= 0)
            && g.player_unit_ids(pid).into_iter().any(|uid| {
                let unit = &g.units[&uid];
                let spec = &g.rules.units[unit.kind];
                spec.class == "military"
                    && spec.is_melee_capable()
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && unit.hp >= 70
                    && g.unit_strength(unit, false) >= 30.0
                    && (6..=8).contains(&g.wdist(unit.pos, objective))
            });
        let formation_ready = units.len() >= 3 || (units.len() >= 2 && ratio >= 1.60);
        let minimum_ratio = if committed_domination { 0.90 } else { 1.05 };
        formation_ready && (has_capturer || capturer_close_behind) && ratio + 1e-9 >= minimum_ratio
    }

    /// Own the diplomatic end of an appointed attack. Returning `true` means
    /// the frozen appointment consumed the war-opening decision this turn,
    /// even when it deliberately held for staging or a Formal-War timer.
    pub(super) fn timed_war_opening(&mut self, g: &mut Game, pid: usize, target: usize) -> bool {
        let Some(plan) = self
            .war_plan
            .as_ref()
            .filter(|plan| plan.target_player == target)
            .cloned()
        else {
            return false;
        };
        if self.urgent_victory_threat(g, target) {
            self.record_war_abort("victory denial superseded the appointment");
            self.war_plan = None;
            self.war_status = WarPackageStatus::default();
            return false;
        }

        // Denounce during mobilization so the legal preparation clock and the
        // physical march run together. A ready casus belli is never exercised
        // until the package itself reaches Strike.
        if plan.phase != WarPhase::Strike {
            if !matches!(plan.phase, WarPhase::Research) {
                if let Some(action @ Action::Denounce { .. }) =
                    self.preferred_war_opening(g, pid, target)
                {
                    let _ = g.apply(pid, &action);
                }
            }
            return true;
        }

        let status = self.war_package_status(g, pid, &plan);
        let complete = self.timed_war_launch_ready(g, pid, &plan, &status);
        if !complete {
            // Phase and package are recomputed at turn start, but a deal,
            // upgrade failure, or other earlier action can change the board.
            // The launch gate is repeated immediately before the declaration.
            return true;
        }
        // `coalition_before_war`: the package is complete; invite the
        // target's neighbours to a joint war and hold for the answer. See
        // `advanced/coalition.rs`.
        if self.coalition_invites_before_declaring(g, pid, target) {
            return true;
        }
        let Some(action) = self.preferred_war_opening(g, pid, target) else {
            return true;
        };
        let declares = matches!(
            action,
            Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }
        );
        // The appointment's Gold reserve is an upgrade bill, and this pass only
        // ever adds to the treasury, so it runs even while elective bilateral
        // spending is held.
        self.base.war_eve_liquidation(g, pid, &action);
        if g.apply(pid, &action).is_err() || !declares {
            return true;
        }

        if let Some(active) = self.war_plan.as_mut() {
            active.declared_turn = Some(g.turn);
            active.phase = WarPhase::Exploit;
        }
        self.war_census.declarations += 1;
        self.war_census.complete_package_declarations += 1;
        if let Some(tech_turn) = plan.tech_turn {
            let elapsed = g.turn.saturating_sub(tech_turn);
            self.war_census.tech_to_declaration_turns += elapsed;
            self.war_census.tech_to_declaration_samples.push(elapsed);
        }
        self.major_war_since = Some(g.turn);
        self.last_campaign_progress = g.turn;
        think!(self.journal(), Military, Strategy,
               "Launching the appointed attack on {}", g.players[target].civ;
               "four modern bodies and the breach package are staged at {:.2} local strength; objective city {} is pinned",
               status.local_strength_ratio, plan.objective_city);
        true
    }

    /// Return the cheapest city-state that an idle Envoy pool can take once a
    /// peace with `other` ends the Suzerain-derived war.
    ///
    /// Envoys run before diplomacy. Any pool that remains here therefore
    /// survived the ordinary positive-placement pass; this is a bridge-only
    /// way to turn that deliberately banked remainder into an immediate
    /// city-state investment. A direct city-state war does not qualify: peace
    /// with its Suzerain would not make Envoys legal there.
    fn banked_envoy_reclaim_after_peace(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
        plan: &StrategicPlan,
    ) -> Option<(usize, i64)> {
        if !self.bank_envoys {
            return None;
        }
        let held = g.players.get(pid)?.envoys_free;
        let active_campaign = plan.target_player == Some(other)
            || self
                .war_plan
                .as_ref()
                .is_some_and(|war| war.target_player == other);
        let hostile_suzerain = g.players.get(other).is_some_and(|player| {
            player.alive && !player.is_minor && !player.is_barbarian && !player.is_free_city
        });
        if held <= 0 || active_campaign || !hostile_suzerain {
            return None;
        }

        g.players
            .iter()
            .filter(|minor| {
                minor.alive
                    && minor.is_minor
                    && !minor.is_barbarian
                    && !minor.is_free_city
                    && g.has_met(pid, minor.id)
                    && g.is_at_war(pid, minor.id)
                    && !g.at_war.contains(&(pid.min(minor.id), pid.max(minor.id)))
                    && g.suzerain_of(minor.id) == Some(other)
            })
            .filter_map(|minor| {
                let ours = g.envoys_at(pid, minor.id);
                let rival = g
                    .players
                    .iter()
                    .filter(|player| {
                        player.id != pid
                            && player.alive
                            && !player.is_minor
                            && !player.is_barbarian
                            && !player.is_free_city
                    })
                    .map(|player| g.envoys_at(player.id, minor.id))
                    .max()
                    .unwrap_or_default();
                let needed = (3_i64.max(rival.saturating_add(1)) - ours).max(1);
                (held >= needed).then_some((minor.id, needed))
            })
            .min_by_key(|(minor, needed)| (*needed, *minor))
    }

    pub(super) fn advanced_diplomacy(&mut self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        // `recovery-peace-waits`: the current Recovery spell. See
        // `one_war::recovery_peace_ready`.
        if plan.strategy == GrandStrategy::Recovery {
            self.recovery_since.get_or_insert(g.turn);
        } else {
            self.recovery_since = None;
        }
        // `faith-buys-the-settler`: Monumentality first while seats are
        // still free; the ordinary choice fills any slot left. Exact no-op
        // while the gene is off. See `advanced/faith_buys_the_settler.rs`.
        self.dedicate_to_monumentality(g, pid, plan);
        if self.golden_dedication_serves_the_conquest {
            let age = g.players[pid].age.clone();
            for dedication in
                crate::ai::choose_conquest_dedications(g, pid, self.base.w.dedication_choice)
            {
                think!(self.journal(), Diplomacy, Decision,
                       "Dedicating the {age} age to {}", dedication.as_str();
                       "golden-dedication-serves-the-conquest: the Golden half a conquest \
                        spends first, ahead of the alphabetical Exodus of the Evangelists");
            }
        } else {
            crate::ai::choose_dedications(g, pid, self.base.w.dedication_choice);
        }
        self.propose_protective_friendship(g, pid);
        let incoming: Vec<u32> = g
            .pending_deals
            .iter()
            .filter(|deal| deal.to == pid && deal.expires >= g.turn)
            .map(|deal| deal.id)
            .collect();
        for deal_id in incoming {
            let Some((accept, peace, worth, from)) = g
                .pending_deals
                .iter()
                .find(|deal| deal.id == deal_id)
                .map(|deal| {
                    let worth = self.incoming_deal_value(g, pid, deal, plan);
                    let pins_objective = deal.peace
                        && (self.war_plan.as_ref().is_some_and(|war| {
                            war.phase == WarPhase::Exploit
                                && war.target_player == deal.from
                                && g.cities
                                    .get(&war.objective_city)
                                    .is_some_and(|city| city.owner == deal.from)
                        })
                            // `air-surge-2`: the wing's own front, as for our
                            // own offers. See `air_surge_holds_front`.
                            || self.air_surge_holds_front(g, pid, deal.from)
                            // `no-peace-with-a-launcher`: see
                            // `advanced/launcher_war.rs`.
                            || self.launcher_keeps_the_war(g, pid, deal.from)
                            // `holdout-keeps-the-faith-war`: see
                            // `advanced/holdout_faith_war.rs`.
                            || (deal.peace
                                && self.holdout_keeps_the_faith_war_with(g, pid, deal.from))
                            // `faith-war-before-the-flip`: see
                            // `advanced/faith_war_before_the_flip.rs`.
                            || (deal.peace
                                && self.faith_war_keeps_the_war(g, pid, deal.from)));
                    (
                        worth >= 0.0 && !pins_objective,
                        deal.peace,
                        worth,
                        deal.from,
                    )
                })
            else {
                continue;
            };
            if peace && !accept && self.holdout_keeps_the_faith_war_with(g, pid, from) {
                self.journal_holdout_keeps_the_faith_war(g, pid, from, true);
            }
            if peace && !accept && self.faith_war_keeps_the_war(g, pid, from) {
                self.journal_faith_war_keeps(g, pid, from, true);
            }
            if self.journal().wants(crate::reasoning::Level::Decision) {
                let who = g
                    .players
                    .get(from)
                    .map(|player| player.civ.clone())
                    .unwrap_or_else(|| format!("player {from}"));
                let kind = if peace { "peace offer" } else { "deal" };
                think!(self.journal(), Diplomacy, Decision,
                       "{} {who}'s {kind}", if accept { "Accepting" } else { "Refusing" };
                       "worth {worth:+.0} to the {} plan", plan.strategy.as_str());
            }
            let action = if accept {
                Action::AcceptDeal { deal: deal_id }
            } else {
                Action::RejectDeal { deal: deal_id }
            };
            if g.apply(pid, &action).is_ok() && accept && peace {
                // An accepted peace offer is the negotiated equivalent of
                // MakePeace: remember the stand-down so this AI does not
                // redeclare as soon as the mandatory treaty expires.
                self.peace_until = g.turn.saturating_add(30);
                self.major_war_since = None;
            }
        }
        if let Some(session) = g.congress.clone() {
            for resolution in session.resolutions {
                if resolution.ballots.contains_key(&pid) {
                    continue;
                }
                // In an explicit victory evaluation every major shares the
                // same objective. Civ VI awards a Diplomatic Victory Point for
                // predicting any winning resolution, including International
                // Aid, so repeated participation can end a healthy science or
                // culture race with the wrong victory. Explicit non-diplomatic
                // targets abstain; adaptive agents still participate normally.
                if self.victory_target.is_some()
                    && self.victory_target != Some(VictoryTarget::Diplomacy)
                    && g.emergency_proposal_for_resolution(&resolution.id)
                        .is_none()
                {
                    continue;
                }
                if let Some(choice) = self.congress_choice(g, pid, &resolution, plan.strategy) {
                    let votes = self.congress_ballot_size(g, pid, plan.strategy);
                    think!(self.journal(), Diplomacy, Decision,
                           "Voting {} on {}", plain(&choice), plain(&resolution.id);
                           "{votes} vote{} behind it, on the {} plan",
                           if votes == 1 { "" } else { "s" }, plan.strategy.as_str());
                    let _ = g.apply(
                        pid,
                        &Action::CongressVote {
                            resolution: Name::new(&resolution.id),
                            choice,
                            votes,
                        },
                    );
                }
            }
        }
        let denied_partner = plan.target_player.filter(|target| {
            plan.strategy == GrandStrategy::Conquest
                || g.is_at_war(pid, *target)
                || self.rival_victory_pressure(g, *target).progress >= 78
        });
        // A peace treaty is a hard block on `start_war`, so while one holds the
        // campaign target cannot be attacked and withholding trade from it buys
        // nothing — it only forfeits the value, and the chance to load the
        // rival up with contracts that the next declaration cancels. The
        // relationship denials below are deliberately left in place: an
        // Alliance or a Friendship is the one thing that would make the war
        // itself illegal when the treaty lapses.
        let denied_trade_partner =
            denied_partner.filter(|target| g.peace_treaty_until(pid, *target).is_none());
        // Quick deals can hide a Gold component inside a balanced quote. Hold
        // all elective bilateral spending while an exact upgrade bill is
        // reserved; trade resumes immediately when the appointment ends.
        if self.war_plan.is_none() {
            self.strategic_bilateral_trade(g, pid, denied_trade_partner, plan.strategy);
        }
        // `culture-threat-early`: a culture threat is denounced. See
        // `advanced/culture_strategy.rs`.
        if let Some(rival) = self.culture_threat_denunciation(g, pid) {
            think!(self.journal(), Diplomacy, Decision,
                   "Denouncing {}", g.players[rival].civ;
                   "their tourism is a culture threat");
        }
        // `science-threat-denial`: a science threat is denounced, on the
        // same primitive. See `advanced/science_threat_denial.rs`.
        if let Some(rival) = self.science_threat_denunciation(g, pid) {
            think!(self.journal(), Diplomacy, Decision,
                   "Denouncing {}", g.players[rival].civ;
                   "their space race is a science threat");
        }
        // `coalition_before_war`: an alliance with a neighbour of the war
        // desk's target, ahead of the stock cadence. See
        // `advanced/coalition.rs`.
        self.coalition_alliance_step(g, pid);
        // `military-alliance-shield`: a Military Alliance with the empire
        // fighting our enemy or the neighbour at our border. On a turn it
        // asks, the stock ask waits: the live host opens one alliance
        // session a batch (`hold_deals_beside_an_alliance`), and a held ask
        // would still be remembered as a refusal. See
        // `advanced/military_alliance_shield.rs`.
        let shield_asked = self.military_alliance_shield_step(g, pid, plan, denied_partner);
        // `research-alliance-first` runs inside `propose_strategic_alliance`:
        // the kind, the cadence, the ranking's science term and the partner
        // bar. See `advanced/research_alliance.rs`.
        if !shield_asked {
            self.propose_strategic_alliance(g, pid, plan, denied_partner);
        }
        // `embassy-finds-the-capital`: an Embassy to a rival whose original
        // capital we have never seen, the plan's target first; the stock
        // cadence below opens no second session on a turn it sends one. See
        // `advanced/embassy_capital.rs`.
        let embassy_sent = self.embassy_finds_the_capital_step(g, pid, plan.target_player);
        // Relationship mechanics must be part of a strategic AI turn too.
        // Send one mission to the best non-hostile major, preferring the
        // Embassy that supersedes a prior Delegation. This mirrors the basic
        // controller's cadence but uses the plan-aware relationship score.
        if !embassy_sent && (g.turn % 8 == pid as u32 % 8 || self.mission_cadence_due(g, pid)) {
            let mission = g
                .players
                .iter()
                .filter(|other| {
                    other.id != pid
                        && other.alive
                        && !other.is_minor
                        && !other.is_barbarian
                        && g.has_met(pid, other.id)
                        && !g.is_at_war(pid, other.id)
                        && Some(other.id) != denied_partner
                        && g.players[pid]
                            .grievances
                            .get(&other.id)
                            .copied()
                            .unwrap_or(0.0)
                            < 50.0
                        // `delegations-reach-every-rival`: skip a rival that
                        // already hosts our mission or was just asked.
                        && self.mission_partner_open(g, pid, other.id)
                })
                .max_by(|left, right| {
                    g.relationship_opinion(pid, left.id)
                        .total_cmp(&g.relationship_opinion(pid, right.id))
                        .then(right.id.cmp(&left.id))
                })
                .and_then(|partner| {
                    let floor = self.war_treasury_floor(g, pid);
                    let embassy_ready = g.players[pid]
                        .civics
                        .contains(&crate::name!("diplomatic_service"))
                        && g.players[pid].gold >= floor + 25.0
                        && !g
                            .diplomatic_mission_to(pid, partner.id)
                            .is_some_and(|mission| mission.kind == "embassy");
                    if embassy_ready {
                        Some(Action::SendEmbassy { player: partner.id })
                    } else if g.players[pid].gold >= floor + 10.0
                        && g.diplomatic_mission_to(pid, partner.id).is_none()
                    {
                        Some(Action::SendDelegation { player: partner.id })
                    } else {
                        None
                    }
                });
            if let Some(action) = mission {
                let _ = g.apply(pid, &action);
                self.record_mission_ask(g, &action);
            }
        }
        let my_power = g.military_power(pid);
        let rivals: Vec<usize> = g
            .players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_barbarian && g.has_met(pid, p.id))
            .map(|p| p.id)
            .collect();
        self.peace_offers.clear();
        self.peace_routed.clear();
        self.peace_asks_city.clear();
        self.religious_interception_war = self
            .religious_interception_war
            .filter(|(rival, _)| g.is_at_war(pid, *rival));
        // `faith-war-before-the-flip`: see
        // `advanced/faith_war_before_the_flip.rs`.
        self.faith_war_track(g, pid);
        for other in &rivals {
            // `air-surge-2`: the Domination wing's own front. See
            // `AdvancedAi::air_surge_holds_front`.
            let air_front = self.air_surge_holds_front(g, pid, *other);
            let appointed_objective = air_front
                || self.war_plan.as_ref().is_some_and(|war| {
                    war.phase == WarPhase::Exploit && war.target_player == *other
                });
            let siege_grace = self.domination_siege_has_grace(g, pid, *other);
            let fatigued = self.major_war_since.is_some_and(|started| {
                g.turn.saturating_sub(started) >= 24
                    && g.turn.saturating_sub(self.last_campaign_progress) >= 12
            }) && !self.domination_siege_is_progressing(g, pid, *other, plan)
                && !self.domination_siege_train_mobilizing(g, pid, *other, plan)
                && !self.domination_front_crushed(g, pid, *other)
                && !siege_grace
                // The conquest opening's war answers to the opening's own
                // peace (`conquest_sue_for_peace`). Live King
                // civvis-20261004T025448Z declared on Spain at turn 51 and
                // had 9 bodies on Barcelona's ring when this clause offered
                // "the war has stalled" peace at 81; Spain took it and the
                // game's one opening closed without a capture.
                && !self.conquest_opening_war(*other);
            let peace_pending = g.pending_deals.iter().any(|deal| {
                deal.peace
                    && ((deal.from == pid && deal.to == *other)
                        || (deal.from == *other && deal.to == pid))
                    && deal.expires >= g.turn
            });
            let envoy_reclaim = self.banked_envoy_reclaim_after_peace(g, pid, *other, plan);
            // `one_war_at_a_time`: peace on every second front, and on the
            // front once the tide has turned; the fatigue clause stands down
            // while the front still has something to take. See
            // `advanced/one_war.rs`.
            let one_war_peace = self.one_war_peace(g, pid, *other);
            let one_war_presses = self.one_war_presses(g, pid, *other);
            // `war-policy-via-board`: the `0.62` "outmatched" term becomes
            // "no Siege row against them is feasible AND the tide is
            // against us (or an urgent Defend row is unserved for three
            // turns)"; every other term stands. See
            // `advanced/war_policy.rs`.
            let policy_peace = self.war_policy_peace(g, pid, *other);
            let religious_interception = self.religious_interception_holds_war(g, pid, *other);
            let interception_handoff =
                self.religious_interception_handoff_peace(g, pid, *other, plan);
            // `faith-war-before-the-flip`: the gene's war has done its work.
            let faith_war_exit = self.faith_war_exit_peace(g, pid, *other, plan);
            let outmatched = if self.war_policy_via_board {
                policy_peace.is_some()
            } else {
                my_power < g.military_power(*other) * 0.62
            };
            // `the-giant-gets-the-peace-offer`: the war we are losing outright
            // is offered peace too. See `advanced/giant_peace.rs`.
            let outmatched = outmatched || self.giant_gets_the_peace_offer_from(g, pid, *other);
            // An explicit Science seat has no offensive use for a war it did
            // not appoint. A losing proposal can still be refused by the
            // attacker, so make the direct-peace condition part of the outer
            // offer gate as well; otherwise a merely outmatched (rather than
            // catastrophically outmatched) besieged city never reaches it.
            let science_defensive_peace = self.victory_target == Some(VictoryTarget::Science)
                && self.war_plan.is_none()
                && plan.strategy != GrandStrategy::Conquest
                && plan.threatened_city.is_some()
                && my_power < g.military_power(*other) * 0.85
                && g.peace_available_at(pid, *other).is_none();
            // `unchosen-war-takes-the-first-peace`: the turn this rival
            // declared a war the expansion cannot afford, once the host will
            // sign. `None` with the gene off. See `advanced/unchosen_war_peace.rs`.
            let unchosen_peace = self.unchosen_war_peace_since(g, pid, *other, plan);
            if g.is_at_war(pid, *other)
                && !g.emergency_war_pair(pid, *other)
                && !g.players[*other].is_minor
                && !peace_pending
                && !religious_interception
                // See `last_capital_war_kept`.
                && !self.last_capital_war_kept(g, pid, *other)
                && (outmatched
                    // The wing's front rides out a Recovery reading. An
                    // unfought surge stands itself down after two in a row
                    // (`AIR_SURGE_RECOVERY_LIMIT`); a fought one keeps its
                    // war unless the outmatched clause above fires.
                    // A home emergency does not buy peace with the rival a
                    // Domination seat is countering or has at its mercy.
                    // Live King civvis-20261003T093332Z (game 30) offered
                    // Brazil "this is not the war the recovery plan is
                    // fighting" at turn 121, at 661 power against 391, and
                    // declared on it again at 122.
                    || (plan.strategy == GrandStrategy::Recovery
                        && plan.target_player != Some(*other)
                        && !siege_grace
                        && !air_front
                        && !(self.active_victory_target(g) == Some(VictoryTarget::Domination)
                            && (self.domination_counter_target(g, pid, *other)
                                || self.domination_capital_prey(g, pid, *other)))
                        // Nor with the only land road to the target. See
                        // `war_holds_the_road`.
                        && !self.war_holds_the_road(g, pid, *other)
                        // Nor with a beaten rival in the fog. See
                        // `unseen_prey`.
                        && !self.unseen_prey(g, pid, *other)
                        // Nor a war we are winning when the Recovery plan
                        // fights no other. See `recovery_keeps_the_war`.
                        && !self.recovery_keeps_the_war(g, pid, *other, plan)
                        // Nor on a Recovery that has not stood. See
                        // `recovery_peace_ready`.
                        && self.recovery_peace_ready(g))
                    || (self.religion_sues_peace
                        && plan.strategy == GrandStrategy::Religion
                        && !appointed_objective)
                    || (!appointed_objective
                        && fatigued
                        && !one_war_presses
                        // See `stalled_peace_spares`.
                        && !self.stalled_peace_spares(g, pid, *other)
                        && g.player_city_ids(*other).len() > 1)
                    // See `peace_when_war_does_not_pay`: the same fatigue
                    // clause, without the appointed-objective exemption that
                    // kept the live King seat at war for 172 of 248 turns.
                    || (self.peace_when_war_does_not_pay
                        && fatigued
                        && !one_war_presses
                        && !air_front
                        && !self.stalled_peace_spares(g, pid, *other)
                        && g.player_city_ids(*other).len() > 1
                        && (!self.treasury_can_carry_a_war(g, pid)
                            || g.turn.saturating_sub(self.last_campaign_progress)
                                >= g.standard_duration(PEACE_STALL_TURNS)))
                    // Neither banked envoys nor a finished religious
                    // interception is worth the wing's front either.
                    || (envoy_reclaim.is_some() && !air_front)
                    // ★★ THE WING'S FRONT WAS A "SECOND FRONT". Live King
                    // 20261001T080758Z reached Advanced Flight at turn 156
                    // and started Bombers against Pella; the land campaign
                    // was aimed at the Zulu, so `one_war_at_a_time` offered
                    // Macedon peace every turn from 157 as its power fell
                    // from 167 to 18 against our thousand, Macedon took it at
                    // 167, and the surge stood down. The wing's own front is
                    // not a second front.
                    // ...except when the front's Domination purpose is
                    // complete. Live King civvis-20261003T040354Z took Rome
                    // at turn 198 (loyalty 100 by 203); the wing had been
                    // redirected to Ostia, so this suppressed the
                    // capital-secured peace every turn while the army
                    // cleared Rome's towns and Kongo -- the science leader,
                    // holding the last original capital we needed -- won a
                    // Technology victory at turn 216 from peace.
                    || (one_war_peace.is_some()
                        && (!air_front
                            || matches!(
                                one_war_peace,
                                Some(
                                    one_war::OneWarPeace::CapitalSecured
                                        | one_war::OneWarPeace::CapitalElsewhere
                                )
                            )))
                    || (interception_handoff && !air_front)
                    || faith_war_exit
                    || science_defensive_peace
                    || unchosen_peace.is_some())
            {
                // `peace-waits-for-the-host-minimum`: no offer to a war
                // declared on us before the host signs any; the bridge's
                // retry window would spend it. `None` with the gene off. See
                // `advanced/unchosen_war_peace.rs`.
                if let Some(age) = self.young_war_peace_waits(g, pid, *other) {
                    if self.journal().wants(crate::reasoning::Level::Decision) {
                        think!(self.journal(), Diplomacy, Decision,
                               "Holding the peace offer to {}", g.players[*other].civ;
                               "peace-waits-for-the-host-minimum: their war on us is {age} turn(s) old and the host signs none before {}; an ask now would hold the next one five turns",
                               g.standard_duration(super::unchosen_war_peace::PEACE_HOST_MINIMUM_STANDARD));
                    }
                    continue;
                }
                // `no-peace-with-a-launcher`: no peace with a rival past its
                // Mars base while a pad of its stands, unless our cities are
                // falling. `false` with the gene off. See
                // `advanced/launcher_war.rs`.
                if self.launcher_keeps_the_war(g, pid, *other) {
                    continue;
                }
                // `holdout-keeps-the-faith-war`: no peace with a faith's
                // founder while we are its last holdout and it is taking our
                // cities, unless our cities are falling. `false` with the gene
                // off. See `advanced/holdout_faith_war.rs`.
                if self.holdout_keeps_the_faith_war_with(g, pid, *other) {
                    self.journal_holdout_keeps_the_faith_war(g, pid, *other, false);
                    continue;
                }
                // `faith-war-before-the-flip`: no peace with the founder of
                // the gene's war while a city of ours is still being
                // converted to its faith, unless our cities are falling.
                // `false` with the gene off. See
                // `advanced/faith_war_before_the_flip.rs`.
                if self.faith_war_keeps_the_war(g, pid, *other) {
                    self.journal_faith_war_keeps(g, pid, *other, false);
                    continue;
                }
                // `prey-war-is-kept`: no peace with prey unless our cities
                // are falling or the one-war rule makes it for the victory
                // counter or a settled capital. Prey never outmatches us on
                // power, and the war board's "no siege is feasible" peace
                // (which `outmatched` reads under `war-policy-via-board`) is
                // the stall the gene keeps. `false` with the gene off. See
                // `advanced/prey_war_is_kept.rs`.
                if !science_defensive_peace
                    && !matches!(
                        one_war_peace,
                        Some(
                            one_war::OneWarPeace::VictoryThreat
                                | one_war::OneWarPeace::CapitalSecured
                                | one_war::OneWarPeace::CapitalElsewhere
                        )
                    )
                    && self.prey_war_is_kept_with(g, pid, *other)
                {
                    self.journal_prey_war_is_kept(g, pid, *other);
                    continue;
                }
                self.peace_offers.insert(*other);
                if policy_peace.is_some() {
                    self.census.war_policy_peace_offers += 1;
                }
                // Only a rout licenses a live tribute; every other reason
                // for the offer is white peace. See `AiReport::peace_routed`.
                // The board's term claims none: its peace is white.
                if (!self.war_policy_via_board && outmatched)
                    || matches!(one_war_peace, Some(one_war::OneWarPeace::Rout))
                {
                    self.peace_routed.insert(*other);
                }
                // `peace-asks-a-city`: a white offer made from strength asks
                // the rival to cede a town. See `PEACE_CITY_ASK_RATIO`. Not
                // on `unchosen-war-takes-the-first-peace`'s offer, which
                // exists to end the war.
                if unchosen_peace.is_none() && self.peace_asks_city_from_strength(g, pid, *other) {
                    self.peace_asks_city.insert(*other);
                }
                if let Some(peace) = one_war_peace {
                    let key = match peace {
                        one_war::OneWarPeace::SecondFront => "one_war:peace:second_front",
                        one_war::OneWarPeace::CapitalSecured => "one_war:peace:capital_secured",
                        one_war::OneWarPeace::CapitalElsewhere => "one_war:peace:capital_elsewhere",
                        one_war::OneWarPeace::VictoryThreat => "one_war:peace:victory_threat",
                        one_war::OneWarPeace::TideTurned => "one_war:peace:tide",
                        one_war::OneWarPeace::Rout => "one_war:peace:rout",
                    };
                    *g.players[pid].counters.entry(key.to_string()).or_insert(0) += 1;
                }
                if self.journal().wants(crate::reasoning::Level::Decision) {
                    let their_power = g.military_power(*other);
                    let because = if let Some(peace) = one_war_peace {
                        peace.reason().to_string()
                    } else if let Some((minor, needed)) = envoy_reclaim {
                        format!(
                            "the plan has no campaign there and {needed} banked Envoy{} can immediately take {} after the Suzerain war ends",
                            if needed == 1 { "" } else { "s" },
                            g.players[minor].civ
                        )
                    } else if outmatched {
                        policy_peace
                            .clone()
                            .unwrap_or_else(|| "outmatched".to_string())
                    } else if let Some(since) = unchosen_peace {
                        format!(
                            "unchosen-war-takes-the-first-peace: they declared at turn {since}, no siege of ours against them is feasible and we hold {} of the {} cities we want",
                            g.player_city_ids(pid).len(),
                            plan.desired_cities
                        )
                    } else if plan.strategy == GrandStrategy::Recovery {
                        "this is not the war the recovery plan is fighting".to_string()
                    } else if self.religion_sues_peace && plan.strategy == GrandStrategy::Religion {
                        "the religion plan cannot spread into a war".to_string()
                    } else if interception_handoff {
                        "the religious interception is over and another city is ready for a conquest campaign".to_string()
                    } else if faith_war_exit
                        && self.faith_strike_war_spares_the_stages(g, pid, *other)
                    {
                        // `faith-strike-spares-the-stages`: see
                        // `advanced/faith_strike.rs`.
                        "faith-strike-spares-the-stages: the heretic strike's war closes the stages our Rock Bands play, so it ends as soon as the host signs".to_string()
                    } else if faith_war_exit {
                        "faith-war-before-the-flip: no spreader of its faith has stood at our cities for five turns and no city of ours is converting".to_string()
                    } else {
                        "the war has stalled".to_string()
                    };
                    think!(self.journal(), Diplomacy, Decision,
                           "Offering peace to {}", g.players[*other].civ;
                           "{because}: {my_power:.0} power against their {their_power:.0}");
                }
                // A live peace order is only an offer. The host may refuse it
                // or require tribute, so its next exported war state must end
                // the war. The report above emits the offer without letting
                // this speculative pass turn the army's threat field off.
                if pid == crate::game::MIRRORED_SEAT && !g.host_observed.is_empty() {
                    continue;
                }
                // Keep this narrow: ordinary adaptive seats still negotiate,
                // and an appointed campaign or Emergency war retains its
                // bilateral commitment.
                if science_defensive_peace
                    && g.apply(pid, &Action::MakePeace { player: *other }).is_ok()
                {
                    self.peace_until = g.turn.saturating_add(30);
                    self.major_war_since = None;
                    if self.journal().wants(crate::reasoning::Level::Strategy) {
                        think!(self.journal(), Diplomacy, Strategy,
                               "Ending the defensive war with {}", g.players[*other].civ;
                               "the Science lane cannot afford a losing siege against a threatened home city");
                    }
                    continue;
                }
                // Peace between majors is bilateral. The former direct
                // MakePeace let an outmatched defender terminate a winning
                // invasion on the first legal turn, even when the conqueror
                // valued the campaign. Keep fighting until the recipient's
                // normal deal valuation accepts this offer.
                let _ = g.apply(
                    pid,
                    &Action::ProposeDeal {
                        player: *other,
                        give_gold: 0.0,
                        request_gold: 0.0,
                        open_borders: false,
                        friendship: false,
                        peace: true,
                        alliance: None,
                    },
                );
            }
        }
        // The host can keep an eliminated major alive and at war. That war
        // still matters tactically, but cannot veto a new capital campaign.
        let major_wars = self.one_war_enemies(g, pid).len();
        if major_wars > 0
            && matches!(
                plan.strategy,
                GrandStrategy::Conquest | GrandStrategy::Recovery
            )
        {
            // `purchases-keep-the-upkeep-reserve`: the levy leaves the
            // army's bill behind; zero, the shipped levy, while it is off.
            let floor = self.purchase_upkeep_floor(g, pid, 0.0);
            self.base.levy_city_state_military(g, pid, true, floor);
        }
        // `first-capital-strikes-the-open-window`: the first capital's
        // opening declares ahead of every other war this turn, the home
        // faith wars included. `false` with the gene off. See
        // `advanced/first_capital_strikes_the_open_window.rs`.
        let opening_first = self.first_capital_strikes_active();
        if opening_first && self.conquest_declaration(g, pid) {
            return;
        }
        // A visible spreader can supply an immediate religious counter even
        // when the founder's cities have not yet been discovered.
        if self.religious_interception_opening(g, pid) {
            return;
        }
        // `faith-strike-wipes-the-spreaders`: the war on a rival faith on the
        // turn several of its spreaders can be condemned at once. The turn's
        // one declaration. See `advanced/faith_strike.rs`.
        if self.faith_strike_opening(g, pid) {
            return;
        }
        // `faith-war-before-the-flip`: a defensive war on a founder whose
        // spreaders work our cities before our majority flips. The turn's one
        // declaration. See `advanced/faith_war_before_the_flip.rs`.
        if self.faith_war_before_the_flip_opening(g, pid) {
            return;
        }
        // `science-threat-denial`: close a denial war that has pillaged the
        // pad, or open one on a threat about to finish. A declaration here is
        // the turn's one declaration. See
        // `advanced/science_threat_denial.rs`.
        if self.science_denial_war_diplomacy(g, pid) {
            return;
        }
        // See `opportunistic_war`: close a raid that has paid, or open one on
        // a prize the board exposes this turn. A declaration here is the
        // turn's one declaration.
        // `first-capital-strikes-the-open-window`: no new raid while the first
        // capital's window stands. `false` with the gene off.
        if !self.first_capital_holds_the_raid(g) && self.opportunistic_war_diplomacy(g, pid, plan) {
            return;
        }
        // `early-conquest-opening`: the assembled force's declaration, and the
        // terms it asks for once the war stops paying. A declaration here is
        // the turn's one declaration. Exact no-op while off. See
        // `advanced/early_conquest.rs`.
        if !opening_first && self.conquest_declaration(g, pid) {
            return;
        }
        // `city_campaign`: a campaign whose every city is ours offers peace,
        // the way the raid closes when it has paid. See
        // `advanced/city_campaign.rs`.
        self.city_campaign_diplomacy(g, pid);
        // See `culture_embargo_target`: the turn's one declaration, on a
        // culture race at match point whose cities we have not found.
        if self.culture_embargo_war(g, pid) {
            return;
        }
        // See `air_surge_diplomacy_target` (`air-surge-2`): while the surge
        // beelines and arms, a lane seat assesses no rival, so without this
        // fallback the Arm-phase denounce below was unreachable and the
        // declaration paid the whole Formal-War clock after the wing was
        // ready.
        // See `second_front_waits_for_the_front`: a faith counter is declared
        // on while the plan, and the army, stay on the front's siege.
        // See `second_front_waits_for_its_war`: any second front still at
        // peace is declared on from here while the running war keeps the plan.
        // See `second_front_displaced` (`blocker-becomes-the-target`): a
        // second front the plan's running war displaced is declared on here
        // too.
        // A fresh two-sortie Culture package owns its narrow air opening.
        // It rechecks home, treasury, readiness and legality before the war.
        if self.open_culture_air_interception(g, pid, plan) {
            return;
        }
        let waiting_second = self.one_war_second_front(g, pid).filter(|rival| {
            !g.is_at_war(pid, *rival)
                && (self.second_front_waits_for_the_front(g, pid, *rival)
                    || self.second_front_waits_for_its_war(g, pid, *rival)
                    || self.second_front_displaced(g, pid, *rival, plan))
        });
        let Some(target) = waiting_second
            .or(plan.target_player)
            .or_else(|| self.air_surge_diplomacy_target())
        else {
            return;
        };
        // `first-capital-strikes-the-open-window`: while the first capital's
        // window stands, its opening owns the seat's declaration. `false`
        // with the gene off.
        if self.first_capital_holds_the_war(g, target, plan.strategy == GrandStrategy::Conquest) {
            return;
        }
        // `one_war_at_a_time`: the appointment and the surge refuse a second
        // front when they are made; this refuses it at the declaration, for
        // the war that arrived in between. See `advanced/one_war.rs`.
        if self.one_war_holds_declaration(g, pid, target) {
            *g.players[pid]
                .counters
                .entry("one_war:held_declarations".to_string())
                .or_insert(0) += 1;
            if self.journal().wants(crate::reasoning::Level::Strategy) {
                think!(self.journal(), Military, Strategy,
                       "Holding off war with {}", g.players[target].civ;
                       "one war at a time, and a major war is already being fought");
            }
            return;
        }
        if self.timed_war_opening(g, pid, target) {
            return;
        }
        // The air surge owns the declaration for its own objective. Like the
        // timed war it consumes the turn's one war-opening decision even when
        // it deliberately holds for the wing.
        if self.air_surge_opening(g, pid, target) {
            return;
        }
        let emergency_target = g
            .emergency_objective(pid)
            .is_some_and(|objective| objective.target == target);
        // An ancient rush is the same decision taken earlier and on smaller
        // numbers, so it waives the two gates that are calendar rather than
        // condition — the turn-35 floor and the second city — and keeps every
        // gate that is about the war itself. It is still subject to
        // `close_enough`, to a staged stack, and to the peace deadline.
        let rushing = self
            .early_rush_victim(g, pid)
            .is_some_and(|(victim, _)| victim == target);
        // `one_war_second_front`: the next capital's owner or an urgent clock
        // may be declared on beside the burning war.
        let second_front = self.one_war_second_front(g, pid) == Some(target);
        // `launched-racer-yields-the-capital`: the racer past its launch
        // holding the capital that completes Domination. Taking it wins
        // first, and nothing else stops a launched Expedition, so neither the
        // running war, the treasury nor the staging holds it. `false` with
        // the gene off. See `advanced/launched_racer.rs`.
        let launched_finish = self
            .launched_racer_finish(g, pid)
            .is_some_and(|(rival, _)| rival == target);
        if plan.strategy != GrandStrategy::Conquest
            // `faith-war-is-no-front`: a running faith war is not the war
            // that holds this declaration.
            || (!self
                .without_faith_war(g, pid, self.one_war_enemies(g, pid))
                .is_empty()
                && !second_front
                && !launched_finish)
            || (!rushing && g.turn < 35)
            || g.turn < self.peace_until
            || (!rushing && g.player_city_ids(pid).len() < 2)
            || g.is_at_war(pid, target)
            || (!emergency_target && !self.campaign_target_legal(g, pid, target))
        {
            return;
        }
        // See `war_needs_a_treasury`. `strategic_policies` already records
        // the shape of this failure -- "the counter-leader plan held 85 Gold
        // at -59 per turn before it declared" -- but only reacts to it with a
        // policy card, after the declaration. The hold used to be silent: live
        // Emperor civvis-20261014T155202Z held the war on India, its
        // Expedition launched, on 7 of turns 217-224 with no line at all.
        // `launcher-war-skips-the-reserve`: the war `launcher_war_opens`
        // names is not held for Gold. `false` with the gene off. See
        // `advanced/launcher_war.rs`.
        if !launched_finish
            && !self.war_is_affordable(g, pid)
            && !self.launcher_war_waives_the_reserve(g, pid, target)
        {
            self.journal_treasury_holds_the_war(g, pid, target);
            return;
        }
        let target_power = g.military_power(target);
        let close_enough = if waiting_second == Some(target) {
            // The plan's city is the front's; the waiting faith is in reach
            // when any city of its is.
            g.player_city_ids(target).into_iter().any(|cid| {
                let pos = g.cities[&cid].pos;
                Self::city_within_declaration_range(g, pid, pos)
                    || self.denial_reaches_far(g, pid, target, pos)
                    // See `capital_prey_reaches_far`.
                    || self.capital_prey_reaches_far(g, pid, target, pos)
            })
        } else {
            plan.target_city
                .and_then(|cid| g.cities.get(&cid))
                .is_some_and(|target_city| {
                    Self::city_within_declaration_range(g, pid, target_city.pos)
                        || self.denial_reaches_far(g, pid, target, target_city.pos)
                        // See `capital_prey_reaches_far`.
                        || self.capital_prey_reaches_far(g, pid, target, target_city.pos)
                })
        };
        let committed_domination = self.victory_target == Some(VictoryTarget::Domination);
        // An army that has reached the enemy border is the only practical
        // answer to a rival's terminal clock.  Keep the normal power margin
        // for elective wars, but do not discard a staged denial force merely
        // because it is outnumbered: when the threat is already at the same
        // threshold that authorizes a Surprise War, waiting for superiority
        // guarantees that the rival gets the final uncontested turns.
        let urgent_denial = self.urgent_victory_threat(g, target);
        // See `faith_counter_due`: a faith taking our cities is answered by the
        // war itself, like the religious match point below.
        let faith_counter_due = self.faith_counter_due(g, pid, target);
        // `my_power > target_power * 1.32 + 12` is an empire-wide comparison,
        // and at turn 40 both empires are three or four units, so the `+ 12`
        // alone can outweigh the whole ratio. What decides an ancient siege is
        // not the empires' totals but how many takers are standing at the
        // objective, which is exactly what `early_rush_stack_ready` counts.
        // `war-policy-via-board`: the board's Siege bill staged on the
        // objective's ring, no other major war, every Defend row served —
        // in place of the rush count, the campaign bill and the empire
        // ratio. `None` with the gene off. See `advanced/war_policy.rs`.
        let policy = self.war_policy_declaration(g, pid, target, plan);
        if matches!(policy, Some(Err(_))) {
            self.census.war_policy_declarations_held += 1;
        }
        // See `one_war::COUNTER_WAR_POWER_FLOOR`: against a faith, urgency
        // waives the war ratio but not the floor under it, and a staged bill
        // does not stand in for it either.
        let below_counter_floor = urgent_denial && self.counter_war_hopeless(g, pid, target);
        // See `urgent_denial_has_the_edge`: under the gene an urgent clock
        // waives the war ratio and the edge only at 1.5 times the rival's
        // steady power; short of it the declaration holds like an elective war.
        let urgent_edge = self.urgent_denial_has_the_edge(g, pid, target);
        if close_enough && urgent_denial && !urgent_edge {
            think!(self.journal(), Military, Detail,
                   "Holding the urgent denial on {}", g.players[target].civ;
                   "our {my_power:.0} against their {:.0} steady (< {:.1}×): an urgent war short of the edge was routed in G175 and staged wars under 1.5 times routed 13 of 17 on October 4-5",
                   self.steady_rival_power(g, target), one_war::DECLARATION_EDGE_RATIO);
        }
        let urgent_waiver = urgent_denial && urgent_edge;
        // `launcher-war-ignores-the-edge`: a racer past its Moon and short of
        // the launch is fought from 0.8 times its steady power, staged or
        // not; its pads are raided, not besieged. `false` with the gene off.
        // See `advanced/launcher_war.rs`.
        let launcher_ready =
            launched_finish || (!below_counter_floor && self.launcher_war_opens(g, pid, target));
        // See `road_blocker_front` (`blocker-becomes-the-target`): the army
        // already stands at the border the war opens, and the blocker has
        // passed the version-2 edge, so no staged siege or Board bill is
        // asked of it.
        let road_opening = self.road_blocker_front(g, pid) == Some(target);
        // See `capital_moves_on_second_front` (`capital-taken-moves-on`): the
        // army stands on the beaten rival's towns, the Board writes no Siege
        // row for a rival at peace, and the next capital's owner has passed
        // the version-2 edge, so no staged siege is asked of it.
        let moving_on = self.capital_moves_on_second_front(g, pid) == Some(target);
        let ready = !below_counter_floor
            && (urgent_waiver
                || faith_counter_due
                || road_opening
                || moving_on
                || if let Some(verdict) = &policy {
                    verdict.is_ok()
                } else if rushing {
                    plan.target_city
                        .and_then(|city| g.cities.get(&city))
                        .is_some_and(|city| self.early_rush_stack_ready(g, pid, target, city.id))
                } else if let Some(campaign) = self.campaign_launch_ready(g, pid, target, plan) {
                    // `city_campaign`: the city's own bill on the staging ring,
                    // spare included, in place of the empire ratio. See
                    // `advanced/city_campaign.rs`.
                    campaign
                } else if committed_domination {
                    my_power >= target_power * 0.85 && my_power >= 30.0
                } else {
                    my_power > target_power * 1.32 + 12.0
                });
        let staged = plan
            .target_city
            .and_then(|city| g.cities.get(&city))
            .is_some_and(|city| {
                self.campaign_staged_for_war(
                    g,
                    pid,
                    target,
                    city.pos,
                    // A staged rush stack is already at the objective and has
                    // counted its own takers, so hold it to the domination
                    // ratio rather than the elective-war one.
                    committed_domination || rushing,
                )
            });
        // `declaration-waits-for-the-breaker`: a staged army is not staged
        // for a walled city until a breaker stands on its ring. See
        // `siege_train::declaration_breaker_at_hand`.
        // `racer-blocker-declares`: a war on the racer's road blocker is
        // read at the blocker's city on the road -- its walls, its breach,
        // its first strike -- not at the running war's objective the plan
        // keeps. `None` with the gene off. See `advanced/silent_counter.rs`.
        let racer_road = self
            .racer_blocker_objective(g, pid)
            .filter(|(blocker, _)| *blocker == target)
            .map(|(_, city)| city);
        let objective = racer_road
            .or(plan.target_city)
            .and_then(|city| g.cities.get(&city))
            .map(|city| (city.id, city.pos));
        let breaker_absent = self.declaration_waits_for_the_breaker
            && objective.is_some_and(|(city, _)| !self.declaration_breaker_at_hand(g, pid, city));
        // `declaration-waits-for-the-breach`: a walled objective also waits
        // until the force by it could breach it — the siege's own entry test
        // read before the war rather than after it. It joins the breaker's
        // hold, so the same patience and the same exemptions apply: an
        // urgent denial, a faith counter or a rush is never held, and a war
        // declared on us never passes here. See
        // `siege_train::declaration_breach_reading`.
        let breach_reading = if self.declaration_waits_for_the_breach {
            objective.and_then(|(city, _)| self.declaration_breach_reading(g, pid, city))
        } else {
            None
        };
        let breach_short = breach_reading.is_some_and(|(turns, endurance, _)| {
            turns > endurance * siege_train::DECLARATION_BREACH_SHARE
        });
        let breaker_missing = breaker_absent || breach_short;
        // `racer-blocker-declares`: the road war answers an urgent racer, as
        // an urgent denial does, and opens passage rather than a siege, so
        // the blocker's walls do not hold it either.
        let holdable = breaker_missing
            && staged
            && !urgent_denial
            && !faith_counter_due
            && !rushing
            && racer_road.is_none();
        let breaker_held = self.declaration_hold_patience(
            g,
            objective.map(|(_, pos)| pos),
            breaker_missing,
            holdable,
        );
        if breaker_held && breach_short && !breaker_absent {
            if let (Some(city), Some((turns, endurance, guns))) = (
                objective.and_then(|(city, _)| g.cities.get(&city)),
                breach_reading,
            ) {
                think!(self.journal(), Military, Detail,
                       "Holding the declaration on {} for the breach", g.players[target].civ;
                       "{} stands behind {} walls; the force within {} tiles reads {turns:.1} turns \
                        against {endurance:.1} endurance with {guns} gun(s) fit — a war opened now \
                        gives it the turns to raise the next tier",
                       city.name, city.wall_hp, siege_train::MUSTER_BREACH_FAR;
                       city.pos);
            }
        } else if breaker_held {
            if let Some(city) = objective.and_then(|(city, _)| g.cities.get(&city)) {
                think!(self.journal(), Military, Detail,
                       "Holding the declaration on {} for a wall-breaker", g.players[target].civ;
                       "{} stands behind {} walls and no siege gun, ram or tower of ours is on its ring; \
                        a war opened now gives it the turns to raise the next tier",
                       city.name, city.wall_hp;
                       city.pos);
            }
        }
        let staged = staged && !breaker_held;
        // The Culture clock may expire before capture units reach the ring.
        // A concrete, safe Theater Square sortie can start denial meanwhile.
        let air_ready = !staged && self.urgent_culture_air_opening_ready(g, pid, target, plan);
        // A religious match point is countered by the war itself: Apostles
        // and Inquisitors may condemn the founder's units only at war, and the
        // army pillages its Holy Sites on the way. It need not wait for a
        // siege bill. Live King civvis-20261003T113755Z read "the army has not
        // finished staging" at turns 145-150 at 578-602 power against 210-362
        // while Khmer Buddhism took our cities, and lost at 153.
        let urgent_religion_counter = urgent_denial
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && self.rival_victory_pressure(g, target).strategy == GrandStrategy::Religion
            && my_power >= target_power;
        // See `faith_counter_has_the_edge`: under the gene the urgent counter
        // needs the edge too, as `faith_counter_due` already does.
        // See `counter_war_has_the_emperor_edge`
        // (`counter-war-needs-the-emperor-edge`).
        let faith_edge = self.faith_counter_has_the_edge(g, pid, target)
            && self.counter_war_has_the_emperor_edge(g, pid, target);
        if close_enough && urgent_religion_counter && !faith_edge {
            if self.faith_counter_has_the_edge(g, pid, target) {
                // Only `counter-war-needs-the-emperor-edge` holds it.
                think!(self.journal(), Military, Detail,
                       "Holding the faith counter on {}", g.players[target].civ;
                       "{my_power:.0} power against their steady {:.0}: an unstaged counter war needs {:.1} times their power; Emperor G188 and G192 opened theirs at 1.6 and were routed",
                       self.steady_rival_power(g, target), one_war::COUNTER_WAR_EMPEROR_EDGE);
            } else {
                think!(self.journal(), Military, Detail,
                       "Holding the faith counter on {}", g.players[target].civ;
                       "{my_power:.0} power against their steady {:.0}: a counter war short of {:.1} times their power took no city in October 4-5's three",
                       self.steady_rival_power(g, target), one_war::DECLARATION_EDGE_RATIO);
            }
        }
        let religion_counter_ready = (urgent_religion_counter && faith_edge) || faith_counter_due;
        // See `culture_counter_due`.
        let culture_counter_ready = !staged && self.culture_counter_due(g, pid, target);
        // See `overwhelming_power_declares`: at that ratio the war itself
        // writes the Siege row that brings the army to the ring.
        // `declaration-waits-for-the-breach`: see `hold_bars_the_unstaged_paths`.
        let unstaged_open = !self.hold_bars_the_unstaged_paths(breaker_held);
        let overwhelming_ready = !staged
            && unstaged_open
            && objective
                .is_some_and(|(_, pos)| self.overwhelming_power_declares(g, pid, target, pos));
        // See `declaration_has_the_edge`: the plain staged war needs the edge;
        // an urgent clock and the counters' own waivers do not.
        let edge =
            urgent_waiver || faith_counter_due || self.declaration_has_the_edge(g, pid, target);
        // `crushing-steady-passes-the-edge`: journal the staged declarations
        // the gene alone lets through.
        if self.crushing_steady_passes_the_edge
            && self.declaration_needs_the_edge_2
            && close_enough
            && ready
            && staged
            && edge
            && !urgent_waiver
            && !faith_counter_due
        {
            let reading = self.declaration_edge_2(g, pid, target);
            if !reading.passes() && reading.crushing_steady() {
                think!(self.journal(), Military, Detail,
                       "The edge reads a crushing steady power on {}", g.players[target].civ;
                       "crushing-steady-passes-the-edge: {:.2} times their steady power ({:.0}) and {:.2} times their 30-turn peak ({:.0}), at {:.0} Production against their {:.0}",
                       reading.steady_ratio(), reading.steady, reading.peak_ratio(), reading.peak,
                       reading.our_production, reading.their_production);
            }
        }
        // `gutted-rival-passes-the-edge`: journal the staged declarations
        // the gene alone lets through.
        if self.gutted_rival_passes_the_edge
            && self.declaration_needs_the_edge_2
            && close_enough
            && ready
            && staged
            && edge
            && !urgent_waiver
            && !faith_counter_due
        {
            let reading = self.declaration_edge_2(g, pid, target);
            if !reading.passes() && !reading.crushing_steady() && reading.gutted() {
                think!(self.journal(), Military, Detail,
                       "The edge reads a gutted {}", g.players[target].civ;
                       "gutted-rival-passes-the-edge: its steady power {:.0} is {:.0}% of its 30-turn peak {:.0}; ours {:.0}",
                       reading.steady, 100.0 * reading.steady / reading.peak.max(1.0), reading.peak, reading.ours);
            }
        }
        if close_enough && ready && staged && !edge {
            if self.declaration_needs_the_edge_2 {
                let reading = self.declaration_edge_2(g, pid, target);
                think!(self.journal(), Military, Detail,
                       "Holding off war with {}", g.players[target].civ;
                       "{:.2} times their 30-turn peak ({:.0}) and {:.2} times their steady power ({:.0}), at {:.0} Production against their {:.0}: a staged war needs {:.1} times the peak, or {:.1} times the steady power against a rival out-produced",
                       reading.peak_ratio(), reading.peak, reading.steady_ratio(), reading.steady,
                       reading.our_production, reading.their_production,
                       one_war::DECLARATION_PEAK_EDGE_RATIO, one_war::DECLARATION_EDGE_RATIO);
            } else {
                think!(self.journal(), Military, Detail,
                       "Holding off war with {}", g.players[target].civ;
                       "{my_power:.0} power against their {target_power:.0}: a staged war short of {:.1} times their power took a city of theirs within 20 turns in 2 of 25 live declarations",
                       one_war::DECLARATION_EDGE_RATIO);
            }
        }
        let staged = staged && edge;
        let road_opening_ready = !staged && unstaged_open && road_opening;
        let moving_on_ready = !staged && unstaged_open && moving_on;
        // `unwalled-target-declares-into-the-strike`: an offensive
        // declaration on an objective with no walls waits, on its own
        // patience, until units that can strike it this turn would take
        // FIRST_STRIKE_SHARE of its health — the rival buys walls on its
        // first turn at war. Counters, denials and rushes are never held.
        let first_strike = if self.unwalled_target_declares_into_the_strike
            && !urgent_denial
            && !faith_counter_due
            && !rushing
        {
            objective.and_then(|(city, _)| {
                let health = f64::from(g.cities.get(&city)?.hp);
                self.first_strike_blows(g, pid, city)
                    .map(|blows| (blows, health))
            })
        } else {
            None
        };
        let offensive = staged || overwhelming_ready || road_opening_ready || moving_on_ready;
        let strike_short = close_enough
            && ready
            && offensive
            && first_strike
                .is_some_and(|(blows, health)| blows < health * siege_train::FIRST_STRIKE_SHARE);
        let strike_held =
            self.first_strike_hold_patience(g, objective.map(|(_, pos)| pos), strike_short);
        if strike_held {
            if let (Some(city), Some((blows, health))) = (
                plan.target_city.and_then(|city| g.cities.get(&city)),
                first_strike,
            ) {
                think!(self.journal(), Military, Detail,
                       "Holding the declaration on {} for a first-turn strike", g.players[target].civ;
                       "{} has no walls; the units that can strike it this turn bring {blows:.0} of its \
                        {health:.0} health, under {:.0}% — a war opened now gives it a turn to buy walls",
                       city.name, siege_train::FIRST_STRIKE_SHARE * 100.0;
                       city.pos);
            }
        }
        let staged = staged && !strike_held;
        let overwhelming_ready = overwhelming_ready && !strike_held;
        let road_opening_ready = road_opening_ready && !strike_held;
        let moving_on_ready = moving_on_ready && !strike_held;
        let opens = close_enough
            && ((ready
                && (staged
                    // `prey-is-declared-on`: see `advanced/prey_declaration.rs`.
                    || self.prey_is_declared_on_now(g, pid, target)
                    || air_ready
                    || religion_counter_ready
                    || culture_counter_ready
                    || overwhelming_ready
                    || road_opening_ready
                    || moving_on_ready))
                || launcher_ready);
        // `declaration-needs-production-parity`: every opening above -- the
        // staged edge, the overwhelming waiver, the rush stack, and the
        // urgent, culture and faith counters -- also needs our Production at
        // 0.8 times the target's (`parity-reads-the-front`: its Production
        // around our cities and the objective). Says so itself.
        if opens
            && !launcher_ready
            && !self.declaration_has_production_parity(
                g,
                pid,
                target,
                objective.map(|(_, pos)| pos),
            )
        {
            return;
        }
        if opens {
            // `coalition_before_war`: invite the target's neighbours to a
            // joint war first, and hold while an answer is due. See
            // `advanced/coalition.rs`. A launcher war does not wait on it.
            if !launcher_ready && self.coalition_invites_before_declaring(g, pid, target) {
                return;
            }
            // `launcher-war-ignores-the-edge`: a denouncement's five turns
            // are five turns of launches; the launcher war opens by surprise.
            let opening = match self.preferred_war_opening(g, pid, target) {
                // `launched-racer-yields-the-capital`: no denouncement's five
                // turns either; see `advanced/launched_racer.rs`.
                Some(Action::Denounce { .. }) | None if launched_finish => {
                    self.launched_racer_opening(g, pid, target, my_power)
                }
                Some(Action::Denounce { .. }) | None if launcher_ready => {
                    self.launcher_war_opening(g, pid, target, my_power)
                }
                opening => opening,
            };
            if let Some(mut action) = opening {
                // See `STRIKE_WHEN_STAGED_RATIO`.
                if let Some(surprise) = self.strike_when_staged(g, pid, target, &action, staged) {
                    action = surprise;
                }
                let denounce_first = matches!(action, Action::Denounce { .. });
                // `prey-waits-for-the-march`: a war the prey waiver alone
                // opens waits for the field army's march; a denouncement's
                // five turns run meanwhile. See `advanced/prey_declaration.rs`.
                let prey_alone = !launcher_ready
                    && !(staged
                        || air_ready
                        || religion_counter_ready
                        || culture_counter_ready
                        || overwhelming_ready
                        || road_opening_ready
                        || moving_on_ready);
                if (self.prey_waits_for_the_march || self.prey_waits_for_the_march_2)
                    && prey_alone
                    && !denounce_first
                {
                    if let Some((city, pos)) = objective {
                        let name = g
                            .cities
                            .get(&city)
                            .map_or_else(String::new, |c| c.name.clone());
                        let tag = self.prey_march_tag();
                        let bar = self.prey_march_bar();
                        match self.prey_march_reading(g, pid, target, pos) {
                            Some(prey_declaration::PreyMarch::Hold(median, held)) => {
                                think!(self.journal(), Military, Strategy,
                                       "Holding the prey war on {} for the march", g.players[target].civ;
                                       "{tag}: the field army's median stands {median} tiles from {name}, over the {bar} the waiver asks; held {held} of {} turns",
                                       prey_declaration::PREY_MARCH_PATIENCE;
                                       pos);
                                return;
                            }
                            Some(prey_declaration::PreyMarch::HoldForGun(gun, held)) => {
                                let stands = if gun == i32::MAX {
                                    "no siege gun of ours is fielded".to_string()
                                } else {
                                    format!("our nearest siege gun stands {gun} tiles out")
                                };
                                think!(self.journal(), Military, Strategy,
                                       "Holding the prey war on {} for a siege gun", g.players[target].civ;
                                       "{tag}: the field army stands within {bar} tiles of {name}, but {stands}, over the {}; held {held} of {} turns",
                                       prey_declaration::PREY_MARCH_GUN_TILES, prey_declaration::PREY_MARCH_PATIENCE;
                                       pos);
                                return;
                            }
                            Some(prey_declaration::PreyMarch::Patience(median)) => {
                                think!(self.journal(), Military, Detail,
                                       "The prey war on {} stops waiting for the march", g.players[target].civ;
                                       "{tag}: {} turns held, and the field army's median stands {median} tiles from {name}",
                                       prey_declaration::PREY_MARCH_PATIENCE;
                                       pos);
                            }
                            Some(prey_declaration::PreyMarch::Stalled(median)) => {
                                think!(self.journal(), Military, Detail,
                                       "The prey war on {} stops waiting for the march", g.players[target].civ;
                                       "{tag}: neither the field army's median ({median} tiles from {name}) nor our nearest siege gun came closer in {} turns",
                                       prey_declaration::PREY_MARCH_STALL;
                                       pos);
                            }
                            Some(prey_declaration::PreyMarch::Within(_)) | None => {}
                        }
                    }
                }
                if denounce_first && self.journal().wants(crate::reasoning::Level::Strategy) {
                    // The opening is a denouncement; the war is the Formal War
                    // it licenses five turns on. Say so: the line used to read
                    // "Declaring war" while only the denouncement was sent.
                    think!(self.journal(), Military, Strategy,
                    "Denouncing {} before the war", g.players[target].civ;
                    "{my_power:.0} power against their {target_power:.0}; a Formal War opens once the denouncement's five turns have passed");
                } else if self.journal().wants(crate::reasoning::Level::Strategy) {
                    let casus = match &action {
                        Action::DeclareWarWithCasusBelli { casus_belli, .. } => {
                            format!(" under a {} casus belli", plain(casus_belli))
                        }
                        _ => String::new(),
                    };
                    let racer_reason: String;
                    let readiness = if air_ready {
                        "a ready aircraft can disrupt the rival's Theater Square immediately"
                    } else if culture_counter_ready {
                        "the war ends the open borders and trade route that carry their tourism to us while the army stages"
                    } else if !staged && religion_counter_ready {
                        // The religion counter declares without a staged
                        // siege; the line read "staged" for 41 of 71 such
                        // October 4-5 declarations with 0-2 bodies on the ring.
                        "the war lets the army condemn the faith taking our cities, before any siege is staged"
                    } else if overwhelming_ready {
                        "an overwhelming army needs no staged siege: the war writes the Siege row that brings it to the ring"
                    } else if road_opening_ready
                        && self.stood_down_blocker_front(g, pid) == Some(target)
                    {
                        "its closed borders shut the road to a siege we stood down, and the war opens it"
                    } else if let Some((_, racer, pressure)) = self
                        .racer_road_blocker(g, pid)
                        .filter(|(blocker, _, _)| *blocker == target)
                    {
                        // See `racer_road_blocker` (`racer-blocker-declares`).
                        let at = objective
                            .and_then(|(city, _)| g.cities.get(&city))
                            .map(|city| format!(" at {}", city.name))
                            .unwrap_or_default();
                        racer_reason = format!(
                            "racer-blocker-declares: they shut the only land road to {} ({} {}%, urgent), and the war opens it{at}",
                            g.players[racer].civ,
                            pressure.strategy.as_str(),
                            pressure.progress
                        );
                        racer_reason.as_str()
                    } else if road_opening_ready {
                        // See `closed_border_front` (`closed-border-blocker-declares`).
                        "its closed borders have held our siege's march three turns at half our power or under, and the war opens the road"
                    } else if moving_on_ready {
                        "the beaten rival's capital is ours and it refuses peace, so the army moves on to the next capital"
                    } else if launched_finish && !staged {
                        // See `launched_racer_finish`.
                        "launched-racer-yields-the-capital: its Exoplanet Expedition is launched and it holds the last capital Domination needs, so taking that capital wins first"
                    } else if launcher_ready && !staged {
                        // See `launcher_war_opens`.
                        "launcher-war-ignores-the-edge: past its Moon with a pad standing, the war raids its pads rather than besieging a city"
                    } else {
                        "the army is staged within reach of the first objective"
                    };
                    think!(self.journal(), Military, Strategy,
                    "Declaring war on {}", g.players[target].civ;
                    "{my_power:.0} power against their {target_power:.0}, {readiness}{casus}{}",
                    if urgent_denial {
                        ", and they are close enough to winning that waiting loses it"
                    } else {
                        ""
                    });
                }
                self.base.war_eve_liquidation(g, pid, &action);
                if g.apply(pid, &action).is_ok() {
                    self.air_surge_join_declared_war(g, pid, target);
                }
            }
        } else if self.journal().wants(crate::reasoning::Level::Detail) {
            // Not opening a war is a decision too, and the observer cannot see
            // it any other way: an army that sits still for thirty turns looks
            // identical to one with no plan.
            let blocker = if !close_enough {
                "no city of theirs is within 18 tiles of one of mine".to_string()
            } else if below_counter_floor {
                format!(
                    "their faith is close to winning, but a war at under {:.0}% of their power cannot stop it",
                    one_war::COUNTER_WAR_POWER_FLOOR * 100.0
                )
            } else if !ready {
                match &policy {
                    Some(Err(reason)) => reason.clone(),
                    _ => "the army is not strong enough yet".to_string(),
                }
            } else {
                "the army has not finished staging".to_string()
            };
            think!(self.journal(), Military, Detail,
                   "Holding off war with {}", g.players[target].civ;
                   "{blocker}; {my_power:.0} power against their {target_power:.0}");
        }
    }

    /// The live bank is a reserve for discovery, not an unlimited vault. Keep
    /// its normal floor small, but preserve the exact price of the cheapest
    /// city-state that a negotiated Suzerain peace could immediately reclaim.
    ///
    /// Production and Envoy allocation run before diplomacy, so the pool must
    /// retain the option until that pass decides whether the peace is
    /// acceptable. Keep this check local: the recently shipped peace helper is
    /// deliberately left intact rather than mechanically refactored here.
    pub(super) fn banked_envoy_liquidity_reserve(&self, g: &Game, pid: usize) -> i64 {
        if !self.bank_envoys {
            return 0;
        }
        // A campaign that names the hostile Suzerain deliberately keeps that
        // war: `banked_envoy_reclaim_after_peace` will not offer it away just
        // to reclaim the city-state. Such a minor cannot justify holding more
        // than the ordinary discovery reserve. Keep this eligibility check
        // local, alongside the other reserve-specific checks, rather than
        // weakening the campaign-protection contract in the peace helper.
        let campaign_owns_suzerain = |suzerain| {
            self.plan
                .as_ref()
                .is_some_and(|plan| plan.target_player == Some(suzerain))
                || self
                    .war_plan
                    .as_ref()
                    .is_some_and(|war| war.target_player == suzerain)
        };
        let reclaim = g
            .players
            .iter()
            .filter_map(|minor| {
                if !minor.alive
                    || !minor.is_minor
                    || minor.is_barbarian
                    || minor.is_free_city
                    || !g.has_met(pid, minor.id)
                    || !g.is_at_war(pid, minor.id)
                    || g.at_war.contains(&(pid.min(minor.id), pid.max(minor.id)))
                {
                    return None;
                }
                let suzerain = g.suzerain_of(minor.id)?;
                let owner = g.players.get(suzerain)?;
                if suzerain == pid
                    || !owner.alive
                    || owner.is_minor
                    || owner.is_barbarian
                    || owner.is_free_city
                    || campaign_owns_suzerain(suzerain)
                {
                    return None;
                }
                let ours = g.envoys_at(pid, minor.id);
                let rival = g
                    .players
                    .iter()
                    .filter(|player| {
                        player.id != pid
                            && player.alive
                            && !player.is_minor
                            && !player.is_barbarian
                            && !player.is_free_city
                    })
                    .map(|player| g.envoys_at(player.id, minor.id))
                    .max()
                    .unwrap_or_default();
                Some((3_i64.max(rival.saturating_add(1)) - ours).max(1))
            })
            .min()
            .unwrap_or_default();
        BANKED_ENVOY_LIQUIDITY_RESERVE.max(reclaim)
    }

    /// Nobel Peace scores Favor generated while its host clock is live. A
    /// suzerainty is therefore a new per-turn Favor stream, not merely the
    /// generic city-state yield the envoy scorer already prices.
    ///
    /// Credit only a route the current envoy pool can finish. The placements
    /// all occur in this action loop, so the new suzerainty starts paying on
    /// the next turn; divide that one concrete stream across the ordinary
    /// Envoys still required, which keeps every step toward the same immediate
    /// outcome valuable without inventing future Influence or Envoys.
    fn nobel_peace_suzerain_score_value(&self, g: &Game, pid: usize, envoys_needed: i64) -> i64 {
        if envoys_needed <= 0 || g.players[pid].envoys_free < envoys_needed {
            return 0;
        }
        (self.nobel_peace_favor_score_value(g, pid, g.suzerain_diplomatic_favor_per_turn(pid), 1.0)
            / envoys_needed as f64)
            .round() as i64
    }

    pub(super) fn advanced_envoys(
        &self,
        g: &mut Game,
        pid: usize,
        strategy: GrandStrategy,
        denied_rival: Option<usize>,
    ) {
        let liquidity_reserve = self.banked_envoy_liquidity_reserve(g, pid);
        let mut placed = 0usize;
        let budget = if self.envoys_open_the_siege_road {
            self.envoy_order_budget.unwrap_or(usize::MAX)
        } else {
            usize::MAX
        };
        while g.players[pid].envoys_free > 0 && placed < budget {
            let target = g
                .players
                .iter()
                .filter(|minor| g.can_send_envoy(pid, minor.id))
                // `city-state-before-the-first-capital`: no envoy to the city
                // the opening is taking. `false` with the gene off.
                .filter(|minor| !self.city_state_stone_spares_envoys(minor.id))
                .map(|minor| {
                    let mine = g.envoys_at(pid, minor.id);
                    let rival = g
                        .players
                        .iter()
                        .filter(|p| !p.is_minor && !p.is_barbarian && p.id != pid)
                        .map(|p| g.envoys_at(p.id, minor.id))
                        .max()
                        .unwrap_or(0);
                    let needed = (3_i64.max(rival + 1) - mine).max(1);
                    let kind = g.cs_type(&minor.civ);
                    let alignment = match (strategy, kind) {
                        (GrandStrategy::Science, "scientific") => 10,
                        (GrandStrategy::Culture, "cultural") => 10,
                        (GrandStrategy::Religion, "religious") => 12,
                        (GrandStrategy::Diplomacy, _) => 10,
                        (GrandStrategy::Conquest, "militaristic") => 10,
                        (GrandStrategy::Expansion, "trade") => 8,
                        (_, "trade") => 4,
                        _ => 2,
                    };
                    let unique_alignment = match (strategy, minor.civ.as_str()) {
                        (GrandStrategy::Science, "Geneva") => 14,
                        (GrandStrategy::Science | GrandStrategy::Conquest, "Hattusa") => 11,
                        (GrandStrategy::Science | GrandStrategy::Culture, "Stockholm") => 10,
                        (GrandStrategy::Conquest, "Kabul") => 14,
                        (GrandStrategy::Conquest | GrandStrategy::Expansion, "Carthage") => 10,
                        (GrandStrategy::Expansion | GrandStrategy::Recovery, "Mohenjo-Daro") => 11,
                        (GrandStrategy::Religion, "Yerevan") => 15,
                        (GrandStrategy::Religion | GrandStrategy::Culture, "Kandy") => 12,
                        (GrandStrategy::Expansion | GrandStrategy::Recovery, "Zanzibar") => 11,
                        (_, "Zanzibar") if g.players[pid].civ == "Aztec" => 12,
                        (
                            GrandStrategy::Science
                            | GrandStrategy::Culture
                            | GrandStrategy::Conquest
                            | GrandStrategy::Expansion,
                            "Auckland",
                        ) => 9,
                        (
                            GrandStrategy::Religion
                            | GrandStrategy::Conquest
                            | GrandStrategy::Recovery,
                            "Valletta",
                        ) => 13,
                        (GrandStrategy::Culture, "Vilnius") => 14,
                        (_, "Stockholm" | "Zanzibar" | "Auckland" | "Valletta") => 5,
                        _ => 2,
                    };
                    let already_secure = g.suzerain_of(minor.id) == Some(pid) && mine > rival + 1;
                    let overfunded_uncontested = self.bank_envoys && already_secure && mine >= 6;
                    let shared_from_partner = g.suzerain_of(minor.id).is_some_and(|leader| {
                        leader != pid
                            && g.alliance_with(pid, leader).is_some_and(|alliance| {
                                alliance.kind == "economic" && alliance.level >= 3
                            })
                    });
                    let type_bonus_value = g
                        .next_envoy_type_bonus(pid, minor.id)
                        .map(|(envoys, yields)| {
                            (self.yield_value(yields, strategy) * 14.0 / envoys as f64).round()
                                as i64
                        })
                        .unwrap_or(0);
                    let denial = denied_rival
                        .is_some_and(|leader| g.suzerain_of(minor.id) == Some(leader))
                        as i64
                        * 140;
                    // The amortised type bonus dips at exactly one envoy: a
                    // fresh city-state buys its level-1 yields for a single
                    // envoy, while one already opened needs two more before it
                    // pays again. With the suzerainty itself priced at zero,
                    // the cheapest step is always to open a city-state the seat
                    // will never finish -- which is what a 20.5-envoy, 0.66-
                    // suzerainty census looks like from the inside. This term
                    // rises as the seat closes on the floor (180, then 90, then
                    // 180 at one away) instead of falling.
                    let suzerain_prize =
                        if self.price_the_suzerainty && g.suzerain_of(minor.id) != Some(pid) {
                            SUZERAIN_PRIZE / needed
                        } else {
                            0
                        };
                    let nobel_peace_suzerain_prize = if g.suzerain_of(minor.id) != Some(pid) {
                        self.nobel_peace_suzerain_score_value(g, pid, needed)
                    } else {
                        0
                    };
                    // `coalition_before_war`: the city-state's place next to
                    // the war desk's target. See `advanced/coalition.rs`.
                    let coalition = self.coalition_city_state_bonus(g, pid, minor.id, needed);
                    // `enemy_of_my_enemy`: the city-state's place ACROSS a
                    // rival. See `advanced/enemy_of_my_enemy.rs`.
                    let across = self.enemy_of_my_enemy_city_state_bonus(g, pid, minor.id, needed);
                    let score = (alignment + unique_alignment) * 10
                        + type_bonus_value
                        + denial
                        + suzerain_prize
                        + nobel_peace_suzerain_prize
                        + coalition
                        + across
                        - needed * 7
                        - if overfunded_uncontested {
                            UNCONTESTED_POST_TIER_ENVOY_PENALTY
                        } else {
                            already_secure as i64 * 80
                        }
                        - shared_from_partner as i64 * 300;
                    let score = if self.envoy_building_dividends {
                        let ours = g.suzerain_of(minor.id) == Some(pid);
                        g.envoy_investment_options(pid, minor.id)
                            .into_iter()
                            .map(|(placements, count, yields)| {
                                let economic = self.yield_value(yields, strategy) * 14.0;
                                let capture = !ours && count >= 3 && count > rival;
                                let defend = ours && mine == rival + 1 && count > mine;
                                // A rival's delegation does not tax Library or
                                // University income. Identity, denial and the
                                // unique prize pay only on a reachable takeover
                                // or defense, and a shared prize is not bought twice.
                                let political = if (capture || defend) && !shared_from_partner {
                                    (alignment + unique_alignment) * 10
                                        + if capture {
                                            denial
                                                + (suzerain_prize
                                                    + nobel_peace_suzerain_prize
                                                    + coalition
                                                    + across)
                                                    * needed
                                        } else {
                                            0
                                        }
                                } else {
                                    0
                                };
                                ((economic + political as f64) / placements as f64).round() as i64
                            })
                            .max()
                            .unwrap_or(0)
                    } else {
                        score
                    };
                    let passage = self.siege_road_envoy_package(g, pid, minor.id, budget - placed);
                    (
                        passage.is_some(),
                        std::cmp::Reverse(passage.unwrap_or(0)),
                        score,
                        std::cmp::Reverse(needed),
                        std::cmp::Reverse(minor.id),
                        minor.id,
                    )
                })
                .max()
                .map(|(passage, _, score, _, _, id)| (id, score, passage));
            let Some((target, score, passage)) = target else {
                break;
            };
            if !passage && self.envoy_building_dividends && score <= 0 {
                think!(self.journal(), Diplomacy, Decision,
                       "Saving envoys for a paying threshold";
                       "{} placements available; no reachable building dividend or worthwhile suzerainty",
                       g.players[pid].envoys_free);
                break;
            }
            // See `bank_envoys`: retain the bounded discovery/reclaim reserve
            // once nothing on the board is worth an Envoy. Any surplus has a
            // productive opportunity cost, so it reinforces the best available
            // city-state instead of accumulating indefinitely.
            if !passage
                && self.bank_envoys
                && score < 0
                && g.players[pid].envoys_free <= liquidity_reserve
            {
                think!(self.journal(), Diplomacy, Decision,
                       "Banking {} envoy{}", g.players[pid].envoys_free,
                       if g.players[pid].envoys_free == 1 { "" } else { "s" };
                       "the best placement left is {} at {score}: every met city-state is \
                        either securely ours past its paying tier or contested beyond its \
                        worth, so this {liquidity_reserve}-Envoy reserve stays ready for the \
                        next discovery or immediate reclaim",
                       g.players[target].civ);
                break;
            }
            if self.journal().wants(crate::reasoning::Level::Decision) {
                let mine = g.envoys_at(pid, target) + 1;
                let suzerain = g
                    .suzerain_of(target)
                    .and_then(|leader| g.players.get(leader))
                    .map(|leader| leader.civ.clone());
                let standing = match &suzerain {
                    Some(civ) if *civ == g.players[pid].civ => "already its Suzerain".to_string(),
                    Some(civ) => format!("{civ} holds it"),
                    None => "nobody is its Suzerain".to_string(),
                };
                think!(self.journal(), Diplomacy, Decision,
                       "Sending an envoy to {}", g.players[target].civ;
                       "a {} city-state worth {score} to the {} plan; {standing}, \
                        this makes {mine} envoy{}",
                       g.cs_type(&g.players[target].civ), strategy.as_str(),
                       if mine == 1 { "" } else { "s" });
            }
            if passage {
                think!(self.journal(), Diplomacy, Decision,
                       "Buying siege passage through {} with envoys", g.players[target].civ;
                       "the full suzerainty fits our remaining placements; its closed border held a march toward an existing enemy city");
            }
            if g.apply(pid, &Action::SendEnvoy { player: target }).is_err() {
                break;
            }
            placed += 1;
            if passage {
                *g.players[pid]
                    .counters
                    .entry("siege-road:envoys".to_string())
                    .or_insert(0) += 1;
            }
            if self.coalition_city_state_bonus(g, pid, target, 1) > 0 {
                *g.players[pid]
                    .counters
                    .entry("coalition:envoys".to_string())
                    .or_insert(0) += 1;
            }
            if self.enemy_of_my_enemy_city_state_bonus(g, pid, target, 1) > 0 {
                *g.players[pid]
                    .counters
                    .entry("eoe:envoys".to_string())
                    .or_insert(0) += 1;
            }
        }
    }

    pub(super) fn advanced_spies(&mut self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        // `culture-denial-heist`: see `Game::heist_reads_the_host_menu`.
        g.heist_reads_the_host_menu = self.culture_denial_heist;
        // `spies-steal-the-lead`: see `Game::spy_targets_read_the_host`.
        g.spy_targets_read_the_host = self.spies_steal_the_lead_on(plan);
        self.spy_orders_until.retain(|_, until| *until > g.turn);
        // The host's report outranks the blind mirror's window: a Spy idle
        // on a later turn than its order has finished or lost that order.
        // See `advanced/spy_patience_reads_the_host.rs`.
        self.release_spent_spy_patience(g);
        let patience_before = self.spy_orders_until.clone();
        // `science-threat-denial`: the threat model walks every rival's
        // cities, so it is read once for the whole pass rather than per
        // candidate city or per candidate mission. Both are empty when the
        // gene is off. See `advanced/science_threat_denial.rs`.
        let denial_pads = self.science_denial_pad_cities(g, pid);
        let denial_seats = self.science_threat_seats(g, pid);
        // `science-denial-spy-reads-the-leader`: the leading threat's launch
        // city, which outranks every other posting. `None` when the gene is
        // off.
        let denial_leader = self.science_denial_leading_pad_city(g, pid);
        // `science-denial-every-pad`: the rivals whose every pad is a target,
        // and the cities a spy disrupts those pads from. Both empty with the
        // gene off. See `advanced/science_denial_every_pad.rs`.
        let every_pad_threats = self.every_pad_threats(g, pid);
        let mut every_pads = Self::every_pad_cities(g, pid, &every_pad_threats);
        // …less the pad cities where the host refused us the disruption with
        // the pad standing (the board's tie filing naming the wrong city).
        self.every_pad_drop_refused(g, pid, &mut every_pads);
        // `culture-denial-heist`: the rivals racing to a Culture Victory and
        // their Theater Square cities. Both empty with the gene off. See
        // `advanced/culture_denial_heist.rs`.
        let culture_heist_targets = self.culture_heist_targets(g, pid);
        let culture_heist_cities = Self::culture_heist_cities(g, pid, &culture_heist_targets);
        // `racer-keeps-the-spies`: the space racers, the one the Spies go to,
        // and the owners whose cities a Spy stays in (every racer and every
        // culture racer). All empty with the gene off. See
        // `advanced/racer_keeps_the_spies.rs`.
        let racer_spy_racers = self.racer_spy_racers(g, pid);
        let racer_spy_leader = Self::racer_spy_leader(g, &racer_spy_racers);
        let racer_spy_stays: BTreeSet<usize> = if racer_spy_racers.is_empty() {
            BTreeSet::new()
        } else {
            racer_spy_racers
                .union(&culture_heist_targets)
                .copied()
                .collect()
        };
        let ids: Vec<u32> = g
            .spies
            .values()
            .filter(|spy| spy.owner == pid)
            .map(|spy| spy.id)
            .collect();
        let infiltrated_cities: BTreeSet<u32> = g
            .spies
            .values()
            .filter(|spy| spy.owner != pid && spy.captured_by.is_none())
            .filter_map(|spy| {
                spy.city
                    .filter(|city| g.cities.get(city).is_some_and(|city| city.owner == pid))
            })
            .collect();
        let home_defender = ids
            .iter()
            .copied()
            .filter(|spy| {
                g.spies[spy].city.is_some_and(|city| {
                    // A spy outlives the city it was posted to: that city can
                    // be razed while the agent is still assigned there, and
                    // indexing the map for it took the whole server down with
                    // it. The infiltration scan above already reads this the
                    // safe way.
                    g.cities.get(&city).is_some_and(|home| {
                        home.owner == pid
                            && (home.districts.contains_key(crate::name!("spaceport"))
                                || infiltrated_cities.contains(&city))
                    })
                })
            })
            .min();
        // `space-denial-promotes-the-rocket-scientist`: Rocket Scientist first
        // on a Culture or Domination seat whose spies answer a space race.
        // False with the gene off. See `advanced/space_denial_rocket_scientist.rs`.
        let rocket_scientist_first = self.denial_wants_the_rocket_scientist(g, pid);
        for spy_id in ids {
            // A Spy whose last order is still running is left alone; the
            // rebuilt mirror cannot see the running operation, so without
            // this the same order goes out every turn. See
            // `spy_mission_patience`.
            // `racer-keeps-the-spies`: …unless it stands in a racer's city
            // that is no longer the leader's, running only a Listening Post
            // or Gain Sources. Never taken with the gene off.
            if self.spy_mission_patience
                && self.spy_orders_until.contains_key(&spy_id)
                && !Self::racer_spy_moves_on(
                    g,
                    spy_id,
                    racer_spy_leader,
                    &racer_spy_racers,
                    self.racer_spy_search(),
                )
            {
                continue;
            }
            // `disrupt-waits-for-the-match-point`: no Disrupt Rocketry before
            // the racer's last projects. See `advanced/disrupt_waits_for_the_match_point.rs`.
            let legal = self.withhold_early_disrupts(g, spy_id, g.legal_spy_actions(pid, spy_id));
            if legal.is_empty() {
                continue;
            }
            // `racer-spies-search-for-the-pad`: a pad found or a city
            // searched where this Spy stands. Never with the gene off.
            self.racer_spy_note_the_search(g, spy_id, racer_spy_leader, &racer_spy_racers, &legal);
            let promotion_priority: &[&str] = match plan.strategy {
                GrandStrategy::Science => &[
                    "technologist",
                    "rocket_scientist",
                    "disguise",
                    "linguist",
                    "quartermaster",
                ],
                GrandStrategy::Culture => &[
                    "cat_burglar",
                    "con_artist",
                    "disguise",
                    "linguist",
                    "surveillance",
                ],
                GrandStrategy::Diplomacy => &[
                    "smear_campaign",
                    "polygraph",
                    "quartermaster",
                    "seduction",
                    "disguise",
                ],
                GrandStrategy::Conquest => &[
                    "license_to_kill",
                    "demolitions",
                    "guerrilla_leader",
                    "covert_action",
                    "ace_driver",
                ],
                _ => &[
                    "quartermaster",
                    "seduction",
                    "con_artist",
                    "technologist",
                    "linguist",
                ],
            };
            let denial_priority: Vec<&str>;
            let promotion_priority: &[&str] = if rocket_scientist_first {
                denial_priority = Self::rocket_scientist_first(promotion_priority);
                &denial_priority
            } else {
                promotion_priority
            };
            if let Some(action) = promotion_priority
                .iter()
                .find_map(|wanted| {
                    legal.iter().find(|action| {
                        matches!(action, Action::PromoteSpy { promotion, .. } if promotion == *wanted)
                    })
                })
                .or_else(|| {
                    legal
                        .iter()
                        .find(|action| matches!(action, Action::PromoteSpy { .. }))
                })
            {
                let _ = g.apply(pid, action);
                continue;
            }
            let current_city = g.spies.get(&spy_id).and_then(|spy| spy.city);
            if Some(spy_id) == home_defender
                && matches!(
                    plan.strategy,
                    GrandStrategy::Science | GrandStrategy::Recovery
                )
                && current_city.is_some_and(|city| g.cities[&city].owner == pid)
            {
                let spaceport = current_city.and_then(|city| {
                    g.cities[&city]
                        .districts
                        .iter()
                        .find_map(|(district, position)| {
                            (g.district_family(*district) == "spaceport").then_some(*position)
                        })
                });
                if let Some(action) = legal
                    .iter()
                    .filter(|action| {
                        matches!(action, Action::SpyMission { mission, .. } if mission == "counterspy")
                    })
                    .max_by_key(|action| match action {
                        Action::SpyMission { target, .. } => {
                            (Some(*target) == spaceport, std::cmp::Reverse(*target))
                        }
                        _ => unreachable!(),
                    })
                {
                    let _ = g.apply(pid, action);
                    self.spy_orders_until.insert(
                        spy_id,
                        g.turn + g.standard_duration(SPY_MISSION_ORDER_PATIENCE),
                    );
                    continue;
                }
            }
            // `science-denial-spy-reads-the-leader`: an idle spy standing in a
            // foreign city that is no science threat's leaves for the leading
            // threat's free launch city. Never taken when the gene is off,
            // since `denial_leader` is `None` then.
            if Self::science_denial_spy_reposts_to_the_leader(
                g,
                pid,
                spy_id,
                current_city,
                denial_leader,
                &denial_seats,
            ) {
                if let Some(action) = legal.iter().find(|action| {
                    matches!(action, Action::AssignSpy { city, .. } if Some(*city) == denial_leader)
                }) {
                    if g.apply(pid, action).is_ok() {
                        *g.players[pid]
                            .counters
                            .entry("denial_spy_posts".to_string())
                            .or_insert(0) += 1;
                    }
                    self.spy_orders_until.insert(
                        spy_id,
                        g.turn + g.standard_duration(SPY_TRAVEL_ORDER_PATIENCE),
                    );
                    continue;
                }
            }
            // `science-denial-every-pad`: an idle spy in a foreign city that
            // is no free pad city of a decisive science threat leaves for the
            // nearest one. Never taken with the gene off (`every_pads` empty).
            // `racer-keeps-the-spies`: …but not out of the leading racer's
            // city, where that gene posted it. Never held with it off.
            let in_racer_leaders_city = racer_spy_leader.is_some_and(|leader| {
                current_city
                    .and_then(|city| g.cities.get(&city))
                    .is_some_and(|city| city.owner == leader)
            });
            if let Some(pad_city) =
                Self::every_pad_repost(g, pid, spy_id, current_city, &every_pads)
                    .filter(|_| !in_racer_leaders_city)
            {
                if let Some(action) = legal.iter().find(
                    |action| matches!(action, Action::AssignSpy { city, .. } if *city == pad_city),
                ) {
                    if g.apply(pid, action).is_ok() {
                        *g.players[pid]
                            .counters
                            .entry("every_pad_posts".to_string())
                            .or_insert(0) += 1;
                    }
                    self.spy_orders_until.insert(
                        spy_id,
                        g.turn + g.standard_duration(SPY_TRAVEL_ORDER_PATIENCE),
                    );
                    continue;
                }
            }
            // `culture-denial-heist`: an idle spy in a foreign city that is no
            // free Theater Square city of a culture racer leaves for the
            // nearest one. Never taken with the gene off (`culture_heist_cities`
            // empty).
            if let Some(heist_city) =
                Self::culture_heist_repost(g, pid, spy_id, current_city, &culture_heist_cities)
            {
                if let Some(action) = legal.iter().find(
                    |action| matches!(action, Action::AssignSpy { city, .. } if *city == heist_city),
                ) {
                    if g.apply(pid, action).is_ok() {
                        *g.players[pid]
                            .counters
                            .entry("culture_heist_posts".to_string())
                            .or_insert(0) += 1;
                    }
                    self.spy_orders_until.insert(
                        spy_id,
                        g.turn + g.standard_duration(SPY_TRAVEL_ORDER_PATIENCE),
                    );
                    continue;
                }
            }
            // A spare spy supplies the wing's sight after the home defender,
            // exact science-pad and culture-heist postings have their turn.
            // The helper also preserves coverage of the leading racer.
            if let Some(action) = self.wing_capital_spy_action(
                g,
                pid,
                spy_id,
                plan,
                &legal,
                racer_spy_leader,
                &culture_heist_targets,
            ) {
                let wing_capital_spotting::WingSpyOrder::Act(action) = action else {
                    continue;
                };
                if g.apply(pid, &action).is_ok() {
                    let travel = matches!(action, Action::AssignSpy { .. });
                    let patience = if travel {
                        SPY_TRAVEL_ORDER_PATIENCE
                    } else {
                        SPY_MISSION_ORDER_PATIENCE
                    };
                    self.spy_orders_until
                        .insert(spy_id, g.turn + g.standard_duration(patience));
                    think!(self.journal(), Military, Decision,
                        "A Spy keeps sight for the Bomber wing";
                        "wing-capital-spotting: an idle spare {} while the wing reaches the active rival's capital",
                        if travel { "posts to the capital" } else { "runs a safe mission at the capital" });
                    continue;
                }
            }
            // `spies-steal-the-lead`: remember a foreign city whose host menu
            // offers this Spy no steal. Exact no-op with the gene off.
            if self.spies_steal_the_lead_on(plan) {
                self.steal_note_dry_city(g, pid, spy_id, current_city);
            }
            // `racer-keeps-the-spies`: an idle Spy outside the leading space
            // racer's cities (and any culture racer's, and any other racer's
            // where Disrupt Rocketry is offered) goes to the leader, before
            // any pad of it is seen. Never taken with the gene off
            // (`racer_spy_leader` is `None`).
            let racer_spy_searching = self.racer_spy_search().is_some_and(|search| {
                Self::racer_spy_searching(g, spy_id, current_city, racer_spy_leader, search, &legal)
            });
            // `spies-steal-the-lead`: a Science aim posts to a Campus city.
            if let Some(post) = Self::racer_spy_post_for(
                g,
                spy_id,
                current_city,
                racer_spy_leader,
                &racer_spy_racers,
                &culture_heist_targets,
                &legal,
                self.racer_spy_search(),
                self.spies_steal_the_lead_on(plan),
                &self.steal_dry_cities,
            ) {
                if let Some(action) = legal.iter().find(
                    |action| matches!(action, Action::AssignSpy { city, .. } if *city == post),
                ) {
                    if g.apply(pid, action).is_ok() {
                        *g.players[pid]
                            .counters
                            .entry("racer_spy_posts".to_string())
                            .or_insert(0) += 1;
                        let city_name = g.cities[&post].name.clone();
                        let racer = g.players[g.cities[&post].owner].civ.to_string();
                        if racer_spy_searching {
                            *g.players[pid]
                                .counters
                                .entry("racer_spy_searches".to_string())
                                .or_insert(0) += 1;
                            let here = current_city
                                .and_then(|city| g.cities.get(&city))
                                .map_or_else(String::new, |city| city.name.clone());
                            let landed = racer_spy_leader
                                .map_or(0, |leader| Self::science_denial_stages(g, leader));
                            let unseen = self
                                .racer_spy_unseen_cities(g, pid)
                                .map_or(0, |(_, unseen)| unseen);
                            think!(self.journal(), Military, Decision,
                                   "A Spy searches {racer}'s {city_name} for the Spaceport";
                                   "racer-spies-search-for-the-pad: {racer} has landed \
                                    {landed} space project(s) and Disrupt Rocketry is not \
                                    offered in {here}, so the pad is elsewhere; {unseen} of \
                                    its cities we have not seen");
                        } else {
                            think!(self.journal(), Military, Decision,
                                   "A Spy goes to {racer}'s {city_name}";
                                   "racer-keeps-the-spies: {racer} leads the space race; the Spy \
                                    waits there for Disrupt Rocketry instead of risking its slot \
                                    elsewhere");
                        }
                    }
                    self.spy_orders_until.insert(
                        spy_id,
                        g.turn + g.standard_duration(SPY_TRAVEL_ORDER_PATIENCE),
                    );
                    continue;
                }
            }
            let offensive = current_city
                .and_then(|city| g.cities.get(&city))
                .is_some_and(|city| city.owner != pid);
            if offensive {
                // `disrupt-before-the-sources`: in a racer's city an offered
                // Disrupt Rocketry comes before the training step.
                let training_yields = g.spies[&spy_id].level < 2
                    && self.training_yields_to_the_disrupt(
                        g,
                        spy_id,
                        &legal,
                        &racer_spy_racers,
                        &every_pad_threats,
                    );
                if training_yields {
                    self.journal_training_yields(g, spy_id);
                }
                if g.spies[&spy_id].level < 2 && !training_yields {
                    // `spies-steal-the-lead`: a Science seat's green Spy
                    // steals a Eureka instead of training. `None` with the
                    // gene off.
                    if let Some(action) = self.steal_the_lead_action(g, plan, spy_id, &legal) {
                        if g.apply(pid, &action).is_ok() {
                            self.steal_the_lead_note(g, pid, spy_id, "the Gain Sources training");
                        }
                        self.spy_orders_until.insert(
                            spy_id,
                            g.turn + g.standard_duration(SPY_MISSION_ORDER_PATIENCE),
                        );
                        continue;
                    }
                    if let Some(action) = legal.iter().find(|action| {
                        matches!(action, Action::SpyMission { mission, .. } if mission == "gain_sources")
                    }) {
                        let _ = g.apply(pid, action);
                        self.spy_orders_until.insert(
                            spy_id,
                            g.turn + g.standard_duration(SPY_MISSION_ORDER_PATIENCE),
                        );
                        continue;
                    }
                }
                // `science-denial-every-pad`: in a decisive science threat's
                // city, Disrupt Rocketry whenever it is offered. `None` with
                // the gene off (`every_pad_threats` empty).
                let every_pad = if self.disrupt_held_for_the_match_point(g, spy_id) {
                    None
                } else {
                    Self::every_pad_disrupt(g, pid, spy_id, &every_pad_threats)
                };
                if let Some(action) = every_pad {
                    if g.apply(pid, &action).is_ok() {
                        *g.players[pid]
                            .counters
                            .entry("every_pad_disrupts".to_string())
                            .or_insert(0) += 1;
                        self.every_pad_note(g, spy_id);
                    }
                    self.spy_orders_until.insert(
                        spy_id,
                        g.turn + g.standard_duration(SPY_MISSION_ORDER_PATIENCE),
                    );
                    continue;
                }
                // `culture-denial-heist`: in a culture racer's city, the Great
                // Work heist whenever it is offered. `None` with the gene off
                // (`culture_heist_targets` empty).
                if let Some(action) =
                    Self::culture_heist_action(g, pid, spy_id, &culture_heist_targets)
                {
                    if g.apply(pid, &action).is_ok() {
                        *g.players[pid]
                            .counters
                            .entry("culture_heists".to_string())
                            .or_insert(0) += 1;
                        self.culture_heist_note(g, spy_id);
                    }
                    self.spy_orders_until.insert(
                        spy_id,
                        g.turn + g.standard_duration(SPY_MISSION_ORDER_PATIENCE),
                    );
                    continue;
                }
                // `racer-keeps-the-spies`: in a space racer's city, Disrupt
                // Rocketry when offered, else only a safe operation, and the
                // Spy keeps its slot. Never taken with the gene off.
                if Self::racer_spy_city_is_a_racers(g, &racer_spy_racers, current_city) {
                    // `spies-steal-the-lead`: no Disrupt Rocketry here, so a
                    // Science seat steals a Eureka rather than wait on a
                    // Listening Post. `None` with the gene off.
                    if let Some(action) = self.steal_the_lead_action(g, plan, spy_id, &legal) {
                        if g.apply(pid, &action).is_ok() {
                            self.steal_the_lead_note(g, pid, spy_id, "a Listening Post");
                        }
                        self.spy_orders_until.insert(
                            spy_id,
                            g.turn + g.standard_duration(SPY_MISSION_ORDER_PATIENCE),
                        );
                        continue;
                    }
                    if let Some(action) = Self::racer_spy_operation(&legal).cloned() {
                        let _ = g.apply(pid, &action);
                        self.spy_orders_until.insert(
                            spy_id,
                            g.turn + g.standard_duration(SPY_MISSION_ORDER_PATIENCE),
                        );
                    }
                    continue;
                }
                let science_denial_pressure = current_city
                    .and_then(|city| g.cities.get(&city))
                    .is_some_and(|city| {
                        if !self.deny_leaders || plan.strategy != GrandStrategy::Science {
                            return false;
                        }
                        let pressure = self.rival_victory_pressure(g, city.owner);
                        pressure.strategy == GrandStrategy::Science
                            && pressure.progress >= SCIENCE_ROCKETRY_DENIAL_PROGRESS
                    });
                let operation = legal
                    .iter()
                    .filter_map(|action| {
                        let Action::SpyMission {
                            spy,
                            mission,
                            target,
                        } = action
                        else {
                            return None;
                        };
                        if matches!(mission.as_str(), "gain_sources" | "counterspy") {
                            return None;
                        }
                        let city = current_city?;
                        let defender = g.cities[&city].owner;
                        // `racer-keeps-the-spies`: no capture-risk operation
                        // outside every racer's cities while a racer exists.
                        // Never taken with the gene off.
                        // `spies-steal-the-lead`: …except Steal Tech Boost on
                        // a Science aim. Unchanged with that gene off.
                        if !racer_spy_racers.is_empty()
                            && !racer_spy_stays.contains(&defender)
                            && self.racer_spy_withholds(plan, mission)
                        {
                            return None;
                        }
                        let active = crate::game::SpyMission {
                            kind: mission.clone(),
                            city,
                            target: *target,
                            started: g.turn,
                            ends: g.turn,
                        };
                        let strategic = match (plan.strategy, mission.as_str()) {
                            (GrandStrategy::Science, "steal_tech_boost") => 320.0,
                            (GrandStrategy::Science, "disrupt_rocketry") => {
                                290.0
                                    + if science_denial_pressure {
                                        SCIENCE_ROCKETRY_DENIAL_PRIORITY
                                    } else {
                                        0.0
                                    }
                            }
                            (GrandStrategy::Culture, "great_work_heist") => 340.0,
                            (GrandStrategy::Culture, "siphon_funds") => 135.0,
                            (GrandStrategy::Diplomacy, "fabricate_scandal") => 330.0,
                            (GrandStrategy::Diplomacy, "listening_post") => 185.0,
                            (GrandStrategy::Conquest, "neutralize_governor") => 310.0,
                            (GrandStrategy::Conquest, "sabotage_production") => 260.0,
                            (GrandStrategy::Conquest, "recruit_partisans") => 245.0,
                            (GrandStrategy::Conquest, "foment_unrest") => 230.0,
                            (GrandStrategy::Conquest, "breach_dam") => 210.0,
                            (_, "siphon_funds") => 150.0,
                            (_, "steal_tech_boost") => 145.0,
                            (_, "great_work_heist") => 135.0,
                            (_, "neutralize_governor") => 125.0,
                            (_, "fabricate_scandal") => 120.0,
                            (_, "listening_post") => 75.0,
                            _ => 100.0,
                        } + if plan.target_player == Some(defender) {
                            90.0
                        } else {
                            0.0
                        }
                        // `science-threat-denial`: `disrupt_rocketry` is the
                        // model's launch sabotage — it pillages the pad and
                        // zeroes the city's spaceport-project progress — so
                        // it outranks every other operation in a threat's
                        // city, at our threshold rather than at Mars. Zero
                        // when the gene is off. See
                        // `advanced/science_threat_denial.rs`.
                        + Self::science_denial_spy_mission_bonus(&denial_seats, defender, mission);
                        Some((
                            strategic * g.spy_success_chance(*spy, &active),
                            mission,
                            action,
                        ))
                    })
                    .max_by(|left, right| {
                        left.0
                            .partial_cmp(&right.0)
                            .unwrap()
                            .then_with(|| right.1.cmp(left.1))
                    })
                    .map(|(_, _, action)| action);
                if let Some(action) = operation {
                    let _ = g.apply(pid, action);
                    self.spy_orders_until.insert(
                        spy_id,
                        g.turn + g.standard_duration(SPY_MISSION_ORDER_PATIENCE),
                    );
                    continue;
                }
            }
            let assignment = legal
                .iter()
                .filter_map(|action| match action {
                    Action::AssignSpy { city, .. } if g.cities[city].owner != pid => {
                        let target = &g.cities[city];
                        let strategic = if plan.target_player == Some(target.owner) {
                            180
                        } else {
                            0
                        } + match plan.strategy {
                            GrandStrategy::Science => {
                                i32::from(target.districts.contains_key(crate::name!("campus")))
                                    * 90
                                    + i32::from(
                                        target.districts.contains_key(crate::name!("spaceport")),
                                    ) * 150
                            }
                            GrandStrategy::Culture => {
                                i32::from(
                                    target
                                        .districts
                                        .contains_key(crate::name!("theater_square")),
                                ) * 140
                            }
                            GrandStrategy::Diplomacy => {
                                i32::from(g.players[target.owner].is_minor) * 180
                            }
                            GrandStrategy::Conquest => {
                                i32::from(g.city_can_strike(target)) * 35
                                    + i32::from(g.players[target.owner].governors.contains(city))
                                        * 120
                            }
                            _ => {
                                i32::from(
                                    target
                                        .districts
                                        .contains_key(crate::name!("commercial_hub")),
                                ) * 70
                            }
                        };
                        Some((
                            strategic
                                + target.pop * 8
                                + target.districts.len() as i32 * 14
                                + target.wonders.len() as i32 * 24
                                // `science-threat-denial`: the pad that is
                                // actually going to launch is the posting,
                                // for one spy. Zero when the gene is off or
                                // a spy of ours is already bound there. See
                                // `advanced/science_threat_denial.rs`.
                                + Self::science_denial_spy_assignment_bonus(
                                    g,
                                    pid,
                                    spy_id,
                                    &denial_pads,
                                    *city,
                                )
                                // `science-denial-spy-reads-the-leader`: the
                                // leading threat's pad outranks the rest.
                                // Zero when the gene is off.
                                + Self::science_denial_leader_spy_assignment_bonus(
                                    g,
                                    pid,
                                    spy_id,
                                    denial_leader,
                                    *city,
                                )
                                // `science-denial-every-pad`: every free pad
                                // city of a decisive threat. Zero with the
                                // gene off.
                                + Self::every_pad_assignment_bonus(
                                    g,
                                    pid,
                                    spy_id,
                                    &every_pads,
                                    *city,
                                )
                                // `culture-denial-heist`: every free Theater
                                // Square city of a culture racer. Zero with
                                // the gene off.
                                + Self::culture_heist_assignment_bonus(
                                    g,
                                    pid,
                                    spy_id,
                                    &culture_heist_cities,
                                    *city,
                                ),
                            std::cmp::Reverse(*city),
                            action,
                        ))
                    }
                    _ => None,
                })
                .max_by_key(|(score, city, _)| (*score, *city))
                .map(|(_, _, action)| action);
            if let Some(action) = assignment {
                let posted = g.apply(pid, action).is_ok();
                // `science-threat-denial`: the seat's record of a posting to
                // a threat's pad, so a screen row can say the espionage rung
                // reached the board. Never taken when the gene is off, since
                // `denial_pads` is empty then.
                if posted
                    && matches!(action, Action::AssignSpy { city, .. } if denial_pads.contains(city))
                {
                    *g.players[pid]
                        .counters
                        .entry("denial_spy_posts".to_string())
                        .or_insert(0) += 1;
                }
                self.spy_orders_until.insert(
                    spy_id,
                    g.turn + g.standard_duration(SPY_TRAVEL_ORDER_PATIENCE),
                );
            }
        }
        self.note_spy_order_turns(g, &patience_before);
    }
}
