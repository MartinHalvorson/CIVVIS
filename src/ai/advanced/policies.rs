//! Policies and governors: which policy cards `AdvancedAi` slots and where
//! it sends its governors.
//!
//! `strategic_policies` builds the policy deck each turn. The card checks
//! beside it say when a card pays or fits: the pantheon faith card,
//! Merchant Confederation, Urban Planning against Colonization and the
//! Builder cards, maintenance relief, loyalty, the Liberalism repair bar,
//! and whether a swap fits. `strategic_governors` appoints and promotes
//! governors (`governor_priority`, `best_governor_city`,
//! `governor_promotion_priority`).
//!
//! Moved verbatim out of the one `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-05). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged.

use super::*;

impl AdvancedAi {
    /// Whether God-King's Faith is what stands between this empire and its
    /// pantheon. See `expansion_pantheon`. True while the empire has no
    /// pantheon, holds less Faith than the pantheon costs, and would collect
    /// under `PANTHEON_FAITH_CARD_FLOOR` Faith a turn without the card — an
    /// empire with a Holy Site or a Faith wonder reaches the price on its own
    /// and keeps its slot for the plan's cards.
    pub(crate) fn pantheon_faith_card_pays(&self, g: &Game, pid: usize) -> bool {
        let Some(player) = g.players.get(pid) else {
            return false;
        };
        if player.pantheon.is_some() || player.faith >= g.pantheon_faith_cost() {
            return false;
        }
        let income: f64 = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.city_yields(cid).faith)
            .sum::<f64>()
            + g.player_yield_extras(pid).faith;
        let card_share = if player.policies.contains(&crate::name!("god_king")) {
            g.policy_effect(pid, "capital_faith")
        } else {
            0.0
        };
        income - card_share < PANTHEON_FAITH_CARD_FLOOR
    }

    /// Whether replacing one active card with another can fit in the current
    /// typed and wildcard policy slots. This mirrors the engine's set-level
    /// seating rule before either action is applied: probing by actually
    /// unslotting a card writes a real action even when the candidate then
    /// fails and the old card is restored.
    fn policy_swap_fits(g: &Game, pid: usize, current: &Name, candidate: &str) -> bool {
        let slots = g.gov_slots(pid);
        let (mut military, mut economic, mut diplomatic, mut wildcard) = (0i64, 0i64, 0i64, 0i64);
        let mut count = |card: &str| match g.rules.policies[card].slot.as_str() {
            "military" => military += 1,
            "economic" => economic += 1,
            "diplomatic" => diplomatic += 1,
            _ => wildcard += 1,
        };
        for card in g.players[pid]
            .policies
            .iter()
            .filter(|card| *card != current)
        {
            count(card);
        }
        count(candidate);
        let overflow = (military - slots.military).max(0)
            + (economic - slots.economic).max(0)
            + (diplomatic - slots.diplomatic).max(0);
        overflow + wildcard <= slots.wildcard
    }

    /// Whether the live bridge has a governed city whose measured Loyalty
    /// decline is close enough to warrant the Governor-specific policy card.
    /// Frozen controllers do not have the host rate signal and therefore keep
    /// the recorded policy deck exactly as it was.
    pub(super) fn loyalty_policy_needed(&self, g: &Game, pid: usize) -> bool {
        self.base.loyalty_rate_alarm
            && g.player_city_ids(pid).into_iter().any(|city| {
                self.base
                    .loyalty_emergency(g, city)
                    .is_some_and(|turns| turns.is_finite() && turns <= LOYALTY_POLICY_TRIGGER_TURNS)
                    && g.governor_effect(pid, city, "loyalty") > 0.0
            })
    }

    /// Reassess policy cards as the civic tree advances instead of treating
    /// the first cards which filled a government as permanent.  Each plan has
    /// a complete late-game portfolio, while temporary Dark Age cards are
    /// admitted only when their explicit downside is safe for the live empire.
    /// Typed cards preferentially replace cards of their own type so wildcard
    /// capacity remains useful.
    /// `merchant-confederation-earns-its-slot`: our placed Envoys and the Gold
    /// a turn Merchant Confederation pays for them, when the card is held or
    /// available, the government has a diplomatic slot, and the Gold reaches
    /// [`MERCHANT_CONFEDERATION_GOLD_FLOOR`]. None with the gene off.
    ///
    /// The static portfolios never name the card outside Diplomacy, so a
    /// Conquest deck left it to the base deck, which scored it 0.0 (see
    /// `empire_reading`), and once the plan's own diplomatic cards unlocked
    /// they evicted anything else. Live Emperor civvis-20261014T083713Z:
    /// available from t100, never slotted, 41 Envoys at t150 and 63 at t205
    /// while income ran -11 to -48 a turn, the slot holding Charismatic
    /// Leader and then Gunboat Diplomacy.
    pub(super) fn merchant_confederation_pays(&self, g: &Game, pid: usize) -> Option<(i64, f64)> {
        if !self.merchant_confederation_earns_its_slot
            && !self.merchant_confederation_earns_its_slot_2
        {
            return None;
        }
        let card = crate::name!("merchant_confederation");
        let spec = g.rules.policies.get(&card)?;
        let held = g.players[pid].policies.contains(&card);
        if !held && !g.available_policies(pid).contains(&card) {
            return None;
        }
        if g.gov_slots(pid).diplomatic < 1 {
            return None;
        }
        let per_envoy = spec.effects.get("gold_per_envoy").copied().unwrap_or(0.0);
        let envoys: i64 = g.players[pid].envoys.iter().map(|(_, count)| *count).sum();
        let gold = per_envoy * envoys as f64;
        (gold >= MERCHANT_CONFEDERATION_GOLD_FLOOR).then_some((envoys, gold))
    }

    /// Whether the deck holds a maintenance-relief card (Conscription,
    /// Levée en Masse). With `policy-deck-hysteresis` the deck the turn began
    /// with counts too. The base governor's `revise_policy_deck` runs first
    /// in the turn and may unslot the card, and the relief then read as
    /// already gone. The emergency dropped it while the Gold still covered
    /// the bill, and the host charged the full bill next turn. Live King
    /// 2026-10-03T040354Z t206: Levée left the deck at 33 Gold and +2.9 a
    /// turn, the bill rose from 206 to 272, and the treasury hit zero.
    pub(super) fn maintenance_relief_held(&self, g: &Game, pid: usize) -> bool {
        let relief = |deck: &BTreeSet<Name>| {
            deck.iter()
                .any(|card| matches!(card.as_str(), "conscription" | "levee_en_masse"))
        };
        relief(&g.players[pid].policies)
            || ((self.policy_deck_hysteresis || self.policy_deck_hysteresis_2)
                && relief(&self.turn_start_policies))
    }

    /// Whether a held maintenance relief still pays for itself: with
    /// `policy-deck-hysteresis-2`, the maintenance emergency outlasts a
    /// treasury over the reserve while the income is below the discount the
    /// card gives (1 Gold per military unit, Conscription's; Levée's is 2).
    /// Live King 2026-10-03T081800Z t205: a mid-turn windfall took the
    /// treasury from 250 to 485, over the 300 reserve, so the emergency ended
    /// and Conscription, which no base conquest list names, was the first card
    /// evicted. Income went from +14 to -15 and the unit bill from 104 to 133;
    /// the treasury was empty at t211 and the army disbanded from 34 units to
    /// 7 by t234.
    pub(super) fn relief_outruns_income(&self, g: &Game, pid: usize, military: usize) -> bool {
        self.policy_deck_hysteresis_2
            && self.maintenance_relief_held(g, pid)
            && g.players[pid].gold_per_turn < military as f64
    }

    /// The Amenity surplus under which a two-district city calls for
    /// Liberalism. Liberalism is +1 Amenity in exactly those cities, so once
    /// it is slotted a city that sits at 0 needs it to stay content. Without
    /// `policy-deck-hysteresis` the rule asked only while a deficit showed.
    /// It then swapped Liberalism out the turn after it had repaired the
    /// deficit, and back in a turn later: live King 2026-10-03T040354Z
    /// alternated Aesthetics and Liberalism every turn from 110 to 132.
    pub(super) fn liberalism_repair_bar(&self, g: &Game, pid: usize) -> i64 {
        if (self.policy_deck_hysteresis || self.policy_deck_hysteresis_2)
            && g.players[pid]
                .policies
                .contains(&crate::name!("liberalism"))
        {
            1
        } else {
            0
        }
    }

    /// See `colonization_earns_its_slot`: whether Urban Planning, available
    /// to this empire, adds more production than Colonization's +50% adds to
    /// the cities whose queue opens with a Settler.
    fn urban_planning_outearns_colonization(g: &Game, pid: usize, city_ids: &[u32]) -> bool {
        let planning = Name::new("urban_planning");
        let colonization = Name::new("colonization");
        if !g.available_policies(pid).contains(&planning)
            && !g.players[pid].policies.contains(&planning)
        {
            return false;
        }
        let pct = |card: &Name, effect: &str| {
            g.rules
                .policies
                .get(card)
                .and_then(|spec| spec.effects.get(effect).copied())
                .unwrap_or(0.0)
        };
        let settler_gain: f64 = city_ids
            .iter()
            .filter(|city| {
                matches!(
                    g.cities[city].queue.first(),
                    Some(Item::Unit { unit }) if unit == "settler"
                )
            })
            .map(|city| {
                g.city_yields(*city).production * pct(&colonization, "settler_production_pct")
                    / 100.0
            })
            .sum();
        let planning_gain = pct(&planning, "city_production") * city_ids.len() as f64;
        planning_gain > settler_gain
    }

    /// See `colonization_earns_its_slot_2`: whether Urban Planning, available
    /// to this empire, adds more production than Ilkum's +30% adds to the
    /// cities whose queue opens with a Builder, while no card adding build
    /// charges (Serfdom, Public Works) is on the menu or slotted.
    fn urban_planning_outearns_builder_cards(g: &Game, pid: usize, city_ids: &[u32]) -> bool {
        let planning = Name::new("urban_planning");
        let held_or_offered = |card: &Name| {
            g.players[pid].policies.contains(card) || g.available_policies(pid).contains(card)
        };
        if !held_or_offered(&planning)
            || ["serfdom", "public_works"]
                .iter()
                .any(|card| held_or_offered(&Name::new(card)))
        {
            return false;
        }
        let pct = |card: &str, effect: &str| {
            g.rules
                .policies
                .get(&Name::new(card))
                .and_then(|spec| spec.effects.get(effect).copied())
                .unwrap_or(0.0)
        };
        let ilkum_gain: f64 = city_ids
            .iter()
            .filter(|city| {
                matches!(
                    g.cities[city].queue.first(),
                    Some(Item::Unit { unit }) if unit == "builder"
                )
            })
            .map(|city| {
                g.city_yields(*city).production * pct("ilkum", "builder_production_pct") / 100.0
            })
            .sum();
        pct("urban_planning", "city_production") * city_ids.len() as f64 > ilkum_gain
    }

    pub(super) fn strategic_policies(&self, g: &mut Game, pid: usize, strategy: GrandStrategy) {
        if g.host_policy_change_blocked(pid) {
            return;
        }
        let objective = self.decision_objective(strategy);

        // A successor card removes its predecessor from the policy menu.  An
        // already slotted predecessor used to survive forever, which is how
        // future governments reached the archive still running Ilkum and
        // Colonization.  Retire those cards before choosing the new portfolio.
        let obsolete: HashSet<Name> = g
            .rules
            .policies
            .values()
            .filter(|policy| {
                policy
                    .civic
                    .as_ref()
                    .is_none_or(|civic| g.players[pid].civics.contains(civic))
            })
            .flat_map(|policy| policy.replaces.iter().copied())
            .collect();
        let obsolete_active: Vec<Name> = g.players[pid]
            .policies
            .iter()
            .filter(|card| obsolete.contains(&Name::new(card.as_str())))
            .cloned()
            .collect();
        for card in obsolete_active {
            think!(self.journal(), Policies, Decision, "Retiring {}", plain(&card);
                   "a successor card has replaced it on the menu");
            let _ = g.apply(pid, &Action::UnslotPolicy { policy: card });
        }

        let mut desired: Vec<&str> = match objective {
            GrandStrategy::Science => vec![
                "integrated_space_cell",
                "future_victory_science",
                "five_year_plan",
                "rationalism",
                "nobel_prize",
                "international_space_agency",
                "ecommerce",
                "market_economy",
                "new_deal",
                "levee_en_masse",
                "military_research",
                "cryptography",
                "future_counter_science",
            ],
            GrandStrategy::Culture => vec![
                "future_victory_culture",
                "heritage_tourism",
                "satellite_broadcasts",
                "online_communities",
                "collective_activism",
                "sports_media",
                "grand_opera",
                "symphonies",
                "ecommerce",
                "new_deal",
                "gunboat_diplomacy",
                "cryptography",
                "communications_office",
                "levee_en_masse",
            ],
            GrandStrategy::Religion => vec![
                "religious_orders",
                "simultaneum",
                "scripture",
                "revelation",
                "wars_of_religion",
                "collectivization",
                "new_deal",
                "ecommerce",
                "levee_en_masse",
                "gunboat_diplomacy",
                "cryptography",
                "wisselbanken",
                "raj",
            ],
            GrandStrategy::Diplomacy => vec![
                "future_victory_diplomatic",
                "gunboat_diplomacy",
                "charismatic_leader",
                "raj",
                "merchant_confederation",
                "diplomatic_league",
                "containment",
                "collective_activism",
                "international_space_agency",
                "cryptography",
                "communications_office",
                "wisselbanken",
                "ecommerce",
                "new_deal",
                "levee_en_masse",
            ],
            GrandStrategy::Conquest => vec![
                "future_victory_domination",
                "future_counter_domination",
                "military_first",
                "strategic_air_force",
                "lightning_warfare",
                "total_war",
                "propaganda",
                "levee_en_masse",
                "force_modernization",
                "after_action_reports",
                "logistics",
                "five_year_plan",
                "ecommerce",
                "gunboat_diplomacy",
                "cryptography",
                "new_deal",
            ],
            GrandStrategy::Expansion => vec![
                "expropriation",
                "public_works",
                "five_year_plan",
                "economic_union",
                "collectivization",
                "ecommerce",
                "market_economy",
                "new_deal",
                "colonial_taxes",
                "colonial_offices",
                "gunboat_diplomacy",
                "levee_en_masse",
                "logistics",
                "rationalism",
                "cryptography",
            ],
            GrandStrategy::Recovery => vec![
                "new_deal",
                "liberalism",
                "civil_prestige",
                "collectivization",
                "five_year_plan",
                "ecommerce",
                "market_economy",
                "wisselbanken",
                "raj",
                "gunboat_diplomacy",
                "cryptography",
                "communications_office",
                "levee_en_masse",
                "logistics",
                "public_works",
            ],
        };

        // For the short defender-only declaration window, let military
        // production and maintenance cards displace the peaceful portfolio.
        // The helper is an exact no-op when its opt-in gene is off.
        self.prioritize_surprise_defense_policies(g, pid, &mut desired);

        // A Governor already in a city is the one place where Praetorium's
        // effect can apply. Put it ahead of the ordinary victory portfolio
        // only for a live, measured decline with a short runway; a healthy
        // governed city must not trade away a lane card for a merely
        // hypothetical loyalty benefit.
        let loyalty_policy_needed = self.loyalty_policy_needed(g, pid);
        if loyalty_policy_needed {
            desired.insert(0, "praetorium");
        }
        // `bleeding-capital-loyalty`: Limitanei's +2 with the garrison while a
        // captured original capital bleeds. See `one_war::bleeding_capital`.
        if self.bleeding_capital_loyalty && self.bleeding_capital(g, pid).is_some() {
            desired.insert(0, "limitanei");
        }

        // ⚠ FRONT of the list, not appended: these portfolios are preference
        // ordered and the military slot is contested from the ancient era, which is
        // exactly when loyalty pressure starts. Appending would put `limitanei`
        // behind sixteen late-game cards and it would never be reached — which is
        // indistinguishable from today's behaviour and would make the arm measure
        // nothing.
        if self.garrison_loyalty_policy
            && matches!(
                objective,
                GrandStrategy::Conquest | GrandStrategy::Expansion
            )
        {
            desired.insert(0, "limitanei");
        }
        if g.has_ability(pid, "taxis") && objective == GrandStrategy::Conquest {
            desired.splice(0..0, ["chivalry", "maneuver", "raid", "conscription"]);
        }

        let city_ids = g.player_city_ids(pid);
        let unit_ids = g.player_unit_ids(pid);
        let settlers = unit_ids
            .iter()
            .filter(|unit| g.units[unit].kind == "settler")
            .count();
        let city_goal = self
            .plan
            .as_ref()
            .map(|plan| plan.desired_cities)
            .unwrap_or_else(|| self.base.w.city_target.ceil().max(1.0) as usize);
        let expansion_complete = strategy != GrandStrategy::Expansion
            && settlers == 0
            && !city_ids.is_empty()
            && city_ids.len() >= city_goal;
        // Time live economic slots to what their cities are producing. Static
        // victory portfolios begin with late cards, so Science used to replace
        // Colonization and Ilkum while actively building their units. Policy
        // selection also precedes production, which makes the city-plan gap
        // the signal for pre-slotting Colonization this turn.
        // The rapid-expansion gene needs the same timely Settler card as the
        // wide-map plan: its opening target is intentionally active before
        // the normal map-capacity treatment would turn this portfolio arm
        // on.  Without it, the gene starts its second wave at the full price
        // even after Early Empire unlocks Colonization.
        // `expansion-scales-with-difficulty` needs the same timely Settler
        // card for the same reason: its target is deliberately active before
        // the map-capacity treatment would arm this portfolio arm.
        if self.wide_map_capacity
            || self.rapid_city_expansion_2
            || self.expansion_scales_with_difficulty
        {
            let settler_queued = city_ids.iter().any(|city| {
                matches!(
                    g.cities[city].queue.first(),
                    Some(Item::Unit { unit }) if unit == "settler"
                )
            });
            let builder_queued = city_ids.iter().any(|city| {
                matches!(
                    g.cities[city].queue.first(),
                    Some(Item::Unit { unit }) if unit == "builder"
                )
            });
            let expansion_active = settler_queued || city_ids.len() + settlers < city_goal;
            // `colonization-earns-its-slot`: an expansion phase with no
            // Settler worth half Urban Planning's yield in production leaves
            // the slot to Urban Planning this turn.
            let expansion_active = expansion_active
                && !((self.colonization_earns_its_slot || self.colonization_earns_its_slot_2)
                    && Self::urban_planning_outearns_colonization(g, pid, &city_ids));
            // Version 2: and a queued Builder only for a charge card or an
            // Ilkum that out-produces Urban Planning.
            let builder_queued = builder_queued
                && !(self.colonization_earns_its_slot_2
                    && Self::urban_planning_outearns_builder_cards(g, pid, &city_ids));
            if expansion_active || builder_queued {
                const TIMED_ECONOMY: [&str; 6] = [
                    "expropriation",
                    "colonization",
                    "public_works",
                    "serfdom",
                    "ilkum",
                    "urban_planning",
                ];
                desired.retain(|card| !TIMED_ECONOMY.contains(card));
                let mut timed = Vec::new();
                if expansion_active {
                    timed.extend(["expropriation", "colonization"]);
                }
                if builder_queued {
                    timed.extend(["public_works", "serfdom", "ilkum"]);
                }
                let committed_slots = usize::from(expansion_active) + usize::from(builder_queued);
                if g.gov_slots(pid).economic.max(0) as usize > committed_slots {
                    timed.push("urban_planning");
                }
                timed.extend(desired);
                desired = timed;
            }
        }
        let at_war = g
            .at_war
            .iter()
            .any(|(first, second)| *first == pid || *second == pid);
        let military = unit_ids
            .iter()
            .filter(|unit| g.rules.units[g.units[unit].kind].class == "military")
            .count();
        let at_major_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        });
        // A zero treasury turns a negative cash flow into an immediate
        // empire-wide Amenity penalty before recovery production can finish a
        // Commercial Hub. The live war governor already knows it must keep
        // the defending force, so use the current maintenance discount before
        // asking that force to carry another turn of bankruptcy.
        //
        // This must also happen while a confirmed Conquest campaign is still
        // staging. On the live Rome board, the counter-leader plan held 85
        // Gold at -59 per turn before it declared: waiting for the declaration
        // (or for Gold to reach exactly zero) let the maintenance bill consume
        // its entire emergency reserve first. The same `100 + 25/city`
        // runway that diverts production into recovery is the relevant bound
        // here. An ordinary peaceful deficit still keeps its normal policy
        // portfolio; only a major war or a named Conquest target may trade a
        // military card for solvency.
        //
        // `levee_en_masse` is deliberately listed first: its civic retires
        // `conscription`, so the loop below takes the strongest available
        // version without a second card-selection path.
        let recovery_reserve = 100.0 + 25.0 * city_ids.len() as f64;
        let staged_conquest = objective == GrandStrategy::Conquest
            && self.plan.as_ref().is_some_and(|plan| {
                plan.target_player.is_some_and(|target| {
                    g.players.get(target).is_some_and(|player| {
                        player.alive && !player.is_minor && !player.is_barbarian
                    })
                })
            });
        // Explicit domination already shares the production recovery guard.
        // Its army needs the same maintenance relief even when the optional
        // war-economy flag is off (native 085522, available Conscription).
        let domination_target = self.active_victory_target(g) == Some(VictoryTarget::Domination);
        // Positive income may be the discount's own effect. Keep it while
        // rebuilding the reserve instead of immediately restoring the bill.
        let retained_domination_relief = domination_target && self.maintenance_relief_held(g, pid);
        // A discounted upgrade still needs cash. Give a named offensive
        // upkeep relief when four turns of income cannot fund its cohort.
        let upgrade_funding_relief = domination_target
            && (at_major_war || staged_conquest)
            && self.domination_upgrade_funding_shortfall(g, pid);
        let maintenance_emergency = (self.war_economy || domination_target)
            && (at_major_war || staged_conquest)
            && military > 0
            && (g.players[pid].gold < recovery_reserve
                || (domination_target && self.relief_outruns_income(g, pid, military)))
            && (g.players[pid].gold_per_turn < -0.5 || retained_domination_relief)
            || upgrade_funding_relief;
        if maintenance_emergency {
            desired.retain(|card| !matches!(*card, "conscription" | "levee_en_masse"));
            desired.splice(0..0, ["levee_en_masse", "conscription"]);
        }
        if self.victory_planning
            && objective == GrandStrategy::Conquest
            && military >= city_ids.len().max(1).saturating_mul(2)
        {
            // Military and economic cards mostly use different typed slots. Once the
            // field army has reached the same two-per-city ceiling as production,
            // promote the science/culture multipliers which keep that army current.
            // This preserves the conquest plan and all of its tactical behavior; it
            // only prevents a winning war from freezing the civic economy forever.
            const WARTIME_ECONOMY: [&str; 5] = [
                "rationalism",
                "grand_opera",
                "five_year_plan",
                "aesthetics",
                "natural_philosophy",
            ];
            desired.retain(|card| !WARTIME_ECONOMY.contains(card));
            desired.splice(0..0, WARTIME_ECONOMY);
        }
        // The deck has the same lane-keyed shape as every other science term.
        // Counted over the six lists above, the Campus multipliers appear in
        // exactly two: Science and Expansion. **The Religion and Culture decks
        // contain no science card at all** — and the deployment genome is
        // `adv-religious`, so in the live game Rationalism and Natural
        // Philosophy are unreachable unless a Conquest army happens to saturate
        // and trigger `WARTIME_ECONOMY` above. That is exactly what the two
        // recorded runs show: the religion-heavy `civvis-20260803T005930Z`
        // finished with all three science cards *available and unslotted*,
        // while `civvis-20260802T220818Z` had Rationalism and Natural
        // Philosophy in and reached 60.9 science against the other's 12.3.
        //
        // Both cards multiply Campus buildings, so the empire must own a Campus
        // for them to mean anything — that gate is the whole precondition, and
        // it is why this cannot fire in a seat these cards would be wasted in.
        // Inserted after the lane's own leading cards rather than in front of
        // them: a Religion empire keeps its opening, and picks these up with a
        // slot it would otherwise fill from the tail of its list.
        if self.research_economy {
            const RESEARCH_MULTIPLIERS: [&str; 2] = ["rationalism", "natural_philosophy"];
            let has_campus = city_ids
                .iter()
                .any(|city| g.city_has_district_family(&g.cities[city], crate::name!("campus")));
            if has_campus {
                desired.retain(|card| !RESEARCH_MULTIPLIERS.contains(card));
                let at = desired.len().min(RESEARCH_DECK_INSERT);
                desired.splice(at..at, RESEARCH_MULTIPLIERS);
            }
        }

        // A host-observed Amenity deficit is an immediate yield loss, but a
        // repair district needs several turns before it changes that fact. If
        // Liberalism can help at least two already-developed cities, use the
        // one card that repairs both now. `desired_set` below intentionally
        // protects every ordinary desired card from replacement, so merely
        // inserting Liberalism would leave a currently slotted Aesthetics
        // untouched. Demote that narrow, same-slot culture-adjacency donor
        // explicitly; Grand Opera and expansion's Expropriation stay wanted.
        //
        // This shares the live-only crisis arm with the concrete repair chain:
        // normal and frozen controller decks return before reading city
        // Amenities at all.
        // Luxury allocation is empire-wide: one memo scope per sweep, or each
        // city's surplus allocates the whole empire again.
        if self.amenity_project_preemption_on() && {
            let _memo = g.query_memo();
            city_ids
                .iter()
                .filter(|cid| {
                    let city = &g.cities[cid];
                    g.city_specialty_district_count(city) >= 2
                        && g.city_amenity_surplus(city) < self.liberalism_repair_bar(g, pid)
                })
                .count()
                >= 2
        } {
            desired.retain(|card| !matches!(*card, "aesthetics" | "liberalism"));
            desired.insert(0, "liberalism");
        }
        // `culture-keeps-its-aesthetics`: read after the Liberalism repair so
        // the demotion above cannot unslot it on a Culture target. See
        // `advanced/culture_aesthetics.rs`.
        if self.culture_aesthetics_leads(g, &city_ids) {
            desired.retain(|card| !matches!(*card, "aesthetics" | "natural_philosophy"));
            desired.insert(0, "aesthetics");
        }

        let elite_active = g.players[pid]
            .policies
            .contains(&crate::name!("elite_forces"));
        let elite_affordable = if elite_active {
            g.players[pid].gold_per_turn >= 5.0
        } else {
            g.players[pid].gold_per_turn >= military as f64 * 2.0 + 5.0
        };
        let robber_active = g.players[pid]
            .policies
            .contains(&crate::name!("robber_barons"));
        let robber_pays = city_ids.iter().any(|city| {
            let city = &g.cities[city];
            city.buildings.iter().any(|building| {
                matches!(building.as_str(), "stock_exchange" | "factory")
                    && !city.pillaged_buildings.contains(building)
            })
        });
        let robber_safe = robber_pays && {
            let _memo = g.query_memo();
            city_ids.iter().all(|city| {
                g.city_amenity_surplus(&g.cities[city]) >= if robber_active { 0 } else { 2 }
            })
        };
        let holy_site_cities = city_ids
            .iter()
            .filter(|city| g.city_has_district_family(&g.cities[city], crate::name!("holy_site")))
            .count();

        let mut temporary = Vec::new();
        match objective {
            GrandStrategy::Science if holy_site_cities * 2 >= city_ids.len().max(1) => {
                temporary.push("monasticism");
            }
            GrandStrategy::Religion if g.players[pid].religion.is_some() => {
                temporary.push("inquisition");
            }
            GrandStrategy::Conquest if at_war && elite_affordable => {
                temporary.push("elite_forces");
            }
            _ => {}
        }
        if expansion_complete
            && matches!(
                objective,
                GrandStrategy::Science
                    | GrandStrategy::Culture
                    | GrandStrategy::Diplomacy
                    | GrandStrategy::Recovery
            )
        {
            temporary.push("isolationism");
        }
        if robber_safe
            && matches!(
                objective,
                GrandStrategy::Science
                    | GrandStrategy::Culture
                    | GrandStrategy::Expansion
                    | GrandStrategy::Recovery
            )
        {
            temporary.push("robber_barons");
        }
        // ★★★★★ CITIES BLEEDING LOYALTY HAD ONE LEVER AND NO POLICY RESPONSE.
        //
        // `loyalty_emergency` reads the RATE (that is what #981 fixed). The
        // inherited governor pass can move one established Governor, while the
        // live policy response below can add a small Loyalty buffer when that
        // move cannot reach an already governed city.
        //
        // Measured over 117 city losses in 91 runs: **47 were revolts**, and they died
        // at loyalty 0.1-1.9 carrying -13 to -22 a turn. The warning is long and
        // nothing spends it:
        //
        // ```text
        //   turns loyalty_per_turn was NEGATIVE before the loss   median 13, max 39
        //   turns from loyalty crossing 50 to the loss            median  8
        // ```
        //
        // Five policy cards carry loyalty effects and every one is already honoured by
        // `Game::city_loyalty_per_turn`. **Across those 91 runs only COLONIAL_OFFICES
        // was ever slotted, in 5 of them; the remaining direct loyalty cards were
        // absent from the ordinary deck** — including `limitanei`, which needs only
        // `early_empire`, an ancient civic reached in essentially every game, and
        // sits in a MILITARY slot nothing competes for. The live emergency arm now
        // names `praetorium` when a governed city has a short measured runway.
        //
        // ★ This is also what makes a garrison worth anything. Garrisoning has measured
        // null as a military mechanism (~2pp, parity), but with Limitanei slotted the
        // same unit is +2 loyalty a turn in the city it stands in — the same action
        // priced on the axis that actually loses us cities.
        //
        // Ranked ahead of the plan's ordinary cards because a revolt is unrecoverable
        // and a yield card is not.
        // ★★★★ THE FAITH THAT BUYS THE PANTHEON COMES FROM ONE CARD, AND THE
        // PLAN THREW IT AWAY. See `expansion_pantheon`: God-King is the live
        // seat's only early Faith, the portfolio swapped it out at the first
        // civic after Code of Laws, and the pantheon — a free Settler under
        // Religious Settlements — landed at median t22 and as late as t108.
        // Wanted first, so `desired_set` below keeps it slotted through the
        // civic swap; it leaves the list the turn the pantheon is founded and
        // the slot goes to Colonization exactly as before.
        if self.expansion_pantheon && self.pantheon_faith_card_pays(g, pid) {
            temporary.retain(|card| *card != "god_king");
            temporary.insert(0, "god_king");
        }

        temporary.extend(desired);
        desired = temporary;
        desired.retain(|card| g.rules.policies[*card].offered(&g.players[pid].age, g.world_era));

        // `prophet-race-earns-its-points`: Revelation's +2 Prophet points a
        // turn while the race has a Prophet for this seat. The deck prices a
        // Great Person card at zero, so it never took the wildcard on its own
        // (game 110504: on offer t40-t55, never slotted). Ahead of the lane's
        // portfolio, behind the emergency cards spliced in below; once the
        // race is won or closed it leaves the list and the slot goes back.
        let race_card = self.race_wants_revelation(g, pid).then_some("revelation");
        if let Some(card) = race_card {
            desired.retain(|wanted| *wanted != card);
            desired.insert(0, card);
        }

        // Nobel Peace scores the Favor this seat generates while its host
        // clock is live. A policy can be slotted now and starts that stream on
        // the following turn, so price ordinary direct-Favor cards against the
        // same deadline as a timely building or suzerainty.  Keep Dark Age
        // bargains such as Disinformation Campaign out: their Broadcast Center
        // rate comes with an empire-wide Science/Culture cost and is not a
        // free contest score.
        let mut nobel_peace_direct_favor_cards: Vec<(&str, f64)> = g
            .rules
            .policies
            .iter()
            .filter(|(_, policy)| !policy.dark_age)
            .filter_map(|(name, policy)| {
                let favor_per_turn = policy
                    .effects
                    .get("diplomatic_favor_per_turn")
                    .copied()
                    .unwrap_or(0.0);
                (self.nobel_peace_favor_score_value(g, pid, favor_per_turn, 0.0) > f64::EPSILON)
                    .then_some((name.as_str(), favor_per_turn))
            })
            .collect();
        nobel_peace_direct_favor_cards
            .sort_by(|left, right| right.1.total_cmp(&left.1).then_with(|| left.0.cmp(right.0)));
        let nobel_peace_direct_favor_cards: Vec<&str> = nobel_peace_direct_favor_cards
            .into_iter()
            .map(|(card, _)| card)
            .collect();
        if !nobel_peace_direct_favor_cards.is_empty() {
            // Diplomacy already names Diplomatic Capital in its static deck;
            // remove that ordinary occurrence before putting its live contest
            // action ahead of every plan's normal policy portfolio.
            desired.retain(|card| !nobel_peace_direct_favor_cards.contains(card));
            desired.splice(0..0, nobel_peace_direct_favor_cards.iter().copied());
        }

        let builder_window_card = self.builder_charge_window_card(g, pid);
        if let Some(card) = builder_window_card {
            desired.retain(|wanted| *wanted != card);
            desired.insert(0, card);
        }

        // Tourism defense must enter the protected set before ordinary cards
        // are considered, including when the empire targets another victory.
        let culture_defense_cards = self.culture_defense_cards(g, pid);
        desired.retain(|card| !culture_defense_cards.contains(card));
        desired.splice(0..0, culture_defense_cards.iter().copied());
        let unproductive_economic_cards =
            self.domination_productive_policy_fallbacks(g, pid, &mut desired);
        let upgrade_card = self.domination_upgrade_policy(g, pid);
        if let Some(card) = upgrade_card {
            desired.retain(|wanted| *wanted != card);
            desired.insert(0, card);
        }
        // `medina-quarter-houses-the-districts`: while housing-limited cities
        // with three specialty districts qualify, the housing card joins the
        // portfolio here, ahead of the plan's ordinary cards but behind every
        // card inserted at the front below (a wartime gold card, a treasury
        // card); see `advanced/medina_quarter.rs`. `None` with the gene off.
        let medina_quarter = self.medina_quarter_wanted(g, pid);
        if medina_quarter.is_some() {
            desired.retain(|wanted| *wanted != "medina_quarter");
            desired.insert(0, "medina_quarter");
        }
        let protect_wartime_income = self.domination_wartime_gold_card_pays(
            g,
            pid,
            at_major_war,
            staged_conquest,
            recovery_reserve,
        );
        if let Some(marginal) = protect_wartime_income {
            desired.retain(|card| *card != "economic_union");
            desired.insert(0, "economic_union");
            think!(self.journal(), Policies, Detail,
                   "Economic Union funds the wartime army";
                   "{:.0} Gold at {:+.1}/turn; the card adds about {marginal:.1} Gold/turn \
                    and a policy swap must not bankrupt the campaign",
                   g.players[pid].gold, g.players[pid].gold_per_turn);
        }
        // `merchant-confederation-earns-its-slot`: see
        // `merchant_confederation_pays`. Ahead of the first diplomatic card
        // the plan names other than a loyalty Praetorium, so Gunboat
        // Diplomacy and Cryptography no longer evict it, and a desired card
        // itself, so the ordinary rule keeps it slotted.
        let merchant_pays = self.merchant_confederation_pays(g, pid);
        // `merchant-confederation-earns-its-slot-2`: the paying card may take
        // a diplomatic seat from a wanted card ranked after it; see below.
        let merchant_takes_a_seat =
            self.merchant_confederation_earns_its_slot_2 && merchant_pays.is_some();
        if let Some((envoys, gold)) = merchant_pays {
            desired.retain(|card| *card != "merchant_confederation");
            let at = desired
                .iter()
                .position(|card| {
                    *card != "praetorium"
                        && g.rules
                            .policies
                            .get(card)
                            .is_some_and(|spec| spec.slot == "diplomatic")
                })
                .unwrap_or(desired.len());
            desired.insert(at, "merchant_confederation");
            think!(self.journal(), Policies, Detail,
                   "Merchant Confederation pays the envoys";
                   "merchant-confederation-earns-its-slot: {envoys} Envoys placed, about {gold:.0} Gold a turn; \
                    wanted at {} of {} in the {} portfolio",
                   at + 1, desired.len(), objective.as_str());
        }
        // `urban-planning-fills-the-slot`: Urban Planning at the tail of the
        // portfolio, so it fills a free slot and evicts only an orphaned timed
        // economy card: a Builder or Settler card whose window has closed
        // (Serfdom after its Builder finished, Colonization with no Settler),
        // which the plan no longer wants. A held Urban Planning stays wanted.
        let urban_planning_tail =
            self.urban_planning_fills_the_slot && !desired.contains(&"urban_planning") && {
                let card = crate::name!("urban_planning");
                g.players[pid].policies.contains(&card) || g.available_policies(pid).contains(&card)
            };
        if urban_planning_tail {
            desired.push("urban_planning");
        }
        // `bankruptcy-exits-the-drain`: the gold cards that pay go first while
        // the treasury holds less than one turn of the deficit. See
        // `advanced/solvency.rs`.
        let drain_cards = self.drain_gold_cards(g, pid);
        if !drain_cards.is_empty() {
            for (card, _) in drain_cards.iter().rev() {
                desired.retain(|wanted| *wanted != *card);
                desired.insert(0, card);
            }
            think!(self.journal(), Policies, Detail,
                   "Gold cards first to leave the bankruptcy drain";
                   "bankruptcy-exits-the-drain: {:.0} Gold at {:+.1} a turn; {}",
                   g.players[pid].gold, g.players[pid].gold_per_turn,
                   drain_cards.iter().map(|(card, gold)| format!("{} about {gold:.0}", plain(card)))
                       .collect::<Vec<_>>().join(", "));
        }
        // Use a real policy window before bankruptcy costs the Religion
        // founder its defenders. The held card stays wanted while its removal
        // would restore the deficit; see `religion_seat_cuts_the_upkeep`.
        let religion_upkeep = self.religion_upkeep_card(g, pid);
        if let Some((card, saving)) = religion_upkeep {
            desired.retain(|wanted| *wanted != card);
            desired.insert(0, card);
            think!(self.journal(), Policies, Detail,
                "Upkeep relief funds the Religion army";
                "religion-seat-cuts-the-upkeep: {} saves about {saving:.1} Gold a turn, \
                 {:.0} Gold at {:+.1} a turn", plain(card),
                g.players[pid].gold, g.players[pid].gold_per_turn);
        }
        let desired_set: HashSet<&str> = desired.iter().copied().collect();
        // If circumstances changed, remove a downside-bearing Dark Age card
        // immediately. Isolationism must not coexist with a live Settler.

        let unsafe_dark_cards: Vec<Name> = g.players[pid]
            .policies
            .iter()
            .filter(|card| {
                (g.rules.policies[card].dark_age
                    || (self.victory_planning && card.as_str() == "music_censorship"))
                    && !desired_set.contains(card.as_str())
            })
            .cloned()
            .collect();
        for card in unsafe_dark_cards {
            think!(self.journal(), Policies, Decision, "Dropping the situational card {}", plain(&card);
                   "its downside no longer suits the {} plan", objective.as_str());
            let _ = g.apply(pid, &Action::UnslotPolicy { policy: card });
        }

        // The portfolio itself is a decision, and the order is the whole of
        // the reasoning behind every slot below: an observer who can see the
        // ranking can tell a card that was skipped from one that never made
        // the list.
        if self.journal().wants(crate::reasoning::Level::Detail) {
            let held: Vec<&str> = desired
                .iter()
                .copied()
                .filter(|card| g.players[pid].policies.contains(&Name::new(card)))
                .collect();
            think!(self.journal(), Policies, Detail,
            "Policy portfolio for the {} plan", objective.as_str();
            "wants, in order: {}; already slotted: {}",
            desired.iter().map(|card| plain(card)).collect::<Vec<_>>().join(", "),
            if held.is_empty() {
                "none".to_string()
            } else {
                held.iter().map(|card| plain(card)).collect::<Vec<_>>().join(", ")
            });
        }

        let wanted = desired.len();
        let available: HashSet<Name> = g.available_policies(pid).into_iter().collect();
        // `solvent-or-hold`: each card kept this pass, and the first card it
        // kept out; journalled once after the pass.
        let mut solvency_kept: BTreeMap<Name, &str> = BTreeMap::new();
        for (rank, card) in desired.iter().copied().enumerate() {
            if g.players[pid].policies.contains(&Name::new(card))
                || !available.contains(&Name::new(card))
            {
                continue;
            }
            if g.apply(
                pid,
                &Action::SlotPolicy {
                    policy: Name::new(card),
                },
            )
            .is_ok()
            {
                if let (true, Some(qualifying)) = (card == "medina_quarter", medina_quarter) {
                    think!(self.journal(), Policies, Decision, "Slotted medina quarter";
                           "{qualifying} housing-limited cities with 3 or more specialty \
                            districts gain +2 Housing; into a free slot");
                    continue;
                }
                think!(self.journal(), Policies, Decision, "Slotted {}", plain(card);
                       "priority {} of {wanted} for the {} plan, into a free slot",
                       rank + 1, objective.as_str());
                if drain_cards.iter().any(|(gold_card, _)| *gold_card == card)
                    || religion_upkeep.is_some_and(|(relief, _)| relief == card)
                {
                    self.drain_card_turn.set(Some(g.turn));
                }
                continue;
            }

            let slot = g.rules.policies[card].slot.clone();
            let mut replaceable: Vec<Name> = g.players[pid]
                .policies
                .iter()
                .filter(|current| {
                    if protect_wartime_income.is_some() && current.as_str() == "economic_union" {
                        return false;
                    }
                    // An upkeep insertion must not make a later drain card
                    // evict the Faith card it spared. Native t71 inserted
                    // Conscription, then traded Scripture for Caravansaries.
                    if religion_upkeep.is_some()
                        && Self::religion_upkeep_keeps_card(g, pid, current.as_str())
                    {
                        return false;
                    }
                    if let Some((_, saving)) = religion_upkeep.filter(|(relief, _)| *relief == card) {
                        return !culture_defense_cards.contains(&current.as_str())
                            && !nobel_peace_direct_favor_cards.contains(&current.as_str())
                            && upgrade_card != Some(current.as_str())
                            && builder_window_card != Some(current.as_str())
                            && race_card != Some(current.as_str())
                            && Self::religion_upkeep_may_displace(g, pid, current.as_str(), saving);
                    }
                    // `medina-quarter-houses-the-districts`: the housing card may
                    // take an ordinary Economic card's slot (wanted or not),
                    // never a spared production card or a protected one.
                    if medina_quarter.is_some() && card == "medina_quarter" {
                        let current = current.as_str();
                        return Self::medina_quarter_may_displace(g, current)
                            && !culture_defense_cards.contains(&current)
                            && !nobel_peace_direct_favor_cards.contains(&current)
                            && builder_window_card != Some(current)
                            && upgrade_card != Some(current)
                            && race_card != Some(current);
                    }
                    // `urban-planning-fills-the-slot`: the tail entry takes only
                    // an orphaned timed economy card's slot.
                    if urban_planning_tail && card == "urban_planning" {
                        return URBAN_PLANNING_ORPHANED_TIMED_CARDS.contains(&current.as_str())
                            && !desired_set.contains(current.as_str());
                    }
                    if builder_window_card == Some(card) {
                        // Protection applies even when the current card is
                        // absent from this strategy's ordinary portfolio.
                        return !culture_defense_cards.contains(&current.as_str())
                            && !nobel_peace_direct_favor_cards.contains(&current.as_str())
                            && self.builder_window_can_replace(g, pid, current);
                    }
                    // With `policy-deck-hysteresis-2` the same holds for the
                    // maintenance emergency: a wanted card's rank does not
                    // otherwise let it evict a lower-ranked wanted card, and
                    // live King 2026-10-03T081800Z t206-t208 kept Conscription
                    // out behind Logistics (rank 21) while the treasury drained.
                    if (upgrade_funding_relief
                        || (self.policy_deck_hysteresis_2 && maintenance_emergency))
                        && matches!(card, "conscription" | "levee_en_masse")
                    {
                        // Ordinary desired military cards must not lock out
                        // the cash needed by the upgrade discount itself.
                        return g.rules.policies[current].slot == "military"
                            && !matches!(
                                current.as_str(),
                                "professional_army"
                                    | "force_modernization"
                                    | "limitanei"
                                    | "praetorium"
                            )
                            && !culture_defense_cards.contains(&current.as_str());
                    }
                    if upgrade_card == Some(card) {
                        return g.rules.policies[current].slot == "military"
                            && !matches!(
                                current.as_str(),
                                "conscription" | "levee_en_masse" | "limitanei" | "praetorium"
                            )
                            && !culture_defense_cards.contains(&current.as_str());
                    }
                    // `bankruptcy-exits-the-drain`: a drain gold card takes an
                    // ordinary desired card's slot (100910Z t196-199 held
                    // Merchant Confederation, about 45 Gold, behind conquest
                    // cards); `solvent-or-hold` still guards amenity and upkeep
                    // cards below.
                    if drain_cards.iter().any(|(gold_card, _)| *gold_card == card)
                        && !drain_cards
                            .iter()
                            .any(|(gold_card, _)| *gold_card == current.as_str())
                        && !culture_defense_cards.contains(&current.as_str())
                        && !nobel_peace_direct_favor_cards.contains(&current.as_str())
                        && upgrade_card != Some(current.as_str())
                    {
                        return true;
                    }
                    !desired_set.contains(current.as_str())
                        || self.religion_orders_may_displace(g, pid, &desired, card, current.as_str())
                        || self.domination_multiplier_reclaims_fallback(
                            g, card, current.as_str(), &unproductive_economic_cards,
                        )
                        // A full deck otherwise protects every wanted card,
                        // even when Praetorium leads the emergency portfolio.
                        // Native Russia 20261109T210123Z t132 kept Merchant
                        // Confederation and missed its last free window.
                        // Only this additional permission is same-type: the
                        // ordinary unwanted-card fallback above still works.
                        || (loyalty_policy_needed
                            && card == "praetorium"
                            && g.rules.policies[current].slot == "diplomatic"
                            && current.as_str() != "communications_office"
                            && !culture_defense_cards.contains(&current.as_str())
                            && !nobel_peace_direct_favor_cards.contains(&current.as_str())
                            && !drain_cards
                                .iter()
                                .any(|(gold_card, _)| *gold_card == current.as_str()))
                        // Defensive cards must be able to take an occupied
                        // slot, but never evict each other or a different
                        // typed lane card merely to borrow wildcard capacity.
                        || (culture_defense_cards.contains(&card)
                            && !culture_defense_cards.contains(&current.as_str())
                            && g.rules.policies[current].slot == g.rules.policies[card].slot)
                        // An ordinary desired card normally remains protected
                        // from a policy reassessment. A live Nobel Peace
                        // direct-Favor card is a timed host competition action,
                        // not an ordinary deck wish, so let it take a fitting
                        // wildcard seat now. The normal desired card retakes
                        // that seat after the host withdraws the competition.
                        || (nobel_peace_direct_favor_cards.contains(&card)
                            && !nobel_peace_direct_favor_cards
                                .contains(&current.as_str()))
                        // `merchant-confederation-earns-its-slot-2`: the
                        // paying card takes a diplomatic seat from a wanted
                        // card the portfolio ranks after it, never from a
                        // loyalty Praetorium, a culture-defence or a Nobel
                        // Peace favour card.
                        || (merchant_takes_a_seat
                            && card == "merchant_confederation"
                            && g.rules.policies[current].slot == "diplomatic"
                            && current.as_str() != "praetorium"
                            && !culture_defense_cards.contains(&current.as_str())
                            && !nobel_peace_direct_favor_cards.contains(&current.as_str())
                            && desired
                                .iter()
                                .position(|wanted| *wanted == current.as_str())
                                .is_none_or(|theirs| theirs > rank))
                        // The race's Revelation takes the wildcard seat from
                        // an ordinary desired card, never from a defensive or
                        // emergency one.
                        || (race_card == Some(card)
                            && !culture_defense_cards.contains(&current.as_str())
                            && !nobel_peace_direct_favor_cards.contains(&current.as_str())
                            && upgrade_card != Some(current.as_str())
                            && builder_window_card != Some(current.as_str())
                            && !matches!(current.as_str(), "limitanei" | "praetorium")
                            && !(maintenance_emergency
                                && matches!(current.as_str(), "conscription" | "levee_en_masse")))
                })
                .filter(|current| Self::policy_swap_fits(g, pid, current, card))
                .cloned()
                .collect();
            // `solvent-or-hold`: while income is not positive an amenity or
            // upkeep card gives its slot only to a card of the same kind. See
            // `advanced/solvency.rs`.
            if self.solvency_guard(g, pid) {
                let priced = religion_upkeep
                    .filter(|(relief, _)| *relief == card)
                    .map(|(_, saving)| saving)
                    .or_else(|| protect_wartime_income.filter(|_| card == "economic_union"));
                let kept: Vec<Name> = replaceable
                    .iter()
                    .filter(|current| {
                        self.solvency_keeps_card_priced(g, pid, current.as_str(), card, priced)
                    })
                    .cloned()
                    .collect();
                if !kept.is_empty() {
                    replaceable.retain(|current| !kept.contains(current));
                    for current in kept {
                        solvency_kept.entry(current).or_insert(card);
                    }
                }
            }
            replaceable.sort_by(|first, second| {
                let key = |current: &str| {
                    let policy = &g.rules.policies[current];
                    let era = policy
                        .civic
                        .as_ref()
                        .and_then(|civic| g.rules.civics.get(civic))
                        .map_or(0, |civic| civic.era);
                    (
                        usize::from(policy.slot != slot),
                        usize::from(!unproductive_economic_cards.contains(current)),
                        era,
                    )
                };
                key(first).cmp(&key(second)).then(first.cmp(second))
            });
            if medina_quarter.is_some() && card == "medina_quarter" {
                Self::medina_quarter_weakest_first(g, &desired, &mut replaceable);
            }
            for current in replaceable {
                let _ = g.apply(pid, &Action::UnslotPolicy { policy: current });
                if g.apply(
                    pid,
                    &Action::SlotPolicy {
                        policy: Name::new(card),
                    },
                )
                .is_ok()
                {
                    if drain_cards.iter().any(|(gold_card, _)| *gold_card == card)
                        || religion_upkeep.is_some_and(|(relief, _)| relief == card)
                    {
                        self.drain_card_turn.set(Some(g.turn));
                    }
                    if builder_window_card == Some(card) {
                        think!(self.journal(), Policies, Decision,
                               "Slotted {} over {}", plain(card), plain(&current);
                               "a Builder is almost finished; Serfdom adds two build charges");
                    } else if let (true, Some(qualifying)) =
                        (card == "medina_quarter", medina_quarter)
                    {
                        think!(self.journal(), Policies, Decision,
                               "Slotted medina quarter over {}", plain(&current);
                               "{qualifying} housing-limited cities with 3 or more specialty \
                                districts gain +2 Housing; {} was the Economic card the {} \
                                plan wants least", plain(&current), objective.as_str());
                    } else if self.religion_orders_may_displace(
                        g,
                        pid,
                        &desired,
                        card,
                        current.as_str(),
                    ) {
                        think!(self.journal(), Policies, Decision,
                               "Religious Orders takes the lower-ranked Economic card {}", plain(&current);
                               "religion-seat-slots-the-orders: the founder has an Apostle in the field; \
                                use this policy window for +5 theological strength");
                    } else if loyalty_policy_needed && card == "praetorium" {
                        think!(self.journal(), Policies, Decision,
                               "Slotted praetorium over {}", plain(&current);
                               "a governed city has at most {LOYALTY_POLICY_TRIGGER_TURNS:.0} turns \
                                of measured Loyalty runway; use the occupied diplomatic slot \
                                for +2 Loyalty per turn");
                    } else {
                        think!(self.journal(), Policies, Decision,
                               "Slotted {} over {}", plain(card), plain(&current);
                               "priority {} of {wanted} for the {} plan; {} was the oldest \
                                card the plan does not want",
                               rank + 1, objective.as_str(), plain(&current));
                    }
                    break;
                }
                // A type mismatch can make one particular swap invalid. Put
                // the old card back before trying another candidate so policy
                // reassessment can never silently empty a government.
                let _ = g.apply(pid, &Action::SlotPolicy { policy: current });
            }
        }
        for (current, card) in solvency_kept {
            think!(self.journal(), Policies, Decision,
                   "Keeping {} while income is not positive", plain(&current);
                   "solvent-or-hold: {:+.1} Gold a turn; {} would have taken its slot",
                   g.players[pid].gold_per_turn, plain(card));
        }
    }

    /// The governor order for this turn, after the one board condition the
    /// static table cannot see. See `moksha_defends_the_faithless`.
    pub(super) fn governor_priority_for(
        &self,
        g: &Game,
        pid: usize,
        strategy: GrandStrategy,
    ) -> Vec<&'static str> {
        let base = Self::governor_priority(strategy);
        if !self.moksha_defends_the_faithless
            // An empire with its own faith defends it with Missionaries,
            // Apostles and Inquisitors, all of which it can actually buy.
            // This is for the one that cannot.
            || g.players[pid].religion.is_some()
            || self.home_conversion_threat(g, pid).is_none()
        {
            return base.to_vec();
        }
        let mut order = Vec::with_capacity(base.len());
        let first = if self.domination_needs_economic_governor(g, pid) {
            "pingala"
        } else {
            "moksha"
        };
        order.push(first);
        order.extend(base.iter().copied().filter(|name| *name != first));
        order
    }

    pub(super) fn governor_priority(strategy: GrandStrategy) -> &'static [&'static str] {
        match strategy {
            GrandStrategy::Expansion => &[
                "magnus", "pingala", "liang", "reyna", "victor", "moksha", "amani",
            ],
            GrandStrategy::Science => &[
                "pingala", "magnus", "reyna", "liang", "victor", "moksha", "amani",
            ],
            GrandStrategy::Culture => &[
                "pingala", "reyna", "liang", "magnus", "moksha", "victor", "amani",
            ],
            GrandStrategy::Religion => &[
                "moksha", "pingala", "magnus", "amani", "liang", "victor", "reyna",
            ],
            GrandStrategy::Diplomacy => &[
                "amani", "pingala", "reyna", "magnus", "liang", "victor", "moksha",
            ],
            GrandStrategy::Conquest => &[
                "victor", "magnus", "pingala", "liang", "reyna", "moksha", "amani",
            ],
            GrandStrategy::Recovery => &[
                "victor", "reyna", "magnus", "pingala", "liang", "moksha", "amani",
            ],
        }
    }

    fn governor_promotion_priority(
        strategy: GrandStrategy,
        governor: &str,
    ) -> &'static [&'static str] {
        match governor {
            "pingala" if strategy == GrandStrategy::Culture => &[
                "connoisseur",
                "researcher",
                "grants",
                "curator",
                "space_initiative",
            ],
            "pingala" => &[
                "researcher",
                "connoisseur",
                "grants",
                "space_initiative",
                "curator",
            ],
            "magnus" => &[
                "provision",
                "surplus_logistics",
                "black_marketeer",
                "industrialist",
                "vertical_integration",
            ],
            "liang" => &[
                "zoning_commissioner",
                "aquaculture",
                "reinforced_materials",
                "water_works",
                "parks_and_recreation",
            ],
            "reyna" => &[
                "harbormaster",
                "forestry_management",
                "tax_collector",
                "contractor",
                "renewable_subsidizer",
            ],
            "victor" => &[
                "garrison_commander",
                "defense_logistics",
                "embrasure",
                "air_defense_initiative",
                "arms_race_proponent",
            ],
            "moksha" => &[
                "grand_inquisitor",
                "laying_on_of_hands",
                "citadel_of_god",
                "patron_saint",
                "divine_architect",
            ],
            "amani" => &[
                "emissary",
                "affluence",
                "local_informants",
                "foreign_investor",
                "puppeteer",
            ],
            _ => &[],
        }
    }

    fn best_governor_city(
        &self,
        g: &Game,
        pid: usize,
        governor: &str,
        plan: &StrategicPlan,
    ) -> Option<u32> {
        let occupied: BTreeSet<u32> = g.players[pid]
            .governor_roster
            .values()
            .filter_map(|state| state.city)
            .collect();
        let mut candidates: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|city| !occupied.contains(city))
            .collect();
        if governor == "amani" {
            candidates.extend(
                g.players
                    .iter()
                    .filter(|player| {
                        player.alive
                            && player.is_minor
                            && !player.is_barbarian
                            && !g.is_at_war(pid, player.id)
                    })
                    .flat_map(|player| g.player_city_ids(player.id))
                    .filter(|city| !occupied.contains(city)),
            );
        }
        // `max_by` re-evaluates both sides of every comparison, so each city's
        // yields would otherwise be recomputed a logarithmic number of times.
        let _memo = g.query_memo();
        candidates.into_iter().max_by(|left, right| {
            let value = |city_id: u32| {
                let city = &g.cities[&city_id];
                let yields = g.city_yields(city_id);
                let own = city.owner == pid;
                let commercial = city.districts.keys().any(|district| {
                    matches!(
                        g.district_family(*district).as_str(),
                        "commercial_hub" | "harbor"
                    )
                }) as i32 as f64;
                let holy = city
                    .districts
                    .keys()
                    .any(|district| g.district_family(*district) == "holy_site")
                    as i32 as f64;
                let base = if own {
                    (100.0 - city.loyalty).max(0.0) * 2.0
                } else {
                    0.0
                };
                base + self.conversion_hold_governor_bonus(g, pid, city_id, governor)
                    + match governor {
                        "pingala" => {
                            city.pop as f64 * 14.0
                                + yields.science * 9.0
                                + yields.culture * 9.0
                                + self.science_drive_governor_bonus(g, governor, city_id)
                        }
                        "magnus" => {
                            city.pop as f64 * 5.0
                                + yields.food * 5.0
                                + yields.production * 11.0
                                + matches!(
                                    city.queue.first(),
                                    Some(Item::Unit { unit }) if unit == "settler"
                                ) as i32 as f64
                                    * 180.0
                        }
                        "liang" => yields.production * 10.0 + city.owned_tiles.len() as f64 * 2.0,
                        "reyna" => city.pop as f64 * 8.0 + yields.gold * 13.0 + commercial * 150.0,
                        "victor" => {
                            plan.threatened_city.is_some_and(|target| target == city_id) as i32
                                as f64
                                * 600.0
                                + city.wall_hp.max(0) as f64
                                + city.pop as f64 * 5.0
                        }
                        "moksha" => {
                            yields.faith * 14.0
                                + holy * 180.0
                                + (g.players[pid].holy_city == Some(city_id)) as i32 as f64 * 220.0
                        }
                        "amani" if !own => 600.0 + g.envoys_at(pid, city.owner) as f64 * 55.0,
                        "amani" => (100.0 - city.loyalty).max(0.0) * 5.0,
                        _ => 0.0,
                    }
            };
            value(*left)
                .partial_cmp(&value(*right))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| right.cmp(left))
        })
    }

    pub(super) fn preferred_governor_promotion(
        &self,
        g: &Game,
        pid: usize,
        strategy: GrandStrategy,
        governor: &str,
    ) -> Option<String> {
        let available = g.available_governor_promotions(pid, governor);
        // `curator-sits-with-the-works`: a Culture seat's Pingala keeps the
        // Culture order whatever the plan, never takes Space Initiative, and
        // takes Curator once the works stand. `None` with the gene off.
        self.curator_promotion_order(g, governor)
            .unwrap_or_else(|| Self::governor_promotion_priority(strategy, governor))
            .iter()
            .filter(|promotion| !self.curator_withholds_promotion(g, pid, governor, promotion))
            .find(|promotion| available.iter().any(|candidate| candidate == **promotion))
            .map(|promotion| (*promotion).to_string())
    }

    pub(super) fn strategic_governors(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        while g.governor_titles_available(pid) > 0 {
            // `curator-sits-with-the-works`: Curator first once the works
            // stand. Exact no-op with the gene off.
            if self.promote_the_curator(g, pid) {
                continue;
            }
            let priority = self.governor_priority_for(g, pid, plan.strategy);
            let priority = priority.as_slice();
            // Strategy can change every assessment window, but Governor
            // Titles arrive much more slowly. Finish the earliest incumbent's
            // two-promotion foundation before adapting the roster, otherwise
            // transient wars or victory races recreate the old dilution bug.
            let primary_name = g.players[pid]
                .governor_roster
                .iter()
                .filter(|(_, state)| state.promotions.len() < 2)
                .min_by_key(|(name, state)| (state.assigned_turn, name.as_str()))
                .map(|(name, _)| *name)
                .unwrap_or_else(|| Name::new(priority[0]));
            let primary = primary_name.as_str();
            if !g.players[pid].governor_roster.contains_key(primary) {
                if let Some(city) = self.best_governor_city(g, pid, primary, plan) {
                    if g.apply(
                        pid,
                        &Action::AppointGovernor {
                            governor: Name::new(primary),
                            city,
                        },
                    )
                    .is_ok()
                    {
                        continue;
                    }
                }
            }

            let primary_promotions = g.players[pid]
                .governor_roster
                .get(primary)
                .map(|state| state.promotions.len())
                .unwrap_or(0);
            if primary_promotions < 2 {
                if let Some(promotion) =
                    self.preferred_governor_promotion(g, pid, plan.strategy, primary)
                {
                    if g.apply(
                        pid,
                        &Action::PromoteGovernor {
                            governor: Name::new(primary),
                            promotion: Name::new(&promotion),
                        },
                    )
                    .is_ok()
                    {
                        continue;
                    }
                }
            }

            // After both tier-one promotions are online, establish one
            // complementary governor before completing the primary's tree.
            if g.players[pid].governor_roster.len() < 2 {
                if let Some((governor, city)) = priority.iter().skip(1).find_map(|governor| {
                    (!g.players[pid].governor_roster.contains_key(governor))
                        .then(|| {
                            self.best_governor_city(g, pid, governor, plan)
                                .map(|city| ((*governor).to_string(), city))
                        })
                        .flatten()
                }) {
                    if g.apply(
                        pid,
                        &Action::AppointGovernor {
                            governor: Name::new(&governor),
                            city,
                        },
                    )
                    .is_ok()
                    {
                        continue;
                    }
                }
            }

            if let Some(promotion) =
                self.preferred_governor_promotion(g, pid, plan.strategy, primary)
            {
                if g.apply(
                    pid,
                    &Action::PromoteGovernor {
                        governor: Name::new(primary),
                        promotion: Name::new(&promotion),
                    },
                )
                .is_ok()
                {
                    continue;
                }
            }

            // Add a third regional anchor before investing deeply in the
            // complementary governor. Further titles finish existing trees;
            // only then does the roster widen again.
            if g.players[pid].governor_roster.len() < 3 {
                if let Some((governor, city)) = priority.iter().skip(1).find_map(|governor| {
                    (!g.players[pid].governor_roster.contains_key(governor))
                        .then(|| {
                            self.best_governor_city(g, pid, governor, plan)
                                .map(|city| ((*governor).to_string(), city))
                        })
                        .flatten()
                }) {
                    if g.apply(
                        pid,
                        &Action::AppointGovernor {
                            governor: Name::new(&governor),
                            city,
                        },
                    )
                    .is_ok()
                    {
                        continue;
                    }
                }
            }

            let next_promotion = priority.iter().find_map(|governor| {
                self.preferred_governor_promotion(g, pid, plan.strategy, governor)
                    .map(|promotion| ((*governor).to_string(), promotion))
            });
            if let Some((governor, promotion)) = next_promotion {
                if g.apply(
                    pid,
                    &Action::PromoteGovernor {
                        governor: Name::new(&governor),
                        promotion: Name::new(&promotion),
                    },
                )
                .is_ok()
                {
                    continue;
                }
            }

            let appointment = priority.iter().find_map(|governor| {
                (!g.players[pid].governor_roster.contains_key(governor))
                    .then(|| {
                        self.best_governor_city(g, pid, governor, plan)
                            .map(|city| ((*governor).to_string(), city))
                    })
                    .flatten()
            });
            let Some((governor, city)) = appointment else {
                break;
            };
            let where_ = g
                .cities
                .get(&city)
                .map(|city| city.name.clone())
                .unwrap_or_else(|| format!("city {city}"));
            if g.apply(
                pid,
                &Action::AppointGovernor {
                    governor: Name::new(&governor),
                    city,
                },
            )
            .is_err()
            {
                break;
            }
            think!(self.journal(), Government, Decision, "Posting {} to {where_}", plain(&governor);
                   "the city the {} plan gets most from", plan.strategy.as_str());
        }

        // Firaxis appoints a Governor and assigns its city as two asynchronous
        // player operations. The control mod submits the assignment from the
        // appointment callback, but the completed Rome run proved that the
        // request can report success without sticking: Moksha stayed idle from
        // t22 through t243 and Victor from t83 through t229. On the next turn
        // the authoritative mirror exposes `city: None`; retry it even though
        // there is no new title to spend. Ordinary CIVVIS games never need this
        // because `AppointGovernor` installs the city atomically.
        if self.live_governor_assignment_adapter {
            // A Governor an enemy spy has neutralized comes back unseated and
            // cannot take a post until it recovers, so the retry waits for it:
            // each refused request is a strike, and three strike the order out
            // for ten turns (`ORDER_REFUSAL_COOLDOWN_TURNS`). Live King
            // civvis-20261005T141932Z (game 135) sent Victor's post three times
            // in its neutralized turn 142 and not again until 152, seven turns
            // after he recovered; in game 136 Hamilton flipped at turn 145
            // with Victor recovered and unposted since 141.
            let unassigned = g.players[pid]
                .governor_roster
                .iter()
                .filter(|(_, state)| state.city.is_none() && state.disabled_until <= g.turn)
                .map(|(governor, _)| governor.to_string())
                .collect::<Vec<_>>();
            for governor in unassigned {
                let Some(city) = self.best_governor_city(g, pid, &governor, plan) else {
                    continue;
                };
                if g.apply(
                    pid,
                    &Action::ReassignGovernor {
                        governor: Name::new(&governor),
                        city,
                    },
                )
                .is_ok()
                {
                    let where_ = g
                        .cities
                        .get(&city)
                        .map(|city| city.name.clone())
                        .unwrap_or_else(|| format!("city {city}"));
                    think!(self.journal(), Government, Decision,
                           "Retrying {}'s post to {where_}", plain(&governor);
                           "Firaxis still reports the appointed Governor as unassigned");
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "loyalty_policy_tests.rs"]
mod loyalty_policy_tests;
