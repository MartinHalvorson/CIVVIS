//! Live Great Person activation: getting a claimed person somewhere they can
//! act, on the live host.
//!
//! A person needs a building, a district, a tech, a civic or a Great Work
//! before they can be activated. These read which: the default district and
//! the work and work building a person needs (`live_great_person_work`,
//! `live_great_person_district`), and the tech and civic goals that unlock
//! them. They turn that need into something to produce: a district, a
//! cultural or military item, the chain of work behind it, or an
//! Archaeologist for a missing Artifact (`live_artifact_activation_digger`).
//! `live_great_person_activation_item` picks the item and `_resume_item`
//! returns to a district foundation already placed for one.
//! `live_great_person_activation_path_allowed` keeps the targeted governor's
//! production limits, `low_impact_science_activation_path` withholds a new
//! path while the empire is still expanding, and
//! `prioritize_live_great_person_activation` reserves the fastest idle city
//! for a missing requirement before the governor fills every queue.
//!
//! Moved verbatim out of the `impl BasicAi` blocks in `ai.rs` (2026-11-05).
//! Methods the rest of `ai` calls are `pub(super)`, the same reach a private
//! method of `ai` had; the code is otherwise unchanged.

use super::*;

impl BasicAi {
    pub(super) fn live_great_person_district_item(
        g: &Game,
        pid: usize,
        cid: u32,
        family: &str,
    ) -> Option<Item> {
        if Self::empire_district_family_ready_or_queued(g, pid, family)
            || g.city_has_district_family(&g.cities[&cid], Name::new(family))
        {
            return None;
        }
        let district = Self::civ_district(g, pid, family);
        g.district_sites(cid, district)
            .into_iter()
            .max_by(|left, right| {
                g.district_site_yields(district, *left)
                    .total()
                    .partial_cmp(&g.district_site_yields(district, *right).total())
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(left.cmp(right))
            })
            .map(|pos| Item::District { district, pos })
            .filter(|item| g.can_produce(pid, cid, item))
    }

    /// The buildings that open a slot for `work`: the target first, then the
    /// prerequisite needed to make it buildable.
    pub(super) fn live_great_person_work_chain(work: &str) -> &'static [&'static str] {
        match work {
            "writing" => &["amphitheater"],
            "art" => &["art_museum", "amphitheater"],
            "artifact" => &["archaeological_museum", "amphitheater"],
            "music" => &["broadcast_center", "art_museum", "amphitheater"],
            _ => &[],
        }
    }

    /// A museum already prepares a city for its missing Broadcast Center.
    /// While that target is unavailable, do not replicate its prerequisite
    /// across the empire for the same waiting Musician.
    pub(super) fn live_great_person_work_stage_needed(
        g: &Game,
        pid: usize,
        work: &str,
        family: &str,
    ) -> bool {
        if work != "music" || family == "broadcast_center" {
            return true;
        }
        !g.player_city_ids(pid).into_iter().any(|cid| {
            let city = &g.cities[&cid];
            let theater_ready = city.districts.iter().any(|(district, pos)| {
                g.district_family(*district) == "theater_square"
                    && g.map.tiles.get(pos).is_some_and(|tile| !tile.pillaged)
            });
            let museum_ready = city.buildings.iter().any(|building| {
                !city.pillaged_buildings.contains(building)
                    && [
                        crate::name!("art_museum"),
                        crate::name!("archaeological_museum"),
                    ]
                    .into_iter()
                    .any(|family| g.building_is_family(*building, family))
            });
            let broadcast_ready = city.buildings.iter().any(|building| {
                !city.pillaged_buildings.contains(building)
                    && g.building_is_family(*building, crate::name!("broadcast_center"))
            });
            theater_ready && museum_ready && !broadcast_ready
        })
    }

    pub(super) fn live_great_person_cultural_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        work: &str,
    ) -> Option<Item> {
        if self.artifact_before_more_museums
            && work == "artifact"
            && Self::live_artifact_activation_pending(g, pid)
            && g.cities.values().any(|city| {
                city.owner == pid
                    && city
                        .buildings
                        .contains(&crate::name!("archaeological_museum"))
                    && !city
                        .pillaged_buildings
                        .contains(&crate::name!("archaeological_museum"))
                    && g.city_has_district_family(city, crate::name!("theater_square"))
            })
        {
            // The host requires an EXISTING work. More empty slots cannot
            // supply it. Once one Museum stands, hold this activation path
            // for one bounded digger instead of constructing more Museums.
            return self.live_artifact_activation_digger(g, pid, cid);
        }
        // The host's empty activation-plot list is authoritative. CIVVIS may
        // believe a generic Palace `any` slot is open while Firaxis refuses
        // this exact individual, and trusting the approximation here is what
        // leaves the physical person parked. Build typed capacity until the
        // next host frame exposes a real plot and clears the need.
        let chain = Self::live_great_person_work_chain(work);
        if chain.is_empty() {
            return None;
        }
        let already_queued = g.player_city_ids(pid).into_iter().any(|city| {
            matches!(
                g.cities[&city].queue.first(),
                Some(Item::Building { building })
                    if chain.iter().any(|family| g.building_is_family(building, Name::new(family))
                        && Self::live_great_person_work_stage_needed(g, pid, work, family))
            )
        });
        if already_queued {
            return None;
        }
        for family in chain {
            if !Self::live_great_person_work_stage_needed(g, pid, work, family) {
                continue;
            }
            let buildable_somewhere = g
                .player_city_ids(pid)
                .into_iter()
                .any(|city| Self::civ_building(g, pid, city, family).is_some());
            if buildable_somewhere {
                return Self::civ_building(g, pid, cid, family);
            }
        }
        None
    }

    /// One owned person's missing Artifact, using an existing Museum.
    /// Proximity is a screen, not proof of a legal native walking route.
    pub(crate) fn live_artifact_activation_pending(g: &Game, pid: usize) -> bool {
        g.players[pid].counters.get("great_work:artifact") == Some(&0)
            && g.players[pid]
                .live_great_person_activation_needs
                .iter()
                .any(|need| need.required_great_work.as_deref() == Some("artifact"))
    }

    fn live_artifact_activation_digger(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let city = &g.cities[&cid];
        if city.hp < 200
            || !city
                .buildings
                .contains(&crate::name!("archaeological_museum"))
            || city
                .pillaged_buildings
                .contains(&crate::name!("archaeological_museum"))
            || !g.can_house_additional_great_work(pid, "artifact")
            || g.players[pid]
                .live_open_great_work_slots
                .as_ref()
                .is_some_and(|slots| !slots.contains("artifact"))
            || g.units
                .values()
                .any(|unit| unit.owner == pid && unit.kind == "archaeologist")
            || g.cities.values().any(|other| {
                other.owner == pid
                    && other
                        .queue
                        .iter()
                        .any(|item| matches!(item, Item::Unit { unit } if unit == "archaeologist"))
            })
            || g.units.values().any(|unit| {
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && g.rules.units[&unit.kind].class == "military"
                    && g.wdist(city.pos, unit.pos) <= 6
            })
        {
            return None;
        }
        let nearby_owned_dig = g
            .excavation_sites(pid)
            .iter()
            .any(|(position, improvement)| {
                improvement == "archaeological_dig"
                    && g.wdist(city.pos, *position) <= 6
                    && g.map
                        .get(*position)
                        .and_then(|tile| tile.owner_city)
                        .is_some_and(|owner| {
                            g.cities.get(&owner).is_some_and(|other| other.owner == pid)
                        })
            });
        if !nearby_owned_dig {
            return None;
        }
        let item = Item::Unit {
            unit: crate::name!("archaeologist"),
        };
        if !g.can_produce(pid, cid, &item) {
            return None;
        }
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            g.item_remaining_cost_for_city(pid, cid, &item) / g.city_yields(cid).production.max(0.5)
        });
        let cap = g.standard_duration(20) as f64;
        (turns.is_finite()
            && turns <= cap
            && g.max_turns.saturating_sub(g.turn) as f64 > turns + cap)
            .then_some(item)
    }

    fn live_great_person_military_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        need: &crate::game::LiveGreatPersonActivationNeed,
    ) -> Option<Item> {
        if !matches!(need.kind.as_str(), "general" | "admiral") {
            return None;
        }

        // A host-required Encampment or Harbor must be finished before the
        // body can activate the person. A queued or map-founded district is
        // handled by the earlier district path and must not cause a unit to
        // be parked ahead of its infrastructure.
        if let Some(family) = need.required_district.as_deref() {
            let district_ready = family == "city_center"
                || g.player_city_ids(pid)
                    .into_iter()
                    .any(|city| g.city_has_district_family(&g.cities[&city], Name::new(family)));
            if !district_ready {
                return None;
            }
        }

        match need.kind.as_str() {
            "general" => {
                let land_queued = g.player_city_ids(pid).into_iter().any(|city| {
                    matches!(g.cities[&city].queue.first(), Some(Item::Unit { unit })
                        if g.rules.units[unit].class == "military"
                            && g.rules.units[unit].domain.as_deref() != Some("sea"))
                });
                if land_queued {
                    return None;
                }
                self.best_military(g, pid, cid, None)
                    .map(|unit| Item::Unit {
                        unit: Name::new(&unit),
                    })
            }
            "admiral" => {
                let navy_queued = g.player_city_ids(pid).into_iter().any(|city| {
                    matches!(g.cities[&city].queue.first(), Some(Item::Unit { unit })
                        if g.rules.units[unit].class == "military"
                            && g.rules.units[unit].domain.as_deref() == Some("sea"))
                });
                if navy_queued {
                    return None;
                }
                self.best_naval_unit(g, pid, cid)
                    .map(|unit| Item::Unit { unit })
            }
            _ => None,
        }
    }

    /// A named person can be real, already claimed, and still be the wrong
    /// reason to spend a frontier city's first free queue on a wholly new
    /// district.  The Science lane already classifies these people when it
    /// decides whether Campus projects should race them; keep that same
    /// selectivity at the activation-path boundary.  This only withholds a
    /// *new* path while the caller says the empire is still expanding.  A
    /// placed foundation is sunk production and remains resumable, and a
    /// naturally-ready person remains free to activate through the host.
    fn low_impact_science_activation_path(
        need: &crate::game::LiveGreatPersonActivationNeed,
    ) -> bool {
        matches!(
            need.individual.as_deref(),
            // Hildegard's Holy Site science conversion is useful only after
            // the core Campus/settlement work; Mary Leakey belongs to an
            // Artifact game, not an expanding Science empire.
            Some("hildegard_of_bingen" | "mary_leakey")
        )
    }

    /// Keep the targeted governor's production limits on live-person paths.
    fn live_great_person_activation_path_allowed(
        &self,
        need: &crate::game::LiveGreatPersonActivationNeed,
    ) -> bool {
        !self.exclude_space_race || Self::live_great_person_district(need) != Some("spaceport")
    }

    /// Production that turns an already-owned physical Great Person into a
    /// usable action. Ordinary/headless games never enter this path because
    /// their mirror-only need list is empty.
    pub(super) fn live_great_person_activation_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<Item> {
        if self.minor || self.barb || !g.cities[&cid].queue.is_empty() {
            return None;
        }
        for need in &g.players[pid].live_great_person_activation_needs {
            if !self.live_great_person_activation_path_allowed(need) {
                continue;
            }
            if self.defer_low_impact_science_activation_paths
                && Self::low_impact_science_activation_path(need)
            {
                continue;
            }
            let wonder_engineer = need.kind == "engineer"
                && matches!(
                    need.individual.as_deref(),
                    Some("imhotep" | "gustave_eiffel")
                );
            if wonder_engineer {
                let wonder_queued = g
                    .player_city_ids(pid)
                    .into_iter()
                    .any(|city| matches!(g.cities[&city].queue.first(), Some(Item::Wonder { .. })));
                // A host refusal already says this city cannot start the
                // activation-path wonder CIVVIS proposed. Trying a different
                // speculative wonder in that same city merely walks the whole
                // catalogue after each `build_no_plot`; leave the city to its
                // ordinary production chooser until the host block expires or
                // a different city can supply the path.
                let host_refused_a_wonder_here = g
                    .blocked_wonders
                    .get(&cid)
                    .is_some_and(|wonders| !wonders.is_empty());
                if !wonder_queued && !host_refused_a_wonder_here {
                    if let Some(wonder) = Self::cheapest_available_wonder(g, pid, cid) {
                        return Some(wonder);
                    }
                }
                continue;
            }

            if let Some(family) = Self::live_great_person_district(need) {
                if !Self::empire_district_family_ready_or_queued(g, pid, family) {
                    if let Some(district) =
                        Self::live_great_person_district_item(g, pid, cid, family)
                    {
                        return Some(district);
                    }
                    // A different city may be able to place this district. Do not
                    // substitute the class default or build a later slot stage.
                    continue;
                }
            }

            if let Some(work) = Self::live_great_person_work(need) {
                if let Some(building) = self.live_great_person_cultural_item(g, pid, cid, work) {
                    return Some(building);
                }
            }

            // Formation and promotion people can have infrastructure yet no
            // eligible body. Create one instead of parking the person forever.
            if let Some(item) = self.live_great_person_military_item(g, pid, cid, need) {
                return Some(item);
            }
        }
        None
    }

    /// A live district foundation is an activation path too. Firaxis keeps a
    /// placed district on the map when a city switches to a unit or a second
    /// district, but the ordinary mirror queue only contains the CURRENT
    /// production item. Treating that foundation as merely "ready or queued"
    /// therefore strands a named Great Person: the required district exists,
    /// yet no city ever resumes it.
    ///
    /// Only yield to a queue that is safe to bank. A military body, civilian
    /// expansion unit, defensive repair/walls, wonder, or one-shot project is
    /// an active commitment; the activation path must wait for it. Ordinary
    /// infrastructure, another district, repeatable projects, and products
    /// use the engine's normal per-item progress bank and can be resumed here.
    pub(super) fn live_great_person_activation_resume_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<Item> {
        if self.minor || self.barb {
            return None;
        }
        let queued = g.cities[&cid].queue.first();
        // A city cannot advance two activation districts in one pass. Keep
        // any queued district serving a waiting person, not just the family
        // currently being considered. Otherwise two paused foundations and
        // two needs repeatedly replace each other without ending the pass.
        // Include the host's tail queue: it already supplies that path too.
        if g.cities[&cid].queue.iter().any(|item| {
            matches!(item, Item::District { district, .. }
                if g.players[pid].live_great_person_activation_needs.iter().any(|need|
                    Self::live_great_person_district(need)
                        .is_some_and(|family| g.district_family(*district) == family)))
        }) {
            return None;
        }
        // `activation-resume-waits`: a district already under way finishes first.
        if self.activation_resume_waits
            && matches!(queued, Some(Item::District { .. }))
            && g.cities[&cid].production > 0.0
        {
            return None;
        }
        // `activation-keeps-its-building`: a rung of a waiting person's slot
        // chain already queued here finishes first.
        if self.activation_keeps_its_building
            && matches!(queued, Some(Item::Building { building })
            if g.players[pid].live_great_person_activation_needs.iter().any(|need| {
                Self::live_great_person_work(need).is_some_and(|work| {
                    Self::live_great_person_work_chain(work)
                        .iter()
                        .any(|family| g.building_is_family(building, Name::new(family))
                            && Self::live_great_person_work_stage_needed(g, pid, work, family))
                })
            }))
        {
            return None;
        }
        let queue_can_yield = match queued {
            None => true,
            Some(Item::Building { building }) => g
                .rules
                .buildings
                .get(building)
                .is_none_or(|spec| spec.outer_defense <= 0),
            Some(Item::District { .. }) | Some(Item::Product { .. }) => true,
            Some(Item::Project { project }) => g
                .rules
                .projects
                .get(project)
                .is_some_and(|spec| spec.repeatable),
            Some(Item::Formation { .. })
            | Some(Item::Unit { .. })
            | Some(Item::Wonder { .. })
            | Some(Item::Repair { .. }) => false,
        };
        if !queue_can_yield {
            return None;
        }

        for need in &g.players[pid].live_great_person_activation_needs {
            if !self.live_great_person_activation_path_allowed(need) {
                continue;
            }
            let Some(family) = Self::live_great_person_district(need) else {
                continue;
            };
            let Some((district, pos)) =
                g.cities[&cid]
                    .owned_tiles
                    .iter()
                    .copied()
                    .find_map(|position| {
                        let foundation =
                            g.map.tiles.get(&position)?.district_foundation.as_ref()?;
                        (g.district_family(foundation.district) == family)
                            .then_some((foundation.district, position))
                    })
            else {
                continue;
            };
            let item = Item::District { district, pos };
            if g.can_produce(pid, cid, &item) {
                return Some(item);
            }
        }

        // A physical person can remain stranded while its required or default
        // district is missing: the city may be spending its only queue on a
        // Market, Granary, or repeatable project. That queue is safe to bank
        // in the same way as a paused district foundation, so open the exact
        // missing district before the ordinary governor spends another turn
        // on it. Military bodies, repairs, walls, wonders, and one-shot
        // projects remain protected by `queue_can_yield` above.
        for need in &g.players[pid].live_great_person_activation_needs {
            if !self.live_great_person_activation_path_allowed(need) {
                continue;
            }
            if need.kind == "engineer"
                && matches!(
                    need.individual.as_deref(),
                    Some("imhotep" | "gustave_eiffel")
                )
            {
                // These named Engineers use the wonder path above; their
                // default Industrial Zone is not an activation prerequisite.
                continue;
            }
            let Some(family) = Self::live_great_person_district(need) else {
                continue;
            };
            if !Self::empire_district_family_ready_or_queued(g, pid, family) {
                if let Some(item) = Self::live_great_person_district_item(g, pid, cid, family) {
                    return Some(item);
                }
            }
        }

        // A physical cultural person can remain stranded even after its
        // Theater Square is complete: every compatible Great Work slot may be
        // full while the city spends its only queue on ordinary production.
        // Open the exact missing slot chain before the ordinary governor
        // spends another turn on it.
        for need in &g.players[pid].live_great_person_activation_needs {
            if !self.live_great_person_activation_path_allowed(need) {
                continue;
            }
            let Some(family) = Self::live_great_person_district(need) else {
                continue;
            };
            if !Self::empire_district_family_ready_or_queued(g, pid, family) {
                continue;
            }
            if let Some(work) = Self::live_great_person_work(need) {
                if let Some(item) = self.live_great_person_cultural_item(g, pid, cid, work) {
                    if g.can_produce(pid, cid, &item) {
                        return Some(item);
                    }
                }
            }
            if let Some(item) = self.live_great_person_military_item(g, pid, cid, need) {
                return Some(item);
            }
        }
        None
    }

    /// Reserve the fastest idle city for one missing Great Person activation
    /// requirement before the strategic governor can fill every queue with a
    /// repeatable project. It also resumes a placed-but-paused district before
    /// a second district can hide it. Re-running after the chosen item
    /// completes advances multi-stage cultural chains one concrete prerequisite
    /// at a time.
    pub(crate) fn prioritize_live_great_person_activation(&self, g: &mut Game, pid: usize) -> bool {
        if g.players[pid].live_great_person_activation_needs.is_empty() {
            return false;
        }
        let mut changed = false;
        // Produce reserves a queue; it does not finish the prerequisite.
        // Each city can therefore supply only one new path during this pass.
        let mut reserved = BTreeSet::new();
        loop {
            let choice = g
                .player_city_ids(pid)
                .into_iter()
                .filter(|city| !reserved.contains(city))
                .filter_map(|city| {
                    let item = if g.cities[&city].queue.is_empty() {
                        self.live_great_person_activation_item(g, pid, city)
                    } else {
                        self.live_great_person_activation_resume_item(g, pid, city)
                    }?;
                    // `first-pad-in-the-forge`: the first Spaceport only in a
                    // producer. See `ai/first_pad_in_the_forge.rs`.
                    if !self.first_pad_item_allowed(g, pid, city, &item) {
                        return None;
                    }
                    let remaining = (g.item_cost_for_city(pid, city, &item)
                        - g.cities[&city].production)
                        .max(0.0);
                    let turns = remaining / g.city_yields(city).production.max(0.1);
                    Some((turns, format!("{item:?}"), city, item))
                })
                .min_by(|left, right| {
                    left.0
                        .total_cmp(&right.0)
                        .then_with(|| left.1.cmp(&right.1))
                        .then_with(|| left.2.cmp(&right.2))
                });
            let Some((_, _, city, item)) = choice else {
                break;
            };
            let resumed = !g.cities[&city].queue.is_empty();
            if g.apply(
                pid,
                &Action::Produce {
                    city,
                    item: item.clone(),
                },
            )
            .is_err()
            {
                break;
            }
            changed = true;
            reserved.insert(city);
            if resumed {
                think!(self.journal, Cities, Decision,
                      "Resuming an activation path for a live Great Person";
                      "{} starts {:?}, the fastest missing prerequisite", g.cities[&city].name, item);
            } else {
                think!(self.journal, Cities, Decision,
                      "Building an activation path for an idle Great Person";
                      "{} starts {:?}, the fastest missing prerequisite", g.cities[&city].name, item);
            }
        }
        changed
    }

    fn live_great_person_default_district(kind: &str) -> Option<&'static str> {
        match kind {
            "scientist" => Some("campus"),
            "engineer" => Some("industrial_zone"),
            "merchant" => Some("commercial_hub"),
            "prophet" => Some("holy_site"),
            "writer" | "artist" | "musician" => Some("theater_square"),
            "general" => Some("encampment"),
            "admiral" => Some("harbor"),
            _ => None,
        }
    }

    pub(super) fn live_great_person_work(
        need: &crate::game::LiveGreatPersonActivationNeed,
    ) -> Option<&'static str> {
        if let Some(required) = need.required_great_work.as_deref() {
            return match required {
                "writing" => Some("writing"),
                "art" | "religious_art" => Some("art"),
                "artifact" => Some("artifact"),
                "music" => Some("music"),
                "relic" => Some("relic"),
                _ => None,
            };
        }
        match need.kind.as_str() {
            "writer" => Some("writing"),
            "artist" => Some("art"),
            "musician" => Some("music"),
            _ => None,
        }
    }

    fn live_great_person_work_building(work: &str) -> Option<&'static str> {
        match work {
            "writing" => Some("amphitheater"),
            "art" => Some("art_museum"),
            "artifact" => Some("archaeological_museum"),
            "music" => Some("broadcast_center"),
            _ => None,
        }
    }

    pub(super) fn live_great_person_district(
        need: &crate::game::LiveGreatPersonActivationNeed,
    ) -> Option<&str> {
        match need.required_district.as_deref() {
            Some("city_center") => None,
            Some(family) => Some(family),
            None => Self::live_great_person_default_district(&need.kind),
        }
    }

    /// Technology whose absence prevents an already-recruited physical Great
    /// Person from ever receiving a legal activation plot in the live game.
    pub(crate) fn live_great_person_tech_goal(g: &Game, pid: usize) -> Option<String> {
        for need in &g.players[pid].live_great_person_activation_needs {
            if let Some(family) = Self::live_great_person_district(need) {
                let district = Self::civ_district(g, pid, family);
                if let Some(tech) = g.rules.districts[district].tech.as_deref() {
                    if !g.players[pid].techs.contains(&Name::new(tech)) {
                        return Some(tech.to_string());
                    }
                }
            }
            if let Some(work) = Self::live_great_person_work(need) {
                if let Some(building) = Self::live_great_person_work_building(work) {
                    if let Some(tech) = g.rules.buildings[building].tech.as_deref() {
                        if !g.players[pid].techs.contains(&Name::new(tech)) {
                            return Some(tech.to_string());
                        }
                    }
                }
            }
            // The Musician chain eventually needs a Broadcast Center. Walking
            // to Radio now is safe even if Humanism still gates its Museum;
            // the civic chooser below advances that independent half.
            if need.kind == "musician" && !g.players[pid].techs.contains(&crate::name!("radio")) {
                return Some("radio".to_string());
            }
        }
        None
    }

    /// Civic counterpart of [`Self::live_great_person_tech_goal`]. Cultural
    /// people ask for the complete slot chain, not merely a Theater Square:
    /// Writers need Drama and Poetry, Artists and Musicians need Humanism.
    pub(crate) fn live_great_person_civic_goal(g: &Game, pid: usize) -> Option<String> {
        for need in &g.players[pid].live_great_person_activation_needs {
            if let Some(family) = Self::live_great_person_district(need) {
                let district = Self::civ_district(g, pid, family);
                if let Some(civic) = g.rules.districts[district].civic.as_deref() {
                    if !g.players[pid].civics.contains(&Name::new(civic)) {
                        return Some(civic.to_string());
                    }
                }
            }
            if let Some(work) = Self::live_great_person_work(need) {
                if let Some(building) = Self::live_great_person_work_building(work) {
                    if let Some(civic) = g.rules.buildings[building].civic.as_deref() {
                        if !g.players[pid].civics.contains(&Name::new(civic)) {
                            return Some(civic.to_string());
                        }
                    }
                }
            }
            let cultural = match need.kind.as_str() {
                "writer" => Some("drama_poetry"),
                "artist" | "musician" => Some("humanism"),
                _ => None,
            };
            if let Some(civic) = cultural {
                if !g.players[pid].civics.contains(&Name::new(civic)) {
                    return Some(civic.to_string());
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
