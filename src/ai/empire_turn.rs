//! The empire turn of the baseline controller: what `BasicAi` decides for
//! the whole civilization each turn.
//!
//! `research_with_government` runs it. It covers research and civics, the
//! government and the policy deck review, the secret society, the pantheon
//! (`pantheon_board_value`, `pantheon_effect_reach`) and religion founding,
//! claiming earned Great People (`claim_free_great_people`), and governors,
//! with the loyalty backstop (`loyalty_emergency`,
//! `reassign_governor_for_loyalty`).
//!
//! Moved verbatim out of the main `impl BasicAi` block in `ai.rs`
//! (2026-11-05). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged.

use super::*;

impl BasicAi {
    /// What a pantheon would pay this empire per turn, on the land it already
    /// owns.
    ///
    /// ⚠ Owned tiles rather than built improvements. A pantheon is founded on
    /// about turn twenty with one city and almost nothing improved, so counting
    /// what is *built* would score every candidate at zero and value nothing.
    /// What the choice is actually a read of is the land: how many Deer and
    /// Furs are in the borders, how many Quarry resources, how much Strategic.
    ///
    /// ⚠ Only the per-tile effects are priced. `district_gpp`, `growth_pct`,
    /// `free_builder` and their kind are worth a great deal and are worth the
    /// same everywhere, so pricing them would be inventing weights rather than
    /// reading a board; the shipped order already ranks them against each
    /// other and continues to.
    /// The founder beliefs a founding tries, in order. A Religion-target seat
    /// asks for the Faith-scaling ones first
    /// (`religion-seat-takes-the-faith-beliefs`; see
    /// `advanced/religion_seat_fields_apostles.rs`). Every other founder
    /// belief follows in the catalogue's order, so a founding never fails for
    /// want of a listed one.
    pub(super) fn founder_belief_order(&self, g: &Game) -> Vec<String> {
        let listed: [&str; 5] = if self.founder_takes_the_faith_beliefs {
            [
                "pilgrimage",
                "lay_ministry",
                "tithe",
                "world_church",
                "cross_cultural_dialogue",
            ]
        } else {
            [
                "tithe",
                "world_church",
                "cross_cultural_dialogue",
                "pilgrimage",
                "religious_unity",
            ]
        };
        let mut founders: Vec<String> = listed
            .into_iter()
            .filter(|belief| g.rules.beliefs.founder.contains_key(*belief))
            .map(str::to_string)
            .collect();
        for belief in g.rules.beliefs.founder.keys() {
            if !founders.contains(belief) {
                founders.push(belief.clone());
            }
        }
        founders
    }

    fn pantheon_board_value(&self, g: &Game, pid: usize, belief: &str) -> f64 {
        let Some(spec) = g.rules.beliefs.pantheon.get(belief) else {
            return 0.0;
        };
        spec.effects
            .iter()
            .map(|(effect, amount)| amount * self.pantheon_effect_reach(g, pid, effect))
            .sum()
    }

    /// How many tiles of this empire a per-tile pantheon effect would be paid
    /// on. Zero for an effect that is not paid per tile.
    fn pantheon_effect_reach(&self, g: &Game, pid: usize, effect: &str) -> f64 {
        let worked_by = |improvement: &str, classes: &[&str]| -> f64 {
            g.player_city_ids(pid)
                .into_iter()
                .flat_map(|cid| g.cities[&cid].owned_tiles.clone())
                .filter(|position| {
                    g.map.get(*position).is_some_and(|tile| {
                        !tile.pillaged
                            && tile.resource.as_deref().is_some_and(|resource| {
                                g.rules.resources.get(resource).is_some_and(|spec| {
                                    spec.improvement == improvement
                                        && (classes.is_empty()
                                            || classes.contains(&spec.class.as_str()))
                                })
                            })
                    })
                })
                .count() as f64
        };
        let any_improved = |classes: &[&str]| -> f64 {
            g.player_city_ids(pid)
                .into_iter()
                .flat_map(|cid| g.cities[&cid].owned_tiles.clone())
                .filter(|position| {
                    g.map.get(*position).is_some_and(|tile| {
                        !tile.pillaged
                            && tile.resource.as_deref().is_some_and(|resource| {
                                g.rules.resources.get(resource).is_some_and(|spec| {
                                    !spec.improvement.is_empty()
                                        && classes.contains(&spec.class.as_str())
                                })
                            })
                    })
                })
                .count() as f64
        };
        // The plot pantheons ask the ground itself rather than a resource:
        // Lady of the Reeds and Marshes, Goddess of Fire and Earth Goddess are
        // one `MODIFIER_CITY_PLOT_YIELDS_ADJUST_PLOT_YIELD` each over a plot
        // test, so one closure answers all three.
        let owned_plots = |test: &dyn Fn(&crate::world::Tile, Pos) -> bool| -> f64 {
            g.player_city_ids(pid)
                .into_iter()
                .flat_map(|cid| g.cities[&cid].owned_tiles.clone())
                .filter(|position| {
                    g.map
                        .get(*position)
                        .is_some_and(|tile| !tile.pillaged && test(tile, *position))
                })
                .count() as f64
        };
        let feature_is = |names: &'static [&'static str]| {
            move |tile: &crate::world::Tile, _: Pos| {
                tile.feature
                    .as_deref()
                    .is_some_and(|feature| names.contains(&feature))
            }
        };
        match effect {
            "camp_food" | "camp_production" | "camp_gold" => worked_by("camp", &[]),
            "quarry_faith" | "quarry_production" => worked_by("quarry", &[]),
            "pasture_culture" | "pasture_food" | "pasture_production" => worked_by("pasture", &[]),
            "plantation_culture" | "plantation_food" | "plantation_gold" => {
                worked_by("plantation", &[])
            }
            "fishing_boats_production" | "fishing_boats_food" | "fishing_boats_gold" => {
                worked_by("fishing_boats", &[])
            }
            "resource_mine_faith" => worked_by("mine", &["bonus", "luxury"]),
            "strategic_improved_production" | "strategic_improved_faith" => {
                any_improved(&["strategic"])
            }
            "reeds_production" => owned_plots(&feature_is(&["marsh", "oasis", "floodplains"])),
            "volcanic_geothermal_faith" => {
                owned_plots(&feature_is(&["geothermal_fissure", "volcanic_soil"]))
            }
            "breathtaking_appeal_faith" => owned_plots(&|_, position| g.tile_appeal(position) >= 4),
            // ⚠ The three Holy Site adjacency pantheons — Desert Folklore,
            // Dance of the Aurora, Sacred Path — are deliberately NOT priced
            // here, and the reason is the shape of this function rather than
            // laziness. Everything above is paid on every qualifying tile the
            // empire owns, so counting them IS the payout. An adjacency is paid
            // on at most the six plots around one district, so multiplying the
            // amount by "desert tiles owned" would price Desert Folklore at ten
            // times what it can ever pay on a desert start, which is exactly
            // the invented weight the doc comment above refuses. Reading them
            // honestly means ranking candidate Holy Site plots, which is the
            // district calculator's job and not this one's.
            _ => 0.0,
        }
    }

    pub(super) fn research_with_government(
        &self,
        g: &mut Game,
        pid: usize,
        choose_government: bool,
        pool: Option<&WorkPool>,
    ) {
        if g.players[pid].research.is_none() {
            let avail = Self::era_window_techs(g, pid);
            if !avail.is_empty() {
                let barbarian_goal = if self.barbarian_tactics {
                    Self::barbarian_military_research_goal(g, pid)
                } else {
                    None
                };
                let great_person_goal = Self::live_great_person_tech_goal(g, pid);
                let great_person_pick =
                    Self::research_step_toward(g, &avail, great_person_goal.as_deref());
                let water_pick =
                    Self::research_step_toward(g, &avail, Self::water_research_goal(g, pid));
                let economic_pick =
                    Self::research_step_toward(g, &avail, Self::economic_research_goal(g, pid));
                let pick = Self::research_step_toward(g, &avail, barbarian_goal.as_deref())
                    .or(great_person_pick)
                    .or(water_pick)
                    .or(economic_pick)
                    .or_else(|| {
                        TECH_PRIORITY
                            .iter()
                            .find(|t| avail.iter().any(|a| a == *t))
                            .map(|t| Name::new(t))
                    })
                    .unwrap_or_else(|| avail[0]);
                let _ = g.apply(
                    pid,
                    &Action::Research {
                        tech: Name::new(&pick),
                    },
                );
            }
        }
        if g.players[pid].civic.is_none() {
            let avail = g.available_civics(pid);
            if !avail.is_empty() {
                let great_person_goal = Self::live_great_person_civic_goal(g, pid);
                let pick = Self::civic_step_toward(g, &avail, great_person_goal.as_deref())
                    .or_else(|| {
                        CIVIC_PRIORITY
                            .iter()
                            .find(|c| avail.iter().any(|a| a == *c))
                            .map(|c| Name::new(c))
                    })
                    .unwrap_or_else(|| avail[0]);
                let _ = g.apply(
                    pid,
                    &Action::Civic {
                        civic: Name::new(&pick),
                    },
                );
            }
        }
        // Great People are not awarded automatically when the points cross
        // the threshold: recruitment is a legal player action. Patronage had
        // a strategic buyer, but the free action had no AI consumer at all,
        // so a winning Scientist or Prophet could remain on the board while
        // every headless civilization waited forever. Claim every earned and
        // currently activatable person before founding a Religion or valuing
        // paid patronage later in the turn.
        Self::claim_free_great_people(g, pid);
        if choose_government {
            for gname in GOV_PRIORITY {
                if let Some(spec) = g.rules.governments.get(gname) {
                    let ok = spec
                        .civic
                        .as_ref()
                        .map(|c| g.players[pid].civics.contains(c))
                        .unwrap_or(true);
                    if ok {
                        if g.players[pid].government.as_deref() != Some(gname) {
                            let _ = g.apply(
                                pid,
                                &Action::Government {
                                    government: Name::new(gname),
                                },
                            );
                        }
                        break;
                    }
                }
            }
        }
        // `deck-prices-growth`: the same weights with the growth surplus
        // priced at the Food weight.
        let priced;
        let deck_weights = if self.deck_prices_growth {
            let mut w = self.w.clone();
            w.pol_growth = w.pol_food * DECK_GROWTH_FOOD_SHARE;
            priced = w;
            &priced
        } else {
            &self.w
        };
        // `deck-follows-the-queue`: a review off the cadence on the turn the
        // queue cards worth slotting change. False with the gene off.
        let queue_changed = self.deck_queue_review_due(g, pid);
        revise_policy_deck(
            g,
            pid,
            deck_weights,
            pool,
            self.maintenance_aware_deck,
            self.deck_reads_player_income,
            queue_changed,
        );
        // The engine's own gate, restated here rather than guessed at. This
        // asked once per player per turn for the whole game and was refused
        // every time in any game without the Secret Societies mode — 894
        // refused orders in one 150-turn six-player game, second only to
        // `Fortify`. The Advanced controller's own
        // `advanced_secret_society` already carried these three conditions;
        // the Basic path it composes over did not.
        if g.game_mode("secret_societies")
            && g.players[pid].secret_society.is_none()
            && g.players[pid]
                .civics
                .contains(&crate::name!("code_of_laws"))
        {
            let society = if self.pursue_religion {
                "voidsingers"
            } else {
                "owls_of_minerva"
            };
            let _ = g.apply(
                pid,
                &Action::ChooseSecretSociety {
                    society: Name::new(society),
                },
            );
        }
        // ⚠⚠⚠ THE THIRD SPELLING OF THE SAME RULE, and the one that actually
        // decides. #1044 fixed the legality gate and the affordability check in
        // `game.rs` and missed this, so on Online — where the real price is 12.5 —
        // the AI sat waiting for 25 faith while Civilization VI had already raised
        // `ENDTURN_BLOCKING_PANTHEON`, and the mod's fallback picked instead.
        // Measured live on `civvis-20260803T231038Z`: faith passed 12.5 on turn 19,
        // the host asked on turn 25, and replaying turns 21 and 23 emitted 7 and 8
        // orders with NO pantheon among them.
        if !self.minor
            && g.players[pid].pantheon.is_none()
            && g.players[pid].faith >= g.pantheon_faith_cost()
        {
            // ⚠⚠ THIS LIST USED TO BE THE WHOLE ROSTER, AND THAT IS WHY IT IS
            // NO LONGER JUST A LIST. It named exactly the six pantheons
            // `beliefs.json` happened to carry, so a pantheon added to the data
            // was unreachable: the loop only ever tried these six and stopped at
            // the first that took. `AGENTS.md` calls this out by name —
            // "discover, never list" — and the follower and founder rosters
            // twenty lines below already do it properly. Pantheons were the one
            // class that did not.
            //
            // The named order is kept as a preference prefix, so nothing changes
            // while only those six exist or while one of them is still free.
            // What changes is the case that used to have no answer at all: every
            // named pantheon taken, and the empire founding none.
            // ★★★★ THE PANTHEON THAT FOUNDS A CITY LEADS THE PREFIX ON THE
            // LIVE SEAT. See `expansion_pantheon`: Religious Settlements is a
            // free Settler in the capital and Fertility Rites a free Builder,
            // and Divine Spark — the shipped first choice, taken in 40 of 40
            // recorded live runs — pays nothing until a district stands.
            // `prophet-race-takes-divine-spark`: while the seat runs the
            // Prophet race, Divine Spark's +1 Prophet point a turn on every
            // Holy Site is worth more than Fertility Rites' Builder: the race
            // closes at t31-44 and live G202334Z lost it 18 points to 30.
            let prefix: &[&str] = if self.expansion_pantheon && self.pantheon_for_the_prophet_race {
                &[
                    "religious_settlements",
                    "divine_spark",
                    "fertility_rites",
                    "god_of_the_forge",
                    "god_of_the_open_sky",
                    "god_of_the_sea",
                ]
            } else if self.expansion_pantheon {
                &[
                    "religious_settlements",
                    "fertility_rites",
                    "divine_spark",
                    "god_of_the_forge",
                    "god_of_the_open_sky",
                    "god_of_the_sea",
                ]
            } else {
                &[
                    "divine_spark",
                    "fertility_rites",
                    "god_of_the_forge",
                    "religious_settlements",
                    "god_of_the_open_sky",
                    "god_of_the_sea",
                ]
            };
            let mut pantheons: Vec<String> = prefix
                .iter()
                .copied()
                .filter(|belief| g.rules.beliefs.pantheon.contains_key(*belief))
                .map(str::to_string)
                .collect();
            for belief in g.rules.beliefs.pantheon.keys() {
                if !pantheons.contains(belief) {
                    pantheons.push(belief.clone());
                }
            }
            // The shipped order is a prior, not an answer: it still decides
            // between candidates the land is silent about, so an empire with
            // nothing to work chooses exactly what it chose before. See
            // `pantheon_reads_the_board`.
            let mut ranked: Vec<(usize, String, f64)> = pantheons
                .iter()
                .enumerate()
                .map(|(rank, belief)| {
                    let prior = (pantheons.len() - rank) as f64 * PANTHEON_PRIOR_STEP;
                    let board = if self.pantheon_reads_the_board {
                        self.pantheon_board_value(g, pid, belief)
                    } else {
                        0.0
                    };
                    (rank, belief.clone(), prior + board)
                })
                .collect();
            // Descending score, and the shipped order breaks every tie, so the
            // result is a total order and does not depend on map iteration.
            ranked.sort_by(|left, right| {
                right
                    .2
                    .partial_cmp(&left.2)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(left.0.cmp(&right.0))
            });
            for (rank, b, score) in &ranked {
                if g.apply(
                    pid,
                    &Action::ChoosePantheon {
                        belief: Name::new(b),
                    },
                )
                .is_ok()
                {
                    think!(self.journal, Faith, Decision, "Founding the pantheon {}", plain(b);
                           "worth {score:.1} on this land, and the {} choice on the standing list",
                           match rank { 0 => "first".to_string(), _ => format!("{}th", rank + 1) });
                    break;
                }
            }
        }
        // The Prophet has already been earned. Use that opportunity even when
        // the victory lane does not invest in a religious race; otherwise a
        // domination seat can hold its Prophet unused through a rival's win.
        // Game::apply still enforces the founding prerequisites and free beliefs.
        if g.players[pid].prophet_pending {
            let mut followers: Vec<String> = [
                "work_ethic",
                "choral_music",
                "feed_the_world",
                "jesuit_education",
                "religious_community",
                "zen_meditation",
            ]
            .into_iter()
            .filter(|belief| g.rules.beliefs.follower.contains_key(*belief))
            .map(str::to_string)
            .collect();
            for belief in g.rules.beliefs.follower.keys() {
                if !followers.contains(belief) {
                    followers.push(belief.clone());
                }
            }
            let founders = self.founder_belief_order(g);
            'found: for follower in followers {
                for founder in &founders {
                    if g.apply(
                        pid,
                        &Action::FoundReligion {
                            follower: Name::new(&follower),
                            founder: Name::new(founder),
                        },
                    )
                    .is_ok()
                    {
                        think!(self.journal, Faith, Decision, "Founding a religion";
                               "on the {} follower belief and the {} founder \
                                belief, the first pair still unclaimed",
                               plain(&follower), plain(founder));
                        break 'found;
                    }
                }
            }
        }
        while g.governor_titles_available(pid) > 0 {
            // anchor the shakiest city
            let target = g
                .player_city_ids(pid)
                .into_iter()
                .filter(|c| !g.players[pid].governors.contains(c))
                .min_by(|a, b| {
                    g.cities[a]
                        .loyalty
                        .partial_cmp(&g.cities[b].loyalty)
                        .unwrap()
                        .then(a.cmp(b))
                });
            if let Some(c) = target {
                let governor = [
                    "pingala", "magnus", "liang", "reyna", "victor", "moksha", "amani",
                ]
                .into_iter()
                .find(|governor| !g.players[pid].governor_roster.contains_key(governor));
                if let Some(governor) = governor {
                    if g.apply(
                        pid,
                        &Action::AppointGovernor {
                            governor: Name::new(governor),
                            city: c,
                        },
                    )
                    .is_err()
                    {
                        break;
                    }
                    continue;
                }
                // A modded ruleset can replace every stock Governor name.
                // Keep the generic assignment tool as the data-driven
                // fallback instead of leaving all of those titles idle.
                if g.apply(pid, &Action::AssignGovernor { city: c }).is_ok() {
                    continue;
                }
            }
            let promotion = [
                "pingala", "magnus", "liang", "reyna", "victor", "moksha", "amani",
            ]
            .into_iter()
            .find_map(|governor| {
                g.available_governor_promotions(pid, governor)
                    .into_iter()
                    .next()
                    .map(|promotion| (governor.to_string(), promotion))
            });
            let Some((governor, promotion)) = promotion else {
                break;
            };
            if g.apply(
                pid,
                &Action::PromoteGovernor {
                    governor: Name::new(&governor),
                    promotion: Name::new(&promotion),
                },
            )
            .is_err()
            {
                break;
            }
        }
        // Appointment is not the end of governor play. If an ungoverned city
        // is already close to revolt, move an idle Governor there, or pull one
        // from a completely loyal city. AdvancedAI runs its plan-aware
        // governor pass first, so this is an emergency backstop rather than a
        // second strategy fighting the first one.
        self.reassign_governor_for_loyalty(g, pid);
        while !self.bank_envoys && g.players[pid].envoys_free > 0 {
            // consolidate on the city-state we already lead in (suzerain push)
            let target = g
                .players
                .iter()
                .filter(|m| g.can_send_envoy(pid, m.id))
                .max_by_key(|m| (g.envoys_at(pid, m.id), std::cmp::Reverse(m.id)))
                .map(|m| m.id);
            match target {
                Some(t) => {
                    if g.apply(pid, &Action::SendEnvoy { player: t }).is_err() {
                        break;
                    }
                }
                None => break,
            }
        }
    }

    /// Take every no-cost Great Person claim the empire has earned.
    ///
    /// The action list is authoritative about activation requirements: a
    /// Scientist still needs a Campus, a Writer needs enough work slots, and a
    /// Prophet needs an active Holy Site or Stonehenge. Applying the actions
    /// rather than duplicating those conditions keeps the AI on the same tool
    /// protocol as a human or learned policy.
    pub(super) fn claim_free_great_people(g: &mut Game, pid: usize) -> usize {
        let threshold_reached = g.players[pid]
            .gpp
            .iter()
            .any(|(kind, points)| *points + f64::EPSILON >= g.gp_cost(pid, kind));
        if !threshold_reached {
            return 0;
        }
        let mut recruits: Vec<Action> = g
            .legal_actions_within(pid, ActionFamilies::EMPIRE)
            .into_iter()
            .filter(|action| matches!(action, Action::RecruitGreatPerson { .. }))
            .collect();
        // Determinism matters for same-seed evaluation. The rules map order is
        // stable today, but make the intended order explicit at this boundary.
        recruits.sort_by(|left, right| format!("{left:?}").cmp(&format!("{right:?}")));
        recruits
            .into_iter()
            .filter(|action| g.apply(pid, action).is_ok())
            .count()
    }

    /// Relocate one Governor to a city in immediate Loyalty danger.
    /// How many turns until this city flips, or `None` if it is not in trouble.
    ///
    /// Smaller is worse. A city already under `LOYALTY_LEVEL_ALARM` counts as an
    /// emergency whatever its rate, because the level threshold is what the
    /// governor logic used before and a city down there is in trouble even while
    /// recovering — it just is not the MOST urgent one if something else is
    /// visibly bleeding out faster.
    ///
    /// Returns `None` when the flag is off, so a frozen controller keeps the old
    /// level-only behaviour exactly.
    pub(crate) fn loyalty_emergency(&self, g: &Game, cid: u32) -> Option<f64> {
        let city = g.cities.get(&cid)?;
        if !self.loyalty_rate_alarm {
            // Frozen behaviour: the level threshold alone, exactly as before.
            return (city.loyalty < LOYALTY_LEVEL_ALARM).then_some(city.loyalty);
        }
        let rate = g.city_loyalty_per_turn(city);
        if rate < -f64::EPSILON {
            // Turns of headroom left at the current rate.
            return Some((city.loyalty / -rate).max(0.0));
        }
        // Stable or recovering: only the old level threshold still flags it, and
        // it ranks behind anything actually falling.
        (city.loyalty < LOYALTY_LEVEL_ALARM).then_some(f64::MAX / 2.0)
    }

    /// ★★★★★ LOYALTY LEVEL IS A LAGGING INDICATOR AND IT IS ALL THIS AI EVER READ.
    ///
    /// Every city loss across every recorded run on this machine, classified by
    /// the city's last sighting before it disappeared — **125 losses**:
    ///
    /// ```text
    ///   52  41.6%  loyalty < 50            revolt
    ///   37  29.6%  loyal, damaged          siege we could contest
    ///   36  28.8%  loyal, UNDAMAGED        gone from full health in one round
    /// ```
    ///
    /// **Loyalty is the single largest cause of city loss, ahead of every military
    /// shape**, and `66 of the 125` were carrying a NEGATIVE loyalty rate when
    /// last seen.
    ///
    /// ⚠ The rate was available the whole time and nothing read it.
    /// `Game::city_loyalty_per_turn` is computed by the engine, mirrored from
    /// Civilization VI (`mirror.rs` asserts it survives a save) and exported in
    /// `obs.rs` — yet it had **zero consumers in `ai.rs`, `ai/advanced.rs`,
    /// `strategic.rs` and `production.rs`**. The only two loyalty readers both
    /// took the LEVEL: this function's `< 70.0`, and the `bread_and_circuses`
    /// project score's `100 - loyalty`.
    ///
    /// A level threshold cannot see a city dying. A city on 100 losing 12 a turn
    /// is eight turns from flipping and reads as perfectly safe; a city on 60
    /// gaining 5 is recovering and reads as the emergency. That is exactly
    /// backwards, and it is why 36 cities vanished at full loyalty.
    ///
    /// So the governor goes to whichever city flips SOONEST, and a level below
    /// the old threshold is treated as an emergency in its own right so a city
    /// already in trouble is never ranked behind a healthy one.
    pub(super) fn reassign_governor_for_loyalty(&self, g: &mut Game, pid: usize) -> bool {
        let spared = self
            .pingala_loyalty_runway
            .map(|runway| ("pingala", runway));
        self.reassign_governor_for_loyalty_sparing(g, pid, spared)
    }

    /// [`Self::reassign_governor_for_loyalty`], with `spared` naming a seated
    /// Governor that stays where it sits unless the target city's
    /// [`Self::loyalty_emergency`] reads within the given turns
    /// (`culture-keeps-pingala-home`, `advanced/culture_pingala_first.rs`).
    pub(crate) fn reassign_governor_for_loyalty_sparing(
        &self,
        g: &mut Game,
        pid: usize,
        spared: Option<(&str, f64)>,
    ) -> bool {
        let target = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|city| !g.players[pid].governors.contains(city))
            .filter(|city| self.loyalty_emergency(g, *city).is_some())
            .min_by(|left, right| {
                self.loyalty_emergency(g, *left)
                    .unwrap_or(f64::MAX)
                    .total_cmp(&self.loyalty_emergency(g, *right).unwrap_or(f64::MAX))
                    .then(left.cmp(right))
            });
        let Some(target) = target else { return false };
        let target_loyalty = g.cities[&target].loyalty;
        // `victor_first_for_a_short_runway`: a city that flips before an
        // ordinary Governor establishes needs Victor's three turns.
        let short_runway = self.victor_first_for_a_short_runway
            && self
                .loyalty_emergency(g, target)
                .is_some_and(|turns| turns < VICTOR_SHORT_RUNWAY_TURNS);
        let mut eligible: Vec<_> = g
            .legal_actions_within(pid, ActionFamilies::EMPIRE)
            .into_iter()
            .filter_map(|action| {
                let Action::ReassignGovernor { governor, city } = &action else {
                    return None;
                };
                if *city != target || !self.capital_governor_may_depart(g, pid, governor) {
                    return None;
                }
                let state = &g.players[pid].governor_roster[governor];
                if let Some((name, within)) = spared {
                    if governor == name
                        && state.city.is_some()
                        && self
                            .loyalty_emergency(g, target)
                            .is_some_and(|turns| turns > within)
                    {
                        return None;
                    }
                }
                let source_loyalty = state
                    .city
                    .and_then(|city| g.cities.get(&city))
                    .map_or(101.0, |city| city.loyalty);
                // An unassigned Governor is free to move. An established one
                // only leaves a city with a substantial Loyalty cushion.
                (state.city.is_none()
                    || (source_loyalty >= 90.0 && source_loyalty - target_loyalty >= 20.0))
                    .then_some((
                        if short_runway {
                            governor == "victor"
                        } else {
                            state.city.is_none()
                        },
                        if short_runway {
                            state.city.is_none()
                        } else {
                            governor == "victor"
                        },
                        source_loyalty,
                        std::cmp::Reverse(*governor),
                        action,
                    ))
            })
            .collect();
        // `curator-sits-with-the-works`: a Pingala holding Curator goes only
        // when no other Governor can. Off, every eligible Governor stays.
        if self.curator_keeps_his_seat {
            let curator = |governor: &Name| {
                *governor == "pingala"
                    && g.players[pid]
                        .governor_roster
                        .get("pingala")
                        .is_some_and(|state| state.promotions.contains("curator"))
            };
            if eligible
                .iter()
                .any(|(_, _, _, governor, _)| !curator(&governor.0))
            {
                eligible.retain(|(_, _, _, governor, _)| !curator(&governor.0));
            }
        }
        let action = eligible
            .into_iter()
            .max_by(|left, right| {
                left.0
                    .cmp(&right.0)
                    .then(left.1.cmp(&right.1))
                    .then(left.2.total_cmp(&right.2))
                    .then(left.3.cmp(&right.3))
            })
            .map(|(_, _, _, _, action)| action);
        action.is_some_and(|action| g.apply(pid, &action).is_ok())
    }
}
