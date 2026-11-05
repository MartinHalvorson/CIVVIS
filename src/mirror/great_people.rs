//! Great People: writing the host's Great People state into the rebuilt game.
//!
//! `apply_great_person_points`, and the live offer blockers and activation
//! needs the host reports (`apply_live_great_person_offer_blockers`,
//! `apply_live_great_person_activation_needs`).
//!
//! Moved verbatim out of `mirror.rs` (2026-11-05). `mirror.rs` re-exports
//! everything here, so every path is unchanged. Items the rest of `mirror`
//! uses are `pub(super)`; the code is otherwise unchanged.

use super::*;

/// Replace CIVVIS's inferred Governor state with the authoritative Firaxis roster.
///
/// Rebuilding cities and completed Civics alone is insufficient: title sources are
/// not all Civics, and neither appointments, promotions nor assignments are derived
/// facts. Without this pass CIVVIS spends the same titles on every reconstructed
/// turn and repeatedly appoints Governors already visible in the host game.
/// Carry the Great Person race across from the host.
///
/// Deliberately **not** part of `apply_governor_state`: that function returns
/// early when the host reports no governors, and the Great Person race has
/// nothing to do with governors. Bundling them would have made this arrive only
/// in games that already had a Governor appointed, which is precisely the kind
/// of silent conditional the mirror has been bitten by before.
pub(super) fn apply_great_person_points(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    apply_live_great_person_activation_needs(game, state, unmapped);
    // Civilization VI names the class `GREAT_PERSON_CLASS_SCIENTIST`; CIVVIS
    // keys the same thing `scientist`. The nine classes correspond one to one,
    // so the translation is the suffix, lowercased — and an unrecognised class
    // is reported rather than dropped, because a new expansion adding one is
    // exactly the case where silence would be wrong.
    if let Some(points) = state.great_person_points.as_ref() {
        let mut gpp = BTreeMap::new();
        for (class, total) in points {
            match class.strip_prefix("GREAT_PERSON_CLASS_") {
                Some(kind) if !kind.is_empty() => {
                    gpp.insert(kind.to_ascii_lowercase(), *total);
                }
                _ => {
                    let issue = format!("great_person_class:{class}");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                }
            }
        }
        game.players[0].gpp = gpp;
    }
    // The live recruit cost lands on CIVVIS's idea of the class's current
    // person via `great_person_offer_costs`, which `gp_cost` consults before
    // its market formula. The two games disagree about WHO is on offer, but
    // the number that gates the recruit decision — cost minus banked points —
    // is the live game's, which is the one the order will be judged by.
    if let Some(costs) = state.great_person_costs.as_ref() {
        for (class, cost) in costs {
            let Some(kind) = class.strip_prefix("GREAT_PERSON_CLASS_") else {
                let issue = format!("great_person_cost_class:{class}");
                if !unmapped.contains(&issue) {
                    unmapped.push(issue);
                }
                continue;
            };
            let kind = kind.to_ascii_lowercase();
            let id = game
                .current_great_person(&kind)
                .map(|(id, _)| id.to_string());
            if let Some(id) = id {
                game.great_person_offer_costs.insert(id, *cost);
            }
        }
    }
    // Which classes have nobody left to recruit anywhere on the host's
    // timeline — the last Great Scientist claimed by anyone — because
    // Firaxis pays such a class's points out as Faith from then on. The mod
    // says so outright (`great_person_exhausted`); before that field, the
    // cost map named every class with an unclaimed entry, so a class with
    // points and no cost was the same answer — except on the turn every
    // class is gone, when the map is `nil` and says nothing. Carry the host's
    // roster rather than CIVVIS's, whose named list is not the host's; `None`
    // on an older export keeps the engine's own answer.
    let kind_of = |class: &str| {
        class
            .strip_prefix("GREAT_PERSON_CLASS_")
            .filter(|kind| !kind.is_empty())
            .map(str::to_ascii_lowercase)
    };
    game.players[0].live_great_person_exhausted = match (
        state.great_person_exhausted.as_ref(),
        state.great_person_costs.as_ref(),
    ) {
        (Some(exhausted), _) => Some(
            exhausted
                .iter()
                .filter_map(|class| kind_of(class))
                .collect(),
        ),
        (None, Some(costs)) => Some(
            state
                .great_person_points
                .iter()
                .flat_map(|points| points.keys())
                .filter(|class| !costs.contains_key(*class))
                .filter_map(|class| kind_of(class))
                .collect(),
        ),
        (None, None) => None,
    };
    game.players[0].live_open_great_work_slots = live_open_great_work_slots(&game.rules, state);
    apply_live_great_person_offer_blockers(game, state, unmapped);
}

/// Carry physical Great People with no currently legal use back into planning.
///
/// The unit-order bridge can already activate a person in place or walk them to
/// any host-provided activation plot. An empty plot list is the remaining hard
/// case: movement cannot solve it, and without this signal the production AI
/// never learns that a district, Great Work slot, Wonder, or eligible military
/// unit is now a prerequisite for consuming an asset the empire already owns.
fn apply_live_great_person_activation_needs(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    let mut needs = Vec::new();
    for unit in &state.units {
        let Some(person) = unit.great_person.as_ref() else {
            continue;
        };
        if person.can_activate {
            continue;
        }
        // A non-empty plot list is only proof of a *place*, not of a use:
        // `GetActivationHighlightPlots` highlights a cultural person's
        // district whether or not a compatible Great Work slot is free.
        // Seven Writers, Artists and Musicians stood on one Theater plot for
        // thirty-plus turns on run civvis-20260817T010950Z, unactivatable,
        // while this gate read their nine highlighted plots as "nothing to
        // build".
        //
        // The tiebreaker was the host's empire-wide empty-slot count, and it
        // was the wrong question — see `StateGreatPerson::slot_starved`. Nine
        // cultural people idled the WHOLE of run civvis-20260822T020434Z with
        // that count reading 24, 4 and 2 while every plot the host offered
        // them read `slot_open: false`. Ask instead whether this person can
        // reach a slot; zero empire-wide is still a need, and still counted.
        let slot_starved = person.slot_starved();
        if !person.activation_plots.is_empty() && !slot_starved {
            continue;
        }
        let kind = person
            .class
            .as_deref()
            .and_then(|class| class.strip_prefix("GREAT_PERSON_CLASS_"))
            .filter(|kind| !kind.is_empty())
            .map(str::to_ascii_lowercase)
            .or_else(|| {
                unit.kind
                    .strip_prefix("UNIT_GREAT_")
                    .filter(|kind| !kind.is_empty())
                    .map(str::to_ascii_lowercase)
            });
        let Some(kind) = kind else {
            let issue = format!("great_person_unit_class:{}", unit.kind);
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
            continue;
        };

        let required_district = person.required_district.as_deref().and_then(|required| {
            if required.eq_ignore_ascii_case("DISTRICT_CITY_CENTER") {
                return Some("city_center".to_string());
            }
            let district = civvis_node_name(&game.rules.districts, required, "DISTRICT_");
            if district.is_none() {
                let issue = format!("great_person_unit_district:{required}");
                if !unmapped.contains(&issue) {
                    unmapped.push(issue);
                }
            }
            district.map(|district| {
                game.district_family(crate::name::Name::new(&district))
                    .to_string()
            })
        });
        let required_missing_building = person
            .required_missing_building
            .as_deref()
            .filter(|required| !required.trim().is_empty())
            .and_then(|required| {
                let building = civvis_node_name(&game.rules.buildings, required, "BUILDING_");
                if building.is_none() {
                    let issue = format!("great_person_unit_building:{required}");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                }
                building
            });
        let required_great_work = person
            .required_great_work
            .as_deref()
            .filter(|required| !required.trim().is_empty())
            .and_then(|required| {
                let work = great_person_required_work_kind(required);
                if work.is_none() {
                    let issue = format!("great_person_unit_work:{required}");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                }
                work.map(str::to_string)
            });
        let individual = person
            .individual
            .as_deref()
            .and_then(|individual| individual.strip_prefix("GREAT_PERSON_INDIVIDUAL_"))
            .filter(|individual| !individual.is_empty())
            .map(str::to_ascii_lowercase);
        needs.push(crate::game::LiveGreatPersonActivationNeed {
            kind,
            individual,
            required_district,
            required_missing_building,
            required_great_work,
        });
    }
    game.players[0].live_great_person_activation_needs = needs;
}

fn apply_live_great_person_offer_blockers(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    let mut blockers = BTreeMap::new();
    let mut individuals = BTreeMap::new();
    let Some(offers) = state.great_person_offers.as_ref() else {
        // A persistent mirror may have received this field last turn from a
        // newer mod and omit it after a rollback. Never keep a stale live-only
        // refusal alive when the current host frame no longer knows it.
        game.players[0].live_great_person_offers = None;
        game.players[0].live_great_person_offer_individuals.clear();
        game.players[0].live_great_person_offer_blockers.clear();
        return;
    };
    let mut offered_classes = BTreeSet::new();

    for (class, offer) in offers {
        let Some(kind) = class
            .strip_prefix("GREAT_PERSON_CLASS_")
            .filter(|kind| !kind.is_empty())
            .map(str::to_ascii_lowercase)
        else {
            let issue = format!("great_person_offer_class:{class}");
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
            continue;
        };
        offered_classes.insert(kind.clone());
        if let Some(individual) = offer
            .individual
            .as_deref()
            .map(str::trim)
            .filter(|individual| !individual.is_empty())
            .map(|individual| {
                individual
                    .strip_prefix("GREAT_PERSON_INDIVIDUAL_")
                    .unwrap_or(individual)
                    .to_ascii_lowercase()
            })
        {
            individuals.insert(kind.clone(), individual);
        }
        let mut reasons = Vec::new();

        if let Some(required_district) = offer
            .required_district
            .as_deref()
            .filter(|district| !district.trim().is_empty())
        {
            // Check the exact host name first. This covers
            // `DISTRICT_CITY_CENTER`, which CIVVIS deliberately does not store
            // in `City::districts` because every city already owns one. Then
            // compare CIVVIS district families so a unique like Russia's
            // Lavra satisfies Firaxis's `DISTRICT_HOLY_SITE` prerequisite
            // without pretending the two literal names are equal.
            let required_family =
                civvis_node_name(&game.rules.districts, required_district, "DISTRICT_")
                    .map(|district| game.district_family(crate::name::Name::new(&district)));
            let active = (required_district.eq_ignore_ascii_case("DISTRICT_CITY_CENTER")
                && !state.cities.is_empty())
                || state.cities.iter().any(|city| {
                    city.districts.iter().any(|district| {
                        if !district.complete || district.pillaged {
                            return false;
                        }
                        district.kind.eq_ignore_ascii_case(required_district)
                            || required_family.is_some_and(|family| {
                                civvis_node_name(&game.rules.districts, &district.kind, "DISTRICT_")
                                    .is_some_and(|district| {
                                        game.district_family(crate::name::Name::new(&district))
                                            == family
                                    })
                            })
                    })
                });
            if !active {
                if required_family.is_none() {
                    let issue = format!("great_person_offer_district:{required_district}");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                }
                reasons.push(format!("requires an active {required_district}"));
            }
        }

        if let Some(required_missing_building) = offer
            .required_missing_building
            .as_deref()
            .filter(|building| !building.trim().is_empty())
        {
            match civvis_node_name(
                &game.rules.buildings,
                required_missing_building,
                "BUILDING_",
            ) {
                Some(required_building) => {
                    let missing = state.cities.iter().all(|city| {
                        !city.buildings.iter().any(|building| {
                            civvis_node_name(&game.rules.buildings, building, "BUILDING_")
                                .is_some_and(|building| {
                                    game.building_is_family(
                                        crate::name::Name::new(&building),
                                        crate::name::Name::new(&required_building),
                                    )
                                })
                        })
                    });
                    if !missing {
                        reasons.push(format!(
                            "requires a city without {required_missing_building}"
                        ));
                    }
                }
                None => {
                    let issue = format!("great_person_offer_building:{required_missing_building}");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                    reasons.push(format!(
                        "requires an unmapped missing building {required_missing_building}"
                    ));
                }
            }
        }

        if let Some(required_great_work) = offer
            .required_great_work
            .as_deref()
            .filter(|work| !work.trim().is_empty())
        {
            match great_person_required_work_kind(required_great_work) {
                Some(work) if live_great_work_offer_has_capacity(game, 0, work) => {}
                Some(work) => reasons.push(format!("requires an open {work} Great Work slot")),
                None => {
                    let issue = format!("great_person_offer_work:{required_great_work}");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                    reasons.push(format!(
                        "requires an unmapped Great Work object {required_great_work}"
                    ));
                }
            }
        }

        if !reasons.is_empty() {
            let individual = offer
                .individual
                .as_deref()
                .filter(|individual| !individual.trim().is_empty())
                .unwrap_or(class);
            blockers.insert(
                kind,
                format!("the live {individual} offer {}", reasons.join("; ")),
            );
        }
    }
    game.players[0].live_great_person_offers = Some(offered_classes);
    game.players[0].live_great_person_offer_individuals = individuals;
    game.players[0].live_great_person_offer_blockers = blockers;
}
