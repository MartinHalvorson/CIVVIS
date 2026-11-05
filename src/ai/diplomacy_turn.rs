//! The diplomatic turn of the baseline controller: deals, city-state levies
//! and spies.
//!
//! `diplomacy` runs `BasicAi`'s diplomacy: deal objectives and quick deals
//! (`deal_objective`, `deal_at_the_ceiling`, `close_quick_deal`), bilateral
//! trade, city-state levies, and the war-eve liquidation. `spies` runs the
//! spy corps.
//!
//! Moved verbatim out of the main `impl BasicAi` block in `ai.rs`
//! (2026-11-05). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged.

use super::*;

impl BasicAi {
    /// Run each available agent once. The baseline establishes sources before
    /// attempting the highest expected-value operation and otherwise embeds
    /// agents in the most developed non-allied foreign city.
    pub(crate) fn spies(&self, g: &mut Game, pid: usize) {
        let ids: Vec<u32> = g
            .spies
            .values()
            .filter(|spy| spy.owner == pid)
            .map(|spy| spy.id)
            .collect();
        for spy_id in ids {
            let legal = g.legal_spy_actions(pid, spy_id);
            if legal.is_empty() {
                continue;
            }
            if let Some(action) = [
                "technologist",
                "con_artist",
                "disguise",
                "linguist",
                "quartermaster",
                "seduction",
            ]
            .into_iter()
            .find_map(|wanted| {
                legal.iter().find(|action| {
                    matches!(action, Action::PromoteSpy { promotion, .. } if promotion == wanted)
                })
            })
            .or_else(|| {
                legal
                    .iter()
                    .find(|action| matches!(action, Action::PromoteSpy { .. }))
            }) {
                let _ = g.apply(pid, action);
                continue;
            }
            let current_city = g.spies.get(&spy_id).and_then(|spy| spy.city);
            let offensive = current_city
                .and_then(|city| g.cities.get(&city))
                .is_some_and(|city| city.owner != pid);
            if offensive {
                if let Some(action) = legal.iter().find(|action| {
                    matches!(action, Action::SpyMission { mission, .. } if mission == "gain_sources")
                }) {
                    let _ = g.apply(pid, action);
                    continue;
                }
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
                        let active = crate::game::SpyMission {
                            kind: mission.clone(),
                            city: current_city?,
                            target: *target,
                            started: g.turn,
                            ends: g.turn,
                        };
                        let value = match mission.as_str() {
                            "steal_tech_boost" => 105.0,
                            "siphon_funds" => 95.0,
                            "great_work_heist" => 90.0,
                            "neutralize_governor" => 82.0,
                            "disrupt_rocketry" => 80.0,
                            "fabricate_scandal" => 74.0,
                            "sabotage_production" => 70.0,
                            "foment_unrest" => 62.0,
                            "breach_dam" => 58.0,
                            "recruit_partisans" => 55.0,
                            "listening_post" => 42.0,
                            _ => 0.0,
                        };
                        Some((g.spy_success_chance(*spy, &active) * value, mission, action))
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
                    continue;
                }
            }
            let assignment = legal
                .iter()
                .filter_map(|action| match action {
                    Action::AssignSpy { city, .. } => {
                        let target = &g.cities[city];
                        (target.owner != pid).then_some((
                            target.pop as i64 * 8
                                + target.districts.len() as i64 * 12
                                + target.wonders.len() as i64 * 20
                                - i64::from(g.players[target.owner].is_minor) * 20,
                            std::cmp::Reverse(*city),
                            action,
                        ))
                    }
                    _ => None,
                })
                .max_by(|left, right| {
                    left.0
                        .cmp(&right.0)
                        .then_with(|| left.1.cmp(&right.1))
                })
                .map(|(_, _, action)| action)
                .or_else(|| {
                    legal
                        .iter()
                        .find(|action| matches!(action, Action::SpyMission { mission, .. } if mission == "counterspy"))
                });
            if let Some(action) = assignment {
                let _ = g.apply(pid, action);
            }
        }
    }

    pub(super) fn diplomacy(&self, g: &mut Game, pid: usize) {
        choose_dedications(g, pid, self.w.dedication_choice);
        let incoming: Vec<u32> = g
            .pending_deals
            .iter()
            .filter(|deal| deal.to == pid && deal.expires >= g.turn)
            .map(|deal| deal.id)
            .collect();
        for deal_id in incoming {
            let accept = g
                .pending_deals
                .iter()
                .find(|deal| deal.id == deal_id)
                .is_some_and(|deal| {
                    let partner_power = g.military_power(deal.from);
                    let grievance = g.players[pid]
                        .grievances
                        .get(&deal.from)
                        .copied()
                        .unwrap_or(0.0);
                    let special_accept = if deal.defensive_pact {
                        grievance < 75.0 && partner_power < g.military_power(pid) * 1.8 + 20.0
                    } else if let Some(target) = deal.joint_war_target {
                        let joint_power = g.military_power(pid) + partner_power;
                        let target_power = g.military_power(target);
                        g.players[pid]
                            .grievances
                            .get(&target)
                            .copied()
                            .unwrap_or(0.0)
                            >= 20.0
                            && joint_power > target_power * 1.2 + 20.0
                    } else if let Some(promise) = deal.promise.as_deref() {
                        match promise {
                            // An early Basic AI still needs nearby expansion
                            // room; later on it can safely give this promise.
                            "no_settling" => g.player_city_ids(pid).len() >= 3,
                            "no_city_state_attack" => !g.players.iter().any(|city_state| {
                                city_state.is_minor
                                    && !city_state.is_barbarian
                                    && g.is_at_war(pid, city_state.id)
                            }),
                            "no_conversion" | "no_spying" => true,
                            _ => false,
                        }
                    } else {
                        false
                    };
                    deal.peace
                        || special_accept
                        || deal.give_gold >= deal.request_gold
                        || ((deal.friendship || deal.alliance.is_some() || deal.open_borders)
                            && grievance < 75.0
                            && partner_power < g.military_power(pid) * 1.8 + 20.0)
                });
            let action = if accept {
                Action::AcceptDeal { deal: deal_id }
            } else {
                Action::RejectDeal { deal: deal_id }
            };
            let _ = g.apply(pid, &action);
        }
        if let Some(session) = g.congress.clone() {
            for resolution in session.resolutions {
                if resolution.ballots.contains_key(&pid) {
                    continue;
                }
                let own_a = format!("A:{pid}");
                let emergency_choice = g
                    .emergency_proposal_for_resolution(&resolution.id)
                    .and_then(|proposal| {
                        if proposal.target == pid {
                            Some("B:oppose".to_string())
                        } else if proposal.eligible.contains(&pid) {
                            Some("A:support".to_string())
                        } else {
                            None
                        }
                    });
                if g.emergency_proposal_for_resolution(&resolution.id)
                    .is_some()
                    && emergency_choice.is_none()
                {
                    continue;
                }
                let choice = emergency_choice
                    .or_else(|| {
                        resolution
                            .ballots
                            .values()
                            .max_by_key(|(choice, votes)| {
                                (*votes, std::cmp::Reverse(choice.clone()))
                            })
                            .map(|(choice, _)| choice.clone())
                            .or_else(|| {
                                (resolution.id == "world_leader"
                                    || resolution.id == "trade_policy"
                                    || resolution.id == "migration_treaty"
                                    || resolution.id == "border_control_treaty"
                                    || resolution.id == "public_relations")
                                    .then(|| {
                                        resolution
                                            .choices
                                            .iter()
                                            .find(|choice| **choice == own_a)
                                            .cloned()
                                    })
                                    .flatten()
                            })
                            .or_else(|| {
                                (resolution.id == "world_ideology")
                                    .then(|| {
                                        let own_government =
                                            g.players[pid].government.as_deref()?;
                                        let wanted = format!("A:{own_government}");
                                        resolution
                                            .choices
                                            .iter()
                                            .find(|choice| **choice == wanted)
                                            .cloned()
                                    })
                                    .flatten()
                            })
                            .or_else(|| {
                                (resolution.id == "mercenary_companies")
                                    .then(|| {
                                        resolution
                                            .choices
                                            .iter()
                                            .find(|choice| choice.as_str() == "B:production")
                                            .cloned()
                                    })
                                    .flatten()
                            })
                            .or_else(|| {
                                let preferred = match resolution.id.as_str() {
                                    "global_energy_treaty" => Some("A:coal_power_plant"),
                                    "public_works_program" => resolution
                                        .choices
                                        .iter()
                                        .find(|choice| choice.starts_with("A:"))
                                        .map(|name| name.as_str()),
                                    "deforestation_treaty" => resolution
                                        .choices
                                        .iter()
                                        .find(|choice| choice.starts_with("A:"))
                                        .map(|name| name.as_str()),
                                    _ => None,
                                }?;
                                resolution
                                    .choices
                                    .iter()
                                    .find(|choice| choice.as_str() == preferred)
                                    .cloned()
                            })
                    })
                    .unwrap_or_else(|| resolution.choices[pid % resolution.choices.len()].clone());
                let votes = if g.players[pid].diplomatic_favor >= 30.0 {
                    3
                } else if g.players[pid].diplomatic_favor >= 10.0 {
                    2
                } else {
                    1
                };
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
        self.bilateral_trade(g, pid);
        let my_power = g.military_power(pid);
        let others: Vec<usize> = g
            .players
            .iter()
            .filter(|o| o.id != pid && o.alive && !o.is_barbarian && g.has_met(pid, o.id))
            .map(|o| o.id)
            .collect();
        for o in &others {
            if g.is_at_war(pid, *o)
                // A city-state follows its Suzerain into a derived war. It
                // cannot settle that principal conflict itself, so only ask
                // for peace when this pair is present in the declared set.
                && g.at_war.contains(&(pid.min(*o), pid.max(*o)))
                && !g.emergency_war_pair(pid, *o)
                && my_power < self.w.peace_ratio * g.military_power(*o)
            {
                let _ = g.apply(pid, &Action::MakePeace { player: *o });
            }
        }
        if self.minor {
            return;
        }
        // Delegations and Resident Embassies are a small, recurring
        // investment rather than an AI-only stat. Establish one with the
        // least aggrieved major on a staggered cadence, preferring the later
        // Embassy replacement once Diplomatic Service is known.
        if g.turn % 9 == pid as u32 % 9 {
            let partner = others
                .iter()
                .copied()
                .filter(|other| {
                    !g.players[*other].is_minor
                        && !g.is_at_war(pid, *other)
                        && g.players[pid].grievances.get(other).copied().unwrap_or(0.0) < 50.0
                })
                .min_by(|first, second| {
                    g.players[pid]
                        .grievances
                        .get(first)
                        .copied()
                        .unwrap_or(0.0)
                        .total_cmp(
                            &g.players[pid]
                                .grievances
                                .get(second)
                                .copied()
                                .unwrap_or(0.0),
                        )
                        .then(first.cmp(second))
                });
            if let Some(partner) = partner {
                let embassy_ready = g.players[pid]
                    .civics
                    .contains(&crate::name!("diplomatic_service"))
                    && g.players[pid].gold >= 25.0
                    && !g
                        .diplomatic_mission_to(pid, partner)
                        .is_some_and(|mission| mission.kind == "embassy");
                let action = if embassy_ready {
                    Some(Action::SendEmbassy { player: partner })
                } else if g.players[pid].gold >= 10.0
                    && g.diplomatic_mission_to(pid, partner).is_none()
                {
                    Some(Action::SendDelegation { player: partner })
                } else {
                    None
                };
                if let Some(action) = action {
                    let _ = g.apply(pid, &action);
                }
            }
        }
        if g.turn % 20 == pid as u32 % 20 {
            if let Some(partner) = others.iter().copied().find(|other| {
                !g.players[*other].is_minor
                    && !g.is_at_war(pid, *other)
                    && g.players[pid].grievances.get(other).copied().unwrap_or(0.0) < 50.0
            }) {
                let alliance = if g.are_friends(pid, partner)
                    && g.players[pid]
                        .civics
                        .contains(&crate::name!("civil_service"))
                    && g.players[partner]
                        .civics
                        .contains(&crate::name!("civil_service"))
                    && g.alliance_with(pid, partner).is_none()
                {
                    let kinds = ["economic", "cultural", "military", "religious", "research"];
                    kinds
                        .into_iter()
                        .cycle()
                        .skip(pid % kinds.len())
                        .take(kinds.len())
                        .find(|kind| {
                            (*kind != "research"
                                || (g.tree_effect(pid, "research_agreements") > 0.0
                                    && g.tree_effect(partner, "research_agreements") > 0.0))
                                && !g.players[pid].alliances.values().any(|alliance| {
                                    alliance.ends > g.turn && alliance.kind == *kind
                                })
                                && !g.players[partner].alliances.values().any(|alliance| {
                                    alliance.ends > g.turn && alliance.kind == *kind
                                })
                        })
                        .map(str::to_string)
                } else {
                    None
                };
                let _ = g.apply(
                    pid,
                    &Action::ProposeDeal {
                        player: partner,
                        give_gold: 0.0,
                        request_gold: 0.0,
                        // `no_free_passage`: passage is sold, not bundled.
                        open_borders: !self.no_free_passage
                            && g.players[pid]
                                .civics
                                .contains(&crate::name!("early_empire")),
                        friendship: true,
                        peace: false,
                        alliance,
                    },
                );
            }
        }
        // A Defensive Pact is deliberately separate from an Alliance. Once
        // an allied partner is in place, offer the explicit 30-turn treaty
        // instead of treating every Alliance as an automatic call to war.
        if g.turn % 15 == pid as u32 % 15 {
            if let Some(partner) = others.iter().copied().find(|other| {
                !g.players[*other].is_minor
                    && g.are_allied(pid, *other)
                    && g.defensive_pact_until(pid, *other).is_none()
                    && !g.pending_deals.iter().any(|deal| {
                        deal.defensive_pact
                            && ((deal.from == pid && deal.to == *other)
                                || (deal.from == *other && deal.to == pid))
                    })
            }) {
                let _ = g.apply(pid, &Action::ProposeDefensivePact { player: partner });
            }
        }
        let at_war = others.iter().any(|o| g.is_at_war(pid, *o));
        if at_war {
            self.levy_city_state_military(g, pid, false, 0.0);
        }
        if !at_war
            && (g.turn as f64) > self.w.war_min_turn
            && g.player_city_ids(pid).len() >= 2
            && !others.is_empty()
        {
            let weakest = *others
                .iter()
                .min_by(|a, b| {
                    g.military_power(**a)
                        .partial_cmp(&g.military_power(**b))
                        .unwrap()
                })
                .unwrap();
            if my_power > self.w.war_ratio * g.military_power(weakest) + self.w.war_margin {
                let formal = g.players[pid]
                    .denounced_until
                    .get(&weakest)
                    .is_some_and(|until| *until > g.turn && *until <= g.turn + 25);
                let action = if formal {
                    Action::DeclareWarWithCasusBelli {
                        player: weakest,
                        casus_belli: "formal_war".to_string(),
                    }
                } else if !g.players[pid]
                    .denounced_until
                    .get(&weakest)
                    .is_some_and(|until| *until > g.turn)
                {
                    Action::Denounce { player: weakest }
                } else {
                    return;
                };
                // Sell into the declaration before it lands. A denouncement is
                // not a declaration and the pass knows the difference.
                self.war_eve_liquidation(g, pid, &action);
                let _ = g.apply(pid, &action);
            }
        }
    }

    /// Turn spare wartime Gold into immediately usable troops when this AI is
    /// a city-state's Suzerain. `urgent` lets the strategic AI spend deeper
    /// into its treasury for Conquest/Recovery plans; the general AI retains
    /// a larger economic reserve. `floor` is Gold the levy must leave in the
    /// treasury on top of that share (`purchases-keep-the-upkeep-reserve`);
    /// zero keeps the shipped levy.
    pub(crate) fn levy_city_state_military(
        &self,
        g: &mut Game,
        pid: usize,
        urgent: bool,
        floor: f64,
    ) {
        if self.minor || self.barb {
            return;
        }
        let reserve_share = if urgent { 0.20 } else { 0.40 };
        let spendable = (g.players[pid].gold * (1.0 - reserve_share) - 20.0)
            .min(g.players[pid].gold - floor)
            .max(0.0);
        let best = g
            .players
            .iter()
            .filter(|minor| minor.is_minor && !minor.is_barbarian && minor.alive)
            .filter_map(|minor| {
                let cost = g.levy_cost(pid, minor.id)?;
                if cost > spendable + f64::EPSILON {
                    return None;
                }
                let strength = g
                    .units
                    .values()
                    .filter(|unit| unit.owner == minor.id && unit.levied_from.is_none())
                    .filter(|unit| g.rules.units[unit.kind].class == "military")
                    .map(|unit| g.unit_strength(unit, true))
                    .sum::<f64>();
                Some((
                    strength / cost.max(1.0),
                    strength,
                    std::cmp::Reverse(minor.id),
                    minor.id,
                ))
            })
            .max_by(|left, right| {
                left.0
                    .partial_cmp(&right.0)
                    .unwrap()
                    .then_with(|| left.1.partial_cmp(&right.1).unwrap())
                    .then(left.2.cmp(&right.2))
            })
            .map(|(_, _, _, minor)| minor);
        if let Some(player) = best {
            let _ = g.apply(pid, &Action::LevyMilitary { player });
        }
    }

    /// What a sharpened quote leaves the counterparty, in Gold above the
    /// `validate_trade` bar of 0.25: two, like the war-eve lane, so a price
    /// floored to whole Gold cannot fall through the rounding.
    pub(super) const DEAL_CEILING_MARGIN: f64 = 2.0;

    /// The number a quote is chosen by. Shipped: the most balanced quote on
    /// the board, `min(our gain, their gain)`. `deals_for_our_gain`: ours.
    pub(crate) fn deal_objective(&self, deal: &crate::game::QuickDeal) -> f64 {
        if self.deals_for_our_gain {
            deal.my_value
        } else {
            deal.my_value.min(deal.partner_value)
        }
    }

    /// `deals_at_the_ceiling`: the same quote with its Gold moved to the
    /// counterparty's walk-away less the margin. A sale asks for more, as far
    /// as their treasury (less the reserve `quoted_payment` keeps) goes; a
    /// purchase pays less, down to nothing in lump Gold. `None` when the gene
    /// is off or there is less than a whole Gold to move. The values are
    /// re-read so the caller's own filters still hold.
    pub(crate) fn deal_at_the_ceiling(
        &self,
        g: &Game,
        pid: usize,
        deal: &crate::game::QuickDeal,
    ) -> Option<crate::game::QuickDeal> {
        if !self.deals_at_the_ceiling {
            return None;
        }
        let (_, partner_net) = g.trade_utilities(pid, deal.partner, &deal.offer, &deal.request);
        let slack = (partner_net - Self::DEAL_CEILING_MARGIN).floor();
        if slack < 1.0 {
            return None;
        }
        let mut sharp = deal.clone();
        if deal.direction == "sell" {
            let treasury = g.players[deal.partner].gold;
            let reserve = (treasury * 0.30).min(40.0);
            let spendable = (treasury - reserve).max(0.0).floor();
            let extra = slack.min(spendable - deal.request.gold).max(0.0);
            if extra < 1.0 {
                return None;
            }
            sharp.request.gold += extra;
        } else {
            let extra = slack.min(deal.offer.gold.floor());
            if extra < 1.0 {
                return None;
            }
            sharp.offer.gold -= extra;
        }
        let (mine, theirs) = g.trade_utilities(pid, sharp.partner, &sharp.offer, &sharp.request);
        sharp.my_value = mine;
        sharp.partner_value = theirs;
        Some(sharp)
    }

    /// Close a quote: at the ceiling when the gene sharpens it, else as
    /// quoted — and as quoted when the sharpened terms are refused, so the
    /// gene can only ever add to what the shipped exchange would have got.
    pub(crate) fn close_quick_deal(
        &self,
        g: &mut Game,
        pid: usize,
        deal: crate::game::QuickDeal,
    ) -> bool {
        // Never a gift, never a demand: the engine allows the first (as
        // Civilization VI does) and this controller does not make one.
        if deal.offer.is_empty() || deal.request.is_empty() {
            return false;
        }
        if let Some(sharp) = self.deal_at_the_ceiling(g, pid, &deal) {
            let kind = sharp.item.as_str();
            let direction = sharp.direction.as_str();
            let gold = sharp.request.gold - sharp.offer.gold;
            let margin = Self::DEAL_CEILING_MARGIN;
            think!(self.journal, Diplomacy, Decision,
                   "{direction} of {kind} priced at the counterparty's ceiling";
                   "the midpoint quote left half the surplus on the table; this one leaves \
                    {margin} Gold and nets {gold:.0}");
            if g.apply(
                pid,
                &Action::Trade {
                    player: sharp.partner,
                    offer: Box::new(sharp.offer),
                    request: Box::new(sharp.request),
                },
            )
            .is_ok()
            {
                return true;
            }
        }
        g.apply(
            pid,
            &Action::Trade {
                player: deal.partner,
                offer: Box::new(deal.offer),
                request: Box::new(deal.request),
            },
        )
        .is_ok()
    }

    /// Execute at most one pre-negotiated exchange on a staggered cadence.
    /// `Game::quick_deals` has already valued both sides, and `Action::Trade`
    /// revalidates the contract atomically, so the AI never relies on gifts,
    /// exploits stale quotes, or trades when either empire would lose value.
    pub(crate) fn bilateral_trade(&self, g: &mut Game, pid: usize) {
        self.bilateral_trade_excluding(g, pid, None);
    }

    pub(crate) fn bilateral_trade_excluding(
        &self,
        g: &mut Game,
        pid: usize,
        excluded_partner: Option<usize>,
    ) {
        if self.minor || self.barb || g.turn % 6 != (pid as u32 % 6) {
            return;
        }
        let best = g
            .quick_deals(pid)
            .into_iter()
            .filter(|deal| Some(deal.partner) != excluded_partner)
            .max_by(|left, right| {
                self.deal_objective(left)
                    .partial_cmp(&self.deal_objective(right))
                    .unwrap()
            });
        let Some(deal) = best.filter(|deal| deal.my_value >= 2.0 && deal.partner_value >= 2.0)
        else {
            return;
        };
        self.close_quick_deal(g, pid, deal);
    }

    /// The most contracts one declaration will sell into. Every accepted quote
    /// takes its own asset off the next round — a sold luxury copy is worth
    /// only a duplicate's price to a rival that now has one, and granted Open
    /// Borders are worth nothing twice — so the pass drains rather than
    /// repeats. This bound is the backstop, not the mechanism.
    const WAR_EVE_CONTRACTS: usize = 8;

    /// Turn the promises a declaration is about to cancel into Gold that is
    /// already banked when it lands.
    ///
    /// Nothing here is a bluff the engine has to be talked into.
    /// `Game::war_eve_deals` prices each cancellable commitment at the buyer's
    /// own walk-away, so `validate_trade` still sees a rival profiting by its
    /// own reckoning at the moment it signs; then
    /// `end_bilateral_relations_for_war` hands the luxury copy back and stops
    /// the instalments. A declaration that fails to apply leaves ordinary
    /// mutually profitable trades behind, which is why this may run before the
    /// war is certain.
    ///
    /// It re-quotes after every contract because each sale moves both the
    /// rival's treasury and this empire's uncommitted income, and it takes
    /// nothing whose lump payment does not clear the instalment paid at
    /// signing. Passing the pending action rather than a player id keeps the
    /// pass impossible to point at a civilization this turn is not about to
    /// attack. Returns the net Gold raised.
    pub(crate) fn war_eve_liquidation(&self, g: &mut Game, pid: usize, action: &Action) -> f64 {
        let target = match action {
            Action::DeclareWar { player } | Action::DeclareWarWithCasusBelli { player, .. } => {
                *player
            }
            _ => return 0.0,
        };
        if self.minor || self.barb {
            return 0.0;
        }
        let mut raised = 0.0;
        let mut sold: Vec<String> = Vec::new();
        for _ in 0..Self::WAR_EVE_CONTRACTS {
            let Some(deal) = g.war_eve_deals(pid, target).into_iter().next() else {
                break;
            };
            let net = Game::war_eve_net_gold(&deal);
            if net <= 0.0 {
                break;
            }
            let item = deal.item.clone();
            if g.apply(
                pid,
                &Action::Trade {
                    player: target,
                    offer: Box::new(deal.offer),
                    request: Box::new(deal.request),
                },
            )
            .is_err()
            {
                break;
            }
            raised += net;
            sold.push(plain(&item));
        }
        if raised > 0.0 {
            think!(self.journal, Diplomacy, Decision,
                   "Selling {} what the coming war will cancel", g.players[target].civ;
                   "{raised:.0} Gold in hand now for {}, a thirty-turn contract that ends \
                    with the declaration",
                   sold.join(", "));
        }
        raised
    }
}
