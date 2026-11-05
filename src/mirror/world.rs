//! The world: writing world-level state into the rebuilt game.
//!
//! The World Congress and diplomatic victory points, competitions,
//! city-state quests, the climate, ages and the era countdown, the
//! victories each seat has banked, and research quotes.
//!
//! Moved verbatim out of `mirror.rs` (2026-11-05). `mirror.rs` re-exports
//! everything here, so every path is unchanged. Items the rest of `mirror`
//! uses are `pub(super)`; the code is otherwise unchanged.

use super::*;

pub(super) fn apply_research_quotes(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    let mut quotes = BTreeMap::new();
    for row in &state.research_quotes {
        let quote = crate::game::HostResearchQuote {
            cost: row.c,
            progress: row.p,
        };
        let name = civvis_node_name(&game.rules.techs, &row.t, "TECH_");
        if let Some(name) = name.filter(|_| quote.remaining().is_some()) {
            quotes.insert(Name::new(&name), quote);
        } else {
            let diagnostic = format!("research_quote:{}", row.t);
            if !unmapped.contains(&diagnostic) {
                unmapped.push(diagnostic);
            }
        }
    }
    // Replace, not union: a refreshed or older export cannot retain stale prices
    // or already-spent progress. These quotes belong only to the mirrored seat.
    game.host_research_quotes = Arc::new(BTreeMap::from([(0, quotes)]));
}

/// Put the host's binding World Congress resolutions on the board.
///
/// The model has its own Congress and simulates one when it plans ahead, but on
/// a mirrored board the host's is the one in force: Trade Policy A on this seat
/// (run civvis-20260816T200454Z, t82-101) paid +4 Gold per incoming foreign
/// route in Cumae and +1 route capacity, and Luxury Policy changes what every
/// city's Amenities are. An export without `resolutions` (older mod) leaves the
/// model's own list alone. Effects expire when the host says the next session
/// convenes (`congress_turns_left`), or a standard session length out if the
/// export does not say. Called BEFORE the host-to-model corrections are
/// measured, for the same reason as the age and Dedications: anything that
/// changes model yields must be on the board first.
pub(super) fn apply_host_congress(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    seat_of_host: &std::collections::BTreeMap<usize, usize>,
    unmapped: &mut Vec<String>,
) {
    let Some(resolutions) = &state.resolutions else {
        return;
    };
    let turns_left = state
        .congress_turns_left
        .filter(|turns| *turns >= 0)
        .map(|turns| turns as u32)
        .unwrap_or_else(|| game.standard_duration(30));
    let expires = game.turn.saturating_add(turns_left).saturating_add(1);
    game.active_congress_effects.clear();
    let congress_seat_of_host = congress_seat_of_host(state, seat_of_host, game.players.len());
    for resolution in resolutions {
        match civvis_congress_effect(&game.rules, resolution, &congress_seat_of_host, expires) {
            Some(effect) => game.active_congress_effects.push(effect),
            None if is_known_congress_noop(resolution) => {}
            None => {
                let issue = format!(
                    "congress:{}:{}:{}",
                    resolution.kind, resolution.option, resolution.target
                );
                if !unmapped.contains(&issue) {
                    unmapped.push(issue);
                }
            }
        }
    }
}

/// Copy the host's victory checkboxes onto the mirrored game. Firaxis's
/// `technology`/`conquest` names map to CIVVIS's `science`/`domination`.
pub(super) fn apply_seat_victories(game: &mut crate::game::Game, seat: &Seat) {
    let Some(v) = seat.victories.as_ref() else {
        return;
    };
    let conditions = &mut game.victory_conditions;
    if let Some(on) = v.technology {
        conditions.science = on;
    }
    if let Some(on) = v.culture {
        conditions.culture = on;
    }
    if let Some(on) = v.religious {
        conditions.religious = on;
    }
    if let Some(on) = v.diplomatic {
        conditions.diplomatic = on;
    }
    if let Some(on) = v.conquest {
        conditions.domination = on;
    }
    if let Some(on) = v.score {
        conditions.score = on;
    }
}

/// Put Firaxis's Era Score and age thresholds on the reconstructed board.
///
/// Without this the age CIVVIS reasons about is `Player::default`'s: era score
/// 0 against a golden threshold of 26 and a normal threshold of 12, on turn 1
/// and on turn 200 alike. Both age-reading decisions — `choose_dedications` and
/// the Dark Age wildcard filter in `ai/advanced.rs` — then run against a
/// standing fiction rather than against Rome's actual standing.
///
/// Every field is optional and a negative value means "the host did not
/// answer", so a build whose `Game.GetEras()` is missing a getter leaves that
/// field at whatever the board already held instead of writing a lie into it. A
/// real era score of 0 is ordinary on turn 1 and is applied.
pub(super) fn apply_observed_age(
    player: &mut crate::game::Player,
    heroic_golden_age: Option<bool>,
    golden_age: Option<bool>,
    dark_age: Option<bool>,
) {
    // Heroic outranks Golden. An all-false response is the host's explicit
    // Normal answer; an absent field belongs to an older control mod and must
    // leave the previous observation intact.
    match (heroic_golden_age, golden_age, dark_age) {
        (Some(true), _, _) => player.age = "heroic".to_string(),
        (_, Some(true), _) => player.age = "golden".to_string(),
        (_, _, Some(true)) => player.age = "dark".to_string(),
        (Some(false), Some(false), Some(false)) => player.age = "normal".to_string(),
        _ => {}
    }
}

pub(super) fn apply_player_ages(game: &mut crate::game::Game, state: &StateSnapshot) {
    let player = &mut game.players[0];
    if let Some(activity) = &state.observed_dedication_activity {
        player.last_era_triggers.clone_from(activity);
    }
    if let Some(score) = state.era_score.filter(|value| *value >= 0) {
        player.era_score = score;
    }
    if let Some(baseline) = state.era_score_baseline.filter(|value| *value >= 0) {
        player.era_score_baseline = baseline;
    }
    // Firaxis's Dark Age threshold IS this codebase's normal-age threshold: the
    // score at or above which the next age is Normal rather than Dark.
    if let Some(normal) = state.normal_age_threshold.filter(|value| *value >= 0) {
        player.normal_age_threshold = normal;
    }
    if let Some(golden) = state.golden_age_threshold.filter(|value| *value >= 0) {
        player.golden_age_threshold = golden;
    }
    // ★★★★ THE AGE ITSELF. The three flags crossed and were read by nothing:
    // `Player::age` stayed at its "normal" default on every mirrored board, so
    // `dedication_active` was false for the whole of every live Golden Age and
    // no Dedication ever paid — the host's production ledger showing "+10 from
    // Campus" (Heartbeat of Steam) against a model paying nothing was the
    // largest gap of run civvis-20260816T132247Z.
    apply_observed_age(
        player,
        state.heroic_golden_age,
        state.golden_age,
        state.dark_age,
    );
    // And what it pays: the host's active Commemorations, by their CIVVIS ids.
    // An older export (None) leaves the model's own list alone.
    if let Some(active) = &state.dedications {
        player.dedications.clear();
        for name in active {
            if let Some(dedication) = civvis_dedication_name(name) {
                player.dedications.insert(dedication.to_string());
            }
        }
    }
    if let Some(choices) = state.dedication_choices.filter(|value| *value >= 0) {
        player.dedication_choices = choices as usize;
    }
    if let Some(era) = state.world_era.filter(|value| *value >= 0) {
        // `ERA_NAMES` bounds the model's era ladder; a build that reports an era
        // past its end is clamped rather than allowed to index out of range.
        game.world_era = (era as usize).min(crate::rules::ERA_NAMES.len() - 1);
    }
    apply_era_countdown(game, state.era_countdown);
}

/// The host's era countdown as the model's `world_era_countdown_end`: the turn
/// on which `Game::process_eras` opens the next era. The simulator advances the
/// era at the start of turn `end`, so the old era's last turn is `end - 1`;
/// Firaxis counts 0 on that last turn, hence `turn + countdown + 1`. A negative
/// countdown (not started) clears it: the countdown always runs for
/// `NEXT_ERA_COUNTDOWN_TURNS` once it starts, so an unstarted era cannot end
/// inside `age-closer-2`'s window. An older export (`None`) leaves the model's
/// own value alone.
fn apply_era_countdown(game: &mut crate::game::Game, countdown: Option<i64>) {
    let Some(countdown) = countdown else {
        return;
    };
    game.world_era_countdown_end = u32::try_from(countdown)
        .ok()
        .map(|left| game.turn.saturating_add(left).saturating_add(1));
}

/// Seat the World Congress diplomatic standing, including the majors this seat
/// has not met.
///
/// The rival list is met-gated, so its positions cannot identify seats. The
/// `state.seat.players` roster lets this attach a congress entry to the same
/// stable model seat as the rival loop, including a major omitted from an
/// earlier export.
///
/// Deliberately conservative in three ways:
///
/// - a met rival keeps its own per-turn export, which is sampled every turn
///   against a congress table refreshed once a session. Only a rival whose
///   `dvp` the mod could not read falls back to the congress value. DVP can
///   go *down* (`WC_RES_DIPLOVICTORY` option B is −2), so preferring the
///   larger of the two would latch a stale high standing.
/// - the seat's own entry is ignored; `state.dvp` is the live read.
/// - unmet entries are seated in ascending host-player order, so the same
///   congress table always lands the same way.
pub(super) fn apply_congress_dvp(game: &mut crate::game::Game, state: &StateSnapshot) {
    let Some(congress) = &state.congress_dvp else {
        return;
    };
    // One entry per alive major, the seat included: the host's count of the
    // living, which the board's planted seats cannot give once a major is
    // eliminated. Read by `retire_defeated_majors`.
    if !congress.points.is_empty() {
        game.players[0].live_living_majors = Some(congress.points.len());
    }
    // Host player id -> mirror seat, exactly as the rival loops assign them.
    let seat_of = host_major_seat_map(state, modeled_major_player_count(game));
    let ours = state.seat.local_player.max(0) as usize;
    let mut unmet: Vec<&StateCongressDvpEntry> = Vec::new();
    for entry in &congress.points {
        if entry.player == ours {
            continue;
        }
        match seat_of.get(&entry.player) {
            // A met rival the mod could not read a live `dvp` for. Everyone
            // else keeps the fresher per-turn number.
            Some(&seat) => {
                let stale = state
                    .rivals
                    .iter()
                    .find(|rival| seat_of.get(&rival.player).copied() == Some(seat))
                    .is_none_or(|rival| rival.dvp.is_none());
                if stale && seat < game.players.len() {
                    game.players[seat].dvp = entry.points;
                }
            }
            None => unmet.push(entry),
        }
    }
    unmet.sort_by_key(|entry| entry.player);
    let occupied: BTreeSet<usize> = seat_of.values().copied().collect();
    let free_seats = (1..modeled_major_player_count(game)).filter(|seat| !occupied.contains(seat));
    for (seat, entry) in free_seats.zip(unmet) {
        if seat >= game.players.len() {
            break;
        }
        game.players[seat].dvp = entry.points;
    }
}

/// Mirror the short-lived World Congress competitions that the host has
/// already made available to this seat. Firaxis grants their projects through
/// `UnlocksFromEffect`, so retaining an old answer would be worse than losing
/// one: it would make CIVVIS issue a project the host has withdrawn.
pub(super) fn apply_host_competitions(game: &mut crate::game::Game, state: &StateSnapshot) {
    let Some(emergencies) = state.emergencies.as_ref() else {
        // An absent field is an older mod, not proof that a competition ended.
        return;
    };
    let turn = state.turn.max(1);
    let mut competitions: Vec<crate::game::HostCompetition> = emergencies
        .iter()
        .filter_map(|emergency| {
            if !emergency.begun
                || emergency.turns_left < 0
                || !emergency.ours.member
                || emergency.kind.trim().is_empty()
            {
                return None;
            }
            let ours = emergency
                .ours
                .score
                .filter(|score| score.is_finite())
                .unwrap_or(0.0);
            let leader = emergency
                .scores
                .iter()
                .map(|score| score.score)
                .filter(|score| score.is_finite())
                .fold(ours, f64::max);
            let remaining = u32::try_from(emergency.turns_left).unwrap_or(u32::MAX);
            Some(crate::game::HostCompetition {
                kind: emergency.kind.trim().to_string(),
                // `TurnsLeft == 0` is still the host's final playable turn.
                ends: turn.saturating_add(remaining).saturating_add(1),
                ours,
                leader,
            })
        })
        .collect();
    competitions.sort_by(|left, right| left.kind.cmp(&right.kind));
    game.replace_host_competitions(competitions);
}

/// Seat the request a city-state is making of us, from the host's
/// QuestsManager, on the pair (`Player::quests[minor]`) where
/// `Game::city_state_quest` and the `quest-*` genes read it. The board rolled
/// its own quest for every pair from a hash and paid itself the Envoy when its
/// model said so; the host's actual request never crossed.
///
/// The host's `QUEST_*` type is the board's kind by name (`QUEST_TRAIN_UNIT_TYPE`
/// → `train_unit_type`, all eight); the target the mod recovered from the
/// description is translated the way every other host name is, and an
/// untranslatable one is filed under `unmapped` and kept verbatim, so the
/// kind still reads. `None` (an older export) leaves the board's own roll;
/// `Some([])` clears it.
pub(super) fn apply_host_quest(
    game: &mut crate::game::Game,
    minor: &StateMinor,
    owner: usize,
    unmapped: &mut Vec<String>,
) {
    let Some(quests) = minor.quests.as_ref() else {
        return;
    };
    let mut note = |issue: String| {
        if !unmapped.contains(&issue) {
            unmapped.push(issue);
        }
    };
    game.players[0].quests.remove(&owner);
    let Some(quest) = quests.first() else {
        return;
    };
    let kind = quest
        .kind
        .strip_prefix("QUEST_")
        .unwrap_or(&quest.kind)
        .to_ascii_lowercase();
    if !crate::game::quests::QUEST_KINDS.contains(&kind.as_str()) {
        note(format!("quest:{}", quest.kind));
        return;
    }
    let host_target = quest.target.as_deref().unwrap_or("");
    let translated = match kind.as_str() {
        "train_unit_type" => resolved_civvis_unit_name(&game.rules, host_target),
        "zone_district_type" => civvis_node_name(&game.rules.districts, host_target, "DISTRICT_")
            .map(|district| game.district_family(Name::new(&district)).to_string()),
        "trigger_tech_boost" => civvis_node_name(&game.rules.techs, host_target, "TECH_"),
        "trigger_civic_boost" => civvis_node_name(&game.rules.civics, host_target, "CIVIC_"),
        "recruit_great_person_class" => host_target
            .strip_prefix("GREAT_PERSON_CLASS_")
            .map(|class| class.to_ascii_lowercase()),
        _ => Some(String::new()),
    };
    let target = match translated {
        Some(target) => target,
        None => {
            note(format!(
                "quest_target:{kind}:{}",
                if host_target.is_empty() {
                    "none"
                } else {
                    host_target
                }
            ));
            host_target.to_ascii_lowercase()
        }
    };
    let (pos, mark) = match kind.as_str() {
        "clear_barbarian_camp" => (
            game.camps_near_city_state(owner).into_iter().next(),
            game.players[0].counters.get("camps").copied().unwrap_or(0),
        ),
        "recruit_great_person_class" => (
            None,
            game.players[0]
                .gp_claimed
                .get(&target)
                .copied()
                .unwrap_or(0),
        ),
        _ => (None, 0),
    };
    game.players[0].quests.insert(
        owner,
        crate::game::quests::CityStateQuest {
            kind,
            target,
            era: game.world_era,
            pos,
            mark,
        },
    );
}

/// Carry the host's climate onto the board: the level is the phase
/// (`Game::mirror_set_climate_phase`, which also floods the lowland bands the
/// shipped `CoastalLowlands` table names for it), our own CO2 is the seat's
/// `co2_emissions`, and the rest — the world's CO2, the temperature, the
/// sea-level and disaster forecasts — is `Game::observed_climate`, which
/// `global_co2_emissions` prefers to the board's own sum. `None` (an older
/// export, or a ruleset without `GameClimate`) leaves all of it alone.
pub(super) fn apply_host_climate(game: &mut crate::game::Game, state: &StateSnapshot) {
    let Some(climate) = state.climate.as_ref() else {
        // An older export: the phase the board holds stays, and the bands it
        // floods are re-marked on the map the sync has just re-applied.
        if game.climate_phase > 0 {
            let phase = game.climate_phase;
            game.mirror_set_climate_phase(phase);
        }
        return;
    };
    if (0..=7).contains(&climate.level) {
        game.mirror_set_climate_phase(climate.level as u8);
    }
    let finite = |value: Option<f64>| value.filter(|value| value.is_finite() && *value >= 0.0);
    let counted = |value: Option<i64>| value.filter(|value| *value >= 0);
    if let Some(ours) = finite(climate.co2_ours) {
        game.players[0].co2_emissions = ours;
    }
    game.observed_climate = Some(crate::game::ObservedClimate {
        temperature: climate.temperature.filter(|value| value.is_finite()),
        co2_total: finite(climate.co2_total),
        sea_level_turns: counted(climate.sea_level_turns),
        tiles_flooded: counted(climate.tiles_flooded),
        storm_pct: finite(climate.storm_pct),
        flood_pct: finite(climate.flood_pct),
        drought_pct: finite(climate.drought_pct),
    });
}
