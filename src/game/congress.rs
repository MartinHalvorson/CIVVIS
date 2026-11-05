//! The World Congress: resolutions, emergencies, votes and diplomatic favor.
//!
//! Choosing and pricing votes (`congress_choice_key`, `congress_vote_cost`,
//! `congress_affordable_votes`), the regular candidates and the emergency
//! resolutions, which resolutions are in force (`congress_effect_active`),
//! and the session itself: `convene_congress`, `do_congress_vote`,
//! `resolve_congress`, `process_congress`.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    pub(super) fn trim_policies_to_slots(&mut self, pid: usize) {
        let mut cards = self.players[pid].policies.clone();
        while !self.policies_fit(pid, &cards) {
            let removable = cards
                .iter()
                .rev()
                .find(|card| {
                    self.rules
                        .policies
                        .get(card)
                        .is_none_or(|policy| policy.slot == "wildcard")
                })
                .cloned()
                .or_else(|| cards.iter().next_back().cloned());
            let Some(removable) = removable else {
                break;
            };
            cards.remove(&removable);
        }
        self.players[pid].policies = cards;
    }

    pub(super) fn congress_military_strength_bonus(&self, unit: &Unit) -> f64 {
        let class = self.rules.units[unit.kind].promotion_class.as_str();
        if self.congress_effect_active("military_advisory", "A", class) {
            5.0
        } else if self.congress_effect_active("military_advisory", "B", class) {
            -5.0
        } else {
            0.0
        }
    }

    /// Favor one active city-state suzerainty generates at the start of a
    /// major civilization's turn. Országház multiplies this source, so every
    /// consumer of a prospective suzerainty reads the same rate as upkeep.
    pub(crate) fn suzerain_diplomatic_favor_per_turn(&self, pid: usize) -> f64 {
        1.0 + self.empire_wonder_effect(pid, "suzerain_diplomatic_favor_pct") / 100.0
    }

    fn congress_choice_key(outcome: &str, target: &str) -> String {
        format!("{outcome}:{target}")
    }

    pub(crate) fn congress_choice_parts(choice: &str) -> (&str, &str) {
        // Pre-resolution-variety saves used the target itself as the choice.
        // Treat those ballots as Outcome A so old sessions remain resolvable.
        choice.split_once(':').unwrap_or(("A", choice))
    }

    /// The host's Online-speed ballot table is (2k(k + 1)), where (k)
    /// counts paid votes: a thirteen-vote ballot costs 312 Favor. That is
    /// deliberately not the generic 50%-of-Standard curve (which would be
    /// 390), so keep the observed live rule explicit rather than silently
    /// treating it as ordinary production scaling.
    fn congress_paid_vote_cost_factor(&self) -> f64 {
        match self.game_speed {
            GameSpeed::Online => 2.0,
            _ => 5.0,
        }
    }

    pub(super) fn congress_vote_cost(&self, votes: u32) -> f64 {
        let paid_votes = votes.saturating_sub(1) as f64;
        self.congress_paid_vote_cost_factor() * paid_votes * (paid_votes + 1.0)
    }

    /// The most votes a player can purchase with its present Favor. One vote
    /// is free, and the quadratic inversion keeps the action-space boundary
    /// identical to the spend/refund curve above.
    pub(crate) fn congress_affordable_votes(&self, pid: usize) -> u32 {
        let favor = self
            .players
            .get(pid)
            .map(|player| player.diplomatic_favor)
            .unwrap_or_default()
            .max(0.0);
        let factor = self.congress_paid_vote_cost_factor();
        let paid_votes = ((1.0 + 4.0 * favor / factor).sqrt() - 1.0) / 2.0;
        paid_votes.floor().clamp(0.0, (u32::MAX - 1) as f64) as u32 + 1
    }

    pub(super) fn emergency_resolution_id(id: u32) -> String {
        format!("emergency:{id}")
    }

    fn emergency_id_from_resolution(resolution: &str) -> Option<u32> {
        resolution.strip_prefix("emergency:")?.parse().ok()
    }

    pub(crate) fn emergency_proposal_for_resolution(
        &self,
        resolution: &str,
    ) -> Option<&EmergencyProposal> {
        let id = Self::emergency_id_from_resolution(resolution)?;
        self.pending_emergencies
            .iter()
            .find(|proposal| proposal.id == id)
    }

    pub(crate) fn congress_effect_active(
        &self,
        resolution: &str,
        outcome: &str,
        target: &str,
    ) -> bool {
        self.active_congress_effects.iter().any(|effect| {
            effect.expires > self.turn
                && effect.resolution == resolution
                && effect.outcome == outcome
                && effect.target == target
        })
    }

    pub(super) fn congress_resolution(
        id: &str,
        title: &str,
        targets: impl IntoIterator<Item = String>,
    ) -> Option<CongressResolution> {
        let targets: Vec<String> = targets.into_iter().collect();
        if targets.is_empty() {
            return None;
        }
        let choices = ["A", "B"]
            .into_iter()
            .flat_map(|outcome| {
                targets
                    .iter()
                    .map(move |target| Self::congress_choice_key(outcome, target))
            })
            .collect();
        Some(CongressResolution {
            id: id.to_string(),
            title: title.to_string(),
            choices,
            ballots: BTreeMap::new(),
        })
    }

    pub(super) fn regular_congress_candidates(&self) -> Vec<CongressResolution> {
        let player_targets = || {
            self.players
                .iter()
                .filter(|player| self.victory_eligible(player.id))
                .map(|player| player.id.to_string())
                .collect::<Vec<_>>()
        };
        let mut candidates = Vec::new();
        let mut push = |resolution: Option<CongressResolution>| {
            if let Some(resolution) = resolution {
                candidates.push(resolution);
            }
        };

        push(Self::congress_resolution(
            "mercenary_companies",
            "Mercenary Companies",
            ["production", "gold", "faith"].map(str::to_string),
        ));
        push(Self::congress_resolution(
            "luxury_policy",
            "Luxury Policy",
            self.rules
                .resources
                .iter()
                .filter(|(_, resource)| resource.class == "luxury")
                .filter(|(name, _)| {
                    self.map
                        .tiles
                        .values()
                        .any(|tile| tile.resource.as_deref() == Some(name.as_str()))
                })
                .map(|(name, _)| name.to_string()),
        ));
        push(Self::congress_resolution(
            "trade_policy",
            "Trade Policy",
            player_targets(),
        ));
        if self.world_era <= 4 {
            push(Self::congress_resolution(
                "world_religion",
                "World Religion",
                self.players
                    .iter()
                    .filter_map(|player| player.religion.clone())
                    .collect::<BTreeSet<_>>(),
            ));
        }
        if self.world_era <= 5 {
            let mut districts: BTreeSet<String> = self
                .rules
                .districts
                .iter()
                .filter(|(_, district)| district.buildable)
                .map(|(name, _)| self.district_family(*name).to_string())
                .collect();
            districts.insert("city_center".to_string());
            push(Self::congress_resolution(
                "urban_development_treaty",
                "Urban Development Treaty",
                districts,
            ));
            push(Self::congress_resolution(
                "patronage",
                "Patronage",
                [
                    "general",
                    "admiral",
                    "scientist",
                    "prophet",
                    "writer",
                    "artist",
                    "engineer",
                    "merchant",
                    "musician",
                ]
                .map(str::to_string),
            ));
            push(Self::congress_resolution(
                "border_control_treaty",
                "Border Control Treaty",
                player_targets(),
            ));
        }
        if self.world_era <= 6 {
            push(Self::congress_resolution(
                "military_advisory",
                "Military Advisory",
                self.rules
                    .units
                    .values()
                    .filter(|unit| unit.class == "military" && !unit.promotion_class.is_empty())
                    .map(|unit| unit.promotion_class.clone())
                    .collect::<BTreeSet<_>>(),
            ));
            push(Self::congress_resolution(
                "public_relations",
                "Public Relations",
                player_targets(),
            ));
        }
        if (4..=6).contains(&self.world_era) {
            push(Self::congress_resolution(
                "espionage_pact",
                "Espionage Pact",
                [
                    "siphon_funds",
                    "steal_tech_boost",
                    "great_work_heist",
                    "sabotage_production",
                    "recruit_partisans",
                    "foment_unrest",
                    "neutralize_governor",
                    "disrupt_rocketry",
                    "breach_dam",
                    "fabricate_scandal",
                ]
                .map(str::to_string),
            ));
        }
        if self.world_era >= 4 {
            push(Self::congress_resolution(
                "migration_treaty",
                "Migration Treaty",
                player_targets(),
            ));
        }
        if self.world_era >= 5 {
            push(Self::congress_resolution(
                "heritage_organization",
                "Heritage Organization",
                ["writing", "art", "artifact", "music", "relic"].map(str::to_string),
            ));
            push(Self::congress_resolution(
                "world_ideology",
                "World Ideology",
                self.players
                    .iter()
                    .filter(|player| self.victory_eligible(player.id))
                    .filter_map(|player| player.government.clone())
                    .collect::<BTreeSet<_>>(),
            ));
            push(Self::congress_resolution(
                "global_energy_treaty",
                "Global Energy Treaty",
                ["coal_power_plant", "oil_power_plant", "nuclear_power_plant"].map(str::to_string),
            ));
        }
        if self.world_era >= 6 {
            push(Self::congress_resolution(
                "arms_control",
                "Arms Control",
                player_targets(),
            ));
        }
        if (6..=7).contains(&self.world_era) {
            push(Self::congress_resolution(
                "public_works_program",
                "Public Works Program",
                self.rules
                    .projects
                    .keys()
                    .filter(|project| !project.starts_with("repair_"))
                    .map(|project| project.to_string()),
            ));
            push(Self::congress_resolution(
                "deforestation_treaty",
                "Deforestation Treaty",
                ["forest", "jungle", "marsh"]
                    .into_iter()
                    .filter(|feature| {
                        self.map.tiles.values().any(|tile| {
                            tile.feature.as_deref() == Some(*feature)
                                && tile.district.is_none()
                                && tile.wonder.is_none()
                        })
                    })
                    .map(str::to_string),
            ));
        }
        candidates
    }

    pub(super) fn do_congress_vote(
        &mut self,
        pid: usize,
        resolution: &str,
        choice: &str,
        votes: u32,
    ) -> Result<(), String> {
        if votes == 0 {
            return Err("at least one vote is required".into());
        }
        let favor_cost = self.congress_vote_cost(votes);
        let emergency_voters = self
            .emergency_proposal_for_resolution(resolution)
            .map(|proposal| (proposal.target, proposal.eligible.clone()));
        let congress = self
            .congress
            .as_mut()
            .ok_or_else(|| "no World Congress session is active".to_string())?;
        if self.turn >= congress.closes {
            return Err("World Congress voting has closed".into());
        }
        let ballot = congress
            .resolutions
            .iter_mut()
            .find(|candidate| candidate.id == resolution)
            .ok_or_else(|| "no such World Congress resolution".to_string())?;
        if !ballot.choices.iter().any(|candidate| candidate == choice) {
            return Err("invalid resolution choice".into());
        }
        if let Some((emergency_target, eligible)) = emergency_voters {
            let (outcome, target) = Self::congress_choice_parts(choice);
            let eligible_member = eligible.contains(&pid);
            let target_vote = emergency_target == pid;
            if target != "support" && target != "oppose"
                || (outcome == "A" && (target != "support" || !eligible_member))
                || (outcome == "B" && (target != "oppose" || (!eligible_member && !target_vote)))
                || !matches!(outcome, "A" | "B")
            {
                return Err("civilization is not eligible for that Emergency vote".into());
            }
        }
        if ballot.ballots.contains_key(&pid) {
            return Err("this civilization has already voted on that resolution".into());
        }

        // Every civilization receives its first vote; additional votes cost
        // an escalating amount of Favor on the active game-speed curve.
        if self.players[pid].diplomatic_favor + f64::EPSILON < favor_cost {
            return Err("not enough Diplomatic Favor".into());
        }
        self.players[pid].diplomatic_favor -= favor_cost;
        ballot.ballots.insert(pid, (choice.to_string(), votes));
        Ok(())
    }

    pub(super) fn resolve_congress(&mut self) {
        let Some(session) = self.congress.take() else {
            return;
        };
        if session.resolutions.len() == 1
            && Self::emergency_id_from_resolution(&session.resolutions[0].id).is_some()
        {
            self.resolve_emergency_session(&session.resolutions[0]);
            return;
        }
        // Reconstruct each voter's Favor at the start of the session. Ballots
        // store vote counts rather than a redundant snapshot, and voting has
        // already deducted the corresponding escalating cost.
        let starting_favor: BTreeMap<usize, f64> = self
            .players
            .iter()
            .map(|player| {
                let committed = session
                    .resolutions
                    .iter()
                    .filter_map(|resolution| resolution.ballots.get(&player.id))
                    .map(|(_, votes)| self.congress_vote_cost(*votes))
                    .sum::<f64>();
                (player.id, player.diplomatic_favor + committed)
            })
            .collect();
        for resolution in session.resolutions {
            let tie_break_share = |outcome: &str, target: Option<&str>| {
                resolution
                    .ballots
                    .iter()
                    .filter(|(_, (choice, _))| {
                        let (cast_outcome, cast_target) = Self::congress_choice_parts(choice);
                        cast_outcome == outcome && target.is_none_or(|target| cast_target == target)
                    })
                    .map(|(voter, (_, votes))| {
                        let available = starting_favor.get(voter).copied().unwrap_or(0.0);
                        if available <= f64::EPSILON {
                            0.0
                        } else {
                            self.congress_vote_cost(*votes) / available
                        }
                    })
                    .max_by(f64::total_cmp)
                    .unwrap_or(0.0)
            };
            let mut outcome_totals: BTreeMap<String, u32> = BTreeMap::new();
            for (choice, votes) in resolution.ballots.values() {
                let (outcome, _) = Self::congress_choice_parts(choice);
                *outcome_totals.entry(outcome.to_string()).or_insert(0) += *votes;
            }
            let Some((winning_outcome, _)) = outcome_totals.into_iter().max_by(|left, right| {
                left.1
                    .cmp(&right.1)
                    .then_with(|| {
                        tie_break_share(&left.0, None).total_cmp(&tie_break_share(&right.0, None))
                    })
                    .then_with(|| right.0.cmp(&left.0))
            }) else {
                continue;
            };
            let mut target_totals: BTreeMap<String, u32> = BTreeMap::new();
            for (choice, votes) in resolution.ballots.values() {
                let (outcome, target) = Self::congress_choice_parts(choice);
                if outcome == winning_outcome {
                    *target_totals.entry(target.to_string()).or_insert(0) += *votes;
                }
            }
            let Some((winning_target, _)) = target_totals.into_iter().max_by(|left, right| {
                left.1
                    .cmp(&right.1)
                    .then_with(|| {
                        tie_break_share(&winning_outcome, Some(&left.0))
                            .total_cmp(&tie_break_share(&winning_outcome, Some(&right.0)))
                    })
                    .then_with(|| right.0.cmp(&left.0))
            }) else {
                continue;
            };

            // Votes on a losing outcome receive a full Favor refund. Votes
            // for the winning outcome but a different target receive half.
            // Exact predictions earn the stock +1 Diplomatic Victory Point.
            for (voter, (choice, votes)) in &resolution.ballots {
                let (outcome, target) = Self::congress_choice_parts(choice);
                let cost = self.congress_vote_cost(*votes);
                if outcome != winning_outcome {
                    self.players[*voter].diplomatic_favor += cost;
                } else if target != winning_target {
                    self.players[*voter].diplomatic_favor += cost * 0.5;
                } else if self.victory_eligible(*voter) {
                    self.players[*voter].dvp += 1;
                }
            }

            if resolution.id == "world_leader" {
                if let Ok(target) = winning_target.parse::<usize>() {
                    if self.victory_eligible(target) {
                        if winning_outcome == "A" {
                            self.players[target].dvp += 2;
                            self.add_historic_moment(
                                target,
                                "MOMENT_PLAYER_EARNED_DIPLOMATIC_VICTORY_POINT",
                            );
                        } else {
                            self.players[target].dvp = self.players[target].dvp.saturating_sub(2);
                        }
                    }
                }
                continue;
            }

            if resolution.id == "arms_control" {
                if let Ok(target) = winning_target.parse::<usize>() {
                    let keys = [
                        "project_effect:nuclear_devices",
                        "project_effect:thermonuclear_devices",
                    ];
                    if winning_outcome == "A" && self.victory_eligible(target) {
                        let target_counts = keys.map(|key| {
                            self.players[target].counters.get(key).copied().unwrap_or(0)
                        });
                        let majors: Vec<usize> = self
                            .players
                            .iter()
                            .filter(|player| self.victory_eligible(player.id))
                            .map(|player| player.id)
                            .collect();
                        for player in majors {
                            for (key, count) in keys.into_iter().zip(target_counts) {
                                self.players[player].counters.insert(key.to_string(), count);
                            }
                        }
                    } else if winning_outcome == "B" && self.victory_eligible(target) {
                        for key in keys {
                            self.players[target].counters.insert(key.to_string(), 0);
                        }
                    }
                }
                continue;
            }

            if resolution.id == "trade_policy" && winning_outcome == "B" {
                if let Ok(target) = winning_target.parse::<usize>() {
                    let recalled = self
                        .routes
                        .iter()
                        .filter(|route| {
                            let Some(origin) = self.cities.get(&route.origin) else {
                                return true;
                            };
                            let Some(destination) = self.cities.get(&route.dest) else {
                                return true;
                            };
                            origin.owner != destination.owner
                                && (origin.owner == target || destination.owner == target)
                        })
                        .cloned()
                        .collect();
                    self.recall_trade_routes(recalled);
                }
            }
            let world_ideology_target =
                (resolution.id == "world_ideology").then(|| winning_target.clone());
            self.active_congress_effects.push(CongressEffect {
                resolution: resolution.id,
                outcome: winning_outcome,
                target: winning_target,
                expires: session.convened + self.standard_duration(30),
            });
            if let Some(government) = world_ideology_target {
                let affected: Vec<usize> = self
                    .players
                    .iter()
                    .filter(|player| player.government.as_deref() == Some(&government))
                    .map(|player| player.id)
                    .collect();
                for player in affected {
                    self.trim_policies_to_slots(player);
                }
            }
        }
        let contenders: Vec<(usize, i64)> = self
            .players
            .iter()
            .map(|player| (player.id, player.dvp))
            .collect();
        if let Some(winner) = contenders
            .into_iter()
            .find(|(id, dvp)| self.victory_eligible(*id) && *dvp >= DIPLOMATIC_VICTORY_POINTS)
            .map(|(id, _)| id)
        {
            self.set_winner(winner, "diplomatic");
        }
    }

    pub(super) fn convene_congress(&mut self) {
        self.active_congress_effects
            .retain(|effect| effect.expires > self.turn);
        let mut candidates = self.regular_congress_candidates();
        if candidates.is_empty() {
            return;
        }
        let mut resolutions = Vec::new();
        while !candidates.is_empty() && resolutions.len() < 2 {
            let index = self.rng.below(candidates.len());
            resolutions.push(candidates.swap_remove(index));
        }
        if self.world_era >= 5 {
            let players = self
                .players
                .iter()
                .filter(|player| self.victory_eligible(player.id))
                .map(|player| player.id.to_string());
            if let Some(resolution) =
                Self::congress_resolution("world_leader", "Diplomatic Victory", players)
            {
                resolutions.push(resolution);
            }
        }
        self.congress = Some(CongressSession {
            convened: self.turn,
            closes: self.turn + self.standard_duration(5),
            resolutions,
        });
        // Gathering Storm seats a scored competition from the congress
        // (`EMERGENCY_TRIGGER_WORLD_CONGRESS`), not on its own clock.
        self.open_native_competition();
    }

    /// From the Medieval era, regular sessions open every 30 turns. Voting
    /// remains live for five complete rounds so every sequential player can
    /// spend Favor, after which deterministic majority/tie rules resolve it.
    pub(super) fn process_congress(&mut self) {
        if self.is_finished() {
            return;
        }
        // A battlefield seats no Congress. Its two sides are at war from
        // the first turn and have nothing to legislate over — and a ballot
        // every thirty turns was what stopped a Medieval-or-later arena's
        // End Turn button dead on "Vote in the Congress".
        if self.is_arena() {
            return;
        }
        self.close_native_competition();
        self.active_congress_effects
            .retain(|effect| effect.expires > self.turn);
        if self
            .congress
            .as_ref()
            .is_some_and(|session| self.turn >= session.closes)
        {
            self.resolve_congress();
        }
        if self.congress.is_none() && !self.pending_emergencies.is_empty() {
            self.convene_pending_emergency();
        }
        // Whether the regular session is displaced turns on what actually
        // opened, not on what was queued: `convene_pending_emergency` drops
        // proposals whose target, city or membership has since gone, and a
        // queue that emptied without seating anybody has displaced nothing.
        // Testing the queue instead cost the world its Congress for a full
        // thirty turns over an emergency that never happened.
        if self.congress.is_none()
            && self.world_era >= 2
            && self.turn.is_multiple_of(self.standard_duration(30))
        {
            self.convene_congress();
        }
    }

    fn resolve_emergency_session(&mut self, resolution: &CongressResolution) {
        let Some(id) = Self::emergency_id_from_resolution(&resolution.id) else {
            return;
        };
        let Some(index) = self
            .pending_emergencies
            .iter()
            .position(|proposal| proposal.id == id)
        else {
            return;
        };
        let proposal = self.pending_emergencies.remove(index);
        let support = resolution
            .ballots
            .values()
            .filter(|(choice, _)| Self::congress_choice_parts(choice) == ("A", "support"))
            .map(|(_, votes)| *votes)
            .sum::<u32>();
        let oppose = resolution
            .ballots
            .values()
            .filter(|(choice, _)| Self::congress_choice_parts(choice) == ("B", "oppose"))
            .map(|(_, votes)| *votes)
            .sum::<u32>();
        let passed = support > oppose;
        for (voter, (choice, votes)) in &resolution.ballots {
            let supported = Self::congress_choice_parts(choice) == ("A", "support");
            if supported != passed {
                self.players[*voter].diplomatic_favor += self.congress_vote_cost(*votes);
            }
        }
        if !passed
            || !self
                .cities
                .get(&proposal.city)
                .is_some_and(|city| city.owner == proposal.target)
        {
            return;
        }
        let members: BTreeSet<usize> = resolution
            .ballots
            .iter()
            .filter(|(voter, (choice, _))| {
                proposal.eligible.contains(voter)
                    && Self::congress_choice_parts(choice) == ("A", "support")
                    // Eligibility is captured when the Special Session is
                    // convened, but diplomacy keeps moving while ballots are
                    // open. A supporter can sign peace with the target before
                    // the result resolves; the binding treaty must keep that
                    // civilization out of the resulting coalition.
                    && self
                        .peace_treaty_until(**voter, proposal.target)
                        .is_none()
            })
            .map(|(voter, _)| *voter)
            .collect();
        if members.is_empty() {
            return;
        }
        let ends = self.turn + self.standard_duration(30);
        let coalition_leader = *members.iter().next().unwrap();
        for member in &members {
            // Emergency wars sever every bilateral relationship, including
            // the newer mission, pact, and promise ledgers. Keep this on the
            // same boundary as an ordinary declaration so a Congress result
            // cannot leave an Embassy or Defensive Pact active across war.
            self.end_bilateral_relations_for_war(*member, proposal.target);
            // A congress that names a target names it to everyone it enlists.
            self.record_contact(*member, proposal.target);
            self.at_war.insert(pair(*member, proposal.target));
            self.open_war_front(
                *member,
                proposal.target,
                WarDeclaration {
                    conflict: id,
                    declarer: coalition_leader,
                    target: proposal.target,
                    declared_front: *member == coalition_leader,
                    casus_belli: None,
                    joint_war_until: None,
                },
            );
        }
        let members_vec: Vec<usize> = members.iter().copied().collect();
        for (index, member) in members_vec.iter().enumerate() {
            for peer in members_vec.iter().skip(index + 1) {
                let key = pair(*member, *peer);
                if self.at_war.remove(&key) {
                    self.close_war_record(key, Some(("coalition", *member, *peer)));
                }
                self.players[*member].open_borders_until.insert(*peer, ends);
                self.players[*peer].open_borders_until.insert(*member, ends);
            }
        }
        self.active_emergencies.push(Emergency {
            id,
            kind: proposal.kind,
            target: proposal.target,
            city: proposal.city,
            original_owner: proposal.original_owner,
            members,
            contributions: BTreeMap::new(),
            started: self.turn,
            ends,
        });
        self.share_emergency_visibility();
    }
}
