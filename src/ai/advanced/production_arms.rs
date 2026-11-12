//! The largest arms of `production_value`: what a District, a Wonder or a
//! Project is worth to a city.
//!
//! `AdvancedAi::production_value` prices every item a city could build with
//! one `match` on the item. These three arms ran to 950 lines of it, so each
//! is a method here. `production_value` reads the city once and hands the
//! result over in a [`ProductionFrame`], so each arm still reads `turns`,
//! `threatened` and the rest under the names it always had. The arms' bodies
//! are unchanged.
//!
//! The District arm's `return -10_000.0` is a refusal. It still ends the
//! pricing: `production_value` returns any value at or below `-9_999.0`
//! before it applies anything else.
//!
//! Many of the helpers these arms call are in `advanced/production_values.rs`.

use super::*;

/// What `production_value` has already read about the city and the item, for
/// the arms that live in this module.
#[derive(Clone, Copy)]
pub(super) struct ProductionFrame<'a> {
    pub(super) g: &'a Game,
    pub(super) pid: usize,
    pub(super) cid: u32,
    pub(super) item: &'a Item,
    pub(super) plan: &'a StrategicPlan,
    pub(super) counts: &'a EmpireCounts,
    pub(super) city: &'a crate::game::City,
    pub(super) city_count: usize,
    pub(super) specialization_active: bool,
    pub(super) turns: f64,
    pub(super) remaining_turns: f64,
    pub(super) threatened: bool,
    pub(super) strategic_land_force_gap: bool,
}

impl AdvancedAi {
    /// The `Item::District` arm: what `district` on the plot `pos` is worth.
    pub(super) fn district_production_value(
        &self,
        frame: &ProductionFrame<'_>,
        district: &Name,
        pos: &Pos,
    ) -> f64 {
        let ProductionFrame {
            g,
            pid,
            cid,
            item,
            plan,
            counts,
            city,
            city_count,
            specialization_active,
            threatened,
            ..
        } = *frame;
        if self.culture_preserves_resort_site(g, pid, cid, *district, *pos) {
            return -10_000.0;
        }
        let spec = &g.rules.districts[district];
        let family = g.district_family(*district);
        let science_harbor_reserved = Self::science_harbor_reserved(g, pid);
        let pricing_lane = self.campus_pricing_lane(family.as_str(), plan.strategy);
        // See `campus_first_veto`: the first specialty district of a
        // Science seat's city is the Campus, never the Entertainment
        // Complex the Amenity paths credit its unbuilt Arena.
        if self.campus_first_veto(g, city, family.as_str()) {
            return -10_000.0;
        }
        // See `trade_route_network`: a Commercial Hub (or, on a coast
        // with no Hub, a Harbor) beside a standing Campus is the
        // Science lane's trade capacity, not a generic Gold district.
        let trade_network_district = self.trade_route_network
            && Self::city_holds_district_family(g, city, "campus")
            && (family == "commercial_hub"
                || (family == "harbor"
                    && !Self::city_holds_district_family(g, city, "commercial_hub")));
        // A named Science seat has a narrower district contract than
        // the adaptive stock lane. Once Campus buildings are caught
        // up, generic Culture/Gold/Faith districts otherwise remain
        // legal argmax candidates and can consume the queue before
        // Electricity unlocks the Research Lab. Keep genuinely
        // enabling infrastructure (Campus, Industrial Zone,
        // Spaceport, production/housing/defense districts, and the
        // first Government Plaza/Diplomatic Quarter) available.
        // Harbors are useful expansion infrastructure in the opening,
        // especially when several coastal cities can exploit them;
        // only the late specialized lane bounds them. Holy Sites
        // remain outside the Science lane.
        if self.victory_target == Some(VictoryTarget::Science) && family == "holy_site" {
            return -10_000.0;
        }
        if self.victory_target == Some(VictoryTarget::Science)
            && specialization_active
            && matches!(
                family.as_str(),
                "theater_square" | "commercial_hub" | "preserve"
            )
            && !trade_network_district
        {
            return -10_000.0;
        }
        if self.victory_target == Some(VictoryTarget::Science)
            && specialization_active
            && family == "harbor"
            && science_harbor_reserved
            && !trade_network_district
        {
            return -10_000.0;
        }
        if family == "spaceport" {
            if self.space_race_excluded() {
                return -10_000.0;
            }
            // `first-pad-in-the-forge`: the empire's first pad only in a
            // producer. See `ai/first_pad_in_the_forge.rs`.
            if !self.base.first_pad_city_allowed(g, pid, cid) {
                return -10_000.0;
            }
            let races_science = self.science_drive_active()
                || self.space_race_lane(g, pid)
                || self.raced_target() == Some(VictoryTarget::Science)
                || self.adaptive_science_spaceport_lane(g, pid);
            let current_queued_spaceport = city.queue.first().is_some_and(|queued| {
                queued == item
                    && matches!(queued, Item::District { district, .. }
                        if g.district_family(*district) == "spaceport")
            });
            // See `launch_pad_keeps_its_rung`: resuming the city's own
            // placed pad is not a second pad.
            let resumes_own_foundation = self.launch_pad_keeps_its_rung
                && matches!(item, Item::District { pos, .. }
                if g.map
                    .get(*pos)
                    .and_then(|tile| tile.district_foundation.as_ref())
                    .is_some_and(|foundation| {
                        g.district_family(foundation.district) == "spaceport"
                    }));
            if Self::city_has_spaceport(g, cid)
                || (Self::city_has_spaceport_foundation(g, cid)
                    && !current_queued_spaceport
                    && !resumes_own_foundation)
            {
                // Multiple Spaceports are rules-legal, but one city
                // can execute only one project at a time. Put
                // additional launch sites in other cities for actual
                // parallelism. A placed foundation is already that
                // city's launch-site commitment, even after its queue
                // was switched to another item.
                return -10_000.0;
            }
            if races_science
                && !current_queued_spaceport
                && !self.science_spaceport_city_is_admitted(g, pid, cid)
            {
                // The generic sweep visits every city. Without this
                // hard gate each later city can still spend its queue
                // on the ordinary 250-point Spaceport fallback after
                // the race has already reserved its current
                // launch-site budget.
                return -10_000.0;
            }
        }
        let district_count = g
            .cities
            .values()
            .filter(|candidate| {
                candidate.owner == pid
                    && candidate
                        .districts
                        .keys()
                        .any(|built| g.district_family(*built) == family)
            })
            .count();
        let spaceport_count = if family == "spaceport" {
            Some(Self::science_spaceport_commitments(g, pid))
        } else {
            None
        };
        // ⚠⚠ THIS IS A CLIFF AT HALF THE EMPIRE, AND THE EMPIRE STOPS
        // THERE. `district_count` is how many of MY cities hold this
        // family, so the bonus vanishes the moment half of them do.
        // Measured over 19 live runs, end-of-game Campus coverage is
        // **exactly 50 of 100 cities** — the empire builds Campuses
        // until it is told half is enough, and then stops.
        //
        // Defensible for a Harbor or an Industrial Zone, whose effects
        // are empire-wide. Not for the district the whole science chain
        // hangs off: every Campus carries its OWN Library and
        // University slots, and the funnel cascades from it — 50%
        // Campus, 39% Library, 20% University, **3% Research Lab**.
        // Per-city science correlates 0.709 with a Library and 0.629
        // with a Campus over 13,004 city-turns.
        //
        // So the Campus is exempted from the cliff and keeps asking in
        // every city. The other four families are untouched.
        let core_capped = district_count * 2 >= city_count;
        // ★★★ REPAIRED 2026-08-19 after the 4,000-pair native screen
        // priced the unconditional exemption at −2.8 pp wins (z −3.7,
        // share Δ ~0, seeds 40000000..): "keeps asking in every city"
        // includes the size-three frontier town whose Library the
        // funnel argument above can never staff, and the 130-point ask
        // beat that town's growth buildings in a regime decided at a
        // median of t134. Beyond the half-empire cliff the Campus now
        // keeps asking only in a city big enough to feed the funnel —
        // [`CAMPUS_EVERY_CITY_POP_FLOOR`] — so the coverage promise
        // stays (no city is EVER told half the empire is enough once
        // it can staff a Library) while the towns keep compounding
        // until they qualify. Below the cliff nothing changes.
        // `campus-before-halfway` wires that floor; withheld, the
        // literal `false` of the 2026-08-19 repair stands.
        let campus_keeps_asking = self.campus_before_halfway
            && family == "campus"
            && city.pop >= CAMPUS_EVERY_CITY_POP_FLOOR;

        let balanced_core = if self.first_district_first {
            // `first-district-first`: a slope in place of the cliff.
            // Coverage measured at t150 lands exactly on the cliff —
            // Campus 0.56 of cities, Commercial Hub 0.33, Theatre 0.27
            // — so the ask now shrinks with coverage instead of
            // vanishing at half, and the Campus keeps asking outright
            // in a city of four, the floor the 2026-08-19 repair named
            // and never wired (`campus_keeps_asking` is a literal).
            let base = match family.as_str() {
                "campus" | "theater_square" | "commercial_hub" => 130.0,
                "harbor" | "industrial_zone" => 90.0,
                _ => 0.0,
            };
            if family == "campus" && city.pop >= 4 {
                base
            } else {
                let coverage = district_count as f64 / city_count.max(1) as f64;
                base * (1.0 - 0.5 * coverage.min(1.0))
            }
        } else if !core_capped || campus_keeps_asking {
            match family.as_str() {
                "campus" | "theater_square" | "commercial_hub" => 130.0,
                "harbor" | "industrial_zone" => 90.0,
                _ => 0.0,
            }
        } else {
            0.0
        };

        // Evaluate the rules engine's actual post-construction
        // housing rather than duplicating Aqueduct water rules or
        // the appeal bands used by Neighborhoods and Preserves.
        let mut developed = city.clone();
        developed.districts.insert(Name::new(district), *pos);
        let housing_gain = (g.city_housing(&developed) - g.city_housing(city)).max(0.0);
        let housing_need = (city.pop as f64 + 2.0 - g.city_housing(city)).max(0.0);
        // ★★★★ AN AMENITY DISTRICT IS WORTH THE BUILDING IT WILL HOST.
        // `district_amenity` reads the district's own spec, and an
        // Entertainment Complex carries none — its Arena's +2 arrives
        // one build later — so it scored like an empty plot and never
        // won a queue outside a crisis: no Entertainment Complex at all
        // in four of the last five live runs, 57–83% of city-turns
        // Displeased. See `amenity_district_path`.
        let amenity_gain = g.district_amenity(district, *pos)
            + if self.amenity_district_path {
                Self::hosted_amenity_path(g, pid, &family)
            } else {
                0.0
            };
        let amenity_need = (-g.city_amenity_surplus(city)).max(0) as f64;
        // ★★★★ AND AN EXPANSION DISTRICT IS WORTH THE HALL IT WILL
        // HOST. The same empty-plot defect on the other axis: a
        // Government Plaza carries no yield and no chain debt, so it
        // scores at nothing, while the building that redeems it is
        // priced at 700 + 500 by `expansion_hall` one build later. The
        // scale here is the building arm's own, so this credit and
        // that one reach zero on the same turn. See
        // `expansion_hall_district`.
        // `no-city-limit` prices the Plaza the same way while seats that
        // pay are still free.
        let expansion_path = if self.expansion_hall_district || self.no_city_limit {
            let seats_short = self
                .settlement_target(plan)
                .saturating_sub(city_count + counts.settlers) as f64;
            let scale = (seats_short / EXPANSION_HALL_FULL_SHORTFALL).clamp(0.0, 1.0);
            scale * Self::hosted_expansion_path(g, pid, &family)
        } else {
            0.0
        };
        let great_people = spec.great_person_points.values().sum::<f64>();
        let relevant_great_people = match plan.strategy {
            GrandStrategy::Science => spec
                .great_person_points
                .get("scientist")
                .copied()
                .unwrap_or(0.0),
            GrandStrategy::Culture => ["writer", "artist", "musician"]
                .into_iter()
                .map(|kind| spec.great_person_points.get(kind).copied().unwrap_or(0.0))
                .sum(),
            GrandStrategy::Religion => spec
                .great_person_points
                .get("prophet")
                .copied()
                .unwrap_or(0.0),
            GrandStrategy::Diplomacy => spec
                .great_person_points
                .get("merchant")
                .copied()
                .unwrap_or(0.0),
            GrandStrategy::Conquest => ["general", "admiral"]
                .into_iter()
                .map(|kind| spec.great_person_points.get(kind).copied().unwrap_or(0.0))
                .sum(),
            GrandStrategy::Expansion | GrandStrategy::Recovery => spec
                .great_person_points
                .get("engineer")
                .copied()
                .unwrap_or(0.0),
        };
        let effects = &spec.effects;
        let effect_value = effects.get("governor_titles").copied().unwrap_or(0.0) * 520.0
            + effects.get("envoys").copied().unwrap_or(0.0)
                * if plan.strategy == GrandStrategy::Diplomacy {
                    300.0
                } else {
                    ENVOY_PRODUCTION_VALUE
                }
            + effects
                .get("envoy_if_adjacent_city_center")
                .copied()
                .unwrap_or(0.0)
                * if g.wdist(city.pos, *pos) == 1 {
                    if plan.strategy == GrandStrategy::Diplomacy {
                        300.0
                    } else {
                        ENVOY_PRODUCTION_VALUE
                    }
                } else {
                    0.0
                }
            + effects.get("spy_defense_levels").copied().unwrap_or(0.0) * 75.0
            + effects.get("flood_protection").copied().unwrap_or(0.0) * 160.0
            + effects.get("drought_protection").copied().unwrap_or(0.0) * 55.0
            + effects.get("culture_bomb").copied().unwrap_or(0.0) * 85.0
            + effects.get("naval_passage").copied().unwrap_or(0.0)
                * if plan.strategy == GrandStrategy::Conquest {
                    150.0
                } else {
                    75.0
                }
            + effects
                .get("gold_faith_purchase_discount_pct")
                .copied()
                .unwrap_or(0.0)
                * 8.0
            + effects
                .get("corps_army_discount_pct")
                .copied()
                .unwrap_or(0.0)
                * if plan.strategy == GrandStrategy::Conquest {
                    8.0
                } else {
                    2.0
                }
            + effects.get("free_heavy_cavalry").copied().unwrap_or(0.0)
                * if plan.strategy == GrandStrategy::Conquest {
                    380.0
                } else {
                    180.0
                }
            + effects
                .get("naval_settler_production_pct")
                .copied()
                .unwrap_or(0.0)
                * if plan.strategy == GrandStrategy::Expansion {
                    4.0
                } else {
                    1.5
                }
            + effects.get("naval_heal_full").copied().unwrap_or(0.0) * 90.0
            + effects.get("naval_movement").copied().unwrap_or(0.0) * 130.0
            + effects
                .get("foreign_continent_loyalty")
                .copied()
                .unwrap_or(0.0)
                * 22.0
            + effects.get("tourism_after_flight").copied().unwrap_or(0.0)
                * if plan.strategy == GrandStrategy::Culture {
                    180.0
                } else {
                    35.0
                }
            + effects
                .get("border_growth_on_great_person")
                .copied()
                .unwrap_or(0.0)
                * 90.0
            + effects.get("unlock_apprenticeship").copied().unwrap_or(0.0) * 120.0;

        let strategic_family = match (plan.strategy, family.as_str()) {
            (GrandStrategy::Science, "spaceport") if !specialization_active => 0.0,
            // See `launch_pad_keeps_its_rung`: the pad already begun
            // keeps the first-pad rung in its own city until it
            // stands, so a claim that borrows the queue hands it back.
            (GrandStrategy::Science, "spaceport")
                if self.launch_pad_keeps_its_rung
                    && Self::unfinished_launch_pad_site(g, pid) == Some(cid) =>
            {
                3_000.0
            }
            // See `one_launch_pad`: the rung is empire-wide, so it
            // belongs to one city — the one the race would actually be
            // run in — and it lapses the moment a pad is on its way.
            (GrandStrategy::Science, "spaceport")
                if spaceport_count == Some(0)
                    && self.one_launch_pad
                    && self.launch_pad_site(g, pid) != Some(cid) =>
            {
                250.0
            }
            (GrandStrategy::Science, "spaceport") if spaceport_count == Some(0) => 3_000.0,
            // See `spaceport_surplus_veto`: a pad beyond what the
            // race stage can use is two points of district and a
            // dead queue — the flat arm below kept paying for a
            // ninth pad while the launch chain fit in two cities.
            (GrandStrategy::Science, "spaceport")
                if self.spaceport_surplus_veto
                    && spaceport_count.unwrap_or(0)
                        >= Self::science_drive_desired_pads(&g.players[pid].science_projects) =>
            {
                0.0
            }
            (GrandStrategy::Science, "spaceport") => 250.0,
            (GrandStrategy::Science, "campus") if specialization_active => 170.0,
            // `campus-before-halfway`: the lane's own district keeps
            // its arm through the first half of the clock too.
            (GrandStrategy::Science, "campus") if self.campus_before_halfway => {
                CAMPUS_BEFORE_HALFWAY_DISTRICT
            }
            (GrandStrategy::Science, "campus") => 0.0,
            // One coastal Harbor pays back through empire-wide trade
            // capacity and gold. During the foundation half the first
            // one gets a strong nudge, but additional Harbors remain
            // legal and fall back to ordinary district value. Once
            // specialization begins, the reservation makes this a
            // bounded support package.
            (GrandStrategy::Science, "harbor") if !science_harbor_reserved => 260.0,
            // See `trade_route_network`: the second and every later
            // Hub or Harbor, beside a Campus, is trade capacity.
            (GrandStrategy::Science, "commercial_hub" | "harbor") if trade_network_district => {
                TRADE_NETWORK_DISTRICT
            }
            (GrandStrategy::Conquest, "commercial_hub") if self.trade_route_network => {
                TRADE_NETWORK_CONQUEST_DISTRICT
            }
            // See `tally_culture`: the civic tree pays three a rung.
            (GrandStrategy::Science, "theater_square") if self.tally_culture => 170.0,
            (GrandStrategy::Science, "industrial_zone") => 150.0,
            (GrandStrategy::Religion, "holy_site") => {
                if self.holy_lane_parity {
                    HOLY_LANE_PARITY
                } else {
                    210.0
                }
            }
            (GrandStrategy::Culture, "theater_square") => 850.0,
            (GrandStrategy::Culture, "preserve") => 210.0,
            (GrandStrategy::Diplomacy, "diplomatic_quarter") => 360.0,
            (GrandStrategy::Diplomacy, "commercial_hub") => 150.0,
            (GrandStrategy::Diplomacy, "harbor") => 130.0,
            (GrandStrategy::Diplomacy, "theater_square") => 100.0,
            // See `air_surge_2`: the loop's wars must not starve the
            // labs that armed them. A Conquest window otherwise
            // prices no Campus at all, and a long Exploit is exactly
            // when conquered cities join the empire unlabbed. Below
            // the Encampment, so war infrastructure keeps first
            // claim.
            (GrandStrategy::Conquest, "campus") if self.air_surge_2 => 140.0,
            (GrandStrategy::Conquest, "encampment") => 170.0,
            (GrandStrategy::Conquest, "aerodrome") => 280.0,
            (GrandStrategy::Conquest, "harbor") => 150.0,
            (GrandStrategy::Conquest, "industrial_zone") => 160.0,
            (GrandStrategy::Conquest, "canal") => 120.0,
            (GrandStrategy::Recovery, "industrial_zone") => 190.0,
            (GrandStrategy::Recovery, "dam") => 180.0,
            (GrandStrategy::Recovery, "aqueduct") => 120.0,
            // See `campus_through_expansion`: the Science lane's
            // Campus arm while the Science seat's plan still expands.
            (GrandStrategy::Expansion, "campus") if pricing_lane == GrandStrategy::Science => {
                CAMPUS_THROUGH_EXPANSION_DISTRICT
            }
            (GrandStrategy::Expansion, "commercial_hub" | "harbor") if trade_network_district => {
                TRADE_NETWORK_DISTRICT
            }
            (GrandStrategy::Expansion, "commercial_hub" | "harbor") => 90.0,
            (GrandStrategy::Expansion, "aqueduct" | "neighborhood") => 110.0,
            _ => 0.0,
        };
        let first_copy = match family.as_str() {
            "government_plaza" if district_count == 0 => 420.0,
            "diplomatic_quarter" if district_count == 0 => 180.0,
            "aerodrome" if district_count == 0 && counts.aircraft > 0 => 260.0,
            _ => 0.0,
        };
        // `first-district-first`: THIS city's first specialty district.
        // `first_copy` above pays only the empire's first Government
        // Plaza, Diplomatic Quarter or Aerodrome, which is why the
        // capital's first district was the Plaza in eight of ten King
        // runs while every other city's first district waited a median
        // 19 turns behind Settlers, Walls, Archers and Granaries. A
        // town of one may still grow first; from two the district wins
        // the tie with the filler and loses to a real emergency.
        let first_district = if self.first_district_first
            && spec.specialty
            && city.districts.is_empty()
            && city.pop >= 2
        {
            180.0
        } else {
            0.0
        };
        let development_penalty = if spec.specialty
            && !city.districts.is_empty()
            && city.buildings.len() <= city.districts.len()
        {
            -120.0
        } else {
            0.0
        };
        // Campus coverage, priced in every lane. `balanced_core` above
        // already wants one Campus per two cities, but it stops there
        // and it is shared with three other families; this says the
        // quieter thing that a city without a Campus is an empire-wide
        // research hole whatever the empire is currently playing for.
        // Scaled by the horizon so a Campus begun at turn 240 does not
        // outbid a defender.
        let research_coverage = if self.research_economy
            && family == "campus"
            && !city
                .districts
                .keys()
                .any(|built| g.district_family(*built) == family)
        {
            // ⚠ A PAYBACK horizon, not a game-fraction one. See
            // `RESEARCH_CAMPUS_PAYBACK`: the old scaling priced this at
            // 0.40 with a hundred turns of compounding left, while every
            // rival term is a flat constant that never decays.
            RESEARCH_CAMPUS_COVERAGE * self.chain_horizon(g, ChainRung::District)
        } else {
            0.0
        };
        // See `campus_adjacency_threshold`: crossing 4 raw Science
        // doubles every Campus building this city will ever hold, and
        // the linear yield above cannot see a threshold.
        let adjacency_threshold = self.yield_value(
            Yields {
                science: self.campus_threshold_bonus(g, *district, *pos),
                ..Yields::default()
            },
            pricing_lane,
        ) * 42.0
            * Self::campus_payback_horizon(g);
        self.yield_value(g.district_site_yields(district, *pos), pricing_lane) * 60.0
            - self.seowon_site_loss_value(g, pid, *district, *pos, pricing_lane) * 60.0
            + adjacency_threshold
            + self.yield_value(spec.citizen_yields, pricing_lane) * 24.0
            + spec.defense * if threatened { 5.0 } else { 1.5 }
            + housing_gain * (32.0 + housing_need * 18.0)
            + amenity_gain * (55.0 + amenity_need * 35.0)
            + expansion_path
            + spec.loyalty * if city.loyalty < 76.0 { 22.0 } else { 7.0 }
            + spec.air_slots.max(0) as f64
                * if plan.strategy == GrandStrategy::Conquest || counts.aircraft > 0 {
                    95.0
                } else {
                    25.0
                }
            + spec.appeal
                * if plan.strategy == GrandStrategy::Culture {
                    35.0
                } else {
                    8.0
                }
            + great_people * 30.0
            + relevant_great_people * 85.0
            + balanced_core
            + strategic_family
            + first_copy
            + first_district
            + effect_value
            + development_penalty
            + research_coverage
            + self.culture_floor_district_bonus(g, pid, family)
            + self.gold_floor_district_bonus(g, pid, family, district_count, city_count)
            + self.science_drive_production_bonus(g, pid, cid, item)
            // `encampment-seals-the-pass`: 0.0 with the gene off and
            // for every other family. See `advanced/chokepoints.rs`.
            + self.encampment_seal_bonus(g, pid, family.as_str(), *pos)
    }

    /// The `Item::Wonder` arm: what `wonder` is worth to this city.
    pub(super) fn wonder_production_value(
        &self,
        frame: &ProductionFrame<'_>,
        wonder: &Name,
    ) -> f64 {
        let ProductionFrame {
            g,
            pid,
            cid,
            item,
            plan,
            city,
            city_count,
            turns,
            remaining_turns,
            threatened,
            ..
        } = *frame;
        let spec = &g.rules.wonders[wonder];
        let science_target = self.victory_target == Some(VictoryTarget::Science);
        let wonder_civ =
            !self.civ_blind && matches!(g.players[pid].civ.as_str(), "Egypt" | "China");
        let continuing_wonder = city.queue.first() == Some(item)
            || (self.victory_planning && g.item_invested_production(cid, item) > 0.0);
        // A Domination contract needs production for its campaign,
        // not a new race for fifteen score points. Keep an existing
        // investment's valuation so adopting the target does not
        // cancel a wonder already under construction.
        let score_race_allowed = !science_target
            && (self.victory_target != Some(VictoryTarget::Domination) || continuing_wonder);
        let already_queued = g.cities.values().any(|other| {
            (!self.victory_planning || other.owner == pid)
                && other.id != cid
                && matches!(
                    other.queue.first(),
                    Some(Item::Wonder { wonder: queued, .. }) if queued == wonder
                )
        });
        // See `lane_release_when_hopeless`: a released lane plays for
        // score, and a wonder is fifteen score points -- the largest
        // single item on the board, against two for a district and
        // three for a civic. The live King seat finished seven to the
        // leader's thirteen.
        let lane_opens = plan.strategy == GrandStrategy::Culture
            || self.victory_target == Some(VictoryTarget::Score)
            || self.lane_lost
            || (wonder_civ && self.victory_target.is_none());
        // `religion_founding_site` is the data-level marker for a
        // Stonehenge-like wonder, rather than a name check. Once this
        // civilization has founded its religion, that site and its
        // free-Prophet payoff are spent. In the live trace Rome founded
        // Buddhism at t41, then put 60/90 production into Stonehenge at
        // t65--69 before replacing it at t70. Keep this live-only: a
        // modded equivalent receives the same protection without moving
        // the frozen controller's historical wonder choices.
        let spent_religion_founding_site = self.live_wonder_race
            && g.players[pid].religion.is_some()
            && spec
                .effects
                .get("religion_founding_site")
                .is_some_and(|value| *value > 0.0);
        // The live seat's race: see `live_wonder_race`. One wonder in
        // production empire-wide, in a city with three buildings, once
        // the empire holds three cities — a race the plan does not
        // choose must not out-bid settlers, districts or defenders
        // in more than one queue at a time.
        // And only wonders no more than two eras behind the world's:
        // the one live run whose plan did open the arm asked for six
        // ANCIENT wonders at turn 129 (world era 4) and the host
        // answered `offered: 0` twenty-one times — the Firaxis rivals
        // had long since finished them, and every such refusal hands
        // that city's queue to the mod's fallback for a turn. Two eras
        // rather than one because the live seat itself runs one to two
        // eras behind the world (18 techs at world era 3, 24 at 4 on
        // run civvis-20260815T182350Z), so a one-era floor left it
        // nothing it could actually build.
        let wonder_era = spec
            .tech
            .as_deref()
            .and_then(|tech| g.rules.techs.get(tech).map(|node| node.era))
            .or_else(|| {
                spec.civic
                    .as_deref()
                    .and_then(|civic| g.rules.civics.get(civic).map(|node| node.era))
            })
            .unwrap_or(0);
        // ★★★★ ONE LANE PER SIX CITIES, NOT ONE EMPIRE-WIDE. Measured on
        // run civvis-20260816T011314Z (eight cities, t250 tally 740
        // against 1015): six wonders started, four finished, and a
        // 56-turn stretch (t128–t184) with no wonder in production
        // anywhere while the one racing city took 19–22 turns per
        // wonder. A wonder is fifteen host score points for 400–700
        // production; a Library is one for 90. With eight developed
        // cities the single lane is the binding limit, so a second
        // opens at six cities and a third at twelve — still in cities
        // with three buildings, still never in the same city twice.
        let wonders_in_flight = g
            .cities
            .values()
            .filter(|other| {
                other.owner == pid
                    && other.id != cid
                    && matches!(other.queue.first(), Some(Item::Wonder { .. }))
            })
            .count();
        // `cheapest-wonder-first`: a wonder twelve turns from done
        // in one of the empire's strongest cities is the densest
        // score production buys — fifteen points and an era-score
        // moment — and the guards below were written for the
        // ordinary race, not for a bargain.
        let bargain_race = score_race_allowed
            && self.cheapest_wonder_first
            && self.live_wonder_race
            && !lane_opens
            && plan.strategy != GrandStrategy::Recovery
            && turns <= CHEAPEST_WONDER_TURNS
            && wonder_era + 2 >= g.world_era
            && wonders_in_flight < Self::live_wonder_race_lanes(city_count).max(1)
            && self.wonder_bargain_city(g, pid, cid);
        let live_race_opens = score_race_allowed
            && (bargain_race
                || (self.live_wonder_race
                    && !lane_opens
                    && plan.strategy != GrandStrategy::Recovery
                    && city_count >= 3
                    && city.buildings.len() >= 3
                    && wonder_era + 2 >= g.world_era
                    && wonders_in_flight < Self::live_wonder_race_lanes(city_count)
                    && (!self.cheapest_wonder_first || turns <= CHEAPEST_WONDER_LANE_MAX_TURNS)));
        // See `strategic_wonder_value`. The lane the agent is actually
        // trying to win is its target when it has one, and its plan's
        // strategy when it does not — a targeted agent whose plan has
        // swung to Conquest under pressure is still playing for the
        // target, and `Recovery` refuses the lane outright below.
        // `score-seat-plays-for-points`: the Score lane's wonder at the live
        // race's price, under the live race's own development guards. Exact
        // no-op off, and off a Score target. See `advanced/score_seat.rs`.
        let score_seat_race = lane_opens
            && plan.strategy != GrandStrategy::Recovery
            && self.score_seat_wonder_race_opens(
                city_count,
                city.buildings.len(),
                wonder_era,
                g.world_era,
                wonders_in_flight,
                turns,
            );
        let strategic_lane = self
            .victory_target
            .map(VictoryTarget::strategy)
            .unwrap_or(plan.strategy);
        // ⚠ THE BARS GATE THE VALUE, NOT ONLY THE LANE. An earlier draft
        // applied them to `strategic_opens` alone and still added the
        // raw figure to the score, so a wonder the bars had just
        // refused was boosted anyway wherever some other gate had
        // already opened a lane — the live seat's race, or a Culture
        // plan. The Potala Palace is exactly that case: 1 060
        // production for one of twenty diplomatic points, refused by
        // the cost bar and then handed +1 220 by the live race. One
        // qualification, used for both.
        let strategic_value = if self.strategic_wonders {
            let value = self.strategic_wonder_value(g, pid, cid, spec, strategic_lane);
            if value >= STRATEGIC_WONDER_MIN_VALUE
                && value >= spec.cost * STRATEGIC_WONDER_VALUE_PER_COST
            {
                value
            } else {
                0.0
            }
        } else {
            0.0
        };
        // The same development guards the live race earned the hard
        // way: three cities, three buildings in this one, and one
        // concurrent lane per six cities. A wonder that decides a
        // victory is still 400 to 1 600 production, and an empire that
        // cannot afford a second Settler cannot afford it.
        let strategic_opens = self.strategic_wonders
            && !lane_opens
            && plan.strategy != GrandStrategy::Recovery
            && strategic_value > 0.0
            && city_count >= 3
            && city.buildings.len() >= 3
            && wonders_in_flight < Self::live_wonder_race_lanes(city_count);
        // See `wonder_score_tally`. The fourth gate, and the only one
        // any civilization can pass on merit: the fifteen points
        // `Game::score_parts` pays for a finished wonder, against the
        // production it costs. `&&` short-circuits on the flag, so
        // with the gene off nothing below the first conjunct is
        // evaluated and the arm is byte-identical.
        //
        // ⚠ It never stacks. `!lane_opens` and `!live_race_opens` mean
        // the tally price is added exactly where no other gate already
        // paid for the same wonder, so the Potala Palace cannot be
        // refused by one bar and handed a bonus by another — the
        // mistake the `strategic_value` comment above records.
        let tally_opens = self.wonder_score_tally
            && score_race_allowed
            && !lane_opens
            && !live_race_opens
            && plan.strategy != GrandStrategy::Recovery
            && city_count >= 3
            && city.buildings.len() >= 3
            && wonders_in_flight < Self::live_wonder_race_lanes(city_count)
            && Self::wonder_tally_value()
                + strategic_value
                + self.wonder_ordinary_value(spec, plan.strategy)
                >= spec.cost * WONDER_TALLY_MIN_DENSITY;
        if already_queued
            || threatened
            || spent_religion_founding_site
            || city.buildings.len() < 2
            || turns > remaining_turns * 0.65
            // Lane selection authorizes starting a race; a changed
            // preference alone does not invalidate its existing build.
            || !(continuing_wonder
                || lane_opens
                || live_race_opens
                || strategic_opens
                || tally_opens)
        {
            -10_000.0
        } else {
            self.wonder_ordinary_value(spec, plan.strategy)
                + if live_race_opens {
                    // Priced in the same units as the rest of this
                    // function, which is normalised by (7 + build
                    // turns) before ranking: a Library the live journal
                    // reports "worth 74" carries a raw of ~960, a
                    // Settler "worth 130" ~1 560. The Score lane's
                    // +180 never wins that contest — which is why the
                    // Score lane builds no wonders either. Fifteen
                    // host score points at ~100 raw each puts a
                    // 220-cost wonder at ~80 in a 20-production city
                    // (above a Library, below a Settler) and ~30 in a
                    // 5-production one (below both) — strong cities
                    // race, weak ones keep developing.
                    //
                    // ★★★★ AND HIGHER AS THE TALLY APPROACHES. The
                    // fifteen points are the same whatever the wonder
                    // costs, but the cost climbs by era (220 ancient,
                    // 700–1 000 renaissance and later) while this
                    // seat's production stays at 15–40, so at a fixed
                    // +1 500 the arm went quiet exactly when the tally
                    // was near: run civvis-20260816T030249Z (918
                    // against 967 at t243, eleven techs ahead) started
                    // no wonder at all between t112 and t174, and
                    // finished four. Late in the game a building's
                    // yields have little left to compound, and a
                    // wonder's fifteen points are the densest score
                    // production buys, so the bonus rises with the
                    // game's progress: ×1 at the start, ×2 at the
                    // midpoint, ×3 at the tally. The completion guards
                    // above (a wonder that cannot finish, or would take
                    // most of what is left) still refuse.
                    LIVE_WONDER_RACE_BONUS
                        * Self::live_wonder_race_scale(g)
                        * if bargain_race {
                            (CHEAPEST_WONDER_TURNS / turns.max(1.0)).min(4.0)
                        } else {
                            1.0
                        }
                } else if tally_opens {
                    // The fifteen points, and nothing else. No lane
                    // ramp and no flat sweetener: the density bar in
                    // `tally_opens` has already established that this
                    // wonder is worth its production, so the ranking's
                    // own `raw / (7 + turns)` is left to decide whether
                    // it beats the Settler or the district beside it.
                    Self::wonder_tally_value()
                } else if plan.strategy == GrandStrategy::Culture {
                    320.0
                } else if self.victory_target == Some(VictoryTarget::Score) {
                    if score_seat_race {
                        Self::score_seat_wonder_price(g)
                    } else {
                        180.0
                    }
                } else {
                    0.0
                }
                + if wonder_civ { 120.0 } else { 0.0 }
                // Priced whichever gate opened the lane: on a Culture
                // plan or a Score target the arm was already willing to
                // build a wonder, and this is what decides *which* one.
                + strategic_value
        }
    }

    /// The `Item::Project` arm: what `project` is worth to this city.
    pub(super) fn project_production_value(
        &self,
        frame: &ProductionFrame<'_>,
        project: &Name,
    ) -> f64 {
        let ProductionFrame {
            g,
            pid,
            cid,
            plan,
            city,
            turns,
            remaining_turns,
            threatened,
            strategic_land_force_gap,
            ..
        } = *frame;
        let spec = &g.rules.projects[project];
        let space_race = matches!(
            project.as_str(),
            "launch_earth_satellite"
                | "launch_moon_landing"
                | "launch_mars_colony"
                | "exoplanet_expedition"
                | "lagrange_laser_station"
                | "terrestrial_laser_station"
        );
        let repeatable_economic_project =
            spec.repeatable && (!spec.completion_gpp.is_empty() || !spec.ongoing_yields.is_empty());
        let forbidden_project = (strategic_land_force_gap && repeatable_economic_project)
            || (space_race
                && self.victory_target.is_some()
                && self.victory_target != Some(VictoryTarget::Science)
            // See `lane_release_when_hopeless`. The live King seat
            // stood a Spaceport at turn 225 and launched nothing from
            // it: on `--victory diplomatic` every launch project this
            // arm can offer is refused here, whatever the seat's
            // science lead. A lost lane does not get to forbid the
            // victory that is still open.
                && (self.victory_target == Some(VictoryTarget::Domination)
                    || !self.lane_lost))
            || turns > remaining_turns * 0.8;
        if forbidden_project {
            -10_000.0
        } else {
            let completed = g.players[pid].science_projects.len() as f64;
            match project.as_str() {
                "repair_outer_defenses" => {
                    let missing = (g.city_max_wall_hp(city) - city.wall_hp).max(0);
                    900.0 + missing as f64 * 12.0 + if threatened { 1_500.0 } else { 0.0 }
                }
                "repair_encampment" => {
                    let missing = (100 - city.encampment_hp).max(0)
                        + (g.city_max_wall_hp(city) - city.encampment_wall_hp).max(0);
                    700.0 + missing as f64 * 10.0 + if threatened { 1_150.0 } else { 0.0 }
                }
                "recommission_reactor" => {
                    if city.reactor_age <= 12 {
                        -10_000.0
                    } else {
                        // Maintenance becomes urgent as the reactor's
                        // per-turn accident risk compounds. A fresh
                        // plant must never monopolize production just
                        // because this is a repeatable project.
                        500.0 + (city.reactor_age - 10) as f64 * 75.0
                    }
                }
                "convert_reactor_to_coal"
                | "convert_reactor_to_oil"
                | "convert_reactor_to_uranium" => {
                    let target = match project.as_str() {
                        "convert_reactor_to_coal" => "coal",
                        "convert_reactor_to_oil" => "oil",
                        _ => "uranium",
                    };
                    if target != "uranium"
                        && self.diplomatic_lane_refuses_fossil_power("coal_power_plant")
                    {
                        -10_000.0
                    } else if !self.reactor_marginal {
                        Self::stock_power_plant_value(g, pid, target)
                    } else if let Some(current) = Self::current_power_plant(city) {
                        let target_utility = Self::power_plant_utility(g, pid, target);
                        let improvement =
                            target_utility - Self::power_plant_utility(g, pid, current);
                        if improvement > f64::EPSILON {
                            improvement
                        } else {
                            -10_000.0
                        }
                    } else {
                        -10_000.0
                    }
                }
                "carbon_recapture" => {
                    if g.global_co2_emissions() <= f64::EPSILON
                        && plan.strategy != GrandStrategy::Diplomacy
                    {
                        -10_000.0
                    } else {
                        450.0
                            + g.climate_phase as f64 * 260.0
                            + (g.players[pid].co2_emissions.max(0.0) / 500.0).min(800.0)
                            + if plan.strategy == GrandStrategy::Diplomacy {
                                900.0
                            } else {
                                0.0
                            }
                    }
                }
                "manhattan_project"
                | "operation_ivy"
                | "build_nuclear_device"
                | "build_thermonuclear_device" => self.nuclear_project_value(g, pid, project, plan),
                _ if spec.requires_host_competition() => {
                    // See `competition_victory_points`: first place in
                    // a scored competition pays one or two of the
                    // twenty a diplomatic victory needs, and only the
                    // competition's own score was ever priced.
                    let value = spec
                        .host_competition_kinds()
                        .map(|kind| {
                            self.host_competition_score_value(
                                g,
                                pid,
                                kind,
                                spec.competition_score,
                                turns,
                            ) + self.competition_victory_point_value(
                                g,
                                pid,
                                plan,
                                kind,
                                spec.competition_score,
                            )
                        })
                        .fold(0.0, f64::max);
                    // A host-unlocked project that cannot complete
                    // before the current competition expires is not a
                    // neutral fallback: letting it win a tie would
                    // replace a real build with a refused stale order.
                    if value > 0.0 {
                        value
                    } else {
                        -10_000.0
                    }
                }
                _ if space_race => {
                    3_300.0
                        + completed * 220.0
                        + if plan.strategy == GrandStrategy::Science {
                            650.0
                        } else {
                            0.0
                        }
                }
                _ if !spec.completion_gpp.is_empty()
                    || !spec.ongoing_yields.is_empty()
                    || spec.full_power_while_active
                    || project == "bread_and_circuses" =>
                {
                    self.district_project_value(g, pid, cid, project, plan)
                }
                // Scenario and future projects without an understood
                // economic effect remain legal, but cannot crowd out
                // infrastructure solely because they are repeatable.
                _ => 180.0,
            }
        }
    }
}
