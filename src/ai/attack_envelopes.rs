//! Enemy attack envelopes: where the enemy can strike next turn.
//!
//! `enemy_attack_envelopes` answers from a cache keyed on everything a
//! reach rule reads (`attack_envelope_fingerprint`, the belligerence and
//! city fingerprints); `envelope_sensitive_tiles`,
//! `envelope_board_change` and `envelope_board_delta` decide whether a board
//! change invalidates it; `compute_enemy_attack_envelopes` does the work on
//! a miss; and the audit (`envelope_audit_enabled`,
//! `report_stale_envelope`) reports a cached answer that disagrees with a
//! fresh one.
//!
//! Moved verbatim out of the `impl BasicAi` block in `ai.rs` (2026-11-05).
//! Methods the rest of `ai` calls are `pub(super)`, the same reach a private
//! method of `ai` had; the code is otherwise unchanged.

use super::*;

impl BasicAi {
    /// The next-turn attack envelopes of hostile units the controller can see.
    /// `Game::attack_reach` does the terrain-accurate movement and attack
    /// calculation; this layer decides which combatants count as threats to
    /// this player.
    ///
    /// ★★★★ COMPUTED ONCE PER BOARD, NOT ONCE PER OWN UNIT. `retreat_step`
    /// asks for these for every military unit of the seat on every turn, and
    /// each answer is one `attack_reach` flow field per visible enemy.
    /// Measured 2026-08-19 (`civvis simulate --seed 7311001 --jobs 1
    /// --players 6 --turns 150 --width 74 --height 46 --city-states 9 --speed
    /// online`, `ci` profile): 102.7 s per game with the per-unit recompute
    /// #2059 shipped against 16.7 s the commit before it, and a `sample` of
    /// head put `retreat_step → enemy_attack_envelopes → attack_reach` at a
    /// third of the main thread — a six-fold slowdown of every simulation on
    /// the fleet.
    ///
    /// The key is exact: the fingerprint below covers everything
    /// `attack_reach` reads, so a hit returns what a recompute would have,
    /// byte for byte (`tools/speed_ab.py`: reports agree on every paired
    /// seed). What it buys is the frontier — `plan_general_unit_turn` plans a
    /// whole batch of units against clones of one board, and every unit of
    /// the batch now shares one computation — plus every own unit that steps
    /// without moving anything. A serial path that moves a unit between two
    /// steps still recomputes, because the moved unit's zone of control is
    /// part of every enemy's reach; the exact key does not pretend otherwise.
    pub(crate) fn enemy_attack_envelopes(
        &self,
        g: &Game,
        pid: usize,
    ) -> std::sync::Arc<AttackEnvelopes> {
        // The evaluator deliberately uses an incomplete key, so preserve its
        // old hit path. Production's key is complete and can double as the
        // raw-reach key on a miss instead of hashing every unit a second time.
        let (key, production_reach_key) = if self.envelope_cache_across_own_moves {
            (
                (g.turn, pid, Self::attack_envelope_fingerprint(g, Some(pid))),
                None,
            )
        } else {
            let full_fingerprint = Self::attack_envelope_fingerprint(g, None);
            (
                (g.turn, pid, full_fingerprint),
                Some((g.turn, full_fingerprint)),
            )
        };
        // A poisoned lock only means another worker panicked mid-store; the
        // value inside is a complete entry or `None`, either of which is safe
        // to read.
        let slot = self
            .attack_envelope_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(cache) = slot.as_ref() {
            if cache.key == key {
                return std::sync::Arc::clone(&cache.envelopes);
            }
        }
        drop(slot);
        // ★★★★ ONE MEMO SCOPE FOR THE WHOLE TABLE, NOT ONE PER ENEMY.
        // `Game::flow_past` opens `query_memo()` itself, so with no scope
        // around this every enemy's flood was the *outermost* scope of its
        // own: `air_patrols` — a scan of every unit in the world, hoisted out
        // of `can_enter_past` precisely because a flood asks it once per
        // neighbour of every tile it expands — was rebuilt once per enemy, and
        // so were `passage_improvements`, `traversal` and `movement`. One
        // 150-turn game at the standard shape computes 35,181 tables from
        // 218,269 floods, so 83.9% of those scopes were the second flood of a
        // table paying again for the first one's answers.
        //
        // Exact because a memo scope is only ever a lie if the board moves
        // under it, and this one is opened over `&Game`: nothing inside can
        // mutate it. It is dropped before this function returns, so no caller
        // inherits it.
        // The raw reaches always need the complete board key, even while the
        // evaluator prices stale own moves. Production already formed that
        // exact key above; reusing it removes one complete roster hash per
        // outer-cache miss.
        let reach_key = production_reach_key
            .unwrap_or_else(|| (g.turn, Self::attack_envelope_fingerprint(g, None)));
        let envelopes = std::sync::Arc::new({
            let _memo = g.query_memo();
            let _zoc = g.zoc_batch_memo();
            self.compute_enemy_attack_envelopes(g, pid, reach_key)
        });
        let mut slot = self
            .attack_envelope_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = Some(std::sync::Arc::new(AttackEnvelopeCache {
            key,
            envelopes: std::sync::Arc::clone(&envelopes),
        }));
        envelopes
    }

    /// Everything `attack_reach` reads: every unit's identity, owner, place,
    /// health, movement and fighting state (an own unit's place matters too —
    /// its zone of control ends an enemy's move), every city's identity,
    /// owner and place, the map epoch, and the war ledger. FNV-1a over those
    /// fields in table order, so equal boards hash equal. `skip_owner` leaves
    /// one seat's units out — see `envelope_cache_across_own_moves`.
    ///
    /// ★★★ A WORD AT A TIME. This is the most-executed loop in the envelope
    /// machinery — one 150-turn game at the standard shape asks 51,500 times
    /// and mixes 13,118,014 units doing it — and FNV-1a is defined a *byte* at
    /// a time, so each of the seven fields per unit cost eight rounds of
    /// xor-and-multiply: 734 million rounds a game to answer "is this the same
    /// board". The mix is now the one `vision_key` in `src/game.rs` uses, a
    /// round per field, which is the shape this repository already trusts for
    /// the sight cache's keys.
    ///
    /// Nothing reads the number. It is compared for equality against the
    /// previous ask's and thrown away, so the only property that matters is
    /// that different boards keep giving different values — the field set and
    /// its order are untouched.
    pub(super) fn attack_envelope_fingerprint(g: &Game, skip_owner: Option<usize>) -> u64 {
        let mut hash = FINGERPRINT_SEED;
        let mut mix = |value: u64| {
            hash = (hash ^ value).wrapping_mul(FINGERPRINT_SEED);
            hash ^= hash >> 29;
        };
        mix(g.map.tiles.epoch());
        mix(g.wars.len() as u64);
        for unit in g
            .units
            .values()
            .filter(|unit| Some(unit.owner) != skip_owner)
        {
            mix(u64::from(unit.id));
            mix(unit.owner as u64);
            mix(unit.pos.0 as u64);
            mix(unit.pos.1 as u64);
            mix(u64::from(unit.kind.id()));
            mix(unit.formation as u64);
            mix(u64::from(unit.zoc_stopped));
        }
        mix(g.cities.len() as u64);
        for city in g.cities.values() {
            mix(u64::from(city.id));
            mix(city.owner as u64);
            mix(city.pos.0 as u64);
            mix(city.pos.1 as u64);
        }
        hash
    }

    /// The tiles a board change must touch before an envelope is worth
    /// recomputing: the unit's movement flood and every neighbour of it.
    pub(super) fn envelope_sensitive_tiles(
        g: &Game,
        unit: &crate::game::Unit,
        flood: &[Pos],
    ) -> std::collections::HashSet<Pos> {
        let mut tiles = std::collections::HashSet::with_capacity(flood.len() * 7 + 7);
        // ★★★★★ AN EMPTY FLOOD IS NOT AN EMPTY SENSITIVITY. `flow_past` returns
        // nothing at all when `formation_movement_locked_by_zoc` holds, and a
        // set built only from the flood is then empty — so no board change ever
        // touches it and the envelope is frozen for the rest of the game, still
        // empty long after the lock lifts. That is the leak `speed_ab.py`
        // refused twice in #2159 and inspection missed twice: the audit found
        // it in one run, every report showing tiles *gained* and none lost,
        // around a unit whose cached envelope was empty.
        //
        // The lock is read off this unit's own ground — its `zoc_stopped`,
        // whether it acted without moving, and `in_enemy_zoc_for` at its own
        // tile — so its own tile and their neighbours are always sensitive,
        // flood or no flood. The same goes for a linked peer, whose lock locks
        // the leader.
        let seed = |pos: Pos, tiles: &mut std::collections::HashSet<Pos>| {
            tiles.insert(pos);
            for neighbour in g.nbrs(pos) {
                tiles.insert(neighbour);
            }
        };
        seed(unit.pos, &mut tiles);
        if let Some(peer) = unit.linked_to.and_then(|id| g.units.get(&id)) {
            seed(peer.pos, &mut tiles);
        }
        for tile in flood {
            tiles.insert(*tile);
            for neighbour in g.nbrs(*tile) {
                tiles.insert(neighbour);
            }
        }
        tiles
    }

    /// Whether `CIVVIS_ENVELOPE_AUDIT` asked for the reuse audit. Read once:
    /// the check sits in the hottest path in the simulator, and an environment
    /// lookup per reuse would be measuring the instrument.
    fn envelope_audit_enabled() -> bool {
        static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *ENABLED.get_or_init(|| std::env::var_os("CIVVIS_ENVELOPE_AUDIT").is_some())
    }

    /// ⚠ DIAGNOSTIC ONLY. Describes one envelope that was reused when it should
    /// not have been, with everything needed to name the input nobody tracked.
    fn report_stale_envelope(
        g: &Game,
        unit: &crate::game::Unit,
        stale: &EnvelopeReach,
        fresh: &EnvelopeReach,
    ) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static SEEN: AtomicUsize = AtomicUsize::new(0);
        if SEEN.fetch_add(1, Ordering::Relaxed) >= 6 {
            return;
        }
        let gained: Vec<Pos> = fresh
            .iter()
            .filter(|p| !stale.contains(p))
            .copied()
            .collect();
        let lost: Vec<Pos> = stale
            .iter()
            .filter(|p| !fresh.contains(p))
            .copied()
            .collect();
        let near = |pos: Pos| -> Vec<String> {
            g.units
                .values()
                .filter(|other| g.wdist(other.pos, pos) <= 3)
                .map(|other| {
                    format!(
                        "{}#{}@{:?}p{}{}",
                        other.kind.as_str(),
                        other.id,
                        other.pos,
                        other.owner,
                        if other.air_patrol { "!patrol" } else { "" }
                    )
                })
                .collect()
        };
        eprintln!(
            "STALE turn={} enemy={}#{} at {:?} owner={} formation={} zoc_stopped={}\n               gained={:?}\n  lost={:?}\n  near_gained={:?}\n  near_lost={:?}",
            g.turn,
            unit.kind.as_str(),
            unit.id,
            unit.pos,
            unit.owner,
            unit.formation,
            unit.zoc_stopped,
            &gained[..gained.len().min(6)],
            &lost[..lost.len().min(6)],
            gained.first().map(|p| near(*p)).unwrap_or_default(),
            lost.first().map(|p| near(*p)).unwrap_or_default(),
        );
    }

    /// Everything that moved, appeared or changed spec since the last board
    /// this controller was asked about, as the tiles those changes touched.
    ///
    /// ★★★★★ THE HASH THIS REPLACES WAS 7.1% OF THE MAIN THREAD. Keying each
    /// envelope on a hash of its own neighbourhood is exact, but it costs a
    /// sweep of every unit and city *per enemy, per ask* — and on the profile
    /// after #2151 that sweep cost almost as much as the recomputes it saved.
    ///
    /// A board delta answers the same question far more cheaply. An enemy's
    /// envelope depends only on the map, the war ledger, the cities, and the
    /// tiles [`Self::envelope_sensitive_tiles`] recorded for it. If none of
    /// those changed since the previous ask, an envelope that was right then
    /// is still right now — so the reuse test becomes "did any changed tile
    /// land in this unit's sensitive set", over a change list that is usually
    /// one unit long because the caller has stepped one unit and asked again.
    ///
    /// ⚠ The sensitive set, not a radius. #2151 keyed each envelope on a hash
    /// of a disk sized by a movement-derived reach bound; #2163 replaced that
    /// with the unit's own flood, which is exact rather than generous and does
    /// not go empty under a zone-of-control lock. Nothing computes that radius
    /// any more — `enemy_envelope_key` and the bound are both gone — and a
    /// comment that says otherwise is describing #2151.
    ///
    /// ⚠ The induction only holds while this is refreshed on *every* ask.
    /// `None` means "assume everything changed", which is what a first ask, a
    /// map edit, a war, or any city change returns.
    /// The tiles the changes touched — see [`Self::envelope_board_change`],
    /// which this is the delta half of. Kept because the invariants above are
    /// pinned on the delta alone, and a test should not have to know that the
    /// store's cleanup rides along with it.
    #[cfg(test)]
    pub(super) fn envelope_board_delta(&self, g: &Game) -> Option<Vec<Pos>> {
        self.envelope_board_change(g).touched
    }

    /// ★★★ THE DIFF IS A MERGE, NOT A REBUILD. Both rosters are ordered by
    /// id — `Game.units` is a `BTreeMap` and the stored board is kept in the
    /// same order — so the two are walked side by side. What this replaces
    /// built a fresh `HashMap` of *every unit in the game* on *every ask*,
    /// hashed each id once to insert it and once more to look up its
    /// predecessor, and then threw the old table away. The comparison it
    /// makes, field for field, is identical: `EnvelopeUnit`'s `PartialEq` is
    /// the same one, over the same fields, and the tiles reported are the
    /// same set. Only the order within the returned `Vec` differs, and its
    /// single reader asks whether a sensitive set contains any of them.
    pub(super) fn envelope_board_change(&self, g: &Game) -> EnvelopeChange {
        let mut previous = self
            .envelope_board
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let stamp = (
            g.map.tiles.epoch(),
            Self::envelope_belligerence_fingerprint(g),
            Self::envelope_city_fingerprint(g),
        );
        let mut units: Vec<(u32, EnvelopeUnit)> = Vec::with_capacity(g.units.len());
        let change = match previous.as_ref() {
            Some(prior) if prior.stamp == stamp => {
                let mut touched = Vec::new();
                let mut removed = false;
                let mut before = prior.units.iter().peekable();
                for (id, unit) in g.units.iter() {
                    let now = EnvelopeUnit::of(unit);
                    // Everything the old board held below this id is gone from
                    // the new one: both walks are ascending.
                    while before.peek().is_some_and(|(gone, _)| gone < id) {
                        let (_, gone) = before.next().expect("peeked");
                        removed = true;
                        touched.push(gone.pos);
                        touched.extend(gone.patrol);
                    }
                    match before.peek() {
                        Some((same, was)) if same == id => {
                            if *was != now {
                                touched.push(was.pos);
                                touched.push(now.pos);
                                touched.extend(was.patrol);
                                touched.extend(now.patrol);
                            }
                            before.next();
                        }
                        _ => {
                            touched.push(now.pos);
                            touched.extend(now.patrol);
                        }
                    }
                    units.push((*id, now));
                }
                for (_, gone) in before {
                    removed = true;
                    touched.push(gone.pos);
                    touched.extend(gone.patrol);
                }
                EnvelopeChange {
                    touched: Some(touched),
                    removed,
                }
            }
            _ => {
                units.extend(
                    g.units
                        .iter()
                        .map(|(id, unit)| (*id, EnvelopeUnit::of(unit))),
                );
                // Nothing is known about what left the board, so the store's
                // cleanup must run.
                EnvelopeChange {
                    touched: None,
                    removed: true,
                }
            }
        };
        *previous = Some(EnvelopeBoard { stamp, units });
        change
    }

    /// Who is at war with whom, as `attack_reach` sees it.
    ///
    /// ★★★★★ `wars.len()` WAS NOT ENOUGH AND THE HOLE IS NOT ACADEMIC.
    /// `flow_past` reaches other units only through `in_enemy_zoc_for`, which
    /// asks `is_at_war` — and `is_at_war` consults `at_war` *and*, for a
    /// city-state, `suzerain_of`, which is derived from every major's envoys.
    /// An envoy changes hands with no unit moving, no city changing and no
    /// entry added to `wars`, and it can flip a whole city-state's belligerence
    /// and therefore its zone of control.
    ///
    /// ⚠ Found by `tools/speed_ab.py` refusing a tighter invalidation set:
    /// with a generous radius this was masked, because almost any nearby unit
    /// moving invalidated anyway. `advanced_v1_plays_the_same_game_it_always_did`
    /// passed throughout — the anchor's five profiles do not move an envoy in
    /// a way that matters.
    fn envelope_belligerence_fingerprint(g: &Game) -> u64 {
        let mut hash = FINGERPRINT_SEED;
        let mut mix = |value: u64| {
            hash = (hash ^ value).wrapping_mul(FINGERPRINT_SEED);
            hash ^= hash >> 29;
        };
        mix(g.wars.len() as u64);
        for (left, right) in &g.at_war {
            mix(*left as u64);
            mix(*right as u64);
        }
        for player in g.players.iter() {
            if player.is_minor && !player.is_barbarian {
                mix(player.id as u64);
                mix(g
                    .suzerain_of(player.id)
                    .map_or(u64::MAX, |seat| seat as u64));
            }
        }
        hash
    }

    /// The cities as `attack_reach` sees them: identity, owner and place.
    fn envelope_city_fingerprint(g: &Game) -> u64 {
        let mut hash = FINGERPRINT_SEED;
        for city in g.cities.values() {
            for value in [
                u64::from(city.id),
                city.owner as u64,
                city.pos.0 as u64,
                city.pos.1 as u64,
            ] {
                hash = (hash ^ value).wrapping_mul(FINGERPRINT_SEED);
                hash ^= hash >> 29;
            }
        }
        hash
    }

    pub(super) fn compute_enemy_attack_envelopes(
        &self,
        g: &Game,
        pid: usize,
        reach_key: (u32, u64),
    ) -> AttackEnvelopes {
        let mut store = self
            .enemy_envelope_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let EnvelopeChange {
            touched: delta,
            removed,
        } = self.envelope_board_change(g);
        let out: AttackEnvelopes = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner != pid
                    && g.is_at_war(pid, unit.owner)
                    && g.unit_visible_to(unit.id, pid)
                    && spec.class == "military"
                    && (spec.is_melee_capable() || spec.has_ranged_attack())
            })
            .filter_map(|unit| {
                // ⚠ Air units are never reused: `attack_reach` centres their
                // disk on `air_operation_origin`, not the unit's tile, so a
                // radius around the tile watches the wrong ground. They are a
                // disk, not a flow field, and cost little to redo.
                let reusable = g.rules.units[unit.kind].domain.as_deref() != Some("air");
                if reusable {
                    if let (Some(delta), Some(entry)) = (delta.as_ref(), store.get(&unit.id)) {
                        if delta
                            .iter()
                            .all(|touched| !entry.sensitive.contains(touched))
                        {
                            // ⚠ DIAGNOSTIC ONLY. With CIVVIS_ENVELOPE_AUDIT set,
                            // every reuse is checked against a fresh computation
                            // and the first disagreement is described. This is
                            // how the leak gets named instead of guessed at.
                            if Self::envelope_audit_enabled() {
                                let fresh = EnvelopeReach::from_tiles(g.attack_reach(unit.id));
                                if fresh != *entry.reach {
                                    Self::report_stale_envelope(g, unit, &entry.reach, &fresh);
                                }
                            }
                            return (!entry.reach.is_empty())
                                .then(|| (unit.id, std::sync::Arc::clone(&entry.reach)));
                        }
                    }
                }
                // ⚠ `attack_reach_from_flood` already returns these ascending
                // and distinct; re-collecting them into a second set was
                // building the same answer twice.
                let cached = g.cached_attack_reach_from_flood(reach_key, unit.id);
                let reach: std::sync::Arc<EnvelopeReach> =
                    std::sync::Arc::new(EnvelopeReach::from_shared_tiles(cached.shared_targets()));
                if reusable {
                    store.insert(
                        unit.id,
                        EnemyEnvelope {
                            reach: std::sync::Arc::clone(&reach),
                            sensitive: std::sync::Arc::new(Self::envelope_sensitive_tiles(
                                g,
                                unit,
                                cached.flood(),
                            )),
                        },
                    );
                }
                (!reach.is_empty()).then_some((unit.id, reach))
            })
            .collect();
        // A unit that died or left sight is never asked about again; drop it so
        // a long game does not carry an envelope per unit ever seen.
        //
        // ★★★ ONLY WHEN SOMETHING LEFT THE BOARD, AND THE DELTA IS WHAT KNOWS.
        // This walked the whole per-enemy cache on every ask, hit or miss, to
        // find the removals of a board that usually has none. `removed` is
        // `true` for exactly the asks where the previous board held an id this
        // one does not — and `true` whenever the delta gave up, which is the
        // case where nothing is known.
        //
        // ⚠ The induction, which is the whole of why the skip is exact: every
        // id inserted above belongs to `g.units`, so after a walk the store
        // holds only ids of the board just asked about, and the board this
        // walk is judged against is the one the delta was taken from — the
        // store and the board move together under this same lock. If that
        // board loses no id, every id the store holds is still on this one and
        // a walk would find nothing to drop. That argument does not care which
        // board the previous ask was about, which matters: a speculative clone
        // draws its new ids from the same `next_id` as the board it was cloned
        // from, so two branches *can* mint the same id for different units —
        // and the branch that goes away takes its ids with it, which is a
        // removal, which is a walk.
        if removed {
            store.retain(|id, _| g.units.contains_key(id));
        }
        out
    }
}
