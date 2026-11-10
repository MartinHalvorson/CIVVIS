//! Settlers: when the empire wants one, where it goes, and walking it there.
//!
//! Deciding to train one: the opening book's held Settler
//! (`book_settler_pending`, `play_pending_book_settler`), the gates that say
//! whether a Settler is due (`settler_gates`, `settler_due`,
//! `journal_settler_hold`), and how many may be under way at once
//! (`settler_pipeline_width`). Choosing ground: `settle_value`,
//! `valid_settle_site`, `has_practical_settle_site`,
//! `main_landmass_settlement_room`, and the reachable site searches. Moving
//! one: `settler_step`, `settler_step_toward`, and the counts of Settlers that
//! are walking, parked or stranded (`refresh_settler_idle` keeps those).
//!
//! Moved verbatim out of the `impl BasicAi` block in `ai.rs` (2026-11-06).
//! Methods the rest of `ai` calls are `pub(super)`, the same reach a private
//! method of `ai` had; the code is otherwise unchanged.

use super::*;

impl BasicAi {
    /// Whether the opening book's Settler slot is still waiting for the
    /// capital to reach the host's population floor. See
    /// `opening_settler_waits`.
    pub fn book_settler_pending(&self) -> bool {
        self.book_settler_pending
    }

    /// Play the opening book's held Settler the turn the capital reaches the
    /// host's population floor, ahead of whatever filler took its slot. See
    /// `opening_settler_waits`. Returns true when a Settler was started this
    /// call. Idempotent: the hold clears the turn it is played, given up
    /// (no practical site at the floor), or found already answered by a
    /// Settler in the capital's queue.
    pub fn play_pending_book_settler(&mut self, g: &mut Game, pid: usize) -> bool {
        if !self.book_settler_pending || self.minor || self.barb {
            return false;
        }
        let Some(capital) = g
            .player_city_ids(pid)
            .into_iter()
            .find(|cid| g.cities[cid].is_capital)
        else {
            self.book_settler_pending = false;
            return false;
        };
        let city = &g.cities[&capital];
        if matches!(city.queue.first(), Some(Item::Unit { unit }) if unit == "settler") {
            self.book_settler_pending = false;
            return false;
        }
        if (city.pop as f64) < HOST_SETTLER_MIN_POP {
            return false;
        }
        self.book_settler_pending = false;
        if !self.has_practical_settle_site(g, pid) {
            return false;
        }
        let filler = city.queue.first().cloned();
        let started = g
            .apply(
                pid,
                &Action::Produce {
                    city: capital,
                    item: Item::Unit {
                        unit: crate::name!("settler"),
                    },
                },
            )
            .is_ok();
        if started {
            think!(self.journal, Cities, Decision,
            "{} starts the opening book's settler", g.cities[&capital].name;
            "the capital has reached population {}, the floor the host asks of a \
             Settler; {} resumes afterwards with its progress banked",
            g.cities[&capital].pop,
            match &filler {
                Some(Item::Unit { unit }) => format!("the {unit}"),
                Some(Item::Building { building }) => format!("the {building}"),
                Some(_) => "the queue".to_string(),
                None => "nothing".to_string(),
            });
        }
        started
    }

    /// Advance each owned Settler's idle counter, once per turn.
    ///
    /// Position is the whole tell. A settler with a target it can reach moves
    /// every turn; one that has been standing on the same tile for a dozen
    /// turns is either blocked, refused by every site it can see, or
    /// oscillating between two — and in all three cases it is not going to
    /// found a city on its own.
    pub(super) fn refresh_settler_idle(&mut self, g: &Game, pid: usize) {
        if !(self.settler_strand_discount
            || self.settler_backlog_brake
            || self.settler_backlog_brake_2)
            || self.settler_idle_turn == Some(g.turn)
        {
            return;
        }
        self.settler_idle_turn = Some(g.turn);
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        for uid in g.player_unit_ids(pid) {
            let unit = &g.units[&uid];
            if unit.kind.as_str() != "settler" {
                continue;
            }
            seen.insert(uid);
            match self.settler_idle.get(&uid) {
                Some((last, idle)) if *last == unit.pos => {
                    let idle = idle.saturating_add(1);
                    self.settler_idle.insert(uid, (unit.pos, idle));
                }
                _ => {
                    self.settler_idle.insert(uid, (unit.pos, 0));
                }
            }
        }
        // A founded city consumes its settler and a killed one is simply gone.
        // Either way the id must not keep a stale idle streak that a recycled
        // id could inherit.
        self.settler_idle.retain(|uid, _| seen.contains(uid));
    }

    /// How many Settlers the empire may have walking or queued at once — the
    /// one width `pick_item` asks before starting a Settler and the queue
    /// revalidation in `cities` asks before keeping one. `settlers` counts
    /// the walkers and every queued Settler.
    pub(super) fn settler_pipeline_width(
        &self,
        g: &Game,
        n_cities: usize,
        settlers: usize,
    ) -> usize {
        let seats_short = (self.w.city_target.ceil().max(0.0) as usize).saturating_sub(n_cities);
        // `rapid-city-expansion-2` answers only while the empire trails
        // the opening pace; `None` falls through to the ordinary gates
        // below, exactly as `AdvancedAi::settler_in_flight_allowed` does.
        // An `unwrap_or(1)` here held the delegated governor to one walker
        // for the rest of the game once the band closed: on King
        // `civvis-20260930T225143Z` the capital's queued Settlers were
        // refused "a settler is already in flight" at 4 cities of 10
        // wanted (t65, t70) while the strategic governor's land-grab width
        // allowed three.
        let rapid = if self.rapid_city_expansion_2 && seats_short > 0 {
            advanced::rapid_city_expansion::pipeline_width(
                g,
                n_cities + seats_short,
                n_cities,
                settlers,
            )
        } else {
            None
        };
        if let Some(width) = rapid {
            width
        } else if self.land_grab && seats_short > 0 {
            (crate::ai::LAND_GRAB_PIPELINE_BASE + n_cities / 3).min(seats_short)
        } else if self.parallel_settlers
            && n_cities >= 2
            && ((n_cities + settlers + 1) as f64) < self.w.city_target
        {
            2
        } else {
            1
        }
    }

    /// How many of this player's Settlers have stopped making progress.
    ///
    /// ⚠ THIS EXISTS BECAUSE ONE STUCK SETTLER USED TO END AN EMPIRE'S
    /// EXPANSION FOR THE REST OF THE GAME. The production and gold gates below
    /// both ask `settlers == 0` — "is a settler already in flight" — so a
    /// settler that never founds anything keeps the answer at "yes" forever.
    ///
    /// Measured over the 25 live Civilization VI runs of 2026-08-07/08:
    /// "a settler is already in flight" is **86% of every refusal to build
    /// one** (1,548 of 1,767 logged reasons), the median game's longest-lived
    /// single settler survives **86 turns** of a 250-turn game without
    /// founding, and nine of those runs carried one alive for 82-171 turns
    /// having moved five times or fewer. Those empires finished on a median of
    /// **5 cities against a `city_target` of 7.8 and a window open to turn
    /// 198** — the target and the window were never the binding constraint,
    /// this gate was. Final score tracked city count at r = 0.81, and all 21
    /// completed games were lost with a median score 0.46x the leader's.
    ///
    /// ⚠ THIS IS A DISCOUNT, NOT A LIFTED CAP, and the distinction is the
    /// whole safety argument. `room` above still counts EVERY settler against
    /// `city_target`, so the empire cannot walk more settlers than cities it
    /// still wants — which is what stops a repeat of the uncapped gate that
    /// once ordered seventeen settlers and founded two cities. Discounting a
    /// stranded settler does not rescue it either; it stops one stuck unit
    /// from also costing every future one.
    ///
    /// The Civ VI mod's own ladder already made exactly this fix and records
    /// the same finding ("with `SettlersInFlight = 1` its mere existence stops
    /// the empire ordering another one FOREVER"). The mod is the FALLBACK
    /// decider; under `--civvis-decides` this function is the one that runs,
    /// and it never got the repair.
    pub(super) fn stranded_settlers(&self, g: &Game, pid: usize) -> usize {
        if !self.settler_strand_discount {
            return 0;
        }
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                g.units
                    .get(uid)
                    .is_some_and(|unit| unit.kind.as_str() == "settler")
                    && self
                        .settler_idle
                        .get(uid)
                        .is_some_and(|(_, idle)| *idle >= Self::STRANDED_SETTLER_TURNS)
            })
            .count()
    }

    /// Owned Settlers that have stood on one tile for at least `min_idle`
    /// consecutive turns, as `refresh_settler_idle` counts them — the brake's
    /// reading of the same map `stranded_settlers` reads at twelve. Ungated:
    /// the callers gate on their own gene.
    pub(crate) fn parked_settlers(&self, g: &Game, pid: usize, min_idle: u32) -> usize {
        g.player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                g.units
                    .get(uid)
                    .is_some_and(|unit| unit.kind.as_str() == "settler")
                    && self
                        .settler_idle
                        .get(uid)
                        .is_some_and(|(_, idle)| *idle >= min_idle)
            })
            .count()
    }

    /// Settlers that are actually walking to a site, for the expansion gate.
    pub(super) fn walking_settlers(&self, g: &Game, pid: usize, settlers: usize) -> usize {
        settlers.saturating_sub(self.stranded_settlers(g, pid))
    }

    /// The scripted capital opening is an ordering aid, not a safety exemption.
    ///
    /// `cities` normally leaves a non-empty queue alone, then plays the four
    /// opening-book items directly. That bypassed `pick_item`'s local
    /// barbarian answer: a capital could keep a Settler or Builder queued
    /// while a visible raider already made every civilian step unsafe. The
    /// Firaxis bridge is especially exposed because its hostile Scout is a
    /// real civilian captor but does not carry the native camp-scout marker.
    ///
    /// Interrupt only for the first missing body. Once a local defender is
    /// standing, Walls and ordinary development return to their usual queue
    /// rules; an already-queued non-recon soldier is also already the right
    /// answer and is left alone.
    /// Times the opening's barbarian defense may displace a queued Settler
    /// on a live board before the Settler keeps the queue. The defenders it
    /// raises are drawn off by the board to hunt the raiders, so the local
    /// gap reopens every turn: live King 2026-10-03T145118Z started a Settler
    /// at frame 0 and gave it to a Slinger at frame 1 on turns 12-13, 16-17
    /// and 20-21, built Slingers and Archers from turn 10 to 37, and held one
    /// city until turn 41. A besieged capital still answers through
    /// `besieged_city_item`.
    pub(super) const OPENING_DEFENSE_SETTLER_PREEMPTS: u32 = 2;

    /// The live cap on the opening defense's Settler displacements.
    ///
    /// `opening-defense-preempts-once` (2026-10-10 census, 123 live Emperor
    /// runs): the second displacement fired in 73 of them, a median six turns
    /// after the first, once the first Slinger had been drawn off to hunt.
    /// Those runs saw their first and second Settlers at a median t15/t24
    /// against t11/t19 with no displacement, and the second Settler before
    /// t18 passed the t150 gate 5 of 16 against 3 of 38 after t26. One
    /// Settler was captured by t60 across the window, so the second hold buys
    /// little safety for four turns of the expansion clock.
    pub(super) fn opening_defense_settler_preempts(&self) -> u32 {
        if self.opening_defense_preempts_once {
            1
        } else {
            Self::OPENING_DEFENSE_SETTLER_PREEMPTS
        }
    }

    /// Settlers have a persistent, path-checked destination, so follow that
    /// route rather than taking the generic cheap greedy shortcut first. A
    /// geometrically closer tile can be a one-hex cul-de-sac; with two
    /// movement points the generic mover enters it and immediately routes
    /// back out, repeating the same round trip every turn.
    pub(super) fn settler_step_toward(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        target: Pos,
    ) -> bool {
        if let Some(acted) = self.risk_aware_route_step(g, pid, uid, target, 0) {
            return acted;
        }
        if let Some(next) = g
            .route_step(uid, target, 0)
            .filter(|next| g.can_move(uid, *next))
        {
            return self.path_move(g, pid, uid, next);
        }
        self.step_toward(g, pid, uid, target)
    }

    pub(super) fn settle_value(&self, g: &Game, pos: Pos) -> f64 {
        let mut total = 0.0;
        for p in g.wdisk(pos, 1) {
            if let Some(t) = g.map.get(p) {
                if t.owner_city.is_some() {
                    continue;
                }
                let ys = g.rules.tile_yields(t);
                total += ys.food * self.w.settle_food
                    + ys.production * self.w.settle_prod
                    + ys.gold * self.w.settle_gold;
            }
        }
        total
    }

    pub fn valid_settle_site(&self, g: &Game, pid: usize, pos: Pos) -> bool {
        let Some(tile) = g.map.get(pos) else {
            return false;
        };
        !g.rules.is_water(tile)
            && g.rules.is_passable(tile)
            && !g.tile_is_natural_wonder(tile)
            && !g.cities.values().any(|city| {
                let cap = self.w.min_city_dist.ceil() as i32;
                (g.wdist_capped(city.pos, pos, cap) as f64) < self.w.min_city_dist
            })
            && tile
                .owner_city
                .is_none_or(|cid| g.cities.get(&cid).is_some_and(|city| city.owner == pid))
    }

    /// Count only mutually usable city sites on the capital's connected dry
    /// ground. A cluster of plots inside one city radius is one future city,
    /// not several reasons to postpone an overseas search.
    pub(super) fn main_landmass_settlement_room(
        &self,
        g: &Game,
        pid: usize,
        landmass: &BTreeSet<Pos>,
        wanted: usize,
    ) -> usize {
        let mut sites = Vec::new();
        for pos in landmass {
            if self.valid_settle_site(g, pid, *pos)
                && sites.iter().all(|taken| {
                    let cap = self.w.min_city_dist.ceil() as i32;
                    g.wdist_capped(*taken, *pos, cap) as f64 >= self.w.min_city_dist
                })
            {
                sites.push(*pos);
                if sites.len() >= wanted {
                    break;
                }
            }
        }
        sites.len()
    }

    pub fn has_practical_settle_site(&self, g: &Game, pid: usize) -> bool {
        let shipbuilding = g.players[pid].techs.contains(&crate::name!("shipbuilding"));
        let cartography = g.players[pid].techs.contains(&crate::name!("cartography"));
        // Before embarkation, a city only commits to a site close enough to
        // survive an ordinary settlement race. Existing settlers still use
        // the full path search below, but producing one for a site more than
        // eight steps away routinely loses the site after paying Population.
        let max_steps = if shipbuilding {
            g.map.width + g.map.height
        } else {
            8
        };
        // The observed native seat needs shortest routes for its walking budget.
        // Headless reference games keep their recorded depth-first traversal.
        let shortest_routes = pid == crate::game::MIRRORED_SEAT && !g.host_observed.is_empty();
        let mut frontier: VecDeque<(Pos, i32)> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|city| (g.cities[&city].pos, 0))
            .collect();
        let mut seen: HashSet<Pos> = frontier.iter().map(|(position, _)| *position).collect();
        while let Some((position, steps)) = if shortest_routes {
            frontier.pop_front()
        } else {
            frontier.pop_back()
        } {
            if self.valid_settle_site(g, pid, position) {
                return true;
            }
            if steps >= max_steps {
                continue;
            }
            for next in g.nbrs(position) {
                if seen.contains(&next) {
                    continue;
                }
                let Some(tile) = g.map.get(next) else {
                    continue;
                };
                if !g.rules.is_passable(tile)
                    || (g.rules.is_water(tile) && !shipbuilding)
                    || (tile.terrain == "ocean" && !cartography)
                    || g.city_at(next)
                        .is_some_and(|city| g.cities[&city].owner != pid)
                {
                    continue;
                }
                seen.insert(next);
                frontier.push_back((next, steps + 1));
            }
        }
        false
    }

    pub(super) fn best_reachable_settle_site(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        radius: i32,
    ) -> Option<(Pos, f64)> {
        let from = g.units[&uid].pos;
        let mut candidates: Vec<(Pos, f64)> = g
            .wdisk(from, radius)
            .into_iter()
            .filter(|pos| self.valid_settle_site(g, pid, *pos))
            .map(|pos| {
                let score =
                    self.settle_value(g, pos) - self.w.settle_dist * g.wdist(from, pos) as f64;
                (pos, score)
            })
            .collect();
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
        Self::first_reachable_settle_site(g, uid, &candidates)
    }

    /// Keep reachability checks bounded without assuming the forty most
    /// valuable geometric sites contain a usable one. Before embarkation an
    /// attractive offshore landmass can fill that entire prefix while an
    /// ordinary site on the settler's own landmass ranks just below it. Test
    /// candidates in batches: the multi-goal flood cheaply rejects a wholly
    /// disconnected batch, and individual routes preserve value ordering in
    /// the first batch that contains any reachable site.
    pub(super) fn first_reachable_settle_site(
        g: &Game,
        uid: u32,
        candidates: &[(Pos, f64)],
    ) -> Option<(Pos, f64)> {
        let from = g.units.get(&uid)?.pos;
        for batch in candidates.chunks(40) {
            let contains_current = batch.iter().any(|(pos, _)| *pos == from);
            if !contains_current {
                let goals: HashSet<Pos> = batch.iter().map(|(pos, _)| *pos).collect();
                if g.route_step_to_any(uid, &goals).is_none() {
                    continue;
                }
            }
            if let Some(candidate) = batch
                .iter()
                .find(|(pos, _)| *pos == from || g.route_step(uid, *pos, 0).is_some())
            {
                return Some(*candidate);
            }
        }
        None
    }

    pub(super) fn settler_step(&mut self, g: &mut Game, pid: usize, uid: u32) -> bool {
        if self.minor {
            return false; // city-states and barbarians never settle
        }
        let upos = g.units[&uid].pos;
        let current_target = self.settler_targets.get(&uid).copied().filter(|target| {
            self.valid_settle_site(g, pid, *target)
                && (*target == upos || g.route_step(uid, *target, 0).is_some())
        });
        let target = current_target.or_else(|| {
            let local_radius = if g.player_city_ids(pid).is_empty() {
                2
            } else {
                6
            };
            let local = self.best_reachable_settle_site(g, pid, uid, local_radius);
            // Search distant land even before embarkation. The pathfinder
            // itself rejects disconnected islands; tying the wider search to
            // Shipbuilding stranded settlers whose only site was farther than
            // the local radius on the same landmass.
            let global = self.best_reachable_settle_site(g, pid, uid, g.map.width + g.map.height);
            match (local, global) {
                (Some(local), Some(global)) if global.1 > local.1 + 4.0 => Some(global),
                (Some(local), _) => Some(local),
                (None, global) => global,
            }
            .map(|(target, _)| {
                self.settler_targets.insert(uid, target);
                target
            })
        });
        let Some(target) = target else {
            self.settler_targets.remove(&uid);
            return false;
        };
        if target == upos {
            self.settler_targets.remove(&uid);
            return g.apply(pid, &Action::FoundCity { unit: uid }).is_ok();
        }
        // A linked settler is the follower: the naval military unit is the
        // formation leader and must execute movement for both. Keep the
        // destination for that leader instead of treating the follower's
        // intentionally unavailable Move action as a failed route.
        if let Some(escort) = g.units[&uid].linked_to.filter(|peer| {
            g.units
                .get(peer)
                .is_some_and(|escort| g.rules.units[escort.kind].domain.as_deref() == Some("sea"))
        }) {
            if g.wdist(upos, target) == 1 {
                return g.apply(pid, &Action::UnlinkUnits { unit: escort }).is_ok();
            }
            return false;
        }
        let moved = self.settler_step_toward(g, pid, uid, target);
        if !moved {
            self.settler_targets.remove(&uid);
        }
        moved
    }

    /// The gates of the delegated governor's Settler step, shared with the
    /// steps that must yield to it (`campus_before_the_army_2`).
    pub(super) fn settler_gates(
        &self,
        g: &Game,
        pid: usize,
        city_pop: i32,
        n_cities: usize,
        settlers: usize,
    ) -> SettlerGates {
        let room = ((n_cities + settlers) as f64) < self.w.city_target;
        let stranded = self.stranded_settlers(g, pid);
        // ★★★★ ONE SETTLER AT A TIME IS THE PACE OF THE WHOLE EMPIRE. On the
        // live Civilization VI seat the cities of run civvis-20260815T210845Z
        // were founded at t2/19/45/58/78 and "a settler is already in flight"
        // was the refusal on t19, t42, t43 and t47 — a two-city empire with
        // pop to spare and land open, waiting on one walker. Under
        // `parallel_settlers` a second Settler may start once the empire
        // holds two cities and is still at least two short of its target;
        // the target stays the hard cap and every other gate below is
        // unchanged.
        // ★★★★ AND UNDER THE LAND GRAB THE PIPELINE WIDENS WITH THE
        // EMPIRE: two walkers from the first city, one more per three
        // cities, never more than the seats still short. See `land_grab`
        // and `AdvancedAi::settler_in_flight_allowed`, which opens the
        // same width for the strategic governor.
        let pipeline = self.settler_pipeline_width(g, n_cities, settlers);
        // ★★★★ A PARKED SETTLER HOLDS THE PIPELINE, IT DOES NOT OPEN IT.
        // `stranded-settler-discount` above subtracts a Settler idle
        // twelve turns so its replacement may start — and the replacement
        // refuses the same sites. See `settler_backlog_brake`.
        let v1_scope = self.settler_backlog_brake && n_cities >= Self::SETTLER_BACKLOG_MIN_CITIES;
        let v2_scope =
            self.settler_backlog_brake_2 && n_cities >= Self::SETTLER_BACKLOG_V2_MIN_CITIES;
        let parked = if v1_scope || v2_scope {
            self.parked_settlers(g, pid, Self::SETTLER_BACKLOG_IDLE_TURNS)
        } else {
            0
        };
        let parked_brake = (v1_scope || v2_scope) && parked > 0;
        let none_in_flight = !parked_brake && settlers.saturating_sub(stranded) < pipeline;
        // ★★★★ THE HOST'S FLOOR, NOT THE GENOME'S. See `host_settler_pop`:
        // Civilization VI starts a Settler at population 2, and the live
        // genome's 2.456 held a food-poor capital at one city for forty
        // turns waiting for population 3.
        let settler_min_pop = if self.host_settler_pop || self.rapid_city_expansion_2 {
            self.w.settler_min_pop.min(HOST_SETTLER_MIN_POP)
        } else {
            self.w.settler_min_pop
        };
        let grown = (city_pop as f64) >= settler_min_pop;
        // ★★★★ THE LAND GRAB SETTLES UNTIL A SETTLER CAN NO LONGER
        // REPAY, not until the genome's turn. See `land_grab`.
        let in_window = (g.turn as f64) < self.w.settler_stop_turn
            || ((self.land_grab || self.rapid_city_expansion_2)
                && g.turn + g.standard_duration(LAND_GRAB_SETTLE_HORIZON) < g.max_turns);
        SettlerGates {
            room,
            stranded,
            pipeline,
            parked_brake,
            none_in_flight,
            grown,
            in_window,
        }
    }

    /// Whether the Settler step would train a Settler in this city now.
    /// Diagnostic only: when the empire has room for another city and the
    /// build order picked something other than a Settler for `cid`, journal
    /// the first Settler gate that refused it — the pipeline, the city's
    /// population, the expansion window, a practical site — or, when every
    /// gate admitted it, the earlier step that took the queue. One Detail line
    /// every `SETTLER_HOLD_JOURNAL_STRIDE` turns; it reads only, so no order
    /// changes. See `advanced/settler_admission_journal.rs` for the governor's
    /// twin.
    pub(super) fn journal_settler_hold(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        settlers: usize,
        picked: &Item,
    ) {
        if !g.turn.is_multiple_of(SETTLER_HOLD_JOURNAL_STRIDE)
            || !self.journal.wants(crate::reasoning::Level::Detail)
            || matches!(picked, Item::Unit { unit } if unit == "settler")
        {
            return;
        }
        let city = &g.cities[&cid];
        let gates = self.settler_gates(g, pid, city.pop, n_cities, settlers);
        if !gates.room {
            return;
        }
        let reason = if !gates.none_in_flight {
            if gates.parked_brake {
                "a parked Settler holds the pipeline".to_string()
            } else {
                format!(
                    "the Settler pipeline is full ({settlers} in flight of {})",
                    gates.pipeline
                )
            }
        } else if !gates.grown {
            format!("population {} is below the Settler floor", city.pop)
        } else if !gates.in_window {
            "the expansion window is closed".to_string()
        } else if !self.has_practical_settle_site(g, pid) {
            "no practical settle site".to_string()
        } else {
            format!(
                "outranked by {} earlier in the build order",
                Self::item_label(picked)
            )
        };
        think!(self.journal, Cities, Detail, "Settler held in {}", city.name;
               "{reason}; {n_cities} cities and {settlers} settler{} against a target of {:.1}",
               if settlers == 1 { "" } else { "s" }, self.w.city_target);
    }

    pub(crate) fn settler_due(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        n_cities: usize,
        settlers: usize,
    ) -> bool {
        let gates = self.settler_gates(g, pid, g.cities[&cid].pop, n_cities, settlers);
        gates.room
            && gates.none_in_flight
            && gates.grown
            && gates.in_window
            && self.has_practical_settle_site(g, pid)
    }
}
