//! The seat: writing the seat's own state into the rebuilt game.
//!
//! Who the seat is (`apply_identity`), the host's metrics it reports
//! (`apply_observed_host_metrics`), its religion, its governors, the
//! policies the host lets it choose, its maintenance bill, its strategic
//! stockpiles, and what it observes of each of its units
//! (`apply_unit_observation`).
//!
//! Moved verbatim out of `mirror.rs` (2026-11-05). `mirror.rs` re-exports
//! everything here, so every path is unchanged. Items the rest of `mirror`
//! uses are `pub(super)`; the code is otherwise unchanged.

use super::*;

/// Tell the seat which ground it has actually seen.
///
/// ★★★★★ **NOTHING IN THIS BRIDGE EVER WROTE `explored`, SO IT DESCRIBED A PLACE THE
/// SEAT HAS NEVER BEEN.**
///
/// It was not empty, which is why this went unnoticed — and I asserted "empty" before
/// a test corrected me. `rebuild_game` starts from `Game::new`, which generates an
/// ordinary CIVVIS map and reveals a start position on it, leaving **35 explored plots
/// around a capital that has nothing to do with the real Civilization VI seat**. The
/// set was therefore populated, plausible, and pure fiction.
///
/// What that costs: `BasicAi`'s explore step walks outward in rings and takes the
/// nearest tile that is `!explored` and traversable (`ai.rs`, `has_exploration_target`
/// and the ring search beside it). Real ground the seat HAS seen is almost never inside
/// the generated map's 35, so it reads as unexplored — the unit steps onto a tile it has
/// already stood on, and next turn that tile is still "unexplored" because nothing here
/// ever recorded otherwise. That is the three-tile shuffle the livelock detector
/// reports: a scout, three archers and a heavy chariot moving every single turn and
/// uncovering nothing, invisible to both the idle fraction and the frozen count
/// *because the unit does move*.
///
/// The same fiction misdirects every tile purchase: `Game::plot_purchase_cost` requires
/// `explored.contains(&pos)`, so it quotes prices for ground the seat cannot see and
/// refuses the ground it can.
///
/// The snapshot has carried the answer the whole time — `Snapshot::revealed` is exactly
/// the set of plots the mod has exported for this seat, and the mod exports a plot only
/// once the seat can see it.
///
/// ⚠ This **replaces** rather than extends. Extending would keep the generated map's 35
/// invented plots forever, and they are the whole defect. Replacing is still monotonic
/// in practice because `Snapshot::revealed` itself only ever accumulates, so a plot seen
/// on turn 40 is still explored on turn 200 even when the seat cannot currently see it —
/// which is what `explored` means, as distinct from `is_revealed` at this instant.
/// Idempotent.
/// Put the seat's strategic stockpiles on the board.
///
/// ★★★★ THE BOARD HAD NO STRATEGIC RESOURCES AT ALL. Nothing exported them, so
/// `Game::strategic_stockpile` answered 0 for every resource on every live turn:
/// `can_produce` refused every Swordsman, Knight, Musketman and Bombard, the
/// armies were the resource-free units (AT crews, pike-and-shot, chariots — see
/// the production histograms of every live run), and `unit_is_obsolete` never
/// retired a predecessor whose successor costs a resource. Measured on the won
/// game civvis-20260816T054344Z: a Trebuchet ordered on 29 turns across eight
/// cities, each `civvis_build_unplayable` — the host had Niter and a Bombard,
/// the board had neither. Translated from the host's `RESOURCE_X` to CIVVIS's
/// `x`; a resource the ruleset does not know is reported, not dropped.
pub(crate) fn apply_strategic_stockpiles(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    let Some(seat) = game.players.get_mut(0) else {
        return;
    };
    seat.strategic_resources.clear();
    let Some(stock) = state.strategic_resources.as_ref() else {
        return;
    };
    for (host, amount) in stock {
        let name = host
            .strip_prefix("RESOURCE_")
            .unwrap_or(host)
            .to_ascii_lowercase();
        if game.rules.resources.contains_key(&Name::new(&name)) {
            seat.strategic_resources
                .insert(Name::new(&name), amount.max(0.0));
        } else {
            let issue = format!("strategic_resource:{host}");
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
        }
    }
}

pub(super) fn apply_governor_state(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    let Some(governors) = state.governors.as_deref() else {
        return;
    };

    let mut roster = crate::specmap::SpecMap::default();
    let mut assigned_own_cities = Vec::new();
    for observed in governors {
        let Some(name) = civvis_governor_name(&observed.kind) else {
            let issue = format!("{}:governor", observed.kind);
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
            continue;
        };
        let Some(spec) = game.rules.governors.get(name) else {
            let issue = format!("{}:governor_rules", observed.kind);
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
            continue;
        };
        let establish_turns = game.standard_duration(spec.establish_turns);
        let located_city = (observed.x >= 0 && observed.y >= 0)
            .then(|| game.city_at(crate::hex::offset_to_axial(observed.x, observed.y)))
            .flatten();
        if observed.city >= 0 && located_city.is_none() {
            let issue = format!(
                "{}:governor_city@{},{}",
                observed.kind, observed.x, observed.y
            );
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
        }
        let city = located_city.filter(|cid| {
            game.cities.get(cid).is_some_and(|target| {
                target.owner == 0
                    || (name == "amani"
                        && game.players[target.owner].is_minor
                        && !game.players[target.owner].is_barbarian)
            })
        });
        if located_city.is_some() && city.is_none() {
            let issue = format!("{}:invalid_governor_owner", observed.kind);
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
        }
        if let Some(cid) = city {
            if game.cities[&cid].owner == 0 {
                assigned_own_cities.push(cid);
            }
        }
        let assigned_turn = if city.is_none() {
            game.turn
        } else if observed.established {
            game.turn.saturating_sub(establish_turns)
        } else {
            game.turn
                .saturating_sub(observed.turns_on_site.max(0) as u32)
        };
        let mut promotions = std::collections::BTreeSet::new();
        for host_promotion in &observed.promotions {
            if civ6_governor_base_promotion(name) == Some(host_promotion.as_str()) {
                // Firaxis includes the Governor's appointment/base ability in
                // GetPromotions(). CIVVIS stores that ability on GovernorSpec,
                // outside GovernorState.promotions, so importing it would either
                // duplicate its effects or falsely report a known host fact as
                // unmapped.
                continue;
            }
            match civvis_governor_promotion(host_promotion) {
                Some(promotion) if spec.promotions.contains_key(promotion) => {
                    promotions.insert(promotion.to_string());
                }
                _ => {
                    let issue = format!("{}:governor_promotion", host_promotion);
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                }
            }
        }
        roster.insert(
            name.to_string(),
            crate::game::GovernorState {
                city,
                assigned_turn,
                disabled_until: game
                    .turn
                    .saturating_add(observed.neutralized_turns.max(0) as u32),
                promotions,
            },
        );
    }

    assigned_own_cities.sort_unstable();
    assigned_own_cities.dedup();
    game.players[0].governor_roster = roster;
    game.players[0].governors = assigned_own_cities;
    if let Some(spent) = state.governor_points_spent.filter(|value| *value >= 0) {
        game.players[0].governor_titles_spent = spent as usize;
    }
    if let Some(total) = state.governor_points.filter(|value| *value >= 0) {
        let civic_titles: usize = game.players[0]
            .civics
            .iter()
            .filter_map(|civic| game.rules.civics.get(civic))
            .map(|civic| civic.governor_title)
            .sum();
        let other = (total as usize).saturating_sub(civic_titles);
        if total as usize >= civic_titles {
            game.players[0]
                .counters
                .insert("district_governor_titles".to_string(), other as i64);
        } else {
            game.players[0].counters.remove("district_governor_titles");
            let issue = format!("governor_titles:{total}<civic:{civic_titles}");
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
        }
    }
}

/// Give every seat the civilization Civilization VI is actually playing.
///
/// ⚠ MUST RUN BEFORE ANY CITY IS PLACED. `found_city_for` reads `players[pid].civ`
/// to name a city, so setting identity afterwards leaves the old roster's names on
/// the board — the visible half of the very bug this fixes.
pub(super) fn apply_identity(game: &mut crate::game::Game, state: &StateSnapshot) -> Vec<String> {
    // ★★★★★ THE GAME SPEED CROSSED THE BRIDGE AND WAS THROWN AWAY, so every cost
    // CIVVIS reasoned about was DOUBLE the real one.
    //
    // The ladder plays `GAMESPEED_ONLINE`, the `seat` event carries it, and `Seat`
    // even deserializes it — and `grep -n game_speed src/mirror.rs` answered ZERO,
    // so a mirrored game kept `GameSpeed::Standard`, the `#[default]`. Online is
    // `cost_percent: 50`.
    //
    // What that scales is not a corner: `Game::game_speed` multiplies tech cost,
    // civic cost, the growth threshold, item/production cost and turn durations —
    // `src/lib.rs` has a test called `game_speed_scales_every_cost`. So CIVVIS
    // planned against a world where a settler, a tech and a district each took
    // twice as long as the game would actually charge, on every turn of every run.
    //
    // Same shape as the districts that were carried on `StateCity` and never
    // written onto a city: the field crossed, and nothing read it.
    //
    // ⚠ DIFFICULTY IS DELIBERATELY NOT SET HERE. `Seat` carries it too, but a
    // mirrored rival's strength ALREADY includes its handicap — that is what the
    // export measured — so applying the difficulty on top would count the bonus
    // twice. Speed has no such double-count: it is a cost curve, not a bonus.
    if let Some(speed) = civvis_game_speed(&state.seat.speed) {
        game.game_speed = speed;
    }
    apply_seat_victories(game, &state.seat);
    Arc::make_mut(&mut game.observed_leader_types).clear();
    if !state.seat.leader.is_empty() {
        Arc::make_mut(&mut game.observed_leader_types).insert(0, state.seat.leader.clone());
    }
    let seat_of_host = host_major_seat_map(state, modeled_major_player_count(game));
    for rival in &state.rivals {
        let Some(&seat) = seat_of_host.get(&rival.player) else {
            continue;
        };
        if !rival.leader.is_empty() {
            Arc::make_mut(&mut game.observed_leader_types).insert(seat, rival.leader.clone());
        }
    }
    let mut unmapped = Vec::new();
    let mut known: std::collections::BTreeMap<usize, &'static str> = Default::default();
    let mut note = |seat: usize, civ6: &str, unmapped: &mut Vec<String>| {
        if civ6.is_empty() || seat >= game.players.len() {
            return;
        }
        match civvis_civ_name(civ6) {
            Some(name) => {
                known.insert(seat, name);
            }
            None => unmapped.push(civ6.to_string()),
        }
    };
    note(0, &state.seat.civ, &mut unmapped);
    // A met-gated rival list is not a seat list: use the original Firaxis id
    // through the shared map so a newly met player cannot inherit another
    // civilization's identity when an earlier export omitted them.
    for rival in &state.rivals {
        if let Some(&seat) = seat_of_host.get(&rival.player) {
            note(seat, &rival.civ, &mut unmapped);
        }
    }
    for (minor, seat) in minor_actor_assignments(game, state)
        .into_iter()
        .filter(|(minor, _)| minor.is_city_state())
    {
        match mirrored_city_state_name(game, minor) {
            Some(name) => game.players[seat].civ = name,
            None if !minor.civ.is_empty() => unmapped.push(minor.civ.clone()),
            None => {}
        }
    }
    for (&seat, &name) in &known {
        game.players[seat].civ = name.to_string();
    }

    // ⚠ MOVE THE DEFAULTS OUT OF THE WAY. Seats we have not met keep CIVVIS's roster
    // name, and that roster can collide with one we just learned: Civilization VI
    // named player 1 Greece, `CIV_NAMES[2]` is also Greece, and the standings table
    // showed TWO Greeces. A duplicate is worse than an unknown, because it looks like
    // a real second civilization rather than a seat nobody has met yet.
    let taken: std::collections::BTreeSet<String> =
        known.values().map(|name| name.to_string()).collect();
    let mut spare = crate::game::CIV_NAMES
        .iter()
        .filter(|name| !taken.contains(**name));
    for seat in 0..game.players.len() {
        if known.contains_key(&seat) || game.players[seat].is_minor {
            continue;
        }
        // Free Cities and the barbarians are seats, but they are not civilizations
        // and renaming them would invent a rival that does not exist.
        if !taken.contains(&game.players[seat].civ) {
            continue;
        }
        if let Some(name) = spare.next() {
            game.players[seat].civ = name.to_string();
        }
    }
    unmapped
}

pub(super) fn apply_host_policy_choices(game: &mut crate::game::Game, state: &StateSnapshot) {
    policy_windows::apply(game, state);
    if let Some(names) = &state.available_policies {
        let choices = names
            .iter()
            .filter_map(|name| civvis_node_name(&game.rules.policies, name, "POLICY_"))
            .map(|name| Name::new(&name))
            .collect();
        game.host_policy_choices.insert(0, choices);
    } else {
        game.host_policy_choices.remove(&0);
    }
}

/// The host treasury's bill by source for the seat — see
/// `Game::host_maintenance`. A reading that is absent, non-finite or negative
/// is no reading, and a state with none of the three leaves the board billing
/// itself.
pub(super) fn apply_host_maintenance(game: &mut crate::game::Game, state: &StateSnapshot) {
    let reading = |value: Option<f64>| value.filter(|value| value.is_finite() && *value >= 0.0);
    let bill = crate::game::HostMaintenance {
        units: reading(state.unit_maintenance_total),
        buildings: reading(state.building_maintenance_total),
        districts: reading(state.district_maintenance_total),
    };
    if bill.units.is_some() || bill.buildings.is_some() || bill.districts.is_some() {
        Arc::make_mut(&mut game.host_maintenance).insert(0, bill);
    } else {
        Arc::make_mut(&mut game.host_maintenance).remove(&0);
    }
}

pub(super) fn apply_player_religion(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    game.players[0].prophet_pending = state.prophet_pending;
    let local_religion = state
        .founded_religion
        .as_deref()
        .and_then(civvis_religion_name);
    game.players[0].religion = local_religion.clone();

    // Religion availability is capped by the number already founded worldwide.
    // Preserve exact Firaxis names on otherwise-unidentified major seats so
    // `religions_founded()` sees the same count without claiming knowledge of an
    // unmet founder's civilization. Globally claimed beliefs are handled below.
    if !state.founded_religions.is_empty() {
        let foreign_seats = game
            .players
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, player)| !player.is_minor && !player.is_barbarian)
            .map(|(seat, _)| seat)
            .collect::<Vec<_>>();
        for seat in &foreign_seats {
            game.players[*seat].religion = None;
        }
        let mut foreign = Vec::new();
        for civ6 in &state.founded_religions {
            if let Some(name) = civvis_religion_name(civ6) {
                if local_religion.as_ref() != Some(&name) && !foreign.contains(&name) {
                    foreign.push(name);
                }
            }
        }
        for (seat, name) in foreign_seats.into_iter().zip(foreign) {
            game.players[seat].religion = Some(name);
        }
    }

    let mut local = Vec::new();
    for civ6 in &state.religion_beliefs {
        match civvis_belief_name(&game.rules, civ6) {
            Some(name) if !local.contains(&name) => local.push(name),
            Some(_) => {}
            None => {
                let gap = format!("{civ6}:belief");
                if !unmapped.contains(&gap) {
                    unmapped.push(gap);
                }
            }
        }
    }
    game.players[0].religion_beliefs = local.clone();

    // With the host's per-religion list, every religion's beliefs sit on its
    // founder's seat and nowhere else: a city following Catholicism then reads
    // exactly Catholicism's follower beliefs (`city_religion_belief_effect`),
    // whoever founded it and whether or not the mirror has met them. A founder
    // id maps through the stable host-seat map; a founder the
    // seat has not met keeps the anonymous seat `founded_religions` gave the
    // religion above. Every belief named here is also globally claimed, which
    // is all `taken_religion_beliefs` ever said.
    if !state.religions.is_empty() {
        let seat_of_host: BTreeMap<i64, usize> =
            host_major_seat_map(state, modeled_major_player_count(game))
                .into_iter()
                .filter_map(|(host, seat)| i64::try_from(host).ok().map(|host| (host, seat)))
                .collect();
        let foreign_seats: Vec<usize> = game
            .players
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, player)| !player.is_minor && !player.is_barbarian)
            .map(|(seat, _)| seat)
            .collect();
        let mut translated: Vec<(String, Option<usize>, Vec<String>)> = Vec::new();
        for religion in &state.religions {
            let Some(name) = civvis_religion_name(&religion.religion) else {
                continue;
            };
            if translated.iter().any(|(known, _, _)| *known == name) {
                continue;
            }
            let mut beliefs = Vec::new();
            for civ6 in &religion.beliefs {
                match civvis_belief_name(&game.rules, civ6) {
                    Some(belief) if !beliefs.contains(&belief) => beliefs.push(belief),
                    Some(_) => {}
                    None => {
                        let gap = format!("{civ6}:belief");
                        if !unmapped.contains(&gap) {
                            unmapped.push(gap);
                        }
                    }
                }
            }
            let seat = if local_religion.as_ref() == Some(&name) {
                Some(0)
            } else {
                seat_of_host
                    .get(&religion.founder)
                    .copied()
                    .filter(|seat| foreign_seats.contains(seat))
            };
            translated.push((name, seat, beliefs));
        }
        // Founders the seat has met take their religion onto their own seat;
        // the rest go to whatever foreign seats are left, so the count the
        // engine gates Prophets on (`religions_founded`) stays the host's.
        for &seat in &foreign_seats {
            game.players[seat].religion = None;
            game.players[seat].religion_beliefs.clear();
        }
        let mut spare = foreign_seats
            .iter()
            .copied()
            .filter(|seat| !translated.iter().any(|(_, taken, _)| *taken == Some(*seat)))
            .collect::<Vec<_>>()
            .into_iter();
        for (name, seat, beliefs) in translated {
            let seat = match seat {
                Some(seat) => seat,
                None => match spare.next() {
                    Some(seat) => seat,
                    None => continue,
                },
            };
            if seat != 0 {
                game.players[seat].religion = Some(name);
            }
            game.players[seat].religion_beliefs = beliefs;
        }
        return;
    }

    // Firaxis belief availability is global. One non-local seat can retain the
    // union without pretending we know which unseen founder owns which belief.
    if game.players.len() > 1 {
        let mut claimed_elsewhere = Vec::new();
        for civ6 in &state.taken_religion_beliefs {
            if let Some(name) = civvis_belief_name(&game.rules, civ6) {
                if !local.contains(&name) && !claimed_elsewhere.contains(&name) {
                    claimed_elsewhere.push(name);
                }
            }
        }
        game.players[1].religion_beliefs = claimed_elsewhere;
    }
}

pub(super) fn apply_unit_observation(
    live: &mut crate::game::Unit,
    state: &StateUnit,
    progress: ObservedUnitProgress,
) {
    if state.hp.is_finite() && state.hp > 0.0 {
        live.hp = (state.hp.round() as i32).clamp(1, 100);
    }
    if let Some(xp) = state.xp.filter(|xp| *xp >= 0) {
        live.xp = xp;
    }
    if let Some(level) = state.level.filter(|level| *level >= 1) {
        live.level = level;
    }
    if live.kind == "rock_band" {
        if let Some(sales) = state.album_sales.filter(|sales| *sales >= 0) {
            live.album_sales = sales;
        }
    }
    if let Some(promotions) = progress.promotions {
        live.promotions = promotions;
    }
    if let Some(religion) = progress.religion {
        live.religion = Some(religion);
    }
    // Zero is an authoritative observation: a Builder or religious unit that
    // spent its final charge must not regain the ruleset default on a fresh
    // rebuild, or retain the previous turn's charge count on sync. Negative
    // values remain the mod's "could not read" sentinel.
    let observed_charges = state
        .build_charges
        .into_iter()
        .chain(state.spread_charges)
        .filter(|charges| *charges >= 0)
        .max();
    // Naturalists and Archaeologists execute DESIGNATE_PARK / EXCAVATE,
    // not BUILD_IMPROVEMENT. Their reported Builder/spread zero is not an
    // exhausted Culture operation counter. Keep the simulator's planning
    // budget; native CanStartOperation remains the final legality check.
    // This is deliberately not an assertion of remaining native artifacts.
    if !matches!(live.kind.as_str(), "naturalist" | "archaeologist") {
        if let Some(charges) = observed_charges {
            live.charges = charges;
        }
    }
    live.fortified = state.fortified;
    live.fortify_turns = state.fortify_turns.clamp(0, 2);
    // The host's own `IsEmbarked`, pinned to the observed position: it wins over
    // the "its tile is water" derivation while the unit stands there and lapses
    // the moment the board moves it. Exported since 2026-08 and read by nothing
    // until now; an older export carries `None` and derives as before.
    live.host_embarked = state.embarked.map(|embarked| (live.pos, embarked));
    // ★★★★★ THE LINE THAT MAKES `FORM_ARMY` REACHABLE ON THE LIVE SEAT.
    //
    // `civvis_orders::translate` chooses between `UNITCOMMAND_FORM_CORPS` and
    // `UNITCOMMAND_FORM_ARMY` by reading this unit's `formation` off the mirror
    // (#2373). Nothing ever wrote it, and the live seat rebuilds the mirror from
    // the host export every turn, so every unit was reconstructed at tier 0 and
    // an existing Corps was asked to form another Corps. Firaxis models the two
    // merges as different commands behind different civics — Nationalism and
    // Mobilization — so that is not a near miss, it is the wrong order.
    //
    // ⚠ ONLY A REAL READING WRITES. `None` (an older export) and the mod's `-1`
    // ("asked, could not answer") are unknown, and unknown must not flatten a
    // board that already knows better — a mirror carried across turns may hold a
    // tier CIVVIS itself raised. Accepting the sentinel here is how
    // `GetDefenseStrength` read −1 for the project's whole life unnoticed.
    if let Some(tier) = state.formation.filter(|tier| (0..=2).contains(tier)) {
        live.formation = tier as u8;
    }
    // ⚠ `production_cost` is deliberately NOT rescaled by the tier here.
    // `Game::set_unit_formation` multiplies it by 1.0/1.5/2.0, but that is
    // lifetime accounting only (`unit_accounting_cost`, damage-per-cost) and this
    // function runs on every observation, including repeat syncs of a unit that
    // already carries the tier — so scaling here would compound on each turn
    // rather than converge.
}

pub(super) fn apply_observed_host_metrics(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    snapshot: Option<&Snapshot>,
    unmapped: &mut Vec<String>,
) {
    Arc::make_mut(&mut game.observed_trade_capacity).clear();
    Arc::make_mut(&mut game.observed_yield_adjustments).clear();
    Arc::make_mut(&mut game.observed_public_empire_stats).clear();
    Arc::make_mut(&mut game.observed_majority_religion).clear();
    Arc::make_mut(&mut game.observed_visiting_tourists).clear();
    Arc::make_mut(&mut game.observed_city_loyalty_per_turn).clear();
    Arc::make_mut(&mut game.observed_city_strength).clear();
    Arc::make_mut(&mut game.observed_city_ranged_strength).clear();
    Arc::make_mut(&mut game.observed_city_max_wall_hp).clear();
    Arc::make_mut(&mut game.observed_tourism_per_turn).clear();
    if let Some(capacity) = state.trade_capacity.filter(|capacity| *capacity >= 0) {
        Arc::make_mut(&mut game.observed_trade_capacity).insert(0, capacity);
    }
    // Our tourism per turn as the host counts it; `Game::tourism_per_turn`
    // prefers it, the model figure stays behind `tourism_per_turn_model`.
    if let Some(tourism) = state
        .tourism_per_turn
        .filter(|t| t.is_finite() && *t >= 0.0)
    {
        Arc::make_mut(&mut game.observed_tourism_per_turn).insert(0, tourism);
    }
    apply_public_empire_stats(game, 0, &state.public_stats);
    {
        // Same per-snapshot honesty as the rival counters: unknown is None.
        let count = |value: f64| {
            (value.is_finite() && value >= 0.0 && value <= usize::MAX as f64)
                .then(|| value.round() as usize)
        };
        let observed = Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(0)
            .or_default();
        observed.foreign_tourists = count(state.foreign_tourists);
        observed.domestic_tourists = count(state.domestic_tourists);
        observed.culture_turns_to_victory = state
            .culture_turns_to_victory
            .is_finite()
            .then_some(state.culture_turns_to_victory);
        // Match the local player's World Rankings science lane to the host
        // instead of treating its reconstructed fifty-light-year trip as fact.
        observed.science_victory_points = (state.science_victory_points.is_finite()
            && state.science_victory_points >= 0.0)
            .then_some(state.science_victory_points);
        observed.science_victory_points_per_turn =
            (state.science_victory_points_per_turn.is_finite()
                && state.science_victory_points_per_turn >= 0.0)
                .then_some(state.science_victory_points_per_turn);
        observed.science_victory_points_needed = (state.science_victory_points_needed.is_finite()
            && state.science_victory_points_needed > 0.0)
            .then_some(state.science_victory_points_needed);
        observed.cities_following_religion = state
            .cities_following_religion
            .filter(|value| *value >= 0)
            .map(|value| value as usize);
    }

    // These fields participate in the model itself, so settle them before
    // measuring the host-to-model city correction.
    apply_observed_city_facts(game, state);
    apply_observed_city_economy(game, state, snapshot, unmapped);

    let mut derived = crate::rules::Yields::default();
    for cid in game.player_city_ids(0) {
        derived.add(game.city_yields(cid));
    }
    // The empire collects founder-belief income and the Faith for unused
    // Great Person points beside its cities, and every reader of the per-turn
    // figure adds them (`player_yield_extras`), so the residual measured here
    // is only what CIVVIS still cannot derive.
    derived.add(game.player_yield_extras(0));
    derived.add(game.arena_side_yields(0));
    let mut adjustment = crate::rules::Yields::default();
    if let Some(host_food) = state
        .public_stats
        .food
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        adjustment.food = host_food - derived.food;
    }
    if let Some(host_production) = state
        .public_stats
        .production
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        adjustment.production = host_production - derived.production;
    }
    if let Some(host_science) = state
        .science
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        adjustment.science = host_science - derived.science;
    }
    if let Some(host_culture) = state
        .culture
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        adjustment.culture = host_culture - derived.culture;
    }
    // Faith per turn: the host's top-bar figure against the same sum, applied
    // as a delta like science and culture. Only when the export carries it —
    // an older control mod leaves the model's own figure standing.
    if let Some(host_faith) = state
        .faith_per_turn
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        adjustment.faith = host_faith - derived.faith;
    }
    if adjustment.food != 0.0
        || adjustment.production != 0.0
        || adjustment.science != 0.0
        || adjustment.culture != 0.0
        || adjustment.faith != 0.0
    {
        Arc::make_mut(&mut game.observed_yield_adjustments).insert(0, adjustment);
    }
    // ★★★★★ THE RIVALS' SEATS LAST, AFTER THEIR CITIES ARE FINISHED.
    //
    // A correction is `host − model`, and the model of a rival city moves
    // with every fact the loop above writes onto it — above all its
    // Population, which is the term every yield is a linear function of and
    // which arrives here (rival cities are planted at population one). Derived
    // before that loop, the delta was measured against a size-one city and
    // then paid on the size-eleven one: on run civvis-20260816T175306Z the
    // board read Nubia at 174 Science against the host's 141, 329 Food against
    // 229, every rival over by its own growth. Same reason the seat's own
    // Dedications are applied before this function runs (`apply_player_ages`
    // in both callers). Rival owners come from the stable host-seat map, not
    // from the met-gated export position.
    let seat_of_host = host_major_seat_map(state, modeled_major_player_count(game));
    for rival in &state.rivals {
        let Some(&owner) = seat_of_host.get(&rival.player) else {
            continue;
        };
        if owner >= game.players.len() {
            continue;
        }
        apply_rival_public_economy(game, owner, rival, unmapped);
    }
    // Rebuild from this authoritative frame: revealing the remaining cities
    // must clear the guard so a genuine last-city capture can eliminate a seat.
    game.unseen_city_owners = game
        .observed_public_empire_stats
        .iter()
        .filter_map(|(&owner, stats)| {
            stats
                .city_count
                .filter(|&count| count > game.player_city_ids(owner).len())
                .map(|_| owner)
        })
        .collect();
}

#[cfg(test)]
mod band_facts_tests;
