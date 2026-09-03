//! `siege-preempts-the-queue` and `guard-breaks-the-pin`: a barbarian ring
//! standing on a city's doorstep is answered with a body before anything
//! else is built, and a Settler's stacked guard cuts down the raider whose
//! zone of control pins the pair.
//!
//! ## The live evidence (Emperor, run `civvis-20260901T193130Z`, Rome, Continents/Small)
//!
//! One city at turn 49. The opening Settler walked out alone at t17 and was
//! taken at t18 two tiles from Rome; the barbarian Warrior that took it then
//! stood on it, two tiles from the capital, for 24 turns. A barbarian Slinger
//! stood ADJACENT to the capital from t29 to t38 with two Warriors behind it,
//! and the seat had no military unit at all from t29 to t35 (its one Warrior
//! died at t28). Two more Settlers were built into that ring and stood on the
//! capital tile for 14 and 7 turns — 46 `Settler HELD short` lines in fifty
//! turns, every one *"the next tile refuses it"* (the raider's zone of
//! control, `Game::can_move`) or *"the safe-step guard rejected every
//! neighbour"* — then wandered in circles within three tiles, and one was
//! taken at t49. 155–198 Gold sat in the bank the whole siege.
//!
//! Three things in the shipped code let that happen:
//!
//! 1. **The barbarian answer only sees an EMPTY queue.** `advanced_production`
//!    consults `barbarian_defense_item` under `committed.is_none()`, so a
//!    city building a Settler (t26–t34) never switches to the Warrior the ring
//!    asks for, and the shopping arms buy nothing because no city is bleeding
//!    (`emergency_city_defense_purchase` reads city damage, and
//!    `garrison-under-fire` is ledger-held anyway).
//! 2. **A Scout counts as a defender.** `barbarian_local_defenders` counts
//!    every land unit of class `military`, and a Scout is one. At t36 the
//!    Scout bought two turns earlier made the gap read zero, and the capital
//!    started a Government Plaza with the Slinger still adjacent.
//! 3. **The guard never swings.** `stacked_guard_step` fortifies on the
//!    Settler's tile every turn (*"Guard stands with its settler"*, t38–t45)
//!    while the Settler refuses every step because of that same raider: a
//!    deadlock only the raider's death breaks.
//!
//! Across the eighteen live Emperor games before this one (2026-08-31 to
//! 2026-09-01) the median second city landed at t22 and the median city
//! count at t50 was 3; raiders stood within two tiles of the capital on
//! fourteen or more turns of the t15–t60 window in eleven of them.
//!
//! ## What the genes do
//!
//! `siege-preempts-the-queue` (opt-in, a `BasicAi` flag, screenable):
//! - a raider (`BasicAi::is_barbarian_raider`: no camp guard, no barbarian
//!   Scout) within [`SIEGE_RADIUS`] of a city while `barbarian_defense_gap`
//!   is positive is a *siege*. A city whose queue holds anything but a
//!   military unit switches to the ring's unit answer (`barbarian_defense_item`,
//!   its unit half only — Walls stay on their own gate). The displaced item's
//!   progress is banked by name (`do_produce`) and resumes when the gap
//!   closes;
//! - a besieged city with NO local defender buys that answer outright when
//!   the treasury covers it, ahead of every reserve;
//! - recon units do not count as local defenders
//!   (`barbarian_local_defenders_for_controller`).
//!
//! `guard-breaks-the-pin` (host-only, under `live_formationless_settler_shadow`
//! like `escort-patience-runs-out`): a stacked guard with its attack left
//! strikes a barbarian military unit in its reach when the exchange is worth
//! it — the blow kills, or deals [`GUARD_STRIKE_TRADE_RATIO`] times what it
//! takes — and the guard keeps [`GUARD_STRIKE_MIN_HP_AFTER`] health after
//! the reply. A melee swing may carry the guard off the Settler's tile on the
//! host, so outside our own city it is only thrown when it kills the LAST
//! raider covering that tile; a shot never moves the shooter. The raider
//! adjacent to the Settler — the one whose zone of control holds it — is
//! preferred.
//!
//! The same operator-armed `siege-preempts-the-queue` treatment also exposes
//! the existing major-war city handoff to the live bridge when the current
//! battlefront names a threatened city. A visible hostile with a legal
//! City-Center attack makes a local melee defender outrank Walls; otherwise
//! the ordinary wall-first major-war answer remains. The handoff runs before
//! diplomacy can accept a same-turn peace offer, because that offer clears the
//! planning clone's war state immediately.
//!
//! A wall is still too slow when the attack envelope already reaches the
//! City Center. In that case the same armed treatment buys the best affordable
//! land defender directly into the city before the queue governor or the
//! ordinary Gold scorer can spend the treasury elsewhere.
//!
//! Off (the defaults) nothing here runs.
use super::civilian_safety::REACH_SCAN_RADIUS;
use super::AdvancedAi;
use crate::ai::BasicAi;
use crate::game::{expected_damage, Action, Game, Item};
use crate::reasoning::plain;
use crate::think;
use crate::Pos;

/// A raider this close to a city centre is a siege, not the distant alarm
/// `HOME_THREAT_RADIUS` (6) raises. Three is `BARBARIAN_LOCAL_DEFENDER_RADIUS`:
/// the ground a local defender is counted on is the ground a raider is
/// answered on. In the run above the Slinger stood at one and the Warriors at
/// two and three.
pub(super) const SIEGE_RADIUS: i32 = 3;
/// The guard swings only while it keeps this much health after the reply: a
/// guard at 40 is still a body a raider must break before the Settler is
/// exposed, and a fresh Warrior taking a Slinger's reply (about 16) stays
/// well above it.
pub(super) const GUARD_STRIKE_MIN_HP_AFTER: f64 = 40.0;
/// A non-lethal swing must deal this many times what it takes. A Warrior on a
/// Slinger deals ~55 for ~16; a Warrior on a full-health Warrior deals 30 for
/// 30 and is refused.
pub(super) const GUARD_STRIKE_TRADE_RATIO: f64 = 1.5;

impl AdvancedAi {
    /// Barbarian raiders within [`SIEGE_RADIUS`] of `city_pos`.
    pub(super) fn siege_raiders_near(&self, g: &Game, city_pos: Pos) -> usize {
        g.units
            .values()
            .filter(|unit| {
                BasicAi::is_barbarian_raider(g, unit) && g.wdist(unit.pos, city_pos) <= SIEGE_RADIUS
            })
            .count()
    }

    /// The unit a besieged city switches its queue to, with the raider count
    /// that asked for it, or `None` when the queue already holds a soldier,
    /// no raider is on the doorstep, or the local garrison is not short.
    pub(super) fn siege_preemption_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        current: &Item,
    ) -> Option<(Item, usize)> {
        if !self.base.siege_preempts_the_queue {
            return None;
        }
        let already_a_soldier = match current {
            Item::Unit { unit } | Item::Formation { unit, .. } => g
                .rules
                .units
                .get(unit)
                .is_some_and(|spec| spec.class == "military" && spec.promotion_class != "recon"),
            _ => false,
        };
        if already_a_soldier {
            return None;
        }
        let city = g.cities.get(&cid).filter(|city| city.owner == pid)?;
        let raiders = self.siege_raiders_near(g, city.pos);
        if raiders == 0 || self.base.barbarian_defense_gap(g, pid, cid) == 0 {
            return None;
        }
        match self.base.barbarian_defense_item(g, pid, cid)? {
            item @ Item::Unit { .. } => Some((item, raiders)),
            _ => None,
        }
    }

    /// Buy the ring's answer for a besieged city that has no local defender
    /// at all, when the treasury covers it. Returns what was bought.
    pub(super) fn siege_defender_purchase(
        &mut self,
        g: &mut Game,
        pid: usize,
        cid: u32,
    ) -> Option<Item> {
        if !self.base.siege_preempts_the_queue {
            return None;
        }
        let city = g.cities.get(&cid).filter(|city| city.owner == pid)?;
        let (city_pos, city_name) = (city.pos, city.name.clone());
        let raiders = self.siege_raiders_near(g, city_pos);
        if raiders == 0
            || self
                .base
                .barbarian_local_defenders_for_controller(g, pid, cid)
                > 0
        {
            return None;
        }
        let Some(Item::Unit { unit }) = self.base.barbarian_defense_item(g, pid, cid) else {
            return None;
        };
        let cost = g.unit_purchase_cost(pid, cid, unit.as_str(), "gold")?;
        let gold = g.players[pid].gold;
        if gold + f64::EPSILON < cost {
            think!(self.journal(), Military, Detail, "{} cannot buy its {} under siege", city_name, plain(&unit);
                   "{raiders} raider(s) within {SIEGE_RADIUS} tiles and no defender at all, \
                    but {cost:.0} Gold is asked of {gold:.0}");
            return None;
        }
        let action = Action::Buy {
            city: cid,
            unit,
            formation: 0,
            currency: "gold".to_string(),
        };
        if g.apply(pid, &action).is_err() {
            return None;
        }
        let Action::Buy { unit, .. } = action else {
            return None;
        };
        think!(self.journal(), Military, Decision, "Buying {} for {} under siege", plain(&unit), city_name;
               "{raiders} raider(s) within {SIEGE_RADIUS} tiles and no defender at all; \
                {cost:.0} of {gold:.0} Gold buys the answer now, ahead of every reserve");
        Some(Item::Unit { unit })
    }

    /// Buy an immediate local defender for a named major-war city whose
    /// battlefront can already execute an attack on its City Center.
    ///
    /// The existing major-war handoff starts Walls before damage, but Walls
    /// are production-only in Civ VI and cannot answer an attack arriving in
    /// one turn. The live run `civvis-20260903T110637Z` demonstrated the gap:
    /// Cumae started a six-turn Walls build at t60 and fell at t63; Lugdunum
    /// started a two-turn Walls build at t80 and fell at t84. A purchase is
    /// the only local answer that arrives before the next host combat phase.
    ///
    /// This remains deliberately narrower than `native-emergency-purchase`:
    /// the operator must have armed `siege-preempts-the-queue`, the live
    /// battlefront observation must name the city, a major war must be active,
    /// and the turn-start attack envelope must reach the City Center. The
    /// candidate list is taken from the host/model purchase menu, then chooses
    /// the strongest affordable non-siege land unit rather than asking for a
    /// strongest unit that the treasury or host menu cannot actually provide.
    pub(super) fn imminent_major_war_defense_purchase(
        &mut self,
        g: &mut Game,
        pid: usize,
        threatened_city: Option<u32>,
    ) -> bool {
        if !self.base.siege_preempts_the_queue || !self.battlefront_observation {
            return false;
        }
        let Some(cid) = threatened_city else {
            return false;
        };
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return false;
        };
        let city_name = city.name.clone();
        let active_major_war = g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        });
        if !active_major_war {
            return false;
        }
        let visible = self.battlefront_visibility(g, pid);
        if !Self::imminent_city_attack(g, pid, cid, &visible) {
            return false;
        }

        let candidate = g
            .legal_purchase_actions_for_city(pid, cid)
            .0
            .into_iter()
            .filter_map(|action| {
                let Action::Buy {
                    city: buyer,
                    unit,
                    formation,
                    currency,
                } = action
                else {
                    return None;
                };
                if buyer != cid || formation != 0 || currency != "gold" {
                    return None;
                }
                let spec = g.rules.units.get(&unit)?;
                if spec.class != "military"
                    || spec.siege
                    || spec.promotion_class == "recon"
                    || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                {
                    return None;
                }
                let power = spec.strength.max(spec.ranged_attack_strength());
                Some((
                    power,
                    unit.to_string(),
                    Action::Buy {
                        city: buyer,
                        unit,
                        formation,
                        currency,
                    },
                ))
            })
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    // Keep the same stable smallest-name tie-break as
                    // `BasicAi::best_military`'s ordered rules walk.
                    .then_with(|| right.1.cmp(&left.1))
            });
        let Some((_, _, action)) = candidate else {
            return false;
        };
        let Action::Buy { unit, .. } = &action else {
            return false;
        };
        let Some(cost) = g.unit_purchase_cost(pid, cid, unit.as_str(), "gold") else {
            return false;
        };
        let gold = g.players[pid].gold;
        if g.apply(pid, &action).is_err() {
            return false;
        }
        think!(self.journal(), Military, Decision,
               "Buying {} for {} before an imminent major-war attack", plain(unit), city_name;
               "the battlefront can strike the City Center this turn; {cost:.0} of {gold:.0} Gold places the strongest affordable land defender immediately");
        true
    }

    /// `guard-breaks-the-pin`: the guard standing on its Settler's tile
    /// strikes a barbarian unit in reach when the exchange is worth it.
    /// `true` when a blow was struck (the guard's turn is spent).
    pub(super) fn guard_breaks_the_pin(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        settler_pos: Pos,
    ) -> bool {
        if !self.guard_breaks_the_pin {
            return false;
        }
        let Some(barb) = g.barb_pid else {
            return false;
        };
        let Some(unit) = g.units.get(&uid).cloned() else {
            return false;
        };
        if unit.owner != pid
            || unit.pos != settler_pos
            || unit.moves_left <= 0.0
            || unit.attacks_left <= 0
        {
            return false;
        }
        let spec = &g.rules.units[unit.kind];
        let ranged = spec.has_ranged_attack();
        if !ranged && !spec.is_melee_capable() {
            return false;
        }
        let range = if ranged { g.unit_attack_range(uid) } else { 1 };
        let in_own_city = g
            .city_at(settler_pos)
            .is_some_and(|city| g.cities[&city].owner == pid);
        let covering = self
            .barbarian_reach(g, pid, settler_pos, REACH_SCAN_RADIUS)
            .raiders_covering(g, settler_pos);
        let mut best: Option<(f64, Pos, bool, String)> = None;
        for raider in g.units.values().filter(|raider| {
            raider.owner == barb
                && g.rules.units[raider.kind].class == "military"
                && (1..=range).contains(&g.wdist(raider.pos, unit.pos))
        }) {
            let (dealt, taken) = if ranged {
                let Some((att, def)) = g.ranged_strike_strengths(uid, raider.id, raider.pos) else {
                    continue;
                };
                (expected_damage(att, def), 0.0)
            } else {
                let Some((att, def)) = g.melee_exchange_strengths(uid, raider.id) else {
                    continue;
                };
                (expected_damage(att, def), expected_damage(def, att))
            };
            let lethal = dealt + f64::EPSILON >= f64::from(raider.hp);
            if f64::from(unit.hp) - taken < GUARD_STRIKE_MIN_HP_AFTER {
                continue;
            }
            if !lethal && dealt < GUARD_STRIKE_TRADE_RATIO * taken {
                continue;
            }
            // A melee kill may carry the guard onto the raider's tile on the
            // host. Inside our own city the Settler is safe without it; in
            // the field the swing is thrown only when it removes the last
            // raider that could step onto the Settler next turn.
            if !ranged && !in_own_city && !(lethal && covering <= 1) {
                continue;
            }
            let pins = g.wdist(raider.pos, settler_pos) == 1;
            let score =
                dealt - taken + if lethal { 50.0 } else { 0.0 } + if pins { 25.0 } else { 0.0 };
            if best.as_ref().is_none_or(|(held, ..)| score > *held) {
                best = Some((score, raider.pos, lethal, plain(&raider.kind)));
            }
        }
        let Some((_, at, lethal, victim)) = best else {
            return false;
        };
        let action = if ranged {
            Action::Ranged {
                unit: uid,
                target: at,
            }
        } else {
            Action::Attack {
                unit: uid,
                target: at,
            }
        };
        if g.apply(pid, &action).is_err() {
            return false;
        }
        think!(self.journal(), Expansion, Detail,
               "Guard {} the {victim} pinning its settler", if ranged { "shoots" } else { "cuts down" };
               "its zone of control is what holds the pair on {settler_pos:?}; {}",
               if lethal { "the blow kills it" } else { "the trade favours the guard" }; at);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::advanced::{GrandStrategy, StrategicPlan};

    /// Two founded capitals, no units anywhere, no camps, player 0 to move.
    fn siege_board(seed: u64) -> (Game, u32, Pos) {
        let mut game = Game::new_full(2, 30, 20, seed, 120, 0, true);
        for player in 0..2 {
            let settler = game
                .player_unit_ids(player)
                .into_iter()
                .find(|uid| game.units[uid].kind == "settler")
                .expect("each player opens with a settler");
            game.current = player;
            game.apply(player, &Action::FoundCity { unit: settler })
                .unwrap();
        }
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(uid);
        }
        game.barb_camps.clear();
        game.current = 0;
        let cid = game.player_city_ids(0)[0];
        let home = game.cities[&cid].pos;
        (game, cid, home)
    }

    /// Open land tiles exactly `distance` from `home`, in a stable order.
    fn open_ring(game: &Game, home: Pos, distance: i32) -> Vec<Pos> {
        let mut ring: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                game.wdist(*pos, home) == distance
                    && game.map.get(*pos).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
                    && game.city_at(*pos).is_none()
                    && game.unit_ids_at(*pos).is_empty()
            })
            .collect();
        ring.sort_unstable();
        ring
    }

    /// The live ring: a Slinger adjacent to the capital, two Warriors behind it.
    fn besiege(game: &mut Game, home: Pos) {
        let barb = game.barb_pid.expect("a barbarian seat");
        let adjacent = open_ring(game, home, 1);
        game.spawn_test_unit("slinger", barb, adjacent[0]);
        let second = open_ring(game, home, 2);
        game.spawn_test_unit("warrior", barb, second[0]);
        game.spawn_test_unit("warrior", barb, second[1]);
    }

    fn plan(game: &Game) -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: game.turn,
            rush: false,
        }
    }

    fn is_soldier(game: &Game, item: &Item) -> bool {
        match item {
            Item::Unit { unit } => game
                .rules
                .units
                .get(unit)
                .is_some_and(|spec| spec.class == "military" && spec.promotion_class != "recon"),
            _ => false,
        }
    }

    /// The first civilian build the capital can start: the Settler of the
    /// live run when the rules allow it at this size, else a Builder, else
    /// a Monument.
    fn civilian_build(game: &Game, cid: u32) -> Item {
        let candidates = [
            Item::Unit {
                unit: crate::name!("settler"),
            },
            Item::Unit {
                unit: crate::name!("builder"),
            },
            Item::Building {
                building: crate::name!("monument"),
            },
        ];
        candidates
            .into_iter()
            .find(|item| game.can_produce(0, cid, item))
            .expect("a civilian build is legal in a fresh capital")
    }

    #[test]
    fn the_genes_are_registered_and_ship_off() {
        let fresh = AdvancedAi::new();
        assert!(!fresh.base.siege_preempts_the_queue);
        assert!(!fresh.guard_breaks_the_pin);
        assert!(!AdvancedAi::legacy().base.siege_preempts_the_queue);
        assert!(!AdvancedAi::legacy().guard_breaks_the_pin);
        let siege = crate::ai::gene("siege-preempts-the-queue").expect("registered");
        assert!(siege.opt_in());
        assert!(siege.screenable());
        let guard = crate::ai::gene("guard-breaks-the-pin").expect("registered");
        assert!(guard.live(), "host-only genes ship with the live universe");
        assert!(
            !guard.screenable(),
            "a native board never reaches the stacked-guard path"
        );
        let mut toggled = AdvancedAi::new();
        toggled.enable_siege_preempts_the_queue();
        toggled.enable_guard_breaks_the_pin();
        assert!(toggled.base.siege_preempts_the_queue && toggled.guard_breaks_the_pin);
        toggled.disable_siege_preempts_the_queue();
        toggled.disable_guard_breaks_the_pin();
        assert!(!toggled.base.siege_preempts_the_queue && !toggled.guard_breaks_the_pin);
    }

    #[test]
    fn a_scout_is_not_a_defender() {
        let (mut game, cid, home) = siege_board(90_201);
        besiege(&mut game, home);
        game.spawn_test_unit("scout", 0, home);
        let control = AdvancedAi::new();
        assert_eq!(
            control
                .base
                .barbarian_local_defenders_for_controller(&game, 0, cid),
            1
        );
        assert_eq!(
            control.base.barbarian_defense_gap(&game, 0, cid),
            1,
            "shipped: the Scout closes half the two-body gap"
        );
        let mut treated = AdvancedAi::new();
        treated.enable_siege_preempts_the_queue();
        assert_eq!(
            treated
                .base
                .barbarian_local_defenders_for_controller(&game, 0, cid),
            0
        );
        assert_eq!(treated.base.barbarian_defense_gap(&game, 0, cid), 2);
        game.spawn_test_unit("warrior", 0, home);
        assert_eq!(treated.base.barbarian_defense_gap(&game, 0, cid), 1);
    }

    #[test]
    fn a_siege_switches_a_civilian_build_to_the_defender_and_banks_it() {
        let (mut game, cid, home) = siege_board(90_202);
        besiege(&mut game, home);
        game.players[0].gold = 0.0;
        let civilian = civilian_build(&game, cid);
        game.apply(
            0,
            &Action::Produce {
                city: cid,
                item: civilian.clone(),
            },
        )
        .unwrap();
        game.cities.get_mut(&cid).unwrap().production = 20.0;
        let plan = plan(&game);

        let mut untouched = game.clone();
        let mut control = AdvancedAi::new();
        control.advanced_production(&mut untouched, 0, &plan, false);
        assert_eq!(
            untouched.cities[&cid].queue.first(),
            Some(&civilian),
            "shipped: a committed queue is never shown the barbarian answer"
        );

        let mut treated = AdvancedAi::new();
        treated.enable_siege_preempts_the_queue();
        assert!(treated
            .siege_preemption_item(&game, 0, cid, &civilian)
            .is_some());
        treated.advanced_production(&mut game, 0, &plan, false);
        let head = game.cities[&cid].queue.first().cloned().expect("a queue");
        assert!(
            is_soldier(&game, &head),
            "the ring's answer is a soldier, got {head:?}"
        );
        assert!(
            game.cities[&cid]
                .production_progress
                .values()
                .any(|banked| (banked - 20.0).abs() < 1e-9),
            "the displaced build keeps its progress by name"
        );
        assert!(
            treated
                .siege_preemption_item(&game, 0, cid, &head)
                .is_none(),
            "a queue already holding a soldier is left alone"
        );
    }

    #[test]
    fn a_defenceless_besieged_city_buys_its_defender_ahead_of_every_reserve() {
        let (mut game, _cid, home) = siege_board(90_203);
        besiege(&mut game, home);
        game.players[0].gold = 1_000.0;
        let plan = plan(&game);
        let soldiers = |game: &Game| {
            game.player_unit_ids(0)
                .into_iter()
                .filter(|uid| {
                    let spec = &game.rules.units[game.units[uid].kind];
                    spec.class == "military" && spec.promotion_class != "recon"
                })
                .count()
        };

        let mut untouched = game.clone();
        let mut control = AdvancedAi::new();
        control.advanced_production(&mut untouched, 0, &plan, false);
        assert_eq!(
            soldiers(&untouched),
            0,
            "shipped: nothing buys a body for a pinned city"
        );
        assert!((untouched.players[0].gold - 1_000.0).abs() < 1e-9);

        let mut treated = AdvancedAi::new();
        treated.enable_siege_preempts_the_queue();
        treated.advanced_production(&mut game, 0, &plan, false);
        assert_eq!(
            soldiers(&game),
            1,
            "the besieged capital bought its defender"
        );
        assert!(
            game.players[0].gold < 1_000.0 - 1.0,
            "the purchase was paid for"
        );
        assert!(
            game.player_unit_ids(0)
                .into_iter()
                .any(|uid| game.units[&uid].pos == home),
            "the body stands in the city"
        );
        // A second review does not buy a second body: the gap is now closed
        // by the purchase, and the queue answer covers the rest.
        let gold_after_one = game.players[0].gold;
        treated.advanced_production(&mut game, 0, &plan, false);
        assert_eq!(soldiers(&game), 1);
        assert!((game.players[0].gold - gold_after_one).abs() < 1e-9);
    }

    #[test]
    fn the_guard_cuts_down_the_slinger_pinning_its_settler() {
        let (mut game, _cid, home) = siege_board(90_204);
        let barb = game.barb_pid.expect("a barbarian seat");
        assert!(game.is_at_war(0, barb), "barbarians are always at war");
        let settler = game.spawn_test_unit("settler", 0, home);
        let guard = game.spawn_test_unit("warrior", 0, home);
        let adjacent = open_ring(&game, home, 1);
        let slinger = game.spawn_test_unit("slinger", barb, adjacent[0]);

        let mut control = AdvancedAi::new();
        control.settler_guards.insert(settler, guard);
        let mut untouched = game.clone();
        assert_eq!(
            control.stacked_guard_step(&mut untouched, 0, guard),
            Some(false),
            "shipped: the guard fortifies on the settler's tile"
        );
        assert_eq!(untouched.units[&slinger].hp, 100);

        let mut treated = AdvancedAi::new();
        treated.enable_guard_breaks_the_pin();
        treated.settler_guards.insert(settler, guard);
        assert_eq!(treated.stacked_guard_step(&mut game, 0, guard), Some(true));
        assert!(
            game.units.get(&slinger).is_none_or(|unit| unit.hp < 100),
            "the Slinger took the blow"
        );
        assert!(game.units[&guard].hp > 60, "a Slinger's reply is small");
        assert_eq!(
            game.units[&settler].pos, home,
            "the Settler stays where it was"
        );
    }

    #[test]
    fn the_guard_does_not_swing_into_a_losing_trade() {
        let (mut game, _cid, home) = siege_board(90_205);
        let barb = game.barb_pid.expect("a barbarian seat");
        let settler = game.spawn_test_unit("settler", 0, home);
        let guard = game.spawn_test_unit("warrior", 0, home);
        let adjacent = open_ring(&game, home, 1);
        let swordsman = game.spawn_test_unit("swordsman", barb, adjacent[0]);
        let mut treated = AdvancedAi::new();
        treated.enable_guard_breaks_the_pin();
        treated.settler_guards.insert(settler, guard);
        assert_eq!(
            treated.stacked_guard_step(&mut game, 0, guard),
            Some(false),
            "a Warrior does not swing at a Swordsman for a trade it loses"
        );
        assert_eq!(game.units[&swordsman].hp, 100);
        assert_eq!(game.units[&guard].hp, 100);
    }

    #[test]
    fn in_the_field_the_guard_swings_only_to_clear_the_last_raider() {
        let (mut game, _cid, home) = siege_board(90_206);
        let barb = game.barb_pid.expect("a barbarian seat");
        // The pair stands two tiles out, with a full-health Warrior adjacent
        // and a second raider two tiles beyond it: the swing is not lethal and
        // would leave the Settler covered, so the guard holds.
        let field = open_ring(&game, home, 2)[0];
        let settler = game.spawn_test_unit("settler", 0, field);
        let guard = game.spawn_test_unit("warrior", 0, field);
        let beside = open_ring(&game, field, 1)
            .into_iter()
            .find(|pos| game.city_at(*pos).is_none())
            .expect("open ground beside the pair");
        let raider = game.spawn_test_unit("warrior", barb, beside);
        let mut treated = AdvancedAi::new();
        treated.enable_guard_breaks_the_pin();
        treated.settler_guards.insert(settler, guard);
        assert_eq!(treated.stacked_guard_step(&mut game, 0, guard), Some(false));
        assert_eq!(game.units[&raider].hp, 100);
    }
}
