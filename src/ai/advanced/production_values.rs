//! Production values: what the controller's production pick is worth, and
//! the queue redirects that override it.
//!
//! The values `production_value` reads: `strategic_wonder_value`,
//! `district_project_value` with its Great-Person-race checks and
//! first-building debt, and `support_unit_value`. Then the defensive items:
//! outer-defence repair, pre-emptive major-war defence, and the Campus that
//! finishes before a slow defender. The queue redirects for an unsafe city
//! and for a widespread amenity crisis, the reserved entertainment path,
//! and `emergency_city_defense_purchase`.
//!
//! Moved verbatim out of the one `impl AdvancedAi` block in `advanced.rs`
//! (2026-11-05). Methods the rest of the controller calls are `pub(super)`;
//! the code is otherwise unchanged. `production_value` and
//! `advanced_production` themselves stay in `advanced.rs` for now because
//! an open PR edits them.

use super::*;

mod garrison_firepower;

impl AdvancedAi {
    pub(super) fn culture_preserves_resort_site(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        district: Name,
        pos: Pos,
    ) -> bool {
        self.culture_releases_science_claims(g, pid)
            && BasicAi::culture_resort_site_is_displaced(
                g,
                cid,
                district,
                pos,
                &g.district_sites(cid, district),
            )
    }

    /// ★★★★★ THE WONDERS THE CHOSEN VICTORY ACTUALLY NEEDS.
    ///
    /// Worked backwards from the win condition rather than forwards from the
    /// build menu. The `Item::Wonder` arm of `production_value` prices a
    /// wonder's yields, housing, Amenities, great-work slots and great-person
    /// points — and **nothing in `spec.effects`**, which is where every wonder
    /// that decides a victory keeps its payload. The `Item::Building` arm has
    /// priced `effects` since it was written; the wonder arm never did.
    ///
    /// The consequence is not a mispricing, it is a refusal. `lane_opens` is
    /// true only for a Culture plan, a Score target, or an untargeted Egypt or
    /// China, so every other agent takes the `-10_000` sentinel on every
    /// wonder in the game. Read the data against that:
    ///
    /// * `DIPLOMATIC_VICTORY_POINTS` is 20, and three wonders grant seven of
    ///   them outright — Statue of Liberty 4, Mahabodhi Temple 2, Potala
    ///   Palace 1. **Thirty-five percent of a diplomatic victory sits in the
    ///   build menu and the Diplomacy-targeted agent may not touch it**, while
    ///   `docs/CIV6_LADDER.md`'s census of 199 terminal host events has a
    ///   rival taking that same lane 41 times — the single largest way this
    ///   project loses a live game.
    /// * Science is the lane `victory_eval` completes least often, and Oxford
    ///   University (+20% city science, two free techs), Amundsen-Scott (+20%
    ///   empire science, +10% empire production) and Ruhr Valley (+20% city
    ///   production, which is what the space projects are actually made of)
    ///   are refused by the same line.
    ///
    /// So this prices `spec.effects` **in the lane's own currency**, and the
    /// caller opens a lane when the answer clears `STRATEGIC_WONDER_MIN_VALUE`.
    /// Two pricing rules, no third:
    ///
    /// 1. **A discrete victory currency is priced as its share of the win.**
    ///    `STRATEGIC_WONDER_VICTORY_VALUE` is what finishing the game is worth
    ///    in these units, so a diplomatic victory point is a twentieth of it
    ///    and a free tech is one tech of the tree. Nothing is invented: the
    ///    denominators are the engine's own victory constants.
    /// 2. **A rate effect is converted to the flat yield it is equivalent to**,
    ///    then run through the same `yield_value(..) * 45.0` the arm already
    ///    applies to `spec.yields`. A +20% science multiplier on a city making
    ///    30 science is worth exactly what `"science": 6` in the data would be
    ///    worth, so it is priced as that and needs no constant of its own.
    ///
    /// Rule 2 is why the value is city-specific and turn-specific: Oxford in
    /// the empire's research city and Oxford in a two-population border town
    /// are not the same build, and the arm as written could not tell them
    /// apart. It is also self-limiting — a multiplier on an empire that makes
    /// nothing prices at nothing, so a wrecked or infant empire keeps building
    /// what it needs instead of chasing a wonder it cannot use.
    ///
    /// ⚠ Deliberately NOT priced: an effect the lane does not need. A Religion
    /// plan gets nothing here for Oxford's beakers beyond what `spec.yields`
    /// already pays it. The point is to open a lane for the wonders that
    /// finish the game this agent is playing for — not to make every wonder
    /// attractive to everyone, which is the failure mode the `-10_000` line
    /// was protecting against and still protects against.
    pub(super) fn strategic_wonder_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        spec: &crate::rules::WonderSpec,
        lane: GrandStrategy,
    ) -> f64 {
        let effect = |key: &str| spec.effects.get(key).copied().unwrap_or(0.0);
        // ⚠ NOTHING IS READ OFF THE BOARD UNTIL AN EFFECT ASKS FOR IT. This
        // runs inside `production_value`, which `take_turn` calls once per
        // legal item per city per turn and which the profile puts at ~95% of
        // the main thread. Fifty of the fifty-three wonders name none of the
        // rate effects below, and an empire sweep for each of them would be
        // cubic in city count for an answer of zero. `scaled` is the only
        // gate: no effect, no yield read.
        let scaled = |key: &str, amount: &mut f64, of: &dyn Fn() -> f64| {
            let pct = effect(key);
            if pct != 0.0 {
                *amount += pct / 100.0 * of();
            }
        };
        let city_yields = || g.city_yields(cid);
        let empire_yield = |pick: fn(&crate::rules::Yields) -> f64| -> f64 {
            g.player_city_ids(pid)
                .into_iter()
                .map(|other| pick(&g.city_yields(other)))
                .sum()
        };
        // Rule 1's denominators, all of them the engine's own.
        let victory_point =
            STRATEGIC_WONDER_VICTORY_VALUE / crate::game::DIPLOMATIC_VICTORY_POINTS.max(1) as f64;
        let tech = STRATEGIC_WONDER_VICTORY_VALUE / g.rules.techs.len().max(1) as f64;
        let civic = STRATEGIC_WONDER_VICTORY_VALUE / g.rules.civics.len().max(1) as f64;
        // Rule 2: a percentage becomes the flat yield it is equivalent to, so
        // the arm's existing `yield_value(..) * 45.0` prices it unchanged.
        let mut equivalent = crate::rules::Yields::default();
        // Rule 1 and the reused unit valuations accumulate here instead.
        let mut direct = 0.0;

        match lane {
            // ──────────────────────────────────────────────────────────────
            // Diplomacy. Twenty points win, and seven of them are wonders.
            // Envoys and favour are the indirect route to the same counter: a
            // suzerainty is votes in the Congress, and the Congress is where
            // the other thirteen come from.
            GrandStrategy::Diplomacy => {
                direct += effect("diplomatic_victory_points") * victory_point;
                let suzerainties = g
                    .players
                    .iter()
                    .filter(|minor| {
                        minor.is_minor
                            && !minor.is_barbarian
                            && g.suzerain_of(minor.id) == Some(pid)
                    })
                    .count() as f64;
                // The Building arm pays 300 an envoy in this lane; the same
                // envoy bought by a wonder is the same envoy.
                direct += effect("envoys") * DIPLOMATIC_ENVOY_PRODUCTION_VALUE;
                direct += effect("envoys_per_wonder")
                    * DIPLOMATIC_ENVOY_PRODUCTION_VALUE
                    * g.cities
                        .values()
                        .filter(|other| other.owner == pid)
                        .map(|other| other.wonders.len() as f64)
                        .sum::<f64>();
                // Orszaghaz doubles the favour every suzerainty pays, which is
                // worth what the suzerainties are — nothing at none held.
                direct += effect("suzerain_diplomatic_favor_pct") / 100.0
                    * DIPLOMATIC_ENVOY_PRODUCTION_VALUE
                    * suzerainties;
                direct += effect("diplomatic_policy_slots") * STRATEGIC_WONDER_POLICY_SLOT;
            }
            // ──────────────────────────────────────────────────────────────
            // Science. Beakers to reach the techs, and production to build the
            // projects the space race is actually made of.
            GrandStrategy::Science => {
                scaled("city_science_pct", &mut equivalent.science, &|| {
                    city_yields().science
                });
                scaled("empire_science_pct", &mut equivalent.science, &|| {
                    empire_yield(|yields| yields.science)
                });
                scaled("city_production_pct", &mut equivalent.production, &|| {
                    city_yields().production
                });
                scaled("empire_production_pct", &mut equivalent.production, &|| {
                    empire_yield(|yields| yields.production)
                });
                equivalent.science += effect("incoming_route_city_science");
                direct += effect("free_random_techs") * tech;
                // The Great Library boosts every unresearched ancient and
                // classical tech at half a tech each, so it is worth what is
                // left of that half of the tree and nothing once it is done.
                direct += if effect("ancient_classical_tech_boosts") > 0.0 {
                    let unresearched = g
                        .rules
                        .techs
                        .iter()
                        .filter(|(name, node)| {
                            node.era <= 1 && !g.players[pid].techs.contains(*name)
                        })
                        .count() as f64;
                    unresearched * tech * 0.5
                } else {
                    0.0
                };
            }
            // ──────────────────────────────────────────────────────────────
            // Culture. The win is foreign tourists, and tourism multipliers
            // are the only place in the build menu they arrive in bulk.
            GrandStrategy::Culture => {
                scaled("city_culture_pct", &mut equivalent.culture, &|| {
                    city_yields().culture
                });
                let multiplied = (effect("seaside_resort_tourism_pct")
                    + effect("city_improvement_park_tourism_pct")
                    + effect("city_religious_tourism_pct"))
                    / 100.0
                    + (effect("religious_tourism_unreduced") + effect("renewable_power_tourism"))
                        * 0.25;
                if multiplied != 0.0 {
                    direct += multiplied * g.tourism_per_turn(pid) * STRATEGIC_WONDER_TOURISM_RATE;
                }
                // Appeal is what the seaside resorts and national parks those
                // multipliers act on are made of, so it is priced through the
                // same rate at a quarter weight — it buys the ground, not the
                // tourists.
                let appeal = effect("empire_appeal");
                if appeal != 0.0 || effect("city_appeal") != 0.0 {
                    direct += (appeal * g.player_city_ids(pid).len() as f64
                        + effect("city_appeal"))
                        * STRATEGIC_WONDER_TOURISM_RATE
                        * 0.25;
                }
                direct += effect("free_random_civics") * civic;
                direct += effect("wildcard_policy_slots") * STRATEGIC_WONDER_POLICY_SLOT;
            }
            // ──────────────────────────────────────────────────────────────
            // Religion. The win is converted cities; converted cities are
            // Apostles reaching them. Faith is the fuel, charges are the reach.
            GrandStrategy::Religion => {
                scaled("city_faith_pct", &mut equivalent.faith, &|| {
                    city_yields().faith
                });
                equivalent.faith += effect("regional_faith");
                direct += (effect("apostles")
                    + effect("gurus")
                    + effect("religious_spread_charges")
                    + effect("apostles_gain_martyr"))
                    * STRATEGIC_WONDER_FREE_UNIT;
                direct += effect("free_warrior_monks") * STRATEGIC_WONDER_FREE_UNIT * 0.5;
                direct += effect("guru_purchase_discount_pct") / 100.0 * STRATEGIC_WONDER_FREE_UNIT;
                // A religion the agent does not have is a religious victory it
                // cannot start, so Stonehenge's Prophet is priced in the same
                // units as a diplomatic point. `spent_religion_founding_site`
                // in the caller already refuses it once the religion is
                // founded, and this agrees with it rather than restating it.
                direct += if g.players[pid].religion.is_none() {
                    effect("free_great_prophet") * victory_point
                } else {
                    0.0
                };
            }
            // ──────────────────────────────────────────────────────────────
            // ⚠⚠ CONQUEST IS DELIBERATELY EMPTY, AND IT WAS NOT AT FIRST.
            //
            // The Statue of Zeus' seven free units, the Terracotta Army's
            // promotion of everything standing and the Venetian Arsenal's
            // second hull are the only wonders in the table that put units on
            // the map, so pricing them for this lane looked obvious. Measured,
            // it cost the lane two games in eight: `victory_eval --target all
            // --games 8 --players 6 --turns 250 --speed online --start-seed
            // 21000000` completed domination 1/8 with the arm and 3/8 with
            // `--without strategic-wonders`, and both losses ended on the
            // turn-250 tally instead of on a capital.
            //
            // Small, but it agrees with the mechanism and with this file's
            // own record. A domination victory is not bought in the build
            // menu: it is every rival capital, and the production a wonder
            // takes is the production that was going to take one. The
            // Statue of Zeus' Archers are a classical army arriving in the
            // classical era, worth less than the medieval units the same 400
            // production buys; the Terracotta Army promotes an army the
            // empire already has rather than making it larger. Compare the
            // 2026-08-14 war-half removal, which paid +38 Elo for spending
            // *less* production on fighting — the same trade read the other
            // way round.
            //
            // The four lanes below it are not this shape. Seven of a
            // diplomatic victory's twenty points ARE wonders; a culture
            // victory's tourists arrive nowhere else in bulk. Where a wonder
            // carries the win condition it is priced, and where it merely
            // helps the army it is not.
            GrandStrategy::Conquest => {}
            // Expansion carries the Score target, whose wonders are already
            // priced by the tally itself; Recovery never opens the lane.
            GrandStrategy::Expansion | GrandStrategy::Recovery => {}
        }

        self.yield_value(equivalent, lane) * 45.0 + direct
    }

    /// Whether an opening Great-Person project is one of the few forcing
    /// cases worth production before ordinary infrastructure is in place.
    fn early_project_race_exception(g: &Game, pid: usize, awards: &BTreeMap<String, f64>) -> bool {
        // A religion is a finite opening race. Holy Site Prayers stays live
        // while a Prophet can still be earned, even when a single completion
        // is not enough to claim one immediately.
        if awards
            .get("prophet")
            .is_some_and(|award| *award > f64::EPSILON)
            && g.players[pid].religion.is_none()
            && g.great_person_class_earnable(pid, "prophet")
            && g.great_person_class_offered_now(pid, "prophet")
            && g.live_great_person_offer_blocker(pid, "prophet").is_none()
        {
            return true;
        }

        let Some(award) = awards
            .get("scientist")
            .copied()
            .filter(|award| *award > f64::EPSILON)
        else {
            return false;
        };
        if !g.great_person_class_earnable(pid, "scientist")
            || !g.great_person_class_offered_now(pid, "scientist")
            || g.live_great_person_offer_blocker(pid, "scientist")
                .is_some()
        {
            return false;
        }
        // An early Scientist is worth an opening diversion only when their
        // effect compounds broadly (free/scaled science infrastructure) or
        // supplies at least three Eureka boosts. This admits the Aryabhata /
        // Hypatia class without spending a project merely to clutch Euclid.
        let high_impact =
            if let Some(affinity) = Self::science_live_great_person_affinity(g, pid, "scientist") {
                affinity >= 2.3
            } else {
                let Some((_, scientist)) = g.current_great_person("scientist") else {
                    return false;
                };
                [
                    "free_library",
                    "free_university",
                    "libraries_science",
                    "universities_science",
                    "research_labs_science",
                ]
                .into_iter()
                .any(|effect| {
                    scientist
                        .effects
                        .get(effect)
                        .is_some_and(|value| *value > 0.0)
                }) || scientist
                    .effects
                    .get("tech_boosts")
                    .is_some_and(|boosts| *boosts >= 3.0)
            };
        if !high_impact {
            return false;
        }
        let cost = g.gp_cost(pid, "scientist");
        let mine = g.players[pid].gpp.get("scientist").copied().unwrap_or(0.0);
        mine < cost && mine + award + f64::EPSILON >= cost
    }

    /// Whether this project's own district has a concrete first-building
    /// debt the city can pay now.
    ///
    /// V1 inferred underdevelopment from the global turn number and therefore
    /// suppressed a project even after the relevant district was developed.
    /// V2 asks the board directly. A locked building is not debt yet, a
    /// district family with no building is never debt, and one standing family
    /// building is enough to release the project at any turn.
    fn project_first_building_debt(g: &Game, pid: usize, cid: u32, project: &str) -> bool {
        let spec = &g.rules.projects[project];
        let city = &g.cities[&cid];
        let Some(district) = spec.district else {
            return false;
        };
        let family = g.district_family(district);
        let first_building = match family.as_str() {
            "campus" => "library",
            "holy_site" => "shrine",
            "commercial_hub" => "market",
            "harbor" => "lighthouse",
            "encampment" => "barracks",
            "industrial_zone" => "workshop",
            "theater_square" => "amphitheater",
            _ => return false,
        };
        if city.buildings.iter().any(|building| {
            g.rules.buildings[building]
                .district
                .is_some_and(|built| g.district_family(built) == family)
        }) {
            return false;
        }
        BasicAi::civ_building(g, pid, cid, first_building).is_some()
    }

    /// Whether this completion changes a live Great-Person race immediately.
    ///
    /// V1 made named exceptions only for Prophets and a short list of
    /// Scientists. V2 keeps the building debt subordinate to the board's
    /// generic forcing facts: one completion either recruits the current
    /// person or overtakes the leading rival. The ordinary affinity and
    /// project valuation still decide whether that newly admitted move wins
    /// the queue.
    fn project_changes_great_person_race(
        g: &Game,
        pid: usize,
        awards: &BTreeMap<String, f64>,
    ) -> bool {
        awards.iter().any(|(kind, award)| {
            if *award <= f64::EPSILON
                || !g.great_person_class_earnable(pid, kind)
                || !g.great_person_class_offered_now(pid, kind)
                || g.live_great_person_offer_blocker(pid, kind).is_some()
            {
                return false;
            }
            let cost = g.gp_cost(pid, kind);
            let mine = g.players[pid].gpp.get(kind).copied().unwrap_or(0.0);
            let rival = g
                .players
                .iter()
                .filter(|player| {
                    player.id != pid && player.alive && !player.is_minor && !player.is_barbarian
                })
                .map(|player| player.gpp.get(kind).copied().unwrap_or(0.0))
                .fold(0.0_f64, f64::max);
            (mine < cost && mine + award + f64::EPSILON >= cost)
                || (rival > mine && mine + award > rival)
        })
    }

    /// Whether any completion points advance the empire's active lane.
    ///
    /// A Science empire should not sacrifice its Scientist race merely to
    /// finish a Library first; the project's normal value already compares
    /// that race with the building. V2's restraint is for off-lane project
    /// churn, where a concrete first building is the compounding move.
    fn project_serves_great_person_lane(
        lane: GrandStrategy,
        awards: &BTreeMap<String, f64>,
    ) -> bool {
        awards.iter().any(|(kind, award)| {
            *award > f64::EPSILON
                && match lane {
                    GrandStrategy::Science => matches!(kind.as_str(), "scientist" | "engineer"),
                    GrandStrategy::Culture => {
                        matches!(kind.as_str(), "writer" | "artist" | "musician")
                    }
                    GrandStrategy::Religion => kind == "prophet",
                    GrandStrategy::Diplomacy => kind == "merchant",
                    GrandStrategy::Conquest => matches!(kind.as_str(), "general" | "admiral"),
                    GrandStrategy::Expansion | GrandStrategy::Recovery => {
                        matches!(kind.as_str(), "engineer" | "merchant")
                    }
                }
        })
    }

    /// Evaluate a repeatable district project as a bounded race move. The
    /// ongoing conversion is valued over the actual build horizon, while the
    /// completion award is priced against the live global Great Person race.
    /// This is deliberately analogous to an engine's passed-pawn extension:
    /// a project receives a large tempo bonus only when completing it crosses
    /// a concrete threshold, rather than from a fixed name-based preference.
    pub(super) fn district_project_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        project: &str,
        plan: &StrategicPlan,
    ) -> f64 {
        // Once a current controller has crossed the shared specialization
        // boundary into its Science plan, Commercial Hub Investment spends a
        // mature queue on Gold and Merchant points instead of the Space Race.
        // The development half and emergency Recovery plans retain their
        // ordinary project choices, and the frozen v1 controller stays exact.
        if self.victory_planning
            && Self::victory_specialization_active(g)
            && self.active_victory_target(g) == Some(VictoryTarget::Science)
            && plan.strategy == GrandStrategy::Science
            && project == "commercial_hub_investment"
        {
            return -10_000.0;
        }
        let spec = &g.rules.projects[project];
        let city = &g.cities[&cid];
        let production = g.city_yields(cid).production.max(1.0);
        let item = Item::Project {
            project: Name::new(project),
        };
        let turns = g.item_remaining_cost_for_city(pid, cid, &item) / production;
        let threatened = plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4);
        let denominator = 7.0 + turns.max(1.0);

        let mut ongoing = Yields::default();
        for (kind, percent) in &spec.ongoing_yields {
            let amount = production * percent / 100.0;
            match kind.as_str() {
                "food" => ongoing.food += amount,
                "production" => ongoing.production += amount,
                "gold" => ongoing.gold += amount,
                "science" => ongoing.science += amount,
                "culture" => ongoing.culture += amount,
                "faith" => ongoing.faith += amount,
                _ => {}
            }
        }
        let horizon = turns.clamp(1.0, 16.0);
        // Keep the science-drive's Campus Research Grants credit inside this
        // routine, before the early-project and building-debt caps below. A
        // project bonus added after those caps would recreate the exact
        // overhang the caps were introduced to prevent.
        let mut value = self.yield_value(ongoing, plan.strategy) * horizon * 4.0
            + self.science_drive_production_bonus(g, pid, cid, &item);

        let gpp_awards = g.project_completion_gpp_awards(pid, cid, project);
        let great_person_lane = self.great_person_lane(g, pid, plan);
        let repeatable_gpp_project = spec.repeatable && !spec.completion_gpp.is_empty();
        let named_race_exception = Self::early_project_race_exception(g, pid, &gpp_awards);
        let early_gpp_project_restrained = repeatable_gpp_project
            && !named_race_exception
            && ((self.early_project_restraint
                && g.turn < g.standard_duration(EARLY_PROJECT_RESTRAINT_STANDARD_TURNS))
                || (self.early_project_restraint_2
                    && Self::project_first_building_debt(g, pid, cid, project)
                    && !Self::project_serves_great_person_lane(great_person_lane, &gpp_awards)
                    && !Self::project_changes_great_person_race(g, pid, &gpp_awards)));
        let host_competition_gpp_scores = [
            (
                "EMERGENCY_WORLDS_FAIR",
                Self::host_competition_gpp_score(&gpp_awards, "EMERGENCY_WORLDS_FAIR"),
            ),
            (
                "EMERGENCY_NOBEL_PRIZE_LITERATURE",
                Self::host_competition_gpp_score(&gpp_awards, "EMERGENCY_NOBEL_PRIZE_LITERATURE"),
            ),
            (
                "EMERGENCY_NOBEL_PRIZE_PHYSICS",
                Self::host_competition_gpp_score(&gpp_awards, "EMERGENCY_NOBEL_PRIZE_PHYSICS"),
            ),
        ];
        // See `lane_great_people`: which Great Person class this project's
        // points are worth chasing is a question about the race, not about the
        // settling posture, and this arm runs in every city every turn —
        // unlike patronage, which needs a bank the opening rarely has. Hoisted
        // out of the loop: `raced_lane` short-circuits on any plan that is not
        // `Expansion`, but `victory_focus` behind it is an empire-wide sweep
        // and this is `production_value`'s hot path.
        for (kind, award) in gpp_awards {
            // Patronage outcome B can set this class's completion award to
            // zero. Ongoing yield conversion may still justify the project,
            // but a disabled class has no race tempo to extend.
            if award <= f64::EPSILON {
                continue;
            }
            // A class race is not necessarily a usable named offer. Firaxis can
            // offer Hildegard of Bingen as a Scientist to an empire with no Holy
            // Site, or Mary Leakey to one with no Theater. The live mirror carries
            // that hard prerequisite separately from this simulator's generic
            // class model; once it says the current offer cannot activate, GPP has
            // no race tempo to extend. Ongoing project yields remain above.
            if g.live_great_person_offer_blocker(pid, &kind).is_some() {
                continue;
            }
            let class_affinity: f64 = match (great_person_lane, kind.as_str()) {
                (GrandStrategy::Science, "scientist") => 2.5,
                (GrandStrategy::Culture, "writer" | "artist" | "musician") => 2.6,
                (GrandStrategy::Religion, "prophet") if g.players[pid].religion.is_none() => 2.8,
                (GrandStrategy::Diplomacy, "merchant") => 2.0,
                (GrandStrategy::Conquest, "general" | "admiral") => 2.3,
                (GrandStrategy::Expansion | GrandStrategy::Recovery, "engineer" | "merchant") => {
                    1.8
                }
                (GrandStrategy::Science | GrandStrategy::Culture, "engineer") => 1.6,
                (_, "prophet") if g.players[pid].religion.is_some() => 0.15,
                _ => 0.85,
            };
            let mut affinity = if great_person_lane == GrandStrategy::Science {
                Self::science_live_great_person_affinity(g, pid, &kind).unwrap_or(class_affinity)
            } else {
                class_affinity
            };
            let work = match kind.as_str() {
                "writer" => Some("writing"),
                "artist" => Some("art"),
                "musician" => Some("music"),
                _ => None,
            };
            if work.is_some_and(|work| !g.can_house_additional_great_work(pid, work)) {
                affinity *= 0.20;
            }

            let cost = g.gp_cost(pid, &kind).max(1.0);
            let mine = g.players[pid].gpp.get(&kind).copied().unwrap_or(0.0);
            let rival = g
                .players
                .iter()
                .filter(|player| {
                    player.id != pid && player.alive && !player.is_minor && !player.is_barbarian
                })
                .map(|player| player.gpp.get(&kind).copied().unwrap_or(0.0))
                .fold(0.0_f64, f64::max);
            let useful = award.min((cost - mine).max(0.0));
            value += useful * (5.0 + 5.0 * affinity);
            value += (rival / cost).clamp(0.0, 1.0) * 150.0 * affinity;
            if mine + award + f64::EPSILON >= cost && mine < cost {
                value += 620.0 * affinity;
            }
            if rival > mine && mine + award > rival {
                value += 240.0 * affinity;
            }
        }

        if spec.full_power_while_active {
            let deficit = (g.city_power_demand(city) - g.city_power_supply(city)).max(0.0);
            value += deficit * 55.0 * denominator;
        }

        if project == "bread_and_circuses" {
            let loyalty_need = (100.0 - city.loyalty).max(0.0);
            let nearby_foreign_pressure = g
                .cities
                .values()
                .filter(|other| {
                    other.owner != pid
                        && !g.players[other.owner].is_barbarian
                        && g.alliance_with(pid, other.owner)
                            .is_none_or(|alliance| alliance.kind != "cultural")
                })
                .filter_map(|other| {
                    let distance = g.wdist(city.pos, other.pos);
                    (distance <= 9)
                        .then_some(other.pop.max(1) as f64 * (10 - distance) as f64 / 10.0)
                })
                .sum::<f64>();
            if loyalty_need < 5.0 && nearby_foreign_pressure < 2.0 {
                value -= 260.0;
            } else {
                value += loyalty_need * 8.0
                    + nearby_foreign_pressure * horizon * 7.0
                    + spec
                        .effects
                        .get("completion_loyalty")
                        .copied()
                        .unwrap_or(0.0)
                        * 7.0;
            }
        }

        // A project may exploit completed infrastructure, but should not
        // indefinitely postpone the first building in the district that
        // enables it. This is the economic equivalent of a quiet-move pruning
        // guard: search the forcing race only after basic development exists.
        if let Some(district) = spec.district {
            let family = g.district_family(district);
            let has_family_building = city.buildings.iter().any(|building| {
                g.rules.buildings[building]
                    .district
                    .is_some_and(|built| g.district_family(built) == family)
            });
            let family_has_building = g.rules.buildings.values().any(|building| {
                building.buildable
                    && building
                        .district
                        .is_some_and(|built| g.district_family(built) == family)
            });
            if family_has_building && !has_family_building {
                value -= 420.0;
            }
        }
        if threatened {
            value -= 360.0;
        }
        // A generic early Great-Person project is a tempo loss against the
        // city needs it postpones. Keep the rare Prophet and high-impact
        // Scientist races above, plus emergencies/competitions priced outside
        // this routine, as the forcing exceptions.
        if early_gpp_project_restrained {
            value = value.min(EARLY_GPP_PROJECT_FALLBACK_CAP);
        }
        // ★★★★ THE CHEAP HALF OF A RESEARCH CITY BEFORE THE RACE IN IT. See
        // `buildings_before_projects`.
        // A named Science seat must not let a cheap Commercial Hub Investment
        // (or any other repeatable project) outrank an available Library,
        // University, Research Lab or Workshop merely because the deployment
        // ledger withheld this independently screenable treatment. Every
        // named lane likewise holds a Factory or plant behind the productive
        // city's own Industrial Zone before it spends the queue on a
        // repeatable project. Adaptive seats retain the independently
        // screenable opt-ins.
        let research_project_guard = self.buildings_before_projects
            || (self.phase_specialization_active(g)
                && self.active_victory_target(g) == Some(VictoryTarget::Science));
        // `industrial-chain-debt` historically joined this guard only when
        // `buildings-before-projects` had opened it, so retain that adaptive
        // experiment unchanged. A named victory target is different: its
        // Workshop/Factory/power chain is a production contract in its own
        // right, even in Culture, Religion, Diplomacy, Domination, or Score.
        let industrial_project_guard = self.active_victory_target(g).is_some()
            || (self.industrial_chain_debt && research_project_guard);
        if (research_project_guard || industrial_project_guard) && spec.repeatable {
            let waiting_for = BUILDINGS_BEFORE_PROJECTS
                .iter()
                .take(if research_project_guard {
                    BUILDINGS_BEFORE_PROJECTS.len()
                } else {
                    0
                })
                .chain(CULTURE_BUILDINGS_BEFORE_PROJECTS.iter().take(
                    if research_project_guard && self.culture_building_debt {
                        CULTURE_BUILDINGS_BEFORE_PROJECTS.len()
                    } else {
                        0
                    },
                ))
                // See `industrial_chain_debt`: the Factory and the plant wait
                // in the same line as the Workshop. Every named lane has the
                // production-foundation contract even if the ledger withholds
                // the adaptive opt-in.
                .chain(INDUSTRIAL_BUILDINGS_BEFORE_PROJECTS.iter().take(
                    if industrial_project_guard {
                        INDUSTRIAL_BUILDINGS_BEFORE_PROJECTS.len()
                    } else {
                        0
                    },
                ));
            let owed = waiting_for.into_iter().any(|building| {
                g.rules.buildings.contains_key(building)
                    && g.can_produce(
                        pid,
                        cid,
                        &Item::Building {
                            building: Name::new(building),
                        },
                    )
            });
            if owed {
                value = value.min(PROJECT_BEHIND_BUILDINGS_CAP);
            }
        }
        // Firaxis scores these competitions from the completion awards above:
        // World Fair accepts every GPP class, Nobel Literature accepts the
        // culture classes, and Nobel Physics accepts the STEM classes. This
        // immediate host payoff must remain outside the normal "build the
        // Library first" cap: a competition that ends before the building is
        // complete cannot be resumed later.
        value
            + host_competition_gpp_scores
                .into_iter()
                .map(|(kind, score)| self.host_competition_score_value(g, pid, kind, score, turns))
                .sum::<f64>()
    }

    /// Firaxis's EmergencyScoreSources award one score per Great Person point:
    /// every class at the World's Fair, culture classes for Nobel Literature,
    /// and STEM classes for Nobel Physics. Nobel Peace instead scores Favor,
    /// so a production project's GPP award must not invent a score there.
    fn host_competition_gpp_score(gpp_awards: &BTreeMap<String, f64>, kind: &str) -> f64 {
        let score = |classes: &[&str]| {
            classes
                .iter()
                .filter_map(|class| gpp_awards.get(*class))
                .copied()
                .filter(|award| award.is_finite() && *award > f64::EPSILON)
                .sum()
        };
        match kind {
            "EMERGENCY_WORLDS_FAIR" => gpp_awards
                .values()
                .copied()
                .filter(|award| award.is_finite() && *award > f64::EPSILON)
                .sum(),
            "EMERGENCY_NOBEL_PRIZE_LITERATURE" => score(&["writer", "artist", "musician"]),
            "EMERGENCY_NOBEL_PRIZE_PHYSICS" => score(&["engineer", "merchant", "scientist"]),
            _ => 0.0,
        }
    }

    /// Spend Gold on the one thing that can still keep a besieged city alive
    /// before upgrades, patronage, or the ordinary strategic purchaser can
    /// compare it with development.
    ///
    /// The live garrison doctrine already identifies the exact immediate
    /// defense — Walls first, otherwise a land melee defender — and reclaims
    /// an unsafe production queue for it.  A completed purchase is faster
    /// than that queue.  In the live Rome siege at turn 136, however, the
    /// generic Gold evaluator chose a Builder in the capital at 171/200 city
    /// damage; the Pike and Shot it had just queued needed nine turns.  That
    /// is an emergency, not a normal reserve decision.
    ///
    /// This deliberately shares `garrison_under_fire` with the production
    /// response.  Frozen controllers leave that live-only flag false, so they
    /// return before enumerating a city, a threat, or a purchase menu.
    pub(super) fn emergency_city_defense_purchase(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        // The native-emergency-purchase family supplements the live gate with
        // its selected native signal. Version two narrows that signal to fresh
        // damage corroborated by a visible legal attacker.
        if !self.base.garrison_under_fire && !self.native_emergency_purchase_on() {
            return false;
        }

        // One emergency-purchase option: the sort key (see the push below),
        // the city that would buy it, its name, and the item itself.
        type Candidate = ((i32, i32, i32, std::cmp::Reverse<u32>), u32, String, Item);
        let mut candidates: Vec<Candidate> = Vec::new();
        for city_id in g.player_city_ids(pid) {
            let Some(defence) = self
                .base
                .besieged_city_item(g, pid, city_id)
                .or_else(|| self.native_emergency_item(g, pid, city_id))
            else {
                continue;
            };
            let city = &g.cities[&city_id];
            let city_damage = CITY_MAX_HP.saturating_sub(city.hp);
            let wall_damage = g.city_max_wall_hp(city).saturating_sub(city.wall_hp);
            let combined_damage = city_damage.saturating_add(wall_damage);
            // A city already losing health is the first rescue; a known
            // threatened city then breaks ties ahead of a merely nearby raid.
            let rank = (
                city_damage,
                combined_damage,
                i32::from(plan.threatened_city == Some(city_id)),
                std::cmp::Reverse(city_id),
            );
            candidates.push((rank, city_id, city.name.clone(), defence));
        }

        if candidates.is_empty() {
            return false;
        }
        candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.0));
        let purchases = self.legal_purchase_actions(g, pid);
        let Some((rank, action, city_name, defence)) = candidates.into_iter().find_map(
            |(rank, city_id, city_name, defence)| {
                // Walls are the right production answer, but the live Gold
                // menu can exclude them.  A bleeding unwalled city must then
                // buy the local land defender rather than abandon its only
                // immediate rescue because the preferred queue item is not a
                // purchase action.
                let fallback_defender = matches!(
                    &defence,
                    Item::Building { building }
                        if matches!(building.as_str(), "walls" | "medieval_walls" | "renaissance_walls")
                )
                .then(|| {
                    self.base
                        .best_military(g, pid, city_id, Some(false))
                        .map(|unit| Item::Unit {
                            unit: Name::new(&unit),
                        })
                })
                .flatten();
                for selected in std::iter::once(defence).chain(fallback_defender) {
                    let action = purchases
                        .iter()
                        .find(|action| match (action, &selected) {
                            (
                                Action::Buy {
                                    city,
                                    unit,
                                    formation,
                                    currency,
                                },
                                Item::Unit { unit: wanted },
                            ) => {
                                *city == city_id
                                    && *formation == 0
                                    && currency == "gold"
                                    && unit == wanted
                            }
                            (
                                Action::BuyBuilding {
                                    city,
                                    building,
                                    currency,
                                },
                                Item::Building { building: wanted },
                            ) => *city == city_id && currency == "gold" && building == wanted,
                            _ => false,
                        })
                        .cloned();
                    if let Some(action) = action {
                        return Some((rank, action, city_name, selected));
                    }
                }
                None
            },
        )
        else {
            return false;
        };
        let (city_damage, combined_damage, _, _) = rank;
        let before = g.players[pid].gold;
        if g.apply(pid, &action).is_err() {
            return false;
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            let spent = (before - g.players[pid].gold).max(0.0);
            think!(self.journal(), Economy, Decision,
                "Buying emergency {} for {}", Self::plain_item(&defence), city_name;
                "{spent:.0} Gold defends a city with {city_damage} city and {combined_damage} combined health missing before ordinary purchases");
        }
        true
    }

    pub(super) fn support_unit_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        unit: &str,
        plan: &StrategicPlan,
        counts: &EmpireCounts,
    ) -> f64 {
        let spec = &g.rules.units[unit];
        if spec.class != "support" || unit == "military_engineer" {
            return -10_000.0;
        }
        if spec.anti_air_strength > 0.0 {
            let hostile_aircraft = g
                .units
                .values()
                .filter(|candidate| {
                    candidate.owner != pid
                        && g.is_at_war(pid, candidate.owner)
                        && g.rules.units[candidate.kind].domain.as_deref() == Some("air")
                })
                .count();
            let desired = hostile_aircraft.min(g.player_city_ids(pid).len().div_ceil(2).max(1));
            if desired == 0 || counts.air_defense >= desired {
                return -10_000.0;
            }
            let best_available = g
                .rules
                .units
                .iter()
                .filter(|(name, candidate)| {
                    candidate.class == "support"
                        && candidate.anti_air_strength > 0.0
                        && g.can_produce(pid, cid, &Item::Unit { unit: **name })
                })
                .map(|(_, candidate)| candidate.anti_air_strength)
                .fold(0.0_f64, f64::max);
            if spec.anti_air_strength + 5.0 < best_available {
                return -2_000.0;
            }
            return 340.0
                + spec.anti_air_strength * 3.0
                + hostile_aircraft.min(4) as f64 * 65.0
                + desired.saturating_sub(counts.air_defense) as f64 * 90.0;
        }
        let existing_kinds: Vec<&str> = g
            .units
            .values()
            .filter(|candidate| candidate.owner == pid)
            .map(|candidate| candidate.kind.as_str())
            .chain(
                g.cities
                    .values()
                    .filter(|city| city.owner == pid && city.id != cid)
                    .filter_map(|city| match city.queue.first() {
                        Some(Item::Unit { unit }) => Some(unit.as_str()),
                        _ => None,
                    }),
            )
            .collect();
        let is_breach = matches!(unit, "battering_ram" | "siege_tower");
        let target_cities: Vec<_> = plan
            .target_city
            .and_then(|city| g.cities.get(&city))
            .into_iter()
            .chain(
                g.cities
                    .values()
                    .filter(|city| plan.target_city != Some(city.id)),
            )
            // `early-conquest-opening`: the opening's walled target is one
            // before the war it is being prepared for.
            .filter(|city| {
                city.wall_hp > 0
                    && (g.is_at_war(pid, city.owner)
                        || self.conquest_breach_target(g) == Some(city.id))
            })
            .collect();
        let useful_breach = |kind: &str| {
            target_cities
                .iter()
                .any(|city| g.city_allows_siege_support(city.id, kind))
        };
        if is_breach
            && (!super::super::siege_support::has_attackers(g, pid) || !useful_breach(unit))
        {
            return -10_000.0;
        }
        // Obsolete breach equipment must not occupy the only support slot
        // needed by a replacement or by a balloon for the siege artillery.
        let obsolete_breach = existing_kinds
            .iter()
            .filter(|kind| {
                matches!(**kind, "battering_ram" | "siege_tower") && !useful_breach(kind)
            })
            .count();
        let land_military = counts
            .military
            .saturating_sub(counts.naval + counts.aircraft);
        let field_support = counts
            .support
            .saturating_sub(counts.military_engineers + counts.air_defense + obsolete_breach);
        let desired_support = if land_military >= 8 {
            2
        } else if land_military >= 3 || is_breach {
            // An eligible infantry group needs one usable breach element even
            // before it reaches the ordinary three-unit field-support floor.
            1
        } else {
            0
        };
        if field_support >= desired_support {
            return -10_000.0;
        }

        let has_capability = |effect: &str| {
            existing_kinds.iter().any(|kind| {
                g.rules.units[*kind]
                    .effects
                    .get(effect)
                    .is_some_and(|amount| *amount > 0.0)
            })
        };
        if (spec
            .effects
            .get("adjacent_siege_range")
            .copied()
            .unwrap_or(0.0)
            > 0.0
            && has_capability("adjacent_siege_range"))
            || (spec.effects.get("adjacent_heal").copied().unwrap_or(0.0) > 0.0
                && has_capability("adjacent_heal"))
            || (is_breach
                && existing_kinds.iter().any(|kind| {
                    matches!(*kind, "battering_ram" | "siege_tower")
                        && target_cities.iter().any(|city| {
                            g.city_allows_siege_support(city.id, unit)
                                && g.city_allows_siege_support(city.id, kind)
                        })
                }))
        {
            return -10_000.0;
        }

        let breach_value: f64 = match unit {
            "battering_ram" => 760.0,
            "siege_tower" => 800.0,
            _ => 0.0,
        };
        let siege_range = spec
            .effects
            .get("adjacent_siege_range")
            .copied()
            .unwrap_or(0.0);
        let siege_bombard = spec
            .effects
            .get("adjacent_siege_bombard")
            .copied()
            .unwrap_or(0.0);
        let siege_value = if counts.siege > 0 {
            siege_range * 470.0 + siege_bombard * 38.0
        } else {
            0.0
        };
        let wounded = g
            .units
            .values()
            .filter(|candidate| {
                candidate.owner == pid
                    && candidate.hp < 100
                    && g.rules.units[candidate.kind].class == "military"
            })
            .count() as f64;
        let heal = spec.effects.get("adjacent_heal").copied().unwrap_or(0.0);
        let movement = spec
            .effects
            .get("adjacent_movement")
            .copied()
            .unwrap_or(0.0);
        let logistics_value = if heal > 0.0 {
            heal * 12.0 + wounded.min(4.0) * 85.0 + movement * 210.0
        } else {
            0.0
        };
        let value = breach_value.max(siege_value).max(logistics_value);
        if value > 0.0 {
            value
                + if plan.strategy == GrandStrategy::Conquest {
                    140.0
                } else {
                    0.0
                }
                + self.conquest_breaker_value(g, pid, cid, unit)
        } else {
            -10_000.0
        }
    }

    /// A damaged outer wall is a better emergency queue than recruiting a
    /// defender when Firaxis's three-turn post-attack cooldown has expired.
    /// Keep the host's legality test authoritative: during an active attack
    /// the repair project is not available and the siege picker must still
    /// choose a unit or a wall upgrade instead.
    fn outer_defense_repair_item(g: &Game, pid: usize, city: u32) -> Option<Item> {
        let repair = Item::Project {
            project: Name::new("repair_outer_defenses"),
        };
        g.can_produce(pid, city, &repair).then_some(repair)
    }

    pub(super) fn preemptive_major_war_defense_item(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
        threatened_city: Option<u32>,
        active_major_war: bool,
        imminent_attack: bool,
    ) -> Option<Item> {
        if !active_major_war || threatened_city != Some(city) {
            return None;
        }
        if imminent_attack {
            if let Some(unit) = self
                .base
                .best_military(g, pid, city, Some(false))
                .or_else(|| self.base.best_military(g, pid, city, None))
            {
                let defender = Item::Unit {
                    unit: Name::new(&unit),
                };
                let wall = Item::Building {
                    building: crate::name!("walls"),
                };
                // Native Ephesus t71 offered both in two turns. Imminence
                // alone does not make a unit arrive before fortification.
                // Unknown timings retain the existing defender fallback.
                let valid_turns = |item: &Item| {
                    g.host_production_turns(city, item)
                        .filter(|turns| turns.is_finite() && *turns > 0.0)
                };
                if g.can_produce(pid, city, &wall)
                    && valid_turns(&wall)
                        .zip(valid_turns(&defender))
                        .is_some_and(|(wall, defender)| wall <= defender)
                {
                    return Some(wall);
                }
                return Some(defender);
            }
        }
        for building in ["walls", "medieval_walls", "renaissance_walls"] {
            let wall = Item::Building {
                building: Name::new(building),
            };
            if g.can_produce(pid, city, &wall) {
                return Some(wall);
            }
        }
        self.base
            .best_military(g, pid, city, Some(false))
            .or_else(|| self.base.best_military(g, pid, city, None))
            .map(|unit| Item::Unit {
                unit: Name::new(&unit),
            })
    }

    /// Complete a one-turn Campus building behind intact walls before a
    /// slow defender when the existing imminent-attack check finds no threat.
    /// That check uses visible attack reach and a strength threshold.
    /// Use both native completion quotes; an unknown timing preserves the
    /// existing emergency handoff. This does not defer wall construction,
    /// repairs, actual city damage, or threats that pass that existing check.
    pub(super) fn campus_finishes_before_slow_defender(
        &self,
        g: &Game,
        pid: usize,
        city: u32,
        committed: Option<&Item>,
        defence: &Item,
    ) -> bool {
        if !self.victory_planning
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || g.city_max_wall_hp(&g.cities[&city]) <= 0
            || !matches!(defence, Item::Unit { .. })
        {
            return false;
        }
        let Some(item @ Item::Building { building }) = committed else {
            return false;
        };
        if !g.can_produce(pid, city, item)
            || !g
                .rules
                .buildings
                .get(building)
                .and_then(|spec| spec.district)
                .is_some_and(|district| g.district_family(district) == "campus")
        {
            return false;
        }
        let valid_turns = |item: &Item| {
            g.host_production_turns(city, item)
                .filter(|turns| turns.is_finite() && *turns > 0.0)
        };
        valid_turns(item)
            .zip(valid_turns(defence))
            .is_some_and(|(campus, defender)| campus <= 1.0 && defender > 2.0)
            && !Self::imminent_city_attack(g, pid, city, &g.player_vision_frame(pid))
    }

    /// Let a confirmed city-siege treatment interrupt one unsafe queue.
    /// `BasicAi::besieged_city_item` normally reaches only an empty queue
    /// through `pick_item`, while Recovery's strategic governor skips a
    /// commitment at its safe default preemption margin. That left a city
    /// already losing health able to finish a Campus before it could ask for
    /// the walls or defender the existing evaluator selected. It also left a
    /// confirmed major-war raiding party free to spend a turn breaking the
    /// walls while the city finished a Builder.
    ///
    /// This is deliberately not a second siege heuristic: the established
    /// evaluator still supplies the visibility/damage proof and the exact
    /// defensive item. Before damage, that evaluator proves a multi-unit siege.
    /// A major war is sufficient proof on its own; outside one, require that
    /// the current strategic plan independently names this city as threatened.
    /// That admits the multi-unit barbarian siege the live seat can actually
    /// see, without turning two passing scouts near an otherwise healthy city
    /// into a destructive queue switch.
    /// `release_foreign_production` can also deliberately clear a host-owned
    /// queue just before this pass. The newly empty city needs the same
    /// handoff, or the ordinary governor refills it before the siege evaluator
    /// can act.
    ///
    /// The opt-in `native-emergency-purchase` family likewise answers a city
    /// that has actually been hit. Its Gold purchase is immediate when the
    /// treasury can pay, but a city must still reclaim unsafe production when
    /// it cannot. Reuse the selected family's exact signal here instead of
    /// turning a low treasury into a second threat heuristic.
    ///
    /// Domination also reuses its wall-research approach warning to prepare
    /// an exposed city before damage, even when another city is the plan's
    /// named threat. The same legal wall-versus-defender choice applies.
    /// All paths preserve fortification repairs, walls, and local defenders,
    /// and claim at most one unsafe queue each turn.
    pub(super) fn redirect_unsafe_city_queue_for_defense(
        &self,
        g: &mut Game,
        pid: usize,
        threatened_city: Option<u32>,
    ) -> Option<(u32, Item)> {
        let active_major_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        });
        // `siege-preempts-the-queue` already supplies an operator-armed
        // local-defence contract for a barbarian ring. Reuse that contract for
        // the narrower rival-war case only when the plan has named this city,
        // the battlefront observation is live, and the war is active. This
        // keeps the default and frozen controllers byte-identical while making
        // the existing major-war branch reachable for the deployed live arm
        // that currently carries `siege-preempts-the-queue`.
        let armed_major_war_threat = self.base.siege_preempts_the_queue
            && self.battlefront_observation
            && active_major_war
            && threatened_city.is_some();
        if !self.base.garrison_under_fire
            && !self.native_emergency_purchase_on()
            && !armed_major_war_threat
        {
            return None;
        }
        let visible = armed_major_war_threat.then(|| self.battlefront_visibility(g, pid));
        // The plan names only one threatened city. A second exposed city
        // still needs lead time when a stronger visible attacker approaches.
        // Reuse the wall-research warning and keep the existing one-queue
        // arbitration, legal defender choice, and completed-defense guard.
        let approach_visible = (self.victory_planning
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && active_major_war)
            .then(|| g.player_vision_frame(pid));
        let mut best: Option<(i32, u32, Option<Item>, Item)> = None;
        let mut retained: Option<(i32, u32, Item)> = None;
        let mut counter_rescue: Option<(i32, u32, Option<Item>, Item)> = None;
        for city in g.player_city_ids(pid) {
            let committed = g.cities[&city].queue.first().cloned();
            let siege_defence = self.base.besieged_city_item(g, pid, city);
            let native_defence = self.native_emergency_item(g, pid, city);
            let preemptive_defence = self.preemptive_major_war_defense_item(
                g,
                pid,
                city,
                threatened_city,
                active_major_war,
                armed_major_war_threat
                    && threatened_city == Some(city)
                    && visible
                        .as_ref()
                        .is_some_and(|visible| Self::imminent_city_attack(g, pid, city, visible)),
            );
            let defence = siege_defence
                .or(native_defence)
                .or(preemptive_defence)
                .or_else(|| {
                    let visible = approach_visible.as_ref()?;
                    let walled_original_capital = g.cities.get(&city).is_some_and(|center| {
                        center.is_capital
                            && center.original_owner == pid
                            && g.city_max_wall_hp(center) > 0
                    });
                    if !Self::strong_attacker_approaches_unwalled_city(g, pid, city, visible)
                        && !(walled_original_capital
                            && Self::strong_attacker_approaches_city(g, pid, city, visible, 6))
                    {
                        return None;
                    }
                    self.preemptive_major_war_defense_item(
                        g,
                        pid,
                        city,
                        Some(city),
                        active_major_war,
                        true,
                    )
                });
            let Some(defence) = defence else {
                continue;
            };
            // `besieged_city_item` intentionally answers with a defender once
            // walls already exist. A wounded wall is the shorter local rescue
            // when its repair project is legal, so put that answer ahead of a
            // multi-turn unit without weakening the evidence gate above.
            let defence = Self::outer_defense_repair_item(g, pid, city).unwrap_or(defence);
            let counter = self.spear_for_local_horse(g, pid, city, &defence, committed.as_ref());
            let damage = CITY_MAX_HP.saturating_sub(g.cities[&city].hp);
            let wall_damage = g
                .city_max_wall_hp(&g.cities[&city])
                .saturating_sub(g.cities[&city].wall_hp);
            // Before it is damaged, `besieged_city_item` only supplies a
            // defender after seeing a real multi-unit raiding party. That is
            // enough proof to reclaim one queue during a major war. Outside a
            // major war, the plan's independent threat call supplies the extra
            // proof that this is an active siege rather than passing scouts.
            let confirmed_non_major_siege = threatened_city == Some(city);
            if damage == 0 && wall_damage == 0 && !active_major_war && !confirmed_non_major_siege {
                continue;
            }
            let total_damage = damage.saturating_add(wall_damage);
            // A fresh live frame can already contain the defender selected on
            // the preceding frame. Renew its authority while the same threat
            // evidence holds, before diplomacy can remove the active war.
            // Unsafe queues retain priority over this no-op reservation.
            // Bogotá 117f0 had one Library turn left behind intact walls,
            // versus six turns for the selected defender. Keep the existing
            // commitment authoritative through the later production passes.
            let finish_campus = total_damage == 0
                && self.campus_finishes_before_slow_defender(
                    g,
                    pid,
                    city,
                    committed.as_ref(),
                    &defence,
                );
            // `paused-district-finishes`: an undamaged city one or two turns
            // from a paused foundation keeps it, or is left for the claim.
            let paused_finish = (total_damage == 0)
                .then(|| self.paused_district_finish_item(g, pid, city))
                .flatten()
                .map(|(item, _, _)| item);
            if let Some(item) = committed.as_ref().filter(|item| {
                (Self::active_queue_answers_siege(g, item)
                    || finish_campus
                    || paused_finish.as_ref() == Some(*item))
                    && g.can_produce(pid, city, item)
            }) {
                // A fresh defensive head can be refined, but only after all
                // existing unsafe-queue rescues. Keep the same damage/city
                // priority among retained defensive reservations.
                if let Some(counter) = counter.filter(|_| Self::active_queue_answers_siege(g, item))
                {
                    if counter_rescue
                        .as_ref()
                        .is_none_or(|(old_damage, old_city, _, _)| {
                            total_damage > *old_damage
                                || (total_damage == *old_damage && city < *old_city)
                        })
                    {
                        counter_rescue = Some((total_damage, city, committed.clone(), counter));
                    }
                    continue;
                }
                if retained.as_ref().is_none_or(|(old_damage, old_city, _)| {
                    total_damage > *old_damage || (total_damage == *old_damage && city < *old_city)
                }) {
                    retained = Some((total_damage, city, item.clone()));
                }
                continue;
            }
            if paused_finish.is_some() {
                continue;
            }

            let defence = counter.unwrap_or(defence);
            if best.as_ref().is_none_or(|(best_damage, best_city, _, _)| {
                total_damage > *best_damage || (total_damage == *best_damage && city < *best_city)
            }) {
                best = Some((total_damage, city, committed, defence));
            }
        }

        let rescue = best.or_else(|| {
            counter_rescue.filter(|(damage, city, _, _)| {
                retained.as_ref().is_none_or(|(old_damage, old_city, _)| {
                    damage > old_damage || (damage == old_damage && city < old_city)
                })
            })
        });
        let Some((damage, city, committed, defence)) = rescue else {
            return retained.map(|(_, city, item)| (city, item));
        };
        let city_name = g.cities[&city].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: defence.clone(),
            },
        )
        .is_ok()
        {
            if self.journal().wants(crate::reasoning::Level::Decision) {
                if let Some(committed) = committed {
                    think!(self.journal(), Economy, Decision,
                        "{} pauses {} for {}", city_name, Self::plain_item(&committed), Self::plain_item(&defence);
                        "under fire or facing a confirmed siege with {damage} combined city/wall health missing; this is the highest-priority unsafe queue");
                } else {
                    think!(self.journal(), Economy, Decision,
                        "{} starts {}", city_name, Self::plain_item(&defence);
                        "under fire or facing a confirmed siege with {damage} combined city/wall health missing after a host queue release");
                }
            }
            return Some((city, defence));
        }
        None
    }

    /// A Science project earns useful Great Person points, but it is not worth
    /// preserving while several host-observed cities are deep in the unhappy
    /// bands. The baseline governor already asks for an Entertainment Complex;
    /// this separately gated handoff makes that request reachable when
    /// strategic production has occupied every queue with repeatable projects.
    ///
    /// A new district starts only at the deliberately conservative,
    /// empire-wide threshold of two cities at -3 or worse. Once that district
    /// is underway, one remaining severe city may still complete its direct
    /// amenity building. Recovery, a one-off project, a unit, an active
    /// district, and a victory project retain their existing owners. During an
    /// active Conquest war, a separately stricter broad-pressure arm may
    /// reserve one unthreatened city's amenity district before every usable
    /// specialty slot becomes a non-amenity district; it can pause only a
    /// repeatable project when that reservation needs a queue. One repair
    /// chain at a time keeps the rest of the research economy working and
    /// prevents a crisis from turning into a blanket Entertainment Complex
    /// build-out.
    pub(super) fn redirect_repeatable_projects_for_amenity_crisis(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        broad_wartime_only: bool,
    ) {
        let active_major_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        });
        if !self.amenity_project_preemption_on() || plan.strategy == GrandStrategy::Recovery {
            return;
        }

        let city_ids = g.player_city_ids(pid);
        let city_count = city_ids.len();
        // Each surplus values the whole empire's luxury allocation; one memo
        // shares it across the cities.
        let shortfalls: Vec<(u32, i64)> = {
            let _memo = g.query_memo();
            city_ids
                .iter()
                .map(|cid| (*cid, (-g.city_amenity_surplus(&g.cities[cid])).max(0)))
                .collect()
        };
        let short_cities = shortfalls
            .iter()
            .filter(|(_, shortfall)| *shortfall > 0)
            .count();
        let total_shortfall: i64 = shortfalls.iter().map(|(_, shortfall)| *shortfall).sum();
        let severe: Vec<(u32, i64)> = shortfalls
            .iter()
            .copied()
            .filter(|(_, shortfall)| *shortfall >= 3)
            .collect();
        // The ordinary handoff protects all active Conquest queues. Its one
        // exception is deliberately much stricter than the peaceful -3 band:
        // at least three quarters of a four-or-more-city empire must already
        // be short, with an aggregate deficit of all but one city. This catches
        // the turn-88 shape from `civvis-20260815T230003Z` (three of four
        // cities at -1), while a local wartime problem stays with normal
        // production and the policy deck.
        let broad_wartime_conquest = active_major_war
            && plan.strategy == GrandStrategy::Conquest
            && city_count >= 4
            && short_cities.saturating_mul(4) >= city_count.saturating_mul(3)
            && total_shortfall >= city_count.saturating_sub(1) as i64;
        // The pre-production call exists solely to preserve an amenity slot
        // before the Conquest governor fills an idle queue or turns a
        // repeatable project into a unit. The ordinary late call retains its
        // peaceful severe-crisis role.
        if broad_wartime_only && !broad_wartime_conquest {
            return;
        }
        if (active_major_war || plan.strategy == GrandStrategy::Conquest) && !broad_wartime_conquest
        {
            return;
        }
        if severe.is_empty() && !broad_wartime_conquest {
            return;
        }
        let may_start_district = severe.len() >= 2 || broad_wartime_conquest;

        let is_amenity_repair = |item: &Item| match item {
            Item::District { district, .. } => g.rules.districts[district].amenity > 0.0,
            Item::Building { building } => g.rules.buildings[building].amenity > 0.0,
            _ => false,
        };
        // An item already in the chain owns the single reservation until it
        // completes. The next turn can then choose its direct amenity building.
        if city_ids
            .iter()
            .any(|cid| g.cities[cid].queue.first().is_some_and(is_amenity_repair))
        {
            return;
        }

        let mut best: Option<(u8, i64, f64, u32, String, Item)> = None;
        for cid in city_ids {
            let shortfall = (-g.city_amenity_surplus(&g.cities[&cid])).max(0);
            if shortfall == 0 {
                continue;
            }
            let city = &g.cities[&cid];
            let threatened = plan.threatened_city == Some(cid)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4);
            // The wartime exception never replaces production in a city the
            // defense governor has named or that the host says was just hit.
            if broad_wartime_conquest && threatened {
                continue;
            }
            let redirectable = match g.cities[&cid].queue.first() {
                Some(Item::Project { project }) => {
                    let spec = &g.rules.projects[project];
                    spec.repeatable
                        && (!spec.completion_gpp.is_empty() || !spec.ongoing_yields.is_empty())
                }
                None => true,
                _ => false,
            };
            if !redirectable {
                continue;
            }
            for item in g.producible_items(pid, cid) {
                let (stage, amenity) = match &item {
                    Item::Building { building } => (1, g.rules.buildings[building].amenity),
                    Item::District { district, .. } => (0, g.rules.districts[district].amenity),
                    _ => continue,
                };
                // A district begins only in the deep band that triggered the
                // ordinary emergency. The broad wartime arm is the one
                // exception: its empire-level threshold is stricter, and it
                // must reserve a remaining slot while its local city is still
                // at -1. Its direct amenity building may finish after the
                // district has already lifted that city into the -1/-2 band.
                if amenity <= 0.0
                    || (stage == 0
                        && (!may_start_district || (shortfall < 3 && !broad_wartime_conquest)))
                {
                    continue;
                }
                let key = format!("{item:?}");
                let replace = best.as_ref().is_none_or(
                    |(old_stage, old_shortfall, old_amenity, old_city, old_key, _)| {
                        stage > *old_stage
                            || (stage == *old_stage
                                && (shortfall > *old_shortfall
                                    || (shortfall == *old_shortfall
                                        && (amenity > *old_amenity + f64::EPSILON
                                            || ((amenity - *old_amenity).abs() <= f64::EPSILON
                                                && (cid, key.as_str())
                                                    < (*old_city, old_key.as_str()))))))
                    },
                );
                if replace {
                    best = Some((stage, shortfall, amenity, cid, key, item));
                }
            }
        }

        let Some((_, _, _, city, _, item)) = best else {
            return;
        };
        // See `amenity_project_preemption_2`: the treasury pays for a
        // building when it can, and the queue keeps its project. The engine
        // prices the purchase and refuses it when Gold cannot cover it.
        if self.amenity_project_preemption_2 {
            if let Item::Building { building } = &item {
                if g.apply(
                    pid,
                    &Action::BuyBuilding {
                        city,
                        building: *building,
                        currency: "gold".to_string(),
                    },
                )
                .is_ok()
                {
                    think!(self.journal(), Economy, Decision,
                           "{} buys {} for an amenity crisis", g.cities[&city].name, building;
                           "the treasury covers the repair, so the queue keeps its project";
                           g.cities[&city].pos);
                    return;
                }
            }
        }
        let prior = g.cities[&city].queue.first().cloned();
        let city_name = g.cities[&city].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_ok()
            && self.journal().wants(crate::reasoning::Level::Decision)
        {
            let crisis = if broad_wartime_conquest {
                "broad wartime Amenity pressure"
            } else {
                "severe Amenity crisis"
            };
            match prior {
                Some(prior) => think!(self.journal(), Economy, Decision,
                    "{} pauses {} for {}", city_name, Self::plain_item(&prior), Self::plain_item(&item);
                    "{}: {} of {} cities short, {} total Amenities missing",
                    crisis, short_cities, city_count, total_shortfall),
                None => think!(self.journal(), Economy, Decision,
                    "{} starts {} for {}", city_name, Self::plain_item(&item), crisis;
                    "{} of {} cities short, {} total Amenities missing",
                    short_cities, city_count, total_shortfall),
            }
        }
    }

    /// The threshold used by the idle-city Entertainment Complex path.
    pub(super) fn widespread_amenity_crisis(g: &Game, pid: usize) -> bool {
        Self::amenity_pressure(g, pid).is_some_and(|(city_count, short_cities, total_shortfall)| {
            short_cities * 2 >= city_count && total_shortfall >= city_count as i64
        })
    }

    /// Give one idle city the next Entertainment Complex stage when a
    /// host-observed deficit is widespread. The existing crisis handoff
    /// intentionally leaves Conquest and active wars alone because it would
    /// replace an in-flight project; this narrower route has no such trade:
    /// it only fills a queue that the higher-priority defense and Great Person
    /// passes already left empty.
    ///
    /// One reservation at a time matters here. If the empire has not started
    /// an Entertainment Complex, its first reservation is the district;
    /// otherwise it is the next direct building (normally an Arena). That
    /// makes the already-priced regional path reachable before ordinary
    /// strategic production fills every queue, without turning a broad -10%
    /// yield band into a blanket infrastructure pivot.
    /// A depleted treasury with negative income gives this selected idle
    /// queue to an available ordinary recovery build first. Without one,
    /// the Amenity repair still runs.
    pub(super) fn reserve_idle_entertainment_path_for_widespread_crisis(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.amenity_project_preemption_on() {
            return;
        }

        let city_ids = g.player_city_ids(pid);
        if !Self::widespread_amenity_crisis(g, pid) {
            return;
        }
        let shortfalls: Vec<(u32, i64)> = city_ids
            .iter()
            .map(|cid| (*cid, (-g.city_amenity_surplus(&g.cities[cid])).max(0)))
            .collect();
        let short_cities = shortfalls
            .iter()
            .filter(|(_, shortfall)| *shortfall > 0)
            .count();
        let total_shortfall: i64 = shortfalls.iter().map(|(_, shortfall)| *shortfall).sum();

        let is_direct_entertainment_repair = |item: &Item| match item {
            Item::Building { building } => {
                let spec = &g.rules.buildings[building];
                spec.amenity > 0.0
                    && spec
                        .district
                        .map(|district| g.district_family(district) == "entertainment_complex")
                        .unwrap_or(false)
            }
            _ => false,
        };
        let has_entertainment_complex = city_ids.iter().any(|cid| {
            g.cities[cid]
                .districts
                .keys()
                .any(|district| g.district_family(*district) == "entertainment_complex")
        });
        // An Entertainment Complex, Arena, Zoo, or Stadium already underway
        // owns this one path reservation. Do not create a competing copy while
        // Firaxis still reports the earlier build as current.
        if city_ids.iter().any(|cid| {
            g.cities[cid].queue.first().is_some_and(|item| {
                is_direct_entertainment_repair(item)
                    || matches!(item, Item::District { district, .. }
                            if g.district_family(*district) == "entertainment_complex")
            })
        }) {
            return;
        }

        // Prefer a direct building when the district already exists. Otherwise
        // reserve precisely one legal Entertainment Complex; it is the missing
        // first stage that lets the existing Arena/Zoo path ever bind. Within a
        // stage prefer the deepest local shortfall, then the greatest immediate
        // amenity lift, then the fastest legal completion.
        let mut best: Option<(u8, i64, f64, f64, u32, String, Item)> = None;
        for (cid, shortfall) in shortfalls {
            if shortfall == 0 || !g.cities[&cid].queue.is_empty() {
                continue;
            }
            let city = &g.cities[&cid];
            let threatened = plan.threatened_city == Some(cid)
                || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4);
            if threatened {
                continue;
            }
            // See `campus_first_veto`: the crisis path may not make an
            // Entertainment Complex a city's first specialty district.
            if self.campus_first_veto(g, city, "entertainment_complex") {
                continue;
            }
            let production = g.city_yields(cid).production.max(1.0);
            for item in g.producible_items(pid, cid) {
                let (stage, amenity) = match &item {
                    Item::Building { building } if is_direct_entertainment_repair(&item) => {
                        (1, g.rules.buildings[building].amenity)
                    }
                    Item::District { district, pos }
                        if !has_entertainment_complex
                            && g.district_family(*district) == "entertainment_complex" =>
                    {
                        (0, g.district_amenity(district, *pos))
                    }
                    _ => continue,
                };
                let turns = g.item_remaining_cost_for_city(pid, cid, &item) / production;
                let key = format!("{item:?}");
                let replace = best.as_ref().is_none_or(
                    |(old_stage, old_shortfall, old_amenity, old_turns, old_city, old_key, _)| {
                        stage > *old_stage
                            || (stage == *old_stage
                                && (shortfall > *old_shortfall
                                    || (shortfall == *old_shortfall
                                        && (amenity > *old_amenity + f64::EPSILON
                                            || ((amenity - *old_amenity).abs() <= f64::EPSILON
                                                && (turns + f64::EPSILON < *old_turns
                                                    || ((turns - *old_turns).abs()
                                                        <= f64::EPSILON
                                                        && (cid, key.as_str())
                                                            < (*old_city, old_key.as_str()))))))))
                    },
                );
                if replace {
                    best = Some((stage, shortfall, amenity, turns, cid, key, item));
                }
            }
        }

        let Some((_, _, _, _, city, _, item)) = best else {
            return;
        };
        let recovery = if g.players[pid].gold_per_turn < -0.5
            && g.players[pid].gold < 100.0 + 25.0 * city_ids.len() as f64
        {
            self.base
                .economic_recovery_item(g, pid, city, self.counts(g, pid).traders)
        } else {
            None
        };
        let recovering = recovery.is_some();
        let item = recovery.unwrap_or(item);
        let city_name = g.cities[&city].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_ok()
            && self.journal().wants(crate::reasoning::Level::Decision)
        {
            if recovering {
                think!(self.journal(), Economy, Decision,
                    "{} starts {} for treasury recovery", city_name, Self::plain_item(&item);
                    "{:.0} Gold and {:.1} Gold/turn: the available recovery build precedes a new Amenity reservation",
                    g.players[pid].gold, g.players[pid].gold_per_turn);
            } else {
                think!(self.journal(), Economy, Decision,
                    "{} starts {} for widespread Amenity pressure", city_name, Self::plain_item(&item);
                    "{} of {} cities short, {} total Amenities missing; no Entertainment Complex stage was underway",
                    short_cities, city_ids.len(), total_shortfall);
            }
        }
    }
}
