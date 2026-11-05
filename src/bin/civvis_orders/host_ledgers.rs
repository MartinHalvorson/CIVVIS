//! Per-turn ledgers of what the host did, so the order channel reads the
//! game instead of guessing.
//!
//! Strikes: the damage the host reports on each city (`HostCityDamage`),
//! the cooldown before a city may attack again
//! (`HostCityAttackCooldowns`), the city strikes and the district each one
//! came from (`HostCityStrikes`, `district_strike_source`), and air strikes
//! (`HostAirStrikes`). Espionage and trade: the spies the host says were
//! lost (`SpyLossLedger`) and barter deals that came up short
//! (`BarterShortLedger`).
//!
//! Moved verbatim out of `civvis_orders.rs` (2026-11-05), which re-exports
//! everything here, so every path is unchanged. Items the rest of the binary
//! uses are `pub(super)`; the code is otherwise unchanged.

use super::*;

/// Firaxis blocks outer-defense repairs for three turns after a city takes damage.
///
/// `civvis_orders --serve --fresh-board` deliberately reconstructs the board for
/// every decision, so `City::last_attacked` otherwise returns to its default zero
/// each turn. That made a recently hit city look eligible for
/// `PROJECT_REPAIR_OUTER_DEFENSES`; Firaxis then rejected the order and left an
/// endangered city on its offensive queue. Keep this host-only cooldown beside the
/// other persistent bridge facts, then apply it to the newly reconstructed board
/// before the AI evaluates production.
#[derive(Clone, Copy)]
pub(super) struct HostCityDamage {
    pub(super) garrison: Option<f64>,
    outer_defenses: Option<f64>,
}

impl HostCityDamage {
    fn from_city(city: &civvis::mirror::StateCity) -> Self {
        let bar = |damage: f64, maximum: f64| {
            (damage.is_finite() && maximum.is_finite() && damage >= 0.0 && maximum > 0.0)
                .then_some((damage / maximum).clamp(0.0, 1.0))
        };
        Self {
            garrison: bar(city.damage, city.max_damage),
            outer_defenses: bar(city.wall_damage, city.max_wall_damage),
        }
    }

    fn increased_since(self, previous: Self) -> bool {
        let increased = |now: Option<f64>, was: Option<f64>| {
            now.zip(was)
                .is_some_and(|(now, was)| now > was + f64::EPSILON)
        };
        increased(self.garrison, previous.garrison)
            || increased(self.outer_defenses, previous.outer_defenses)
    }
}

#[derive(Default)]
pub(super) struct HostCityAttackCooldowns {
    pub(super) observed_turn: Option<u32>,
    pub(super) health: std::collections::BTreeMap<i64, HostCityDamage>,
    last_attack: std::collections::BTreeMap<i64, u32>,
}

impl HostCityAttackCooldowns {
    /// Retain a hit only when adjacent state frames prove it. A skipped/replayed
    /// turn is unknown rather than evidence of an attack, which avoids inventing a
    /// host cooldown from an arbitrary long gap.
    pub(super) fn observe(&mut self, state: &civvis::mirror::StateSnapshot) {
        let health: std::collections::BTreeMap<i64, HostCityDamage> = state
            .cities
            .iter()
            .map(|city| (city.id, HostCityDamage::from_city(city)))
            .collect();
        let consecutive = self
            .observed_turn
            .is_some_and(|previous| state.turn == previous.saturating_add(1));
        if consecutive {
            for (&city, &current) in &health {
                if self
                    .health
                    .get(&city)
                    .is_some_and(|previous| current.increased_since(*previous))
                {
                    self.last_attack.insert(city, state.turn);
                }
            }
        } else if self
            .observed_turn
            .is_some_and(|previous| state.turn < previous)
        {
            // A replay starts a new timeline. Retaining a future timestamp would
            // make `saturating_sub` claim every repair is still cooling down.
            self.last_attack.clear();
        }
        self.last_attack
            .retain(|city, attack_turn| health.contains_key(city) && *attack_turn <= state.turn);
        self.observed_turn = Some(state.turn);
        self.health = health;
    }

    /// Restore the host's recent-hit timestamp after a fresh reconstruction.
    pub(super) fn apply(&self, mirror: &mut civvis::mirror::LiveMirror) {
        for (&host_city, &city) in &mirror.cid_of {
            let Some(&attack_turn) = self.last_attack.get(&host_city) else {
                continue;
            };
            if let Some(live) = mirror.game.cities.get_mut(&city) {
                live.last_attacked = attack_turn;
            }
        }
    }
}

/// The ranged strikes the decider has already ordered this turn, per host
/// city, replayed onto every board built for the turn.
///
/// ★★★★ THE EXPORT CARRIES NO "ATTACKS REMAINING" FOR A CITY. A unit's
/// `attacks_remaining` (`Unit:GetAttacksRemaining`) crosses on every frame and
/// the board plans no second shot for it; a city's strike is answered only by
/// `CityManager.CanStartCommand(city, CityCommandTypes.RANGE_ATTACK)`
/// (`Base/Assets/UI/WorldView/CityBannerManager.lua:1555`, `CanRangeAttack`;
/// `WorldInput.lua:2545` asks the same before `RequestCommand`), which the mod
/// asks at order time and which refuses a second strike in the same turn. The
/// decider plans every frame on a throwaway clone of the mirror, so the
/// authoritative board's `struck` stayed false across the turn's replan
/// frames and the same city was ordered to fire again on each one. Measured
/// on run civvis-20260826T184456Z: 66 `city_strike` orders, 31 refused
/// (`city_strike_refused`), every one of the 31 a re-issue on frame 1 or 2 by
/// a city whose frame-0 strike had landed (the target's hit points fell
/// between the frames); walls, range (≤ 2), visibility and war state were in
/// order on all 31, and no frame-0 strike was refused. Over the last five
/// control runs the refusal was 75 of 161 strikes. Remembered here exactly
/// like the host's repair cooldown above, because it belongs to the host's
/// turn and not to the reconstructed board.
///
/// Victor's Embrasure grants one extra strike; the count of strikes ordered
/// is kept so `extra_strikes_used` carries it and `city_can_strike` reads the
/// same rule it reads for a strike applied on the board itself.
#[derive(Default)]
pub(super) struct HostCityStrikes {
    pub(super) turn: Option<u32>,
    pub(super) city: std::collections::BTreeMap<i64, i32>,
    pub(super) encampment: std::collections::BTreeMap<i64, i32>,
    pub(super) district: std::collections::BTreeMap<(i64, (i32, i32)), i32>,
}

impl HostCityStrikes {
    /// Count the strikes that left for the host on this frame.
    pub(super) fn observe(&mut self, turn: u32, orders: &[IssuedOrder]) {
        if self.turn != Some(turn) {
            self.turn = Some(turn);
            self.city.clear();
            self.encampment.clear();
            self.district.clear();
        }
        for order in orders {
            let Some(subject) = order.subject else {
                continue;
            };
            match order.kind.as_str() {
                "city_strike" => *self.city.entry(subject).or_insert(0) += 1,
                "encampment_strike" => *self.encampment.entry(subject).or_insert(0) += 1,
                "district_strike" => {
                    if let Some(source) = order.verb.as_deref().and_then(district_strike_source) {
                        *self.district.entry((subject, source)).or_insert(0) += 1;
                    }
                }
                _ => {}
            }
        }
    }

    /// Spend, on a board built for `turn`, the strikes already ordered on it.
    pub(super) fn apply(&self, mirror: &mut civvis::mirror::LiveMirror, turn: u32) {
        if self.turn != Some(turn) {
            return;
        }
        for (&host_city, &city) in &mirror.cid_of {
            let Some(live) = mirror.game.cities.get_mut(&city) else {
                continue;
            };
            if let Some(&count) = self.city.get(&host_city) {
                live.struck = true;
                live.extra_strikes_used = live.extra_strikes_used.max(count - 1);
            }
            if let Some(&count) = self.encampment.get(&host_city) {
                live.encampment_struck = true;
                live.encampment_extra_strikes_used =
                    live.encampment_extra_strikes_used.max(count - 1);
            }
            for district in &mut live.defending_districts {
                let source = civvis::hex::axial_to_offset(district.pos.0, district.pos.1);
                if let Some(&count) = self.district.get(&(host_city, source)) {
                    district.struck = true;
                    district.extra_strikes_used = district.extra_strikes_used.max(count - 1);
                }
            }
        }
    }
}

/// Offset source coordinates carried separately from the target in a district order.
pub(super) fn district_strike_source(verb: &str) -> Option<(i32, i32)> {
    let (x, y) = verb.split_once(':')?;
    Some((x.parse().ok()?, y.parse().ok()?))
}

/// The aircraft the decider has already sent on a sortie this turn, replayed
/// onto every board built for the turn.
///
/// ★★★ THE REPLAN FRAME ORDERED THE SAME BOMBER AGAIN. An air attack resolves
/// after the frame that ordered it is exported, so the next frame's export can
/// still read the aircraft as unspent, and the board plans a second sortie the
/// host then refuses (`range_attack_refused`, `can_start=false`). Live King
/// 20260930T211803Z and 225143Z: 171 of 267 refused strikes were aircraft, 86
/// of them a bomber that had already flown that turn, and each repeat earns
/// the order a refusal strike toward `HostOrderRefusals`' ten-turn hold on
/// that exact bomber and target. A sortie the host refused this turn (it is in
/// `Game::blocked_strikes`) did not spend the aircraft and is left alone.
#[derive(Default)]
pub(super) struct HostAirStrikes {
    pub(super) turn: Option<u32>,
    pub(super) sorties: std::collections::BTreeSet<(i64, (i32, i32))>,
}

impl HostAirStrikes {
    /// Remember the sorties that left for the host on this frame.
    pub(super) fn observe(&mut self, turn: u32, orders: &[IssuedOrder]) {
        if self.turn != Some(turn) {
            self.turn = Some(turn);
            self.sorties.clear();
        }
        for order in orders {
            if order.kind == "unit" && order.verb.as_deref() == Some("AIR_ATTACK") {
                if let (Some(subject), Some(pos)) = (order.subject, order.pos) {
                    self.sorties.insert((subject, pos));
                }
            }
        }
    }

    /// Spend, on a board built for `turn`, the aircraft that already flew.
    pub(super) fn apply(&self, mirror: &mut civvis::mirror::LiveMirror, turn: u32) {
        if self.turn != Some(turn) {
            return;
        }
        for &(host, (x, y)) in &self.sorties {
            let Some(&uid) = mirror.uid_of.get(&host) else {
                continue;
            };
            let target = civvis::hex::offset_to_axial(x, y);
            if mirror.game.blocked_strikes.contains(&(uid, target)) {
                continue;
            }
            if let Some(unit) = mirror.game.units.get_mut(&uid) {
                unit.attacks_left = 0;
                unit.moves_left = 0.0;
            }
        }
    }
}

/// `denial-spies-count-their-losses`: the turns our Spies were lost on, as
/// the control mod logs them (`unit_lost` with `unit_kind` `UNIT_SPY`). The
/// mirror keeps no unit history across turns, so the bridge reads the run's
/// own `events.jsonl`, from where it stopped last time: the mod appends every
/// turn, and only whole lines are consumed. A shorter file is a new run and
/// starts the ledger over.
#[derive(Default)]
pub(super) struct SpyLossLedger {
    pub(super) offset: u64,
    pub(super) turns: Vec<u32>,
}

impl SpyLossLedger {
    pub(super) fn refresh(&mut self, events: &Path) {
        use std::io::{BufRead, Seek, SeekFrom};
        let Ok(mut file) = std::fs::File::open(events) else {
            return;
        };
        let length = file.metadata().map_or(0, |meta| meta.len());
        if length < self.offset {
            *self = Self::default();
        }
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut reader = std::io::BufReader::new(file);
        let mut line = Vec::new();
        loop {
            line.clear();
            let Ok(read) = reader.read_until(b'\n', &mut line) else {
                return;
            };
            if read == 0 || line.last() != Some(&b'\n') {
                return;
            }
            self.offset += read as u64;
            if let Some(turn) = Self::spy_loss_turn(&line) {
                self.turns.push(turn);
            }
        }
    }

    /// The turn of a `unit_lost` line for one of our Spies, else `None`.
    /// A line that names a `cause` is a removal the mod knows is no loss: a
    /// Spy starting `SPY_TRAVEL_NEW_CITY` leaves the map and arrives under a
    /// new unit id (`cause` `spy_travel`), and 163 of the 253 Deity Spy
    /// removals of 2026-11-04/05 were such journeys.
    fn spy_loss_turn(line: &[u8]) -> Option<u32> {
        let has = |needle: &[u8]| line.windows(needle.len()).any(|window| window == needle);
        if !has(b"unit_lost") || !has(b"UNIT_SPY") {
            return None;
        }
        let event: serde_json::Value = serde_json::from_slice(line).ok()?;
        (event.get("kind")?.as_str()? == "unit_lost"
            && event.get("unit_kind")?.as_str()? == "UNIT_SPY"
            && event.get("cause").is_none_or(serde_json::Value::is_null))
        .then(|| event.get("turn")?.as_u64())
        .flatten()
        .and_then(|turn| u32::try_from(turn).ok())
    }

    /// The losses on or before `turn` (a replay reads a log that runs on).
    pub(super) fn through(&self, turn: u32) -> Vec<u32> {
        self.turns
            .iter()
            .copied()
            .filter(|lost| *lost <= turn)
            .collect()
    }
}

/// `luxury-asks-pay-in-strategics-2`: each `barter_short` answer of the Lua
/// buy arm to one of our luxury asks (one of the items we put up could not
/// go), with the turn, the partner and the items the ask put up (from the
/// same turn's `order_failed` / `order_verified` line for that partner; empty
/// when none is in the log), read from the run's `events.jsonl` the way
/// `SpyLossLedger` reads it: incrementally, whole lines only, and over from
/// the start on a shorter (new) log.
#[derive(Default)]
pub(super) struct BarterShortLedger {
    pub(super) offset: u64,
    pub(super) shorts: Vec<(u32, i64)>,
    pub(super) offered: std::collections::BTreeMap<(u32, i64), Vec<String>>,
}

impl BarterShortLedger {
    pub(super) fn refresh(&mut self, events: &Path) {
        use std::io::{BufRead, Seek, SeekFrom};
        let Ok(mut file) = std::fs::File::open(events) else {
            return;
        };
        let length = file.metadata().map_or(0, |meta| meta.len());
        if length < self.offset {
            *self = Self::default();
        }
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut reader = std::io::BufReader::new(file);
        let mut line = Vec::new();
        loop {
            line.clear();
            let Ok(read) = reader.read_until(b'\n', &mut line) else {
                return;
            };
            if read == 0 || line.last() != Some(&b'\n') {
                return;
            }
            self.offset += read as u64;
            self.read(&line);
        }
    }

    /// Keep a `deal_offer` line our buy arm answered `barter_short`, or the
    /// items of a luxury ask's `order_failed` / `order_verified` line.
    pub(super) fn read(&mut self, line: &[u8]) {
        let has = |needle: &[u8]| line.windows(needle.len()).any(|window| window == needle);
        let short = has(b"barter_short") && has(b"deal_offer");
        if !short && !has(b"LUXURY_ANY;") {
            return;
        }
        let Ok(event) = serde_json::from_slice::<serde_json::Value>(line) else {
            return;
        };
        let text = |key: &str| event.get(key).and_then(|value| value.as_str());
        let Some(turn) = event
            .get("turn")
            .and_then(|turn| turn.as_u64())
            .and_then(|turn| u32::try_from(turn).ok())
        else {
            return;
        };
        match text("kind") {
            Some("deal_offer") if text("reason") == Some("barter_short") => {
                if let Some(target) = event.get("target").and_then(|target| target.as_i64()) {
                    self.shorts.push((turn, target));
                }
            }
            Some("order_failed" | "order_verified") if text("order_kind") == Some("buy") => {
                let (Some(subject), Some(verb)) = (
                    event.get("subject").and_then(|subject| subject.as_i64()),
                    text("verb"),
                ) else {
                    return;
                };
                let Some(list) = verb.strip_prefix("LUXURY_ANY;") else {
                    return;
                };
                let items = list
                    .split(',')
                    .filter_map(|part| part.split_once('=').map(|(name, _)| name.to_string()))
                    .collect();
                self.offered.insert((turn, subject), items);
            }
            _ => {}
        }
    }

    /// The shorts on or before `turn` (a replay reads a log that runs on),
    /// each with the items its ask put up.
    pub(super) fn through(&self, turn: u32) -> Vec<(u32, i64, Vec<String>)> {
        self.shorts
            .iter()
            .filter(|(at, _)| *at <= turn)
            .map(|(at, partner)| {
                let items = self
                    .offered
                    .get(&(*at, *partner))
                    .cloned()
                    .unwrap_or_default();
                (*at, *partner, items)
            })
            .collect()
    }
}
