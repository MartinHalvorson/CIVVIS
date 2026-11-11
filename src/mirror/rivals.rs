//! Rivals: writing what the seat knows of the other civilizations into the
//! rebuilt game.
//!
//! `apply_host_diplomacy` (relationships, agreements, grievances),
//! `apply_rival_public_economy` and `apply_public_empire_stats`, rival tech
//! trees, the wars between rivals, envoys rivals hold, foreign
//! infrastructure and Encampment health, and the strikes foreign units made.
//!
//! Moved verbatim out of `mirror.rs` (2026-11-05). `mirror.rs` re-exports
//! everything here, so every path is unchanged. Items the rest of `mirror`
//! uses are `pub(super)`; the code is otherwise unchanged.

use super::*;

/// Mirror the wars between met rivals into [`Game::observed_rival_wars`].
///
/// The seat's own wars cross as each rival's `at_war`; until this nothing
/// said which rivals were fighting EACH OTHER, so on the live seat every
/// rival read as at peace with every other major. A desk that wants an ally
/// already fighting our enemy (`coalition-before-war-3`,
/// `military-alliance-shield`: a Military Alliance's +5 combat strength holds
/// against "units of players at war with you and your ally",
/// Expansion1_InGameText.xml `LOC_ALLIANCE_LV1_MILITARY_EFFECT_1`) therefore
/// never found one. Each rival's `at_war_with` names the met majors it is at
/// war with; a pair either side names is a war. The set is rebuilt from the
/// export each time, so a peace drops the pair. Rivals on an older export
/// (`None`) contribute nothing. Our own seat is skipped: `at_war` carries
/// those wars.
pub(crate) fn apply_rival_wars(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    seat_of_host: &BTreeMap<usize, usize>,
) {
    let mut wars = BTreeSet::new();
    for rival in &state.rivals {
        let (Some(list), Some(&seat)) =
            (rival.at_war_with.as_ref(), seat_of_host.get(&rival.player))
        else {
            continue;
        };
        if seat == 0 || seat >= game.players.len() {
            continue;
        }
        for host in list {
            let Some(&other) = seat_of_host.get(host) else {
                continue;
            };
            if other != 0 && other != seat && other < game.players.len() {
                wars.insert((seat.min(other), seat.max(other)));
            }
        }
    }
    if *game.observed_rival_wars != wars {
        game.observed_rival_wars = Arc::new(wars);
    }
}

/// Put fog-safe host standings on a reconstructed seat without manufacturing
/// the hidden cities, wonders, or weapons that produced them.
pub(super) fn apply_public_empire_stats(
    game: &mut crate::game::Game,
    owner: usize,
    source: &StatePublicEmpireStats,
) {
    let count = |value: Option<i64>| {
        value
            .filter(|value| *value >= 0)
            .and_then(|value| usize::try_from(value).ok())
    };
    let population = source
        .population
        .filter(|value| *value >= 0)
        .and_then(|value| i32::try_from(value).ok());
    let observed = Arc::make_mut(&mut game.observed_public_empire_stats)
        .entry(owner)
        .or_default();
    observed.city_count = count(source.city_count);
    observed.population = population;
    observed.wonder_count = count(source.wonder_count);
    observed.suzerain_count = count(source.suzerain_count);
    observed.nuclear_devices = source.nuclear_devices.filter(|value| *value >= 0);
    observed.thermonuclear_devices = source.thermonuclear_devices.filter(|value| *value >= 0);
    observed.unit_count = count(source.unit_count);
    observed.military_unit_count = count(source.military_unit_count);
}

/// Put a rival's tree by name on its seat, and let its border be derived
/// from that tree when the host sent the names but not the one-bit answer.
///
/// The names are the same `HasTech`/`HasCivic` loop the counts come from,
/// mapped through `civvis_node_name` exactly as the local seat's lists are,
/// and ASSIGNED (not merged) so a seat that changed hands between exports
/// does not keep a tree it no longer holds. With the tree on the seat,
/// `Game::player_era`, the unit roster and `Game::enforces_borders` (Early
/// Empire's `open_borders` tree effect) all read a rival the way they read a
/// native player. A name CIVVIS lacks is filed in `unmapped`, as ours are.
///
/// The border: the host's own `enforces_borders` bit wins when it crossed;
/// without it, a tree that crossed lets the civic decide (`None`, the native
/// rule); and an export with neither reads as enforced — the conservative
/// answer and the measured one (run civvis-20260826T184456Z, 37 military
/// steps into a rival's closed border, none arrived).
pub(super) fn apply_rival_tree(
    game: &mut crate::game::Game,
    owner: usize,
    rival: &StateRival,
    unmapped: &mut Vec<String>,
) {
    if let Some(names) = &rival.tech_names {
        let mut techs = std::collections::BTreeSet::new();
        for civ6 in names {
            match civvis_node_name(&game.rules.techs, civ6, "TECH_") {
                Some(name) => {
                    techs.insert(crate::name::Name::new(&name));
                }
                None if !unmapped.contains(civ6) => unmapped.push(civ6.clone()),
                None => {}
            }
        }
        game.players[owner].techs = techs;
    }
    if let Some(names) = &rival.civic_names {
        let mut civics = std::collections::BTreeSet::new();
        for civ6 in names {
            match civvis_node_name(&game.rules.civics, civ6, "CIVIC_") {
                Some(name) => {
                    civics.insert(crate::name::Name::new(&name));
                }
                None if !unmapped.contains(civ6) => unmapped.push(civ6.clone()),
                None => {}
            }
        }
        game.players[owner].civics = civics;
    }
    game.players[owner].borders_enforced = match rival.enforces_borders {
        Some(enforced) => Some(enforced),
        None if rival.civic_names.is_some() => None,
        None => Some(true),
    };
}

/// Put a rival's host-reported public standings and economy on its seat:
/// treasury and Faith balances directly, all five top-bar yields as a
/// host-to-model delta, and aggregate HUD totals separately. This makes the
/// standings exact even when the city and unit records are intentionally
/// fog-limited. Fields the host could not read (`-1`) or an older export never
/// sent (NaN) leave the model's own derivation in place.
pub(super) fn apply_rival_public_economy(
    game: &mut crate::game::Game,
    owner: usize,
    rival: &StateRival,
    unmapped: &mut Vec<String>,
) {
    let known = |value: f64| value.is_finite() && value >= 0.0;
    if let Some(civ6) = rival.government.as_deref() {
        match civvis_node_name(&game.rules.governments, civ6, "GOVERNMENT_") {
            Some(government) => game.players[owner].government = Some(government),
            None if !unmapped.iter().any(|entry| entry == civ6) => unmapped.push(civ6.to_string()),
            None => {}
        }
    }
    apply_observed_age(
        &mut game.players[owner],
        rival.heroic_golden_age,
        rival.golden_age,
        rival.dark_age,
    );
    apply_public_empire_stats(game, owner, &rival.public_stats);
    let count = |value: f64| {
        (value.is_finite() && value >= 0.0 && value <= usize::MAX as f64)
            .then(|| value.round() as usize)
    };
    {
        let observed = Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(owner)
            .or_default();
        // The science lane's own number (`GetNumTechsResearched`) where it
        // crossed, else the counted loop an older mod sends; they agree.
        observed.techs = rival
            .techs_researched
            .filter(|value| *value >= 0)
            .map(|value| value as usize)
            .or_else(|| count(rival.techs));
        observed.civics = count(rival.civics);
        observed.cities_following_religion = rival
            .cities_following_religion
            .filter(|value| *value >= 0)
            .map(|value| value as usize);
        observed.military_no_treasury = rival
            .military_no_treasury
            .filter(|value| value.is_finite() && *value >= 0.0);
        observed.tourism_per_turn = known(rival.tourism).then_some(rival.tourism);
        // World Rankings owns the science-race distance, target, and current
        // rate. Do not derive the rate from a laser-project count: repeatable
        // Terrestrial stations contribute only while the host has them powered.
        observed.science_victory_points =
            known(rival.science_victory_points).then_some(rival.science_victory_points);
        observed.science_victory_points_per_turn = known(rival.science_victory_points_per_turn)
            .then_some(rival.science_victory_points_per_turn);
        observed.science_victory_points_needed = (rival.science_victory_points_needed.is_finite()
            && rival.science_victory_points_needed > 0.0)
            .then_some(rival.science_victory_points_needed);
        if known(rival.tourism) {
            Arc::make_mut(&mut game.observed_tourism_per_turn).insert(owner, rival.tourism);
        } else {
            Arc::make_mut(&mut game.observed_tourism_per_turn).remove(&owner);
        }
        // Like `techs`/`civics`: the observed table is rebuilt from each
        // snapshot (`apply_observed_host_metrics` clears it), so absent or
        // refused reads honestly say None for THIS snapshot. The durable
        // record is the player's `science_projects` below.
        observed.foreign_tourists = count(rival.foreign_tourists);
        observed.domestic_tourists = count(rival.domestic_tourists);
        observed.culture_turns_to_victory = rival
            .culture_turns_to_victory
            .is_finite()
            .then_some(rival.culture_turns_to_victory);
    }
    // The rival's majority religion, by the CIVVIS name its founded religion
    // takes (`civvis_religion_name`): `Game::majority_religion_of` reads it
    // before counting the cities the board holds, so the religion lane of the
    // victory tracker (`victory_races`, `religious_conversion_tally`) counts
    // a rival converted by cities the seat has never seen.
    if let Some(name) = rival.religion.as_deref().and_then(civvis_religion_name) {
        Arc::make_mut(&mut game.observed_majority_religion).insert(owner, name);
    }
    // Our draw from this rival, keyed the way `visiting_tourists_from` asks:
    // (tourism source = us, where the tourists come from = them).
    if let Some(tourists) = rival.tourists_visiting_us.filter(|value| *value >= 0) {
        Arc::make_mut(&mut game.observed_visiting_tourists).insert((0, owner), tourists);
    }
    // Era Score on the rival's own seat, the slot `apply_player_ages` fills
    // for ours; the standings (`obs.rs`) show it per player.
    if let Some(score) = rival.era_score.filter(|value| *value >= 0) {
        game.players[owner].era_score = score;
    }
    // The rival's space-race milestones land on its own player record, exactly
    // as the local seat's do — `rival_victory_pressure_with_culture` reads
    // `player.science_projects`, so the science lane of the victory tracker
    // sees the host's truth instead of an empty reconstruction.
    if let Some(projects) =
        completed_strategic_projects(rival.science_projects.as_deref(), unmapped)
    {
        game.players[owner].science_projects = projects;
    }
    if known(rival.gold) {
        game.players[owner].gold = rival.gold;
    }
    // Net income is legitimately negative, so like our own seat's
    // `gold_per_turn` any finite figure is taken (a refused read's -1 is
    // indistinguishable from a real -1 net; an older export is NaN).
    if rival.gold_per_turn.is_finite() {
        game.players[owner].gold_per_turn = rival.gold_per_turn;
    }
    if known(rival.faith) {
        game.players[owner].faith = rival.faith;
    }
    let stats = &rival.public_stats;
    if !known(rival.science)
        && !known(rival.culture)
        && !stats.food.is_some_and(known)
        && !stats.production.is_some_and(known)
        && !known(rival.faith_per_turn)
    {
        Arc::make_mut(&mut game.observed_yield_adjustments).remove(&owner);
        return;
    }
    let mut derived = crate::rules::Yields::default();
    for cid in game.player_city_ids(owner) {
        derived.add(game.city_yields(cid));
    }
    derived.add(game.player_yield_extras(owner));
    derived.add(game.arena_side_yields(owner));
    let mut adjustment = crate::rules::Yields::default();
    if let Some(food) = stats.food.filter(|value| known(*value)) {
        adjustment.food = food - derived.food;
    }
    if let Some(production) = stats.production.filter(|value| known(*value)) {
        adjustment.production = production - derived.production;
    }
    if known(rival.science) {
        adjustment.science = rival.science - derived.science;
    }
    if known(rival.culture) {
        adjustment.culture = rival.culture - derived.culture;
    }
    if known(rival.faith_per_turn) {
        adjustment.faith = rival.faith_per_turn - derived.faith;
    }
    Arc::make_mut(&mut game.observed_yield_adjustments).insert(owner, adjustment);
}

/// The host's own count of strikes a FOREIGN unit has left this turn
/// (`Unit:GetAttacksRemaining()`, the shipped `SelectedUnit.lua:62` read),
/// applied after [`apply_unit_observation`] planted it. The seat's own units
/// have taken it onto `Unit::attacks_left` since the seat capability landed
/// (`LiveMirror::new`, `sync`); `hostiles[]`, `rivals[].units[]` and
/// `minors[].units[]` never carried the key, so an enemy that had already
/// struck stood on the board as one that could still strike. Every foreign
/// planting site — hostiles, rivals and city-states, both import paths —
/// calls this. An export without the key (every recording before 2026-09-01)
/// leaves the fresh-turn allowance `spawn_unit` gave, exactly as before; a
/// negative reading is the mod's "could not read" sentinel and is clamped to
/// none left, as the own-unit paths clamp it.
pub(super) fn apply_foreign_unit_strikes(game: &mut crate::game::Game, uid: u32, unit: &StateUnit) {
    if let Some(attacks) = unit.attacks_remaining {
        if let Some(live) = game.units.get_mut(&uid) {
            live.attacks_left = attacks.max(0);
        }
    }
}

/// Seat 0's UNSPENT envoys, from Firaxis's own `GetTokensToGive`.
///
/// ★★★★★ THIS IS THE LINE THAT LETS CIVVIS SPEND AN ENVOY AT ALL.
/// [`crate::game::Game::legal_actions`] enumerates `Action::SendEnvoy` only
/// behind `if p.envoys_free > 0`, and until now nothing wrote that field on a
/// reconstructed board — so every live decision ran on an empire correctly
/// holding zero, and `AdvancedAi::advanced_envoys` (type-aware, suzerainty-
/// priced, denial-aware, and rated in the deployed bundle) never had a token to
/// place. Measured on the twelve Settler games of 2026-08-15/16: runs end
/// holding **40–70 unspent envoys** with 0 suzerainties in 11 of 12.
///
/// The field carries every reading, including a real 0; `None` and the mod's
/// `-1` ("asked, could not answer") leave the board's count alone rather than
/// zeroing an empire that may be holding envoys — an unknown must not read as
/// "nothing to spend". The actuation path is the `envoy` order kind
/// (`civvis_orders` translates `SendEnvoy`; the mod issues one
/// `GIVE_INFLUENCE_TOKEN` per order through a freshly fetched handle), so what
/// the board wants, the bridge can now deliver.
pub(super) fn apply_mirrored_envoys_free(game: &mut crate::game::Game, state: &StateSnapshot) {
    if let Some(free) = state.envoys_free.filter(|free| *free >= 0) {
        game.players[0].envoys_free = free;
    }
}

/// Write the host's own diplomacy for one rival — seat pair `(0, owner)` —
/// onto the board, assigned from every export so a lapsed state clears.
///
/// Before this the only diplomacy that crossed for a rival was `at_war`,
/// `open_borders` and `can_declare`, and the last was mirrored as a faked
/// denouncement (`denounced_until = turn + 1`) so that a Formal War could be
/// legal on a board whose own five-turn wait never matures. `can_declare`
/// says a war is LEGAL; it never said whether it was ruinous. Every decision
/// that reads the ledger — `preferred_war_opening`'s casus belli choice and
/// its wait, the alliance-partner filter (`grievances < 75`), the coalition
/// and joint-war partner screens, `relationship_opinion`, the declared-friend
/// and alliance bars in `legal_actions` — read an empty ledger on every live
/// turn.
///
/// What each host state writes (`civvis_orders --dump-mirror` reads it back):
/// - `DIPLO_STATE_DENOUNCED`: `denounced_until` / `denounced_since` on the
///   side(s) that denounced, from the host's `GetDenounceTurn` and
///   `GetDenounceTimeLimit` (`DiplomacyActionView.lua:1486-1503`), so the
///   Formal War wait on the board is the host's own clock. When the host
///   gives neither side's turn the pair reads as denounced from this turn.
/// - `DIPLO_STATE_DECLARED_FRIEND`: `friends_until` both sides from
///   `GetDeclaredFriendshipTurn` (`:1510-1511`); `legal_actions` then
///   withholds war and denouncement, as the host does.
/// - `DIPLO_STATE_ALLIED`: `alliances` both sides with the host's type, level
///   and turns to expiry, plus the friendship an alliance implies.
/// - Any other state clears all three on both sides.
/// - Every state, attitudes included: the leader's standing toward us as the
///   host names it (`Player::observed_diplomatic_state`), removed when the
///   export carries none.
/// - Independently of the state: the grievance balance both ways, missions
///   both ways, promises both ways, the Open Borders WE grant, and the
///   visibility level both ways (`Player::observed_visibility`, which
///   `Game::diplomatic_visibility` prefers to its derivation).
///
/// Legacy exports retain the old permission fake: aggregate permission and
/// no active denouncement writes `denounced_until = turn + 1`. This existed
/// before the bridge carried Denounce orders. Modern typed permissions MUST
/// NOT manufacture that clock: permission for Surprise War is not permission
/// for Formal War. The exact per-type facts are applied in step_host_war_types.
///
/// An export without typed permissions keeps its historical behavior.
pub(crate) fn apply_host_diplomacy(game: &mut crate::game::Game, owner: usize, rival: &StateRival) {
    if owner == 0 || owner >= game.players.len() {
        return;
    }
    let turn = game.turn;
    // Read the host's aggregate declaration permission independently of the
    // relationship label. A mature denouncement cannot override a refusal of
    // every war type. New major exports query IsDiplomaticActionValid, matching
    // DiplomacyStatementSupport.lua:167; older exports used CanDeclareWarOn.
    let blocks = Arc::make_mut(&mut game.host_war_blocks);
    blocks.retain(|(actor, target, _)| *actor != 0 || *target != owner);
    if rival.can_declare == Some(false) && !rival.at_war {
        blocks.insert((0, owner, turn));
    }
    let permitted =
        rival.war_declarations.is_none() && rival.can_declare == Some(true) && !rival.at_war;
    let Some(state) = rival.diplomatic_state.as_deref() else {
        game.players[owner].observed_diplomatic_state.remove(&0);
        if permitted {
            game.players[0].denounced_until.insert(owner, turn + 1);
        } else {
            game.players[0].denounced_until.remove(&owner);
        }
        return;
    };
    let state = state.strip_prefix("DIPLO_STATE_").unwrap_or(state);
    game.players[owner]
        .observed_diplomatic_state
        .insert(0, state.to_string());
    let limit = rival
        .denounce_time_limit
        .filter(|limit| *limit > 0)
        .map(|limit| limit as u32);
    let host_turn = |value: Option<i64>| value.filter(|t| *t > 0).map(|t| t as u32);
    // A state that began on `since` runs `limit` more host turns (the
    // denouncement one more, `:1500`); without both it is simply re-read from
    // the next export.
    let expiry = |since: Option<u32>, extra: u32| match (since, limit) {
        (Some(since), Some(limit)) => since.saturating_add(limit).saturating_add(extra),
        _ => turn + 2,
    };

    // Denouncement, each side from its own denounce turn.
    let denounced = state == "DENOUNCED";
    let ours = denounced
        .then(|| host_turn(rival.our_denounce_turn))
        .flatten();
    let theirs = denounced
        .then(|| host_turn(rival.their_denounce_turn))
        .flatten();
    let side_unknown = denounced && ours.is_none() && theirs.is_none();
    for (denouncer, target, since, active) in [
        (0, owner, ours, ours.is_some() || side_unknown),
        (owner, 0, theirs, theirs.is_some() || side_unknown),
    ] {
        let player = &mut game.players[denouncer];
        if active {
            player.denounced_until.insert(target, expiry(since, 1));
            match since {
                Some(since) => {
                    player.denounced_since.insert(target, since);
                }
                None => {
                    player.denounced_since.remove(&target);
                }
            }
        } else {
            player.denounced_until.remove(&target);
            player.denounced_since.remove(&target);
        }
    }
    // The permission fake (see above): the host permits a declaration and
    // the board holds no active denouncement of our own.
    let own_active = game.players[0]
        .denounced_until
        .get(&owner)
        .is_some_and(|until| *until > turn);
    if permitted && !own_active {
        game.players[0].denounced_until.insert(owner, turn + 1);
        game.players[0].denounced_since.remove(&owner);
    }

    // Declared friendship; an alliance implies it, as the engine's own deal
    // acceptance writes both.
    if matches!(state, "DECLARED_FRIEND" | "ALLIED") {
        let until = expiry(host_turn(rival.friendship_turn), 0);
        game.players[0].friends_until.insert(owner, until);
        game.players[owner].friends_until.insert(0, until);
    } else {
        game.players[0].friends_until.remove(&owner);
        game.players[owner].friends_until.remove(&0);
    }

    // Alliance, with the host's type and level; `ends` past this turn so
    // `alliance_with` sees it, and past the host's expiry when it is given.
    if state == "ALLIED" {
        let kind = rival
            .alliance_type
            .as_deref()
            .map(|kind| {
                kind.strip_prefix("ALLIANCE_")
                    .unwrap_or(kind)
                    .to_ascii_lowercase()
            })
            .unwrap_or_else(|| "unknown".to_string());
        let level = rival.alliance_level.unwrap_or(1).max(1);
        let ends = match rival.alliance_turns_left.filter(|left| *left >= 0) {
            Some(left) => turn.saturating_add(left as u32).saturating_add(1),
            None => turn + 2,
        };
        // The engine's own level thresholds, so a reader of `points` agrees
        // with the level the host reports.
        let points = match level {
            1 => 0.0,
            2 => 80.0,
            _ => 240.0,
        };
        let alliance = crate::game::AllianceState {
            kind,
            points,
            level,
            ends,
        };
        game.players[0].alliances.insert(owner, alliance.clone());
        game.players[owner].alliances.insert(0, alliance);
    } else {
        game.players[0].alliances.remove(&owner);
        game.players[owner].alliances.remove(&0);
    }

    // The grievance balance, each direction on the aggrieved side's ledger
    // (`Player::grievances[offender]`), the way `add_direct_grievances`
    // books it. Untouched when the host gave neither number.
    if rival.our_grievances_against_them.is_some() || rival.grievances_against_us.is_some() {
        for (aggrieved, offender, amount) in [
            (0, owner, rival.our_grievances_against_them),
            (owner, 0, rival.grievances_against_us),
        ] {
            let amount = amount.unwrap_or(0.0);
            if amount > 0.0 && amount.is_finite() {
                game.players[aggrieved].grievances.insert(offender, amount);
            } else {
                game.players[aggrieved].grievances.remove(&offender);
            }
        }
    }

    // Missions: an Embassy replaces a Delegation, at most one per counterpart.
    for (sender, host, embassy, delegation) in [
        (0, owner, rival.embassy_at, rival.delegation_at),
        (owner, 0, rival.their_embassy, rival.their_delegation),
    ] {
        if embassy.is_none() && delegation.is_none() {
            continue;
        }
        let kind = if embassy == Some(true) {
            Some("embassy")
        } else if delegation == Some(true) {
            Some("delegation")
        } else {
            None
        };
        let missions = &mut game.players[sender].diplomatic_missions;
        match kind {
            Some(kind) => {
                let sent = missions
                    .get(&host)
                    .filter(|mission| mission.kind == kind)
                    .map(|mission| mission.sent)
                    .unwrap_or(turn);
                missions.insert(
                    host,
                    crate::game::DiplomaticMission {
                        kind: kind.to_string(),
                        sent,
                    },
                );
            }
            None => {
                missions.remove(&host);
            }
        }
    }

    // Promises, keyed as the engine keys them: the promisor's ledger, by the
    // requester, by kind. The host names a kind from the requester's side
    // ("near ME"); three map onto engine kinds, the rest keep their name.
    let promise_kind = |name: &str| match name {
        "DONT_SETTLE_NEAR_ME" => "no_settling".to_string(),
        "DONT_SPY_ON_ME" => "no_spying".to_string(),
        "DONT_CONVERT_MY_CITIES" => "no_conversion".to_string(),
        other => other.to_ascii_lowercase(),
    };
    for (promisor, requester, names) in [
        (0, owner, rival.promises_made.as_ref()),
        (owner, 0, rival.promises_received.as_ref()),
    ] {
        let Some(names) = names else {
            continue;
        };
        if names.is_empty() {
            game.players[promisor].promises.remove(&requester);
            continue;
        }
        let book: BTreeMap<String, u32> = names
            .iter()
            .map(|name| (promise_kind(name), turn + 2))
            .collect();
        game.players[promisor].promises.insert(requester, book);
    }

    // Visibility both ways; a missing reading falls back to the derivation.
    for (viewer, subject, level) in [
        (0, owner, rival.visibility),
        (owner, 0, rival.their_visibility_on_us),
    ] {
        match level {
            Some(level) => {
                game.players[viewer]
                    .observed_visibility
                    .insert(subject, level.max(0) as f64);
            }
            None => {
                game.players[viewer].observed_visibility.remove(&subject);
            }
        }
    }

    // The Open Borders WE grant, the mirror image of `rival.open_borders`.
    match rival.open_borders_granted {
        Some(true) => {
            game.players[0].open_borders_until.insert(owner, turn + 2);
        }
        Some(false) => {
            game.players[0].open_borders_until.remove(&owner);
        }
        None => {}
    }
}

/// Seat every rival's delegation at a city-state from the host's own count
/// (`envoys_by_player`: `GetTokensReceived` per alive major, the shipped
/// CityStates panel's read). Seat 0's count is `minor.envoys`, seeded beside
/// this call and never touched here. A listed export is authoritative for
/// every major seat: a rival absent from it, or at zero, holds nothing there.
/// `None` (an older export) leaves `seed_mirrored_suzerainty`'s minimum
/// winning delegation to do what it did — which could never tell one envoy
/// from five, nor see that we stood one short of a suzerainty.
pub(super) fn apply_host_rival_envoys(
    game: &mut crate::game::Game,
    minor: &StateMinor,
    owner: usize,
    seat_of_host: &std::collections::BTreeMap<usize, usize>,
) {
    let Some(counts) = minor.envoys_by_player.as_ref() else {
        return;
    };
    for pid in 1..game.players.len() {
        if !game.players[pid].is_minor {
            set_mirrored_envoys(&mut game.players[pid], owner, 0);
        }
    }
    for count in counts {
        if count.player < 0 {
            continue;
        }
        let Some(&seat) = seat_of_host.get(&(count.player as usize)) else {
            continue;
        };
        if seat == 0 || seat >= game.players.len() || game.players[seat].is_minor {
            continue;
        }
        set_mirrored_envoys(&mut game.players[seat], owner, count.envoys.max(0));
    }
}

/// Put the districts and wonders the tiles export shows on rival and
/// city-state ground onto the city that owns it.
///
/// A rival city record carries no districts, so until the plot export named
/// them (`d`, `wo`) a rival's economy and defence were modelled from
/// population alone. Our own cities are left to their city record, which
/// carries completion and pillage; here a plot with a district is a standing
/// district. Rebuilt from the export each time it arrives, so a razed or
/// captured district does not linger.
pub(super) fn apply_foreign_infrastructure(game: &mut crate::game::Game, snapshot: &Snapshot) {
    // Per player: the districts seen on their plots, and the wonders.
    type Placed =
        std::collections::BTreeMap<u32, (Vec<(Name, crate::Pos)>, Vec<(Name, crate::Pos)>)>;
    let mut placed: Placed = Default::default();
    let mut any_seen: std::collections::BTreeSet<u32> = Default::default();
    for y in 0..snapshot.height.max(1) {
        for x in 0..snapshot.width.max(1) {
            let Some(plot) = snapshot.plot((x, y)) else {
                continue;
            };
            if plot.d.is_none() && plot.wo.is_none() {
                continue;
            }
            let pos = crate::hex::offset_to_axial(x, y);
            let Some(cid) = game.map.get(pos).and_then(|tile| tile.owner_city) else {
                continue;
            };
            let Some(city) = game.cities.get(&cid) else {
                continue;
            };
            if city.owner == 0 {
                continue;
            }
            any_seen.insert(cid);
            let entry = placed.entry(cid).or_default();
            if let Some(kind) = plot.wo.as_deref() {
                if let Some(name) = civvis_node_name(&game.rules.wonders, kind, "BUILDING_") {
                    entry.1.push((Name::new(&name), pos));
                }
                continue;
            }
            if let Some(kind) = plot.d.as_deref() {
                // A placed-but-unbuilt district is not on the board yet: it
                // pays no adjacency, no route row, no yields of its own.
                if plot.dc == Some(false) {
                    continue;
                }
                if matches!(kind, "DISTRICT_CITY_CENTER" | "DISTRICT_WONDER") {
                    continue;
                }
                if let Some(name) = civvis_node_name(&game.rules.districts, kind, "DISTRICT_") {
                    entry.0.push((Name::new(&name), pos));
                }
            }
        }
    }
    for cid in any_seen {
        let (districts, wonders) = placed.remove(&cid).unwrap_or_default();
        let Some(city) = game.cities.get_mut(&cid) else {
            continue;
        };
        city.districts.clear();
        city.wonders.clear();
        for (name, pos) in &districts {
            city.districts.insert(*name, *pos);
        }
        for (name, pos) in &wonders {
            city.wonders.insert(*name, *pos);
        }
        for (name, pos) in districts {
            if let Some(tile) = game.map.tiles.get_mut(&pos) {
                tile.improvement = None;
                tile.district = Some(name);
                tile.district_foundation = None;
                tile.wonder = None;
                tile.pillaged = snapshot
                    .plot(crate::hex::axial_to_offset(pos.0, pos.1))
                    .is_some_and(|plot| plot.p);
            }
            if game.district_has_defenses(name) {
                if let Some(plot) = snapshot.plot(crate::hex::axial_to_offset(pos.0, pos.1)) {
                    apply_foreign_encampment_health(game, cid, plot);
                }
            }
        }
        retain_observed_defending_districts(game, cid);
        for (name, pos) in wonders {
            if let Some(tile) = game.map.tiles.get_mut(&pos) {
                tile.improvement = None;
                tile.district = None;
                tile.district_foundation = None;
                tile.wonder = Some(name);
            }
        }
    }
}

/// Rival city records omit districts; their independent defenses cross with
/// the plot. Unknown pools use the existing own-city conservative fallback.
/// A measured zero outer capacity means no walls, rather than an unanswered API.
pub(super) fn apply_foreign_encampment_health(game: &mut crate::game::Game, cid: u32, plot: &Plot) {
    let pos = crate::hex::offset_to_axial(plot.x, plot.y);
    let Some(kind) = game.map.get(pos).and_then(|tile| tile.district) else {
        return;
    };
    if !game.district_has_defenses(kind) {
        return;
    }
    let max_wall = game.city_max_wall_hp(&game.cities[&cid]);
    let mut observed = game
        .defending_district_state(cid, pos)
        .unwrap_or_else(|| crate::game::DefendingDistrict::healthy(kind, pos, max_wall));
    observed.pillaged = plot.p;
    observed.hp = plot.dh.as_ref().map_or(100, |health| {
        district_garrison_hp(health.damage, health.max_damage)
    });
    observed.wall_hp = plot.dh.as_ref().map_or(max_wall, |health| {
        district_outer_hp(health.wall_damage, health.max_wall_damage, max_wall)
    });
    game.set_defending_district_state(cid, observed);
}
