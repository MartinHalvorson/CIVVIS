//! `city-state-before-the-first-capital`: a Domination seat's early conquest
//! opening takes one nearby city-state as a stepping stone before, and
//! without spending, the opening on a rival capital.
//!
//! ## Why
//!
//! Deity domination has never won. At turn 50 the live Macedon seat has about
//! a quarter of a Deity rival's production and trades units at 0.45 kills per
//! loss, so `first-capital-before-the-walls` rarely even names a capital. What
//! the seat can beat early is a city-state. Over 110 retained live Deity games
//! on the live setup (Online, six players, Small Continents), a city-state
//! stood within twelve tiles of our capital in about 90% of them (nearest:
//! median 8 tiles, min 6). The nearest at turn 30: defence 24, Ancient Walls
//! (100, never more before about turn 70), population 4-5, about four
//! Warriors and no ranged strike; at turn 50 defence 35, population 6. A
//! suzerain sat on it in 14% of games at turn 30, 36% at 40, 62% at 50 and 83%
//! at 60. Nothing opened that war: the opening's scan skips minors, and the
//! armed `domination-ignores-city-states` keeps the mid-game campaign off
//! them (this gene leaves that alone).
//!
//! ## What the gene does
//!
//! Only on a seat assigned the Domination lane, only with
//! `early-conquest-opening` on, and only through that opening's machinery
//! (reservation, breaker reservation, rally, assembly, bill preview,
//! declaration, pinned campaign):
//!
//! 1. **The target** ([`AdvancedAi::city_state_stone_scan`]): a real
//!    city-state (not the Free Cities, not a barbarian) we have met, whose
//!    city centre we have charted, within [`STONE_REACH_TILES`] of our
//!    capital, walled to at most [`STONE_WALL_CEILING`], with no suzerain at
//!    all (a suzerain is handed the Protectorate War casus belli), one we may
//!    legally declare on, and whose visible field does not outclass our army
//!    (`outclassed_siege::field_outclasses`, the `city-state-harvest` test).
//!    Nearest first, then the lightest visible garrison. Tried only when the
//!    rival-capital scan names nothing, so a capital in hand comes first.
//! 2. **The clock**: named from the turn we hold [`STONE_CITIES`] cities
//!    (the capital trains the first Settler before anything is reserved)
//!    until standard turn [`STONE_NAMING_DEADLINE`] (50 on Online), never
//!    while a major is at war with us and never while `city-state-harvest`
//!    holds a name. A named stone's commit clock is at least that naming
//!    deadline plus the shipped approach grace, and while its force is still
//!    being raised (a body or the breaker missing) or walking to the rally,
//!    the clock does not release it at all before standard turn
//!    [`STONE_HOLD_DEADLINE`] (60 on Online), unless a major is at war with
//!    us ([`AdvancedAi::city_state_stone_commit_hold`]). Every other exit is
//!    unchanged, and a progress line is journalled every few turns.
//! 3. **The force**: [`STONE_MELEE`] melee bodies and [`STONE_RANGED`]
//!    shooters on top of each city's guard (the `first-capital-before-the-walls`
//!    guard rule), plus the shipped breaker reservation's Battering Ram
//!    against the walls, claimed ahead of the bodies once Masonry is known
//!    ([`AdvancedAi::city_state_stone_breaker_lead`]). The reservation trains
//!    whatever melee body scores best now (a Warrior, a Spearman, the unique
//!    line once it is in); the bill preview and the kill-time preview
//!    ([`AdvancedAi::city_state_stone_kill_time`]: the blows the melee bodies
//!    at the rally land before the centre kills them, against its health,
//!    walls and healing), not a unit list, decide readiness. From turn one, while a stone can still be named, research
//!    takes Masonry and then the technology of this civilization's own unique
//!    melee line when it out-fights a Warrior (read from `unique_to`/
//!    `replaces`: Macedon's Hypaspist, Iron Working) as a forced goal
//!    ([`AdvancedAi::city_state_stone_research_goal`]), behind only the
//!    capital-walls and opening-Archery emergencies; the `tech_value` credit
//!    (`conquest_research_value`) alone moved Masonry about ten points.
//!    The declaration waits for a standing breaker (the armed
//!    `opening-yields-to-walls` would otherwise stand the war down on its
//!    first turn). The shipped builder already opens a revealed strategic
//!    deposit (Iron) in our borders before any other tile
//!    (`BasicAi::unopened_strategic_source`), so nothing is added for it.
//! 4. **The declaration**: the shipped surprise war through `raid_opening`,
//!    with the shipped vetoes (bill preview, legality, `one-war-at-a-time`,
//!    treasury), never while a major is at war with us, and without
//!    production parity (a city-state has no handicap economy).
//! 5. **After**: the captured city is kept (never razed or liberated) and the
//!    opening closes WITHOUT closing the game's capital opening, so
//!    `first-capital-before-the-walls` names its target afterwards. Every
//!    other release is the shipped one; no second city-state is tried.
//! 6. While the war runs, [`AdvancedAi::apply_city_state_stone_to_strategy`]
//!    points the plan at the city and lifts a Recovery that has no threatened
//!    city and no major war (the `city-state-harvest` lesson: a small war read
//!    a power gap to an unfought major as "at war and losing ground").
//!
//! Live mirror city ids and minor seats are not stable from one frame to the
//! next, so the stone is keyed by its tile and its civilization and re-read
//! every turn ([`AdvancedAi::city_state_stone_maintain`]). The bridge spares
//! the stone from its envoy-reclaim peace (`civvis_orders`), and our own
//! envoys skip it while it stands.
//!
//! Inert with the flag off, without `early-conquest-opening`, and on any other
//! lane: every entry point returns before reading the board.

use super::city_campaign::NeighbourAppraisal;
use super::early_conquest::{ConquestOpening, CONQUEST_ASSEMBLY_RADIUS};
use super::first_capital_before_the_walls::FIRST_CAPITAL_WALL_CEILING;
use super::outclassed_siege::field_outclasses;
use super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Action, ActionFamilies, Game};
use crate::name::Name;
use crate::think;
use crate::Pos;
use std::collections::BTreeSet;

/// How far from our capital the city-state's city may stand, in the wrapped
/// world distance the opening measures in. In 90% of the 110 live Deity games
/// a city-state stood this close.
pub(crate) const STONE_REACH_TILES: i32 = 12;

/// The most wall HP the city-state may hold: Ancient Walls. The nearest
/// city-state read 100 at turns 30-60 in every census game; a fifth had 200
/// by turn 70.
pub(crate) const STONE_WALL_CEILING: i32 = FIRST_CAPITAL_WALL_CEILING;

/// The last standard turn a stepping stone can be named: 50 on Online. A
/// suzerain sat on the nearest city-state in 62% of live games by turn 50.
pub(crate) const STONE_NAMING_DEADLINE: u32 = 75;

/// The cities the empire holds before a stone is named and the capital's
/// production reserved: the capital's first Settler is never deferred.
pub(crate) const STONE_CITIES: usize = 2;

/// Shooters in the reserved force. One: a shot at a walled centre lands
/// little, and the melee bodies are what open and take it.
pub(crate) const STONE_RANGED: usize = 1;

/// Melee bodies in the reserved force: they strike the walls beside the Ram
/// and take the centre against about four Warriors. Five, not three: over
/// the 47 live Deity games of 2026-11-11 the seat held a median four melee
/// bodies at turn 40 and no Hypaspist before turn 50 (first one at a median
/// turn 60, none by turn 80 in 24), so Warriors carry the strike. A Warrior
/// (20) against a centre of 25 deals about 25 a blow and takes about 37, so
/// it lands three blows: three Warriors deal about 220 against 100 of walls,
/// 200 of health and the centre's healing. The CI replay of the live
/// Auckland board (#5076: four Warriors, three Archers, the Ram beside the
/// walls in 10 of 10) opened the walls in 8 strikes and took the city in 1.
pub(crate) const STONE_MELEE: usize = 5;

/// The most melee bodies the force grows to when the city asks for more than
/// [`STONE_MELEE`] ([`AdvancedAi::city_state_stone_bodies_needed`]). Over 150
/// live Deity games of 2026-11-11 the nearest city-state's centre read a
/// median 24 at turn 30 and 27 at turn 40, behind Ancient walls in every
/// game by turn 30: a Warrior lands three blows of about 23 on it, so the
/// 360 of walls, health and healing take six, not five. A centre past about
/// 32 asks for more Warriors than eight, and the kill-time hold keeps that
/// war undeclared.
pub(crate) const STONE_MELEE_MAX: usize = 8;

/// Bodies reserved beyond the ones the kill-time arithmetic asks for: the
/// preview prices the centre's own blows only, and the city-state's field
/// units (a median military of 123 at turn 30) and its ranged strike cost
/// the strike a body it does not count.
pub(crate) const STONE_SPARE_BODIES: usize = 1;

/// A land unit's full health.
const STONE_BODY_HEALTH: f64 = 100.0;

/// The most blows the kill-time preview counts for one melee body.
pub(crate) const STONE_STRIKE_TURNS: u32 = 4;

/// Turns of the centre's healing the kill-time preview charges.
pub(crate) const STONE_HEAL_TURNS: i32 = 3;

/// What a City Center heals a turn.
pub(crate) const STONE_CITY_HEAL: i32 = 20;

/// What keeping the captured stone is worth in the disposition ranking: it
/// outranks razing or liberating it under any strategy.
pub(crate) const STONE_KEEP_VALUE: f64 = 100_000.0;

/// The last standard turn the commit clock holds a stone whose force is
/// still being raised or walking to the rally: 60 on Online. Before it the
/// stone's own clock is its naming deadline ([`STONE_NAMING_DEADLINE`], 50
/// on Online) plus the shipped approach grace. Live Deity
/// civvis-20261111T061116Z named Hong Kong at turn 15 and released it at 40,
/// "the commit deadline passed before the force ever assembled", with its
/// Battering Ram reserved on turn 35 and three of five bodies pulled onto a
/// barbarian camp and the second city's defence; no suzerain ever came.
pub(crate) const STONE_HOLD_DEADLINE: u32 = 90;

/// The farthest a city buying the stone's Ram may stand from its rally: the
/// reach `breaker-bought-for-the-siege` buys within (`BREAKER_PURCHASE_REACH`).
pub(crate) const STONE_BREAKER_PURCHASE_REACH: i32 = 12;

/// The city-state city the opening stands on, keyed the way a live frame can
/// re-read it: its tile and its civilization.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CityStateStone {
    /// The city's tile.
    pub(crate) pos: Pos,
    /// The city-state's civilization.
    pub(crate) civ: String,
    /// Whether a frame has read the war the opening declared.
    pub(crate) seen_at_war: bool,
    /// Whether the commit clock holds the stone this turn: its force is still
    /// being raised or walking, before [`STONE_HOLD_DEADLINE`] and with no
    /// major at war with us.
    pub(crate) held: bool,
    /// The melee bodies the force reserves: [`STONE_MELEE`] when named, and
    /// up to [`STONE_MELEE_MAX`] once the city's defence asks for more.
    pub(crate) melee: usize,
}

/// What one scan of the board found: the stone to name, or the nearest met
/// city-state and why it is not one.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct StoneScan {
    /// `(distance, garrison, city, city-state)` of the best stone.
    pub(crate) best: Option<(i32, i64, u32, usize)>,
    /// `(distance, city, reason)` of the nearest refused city.
    pub(crate) refused: Option<(i32, u32, &'static str)>,
    /// City-states we have met.
    pub(crate) met: usize,
}

impl AdvancedAi {
    /// Whether this seat plays the gene: the flag, the opening it rides on,
    /// and the Domination lane.
    pub(super) fn city_state_stone_active(&self) -> bool {
        self.city_state_before_the_first_capital
            && self.early_conquest_opening
            && self.victory_target == Some(VictoryTarget::Domination)
    }

    /// Whether the standing opening is the stepping stone. `false` with the
    /// gene off.
    pub(super) fn city_state_stone_stands(&self) -> bool {
        self.city_state_stone_active()
            && self.city_state_stone.is_some()
            && self.conquest_opening.is_some()
    }

    /// The stone's city-state seat while it stands.
    pub(super) fn city_state_stone_target(&self) -> Option<usize> {
        if !self.city_state_stone_stands() {
            return None;
        }
        self.conquest_opening.as_ref().map(|opening| opening.target)
    }

    /// The stone's city tile while it stands, named or at war, for the
    /// host bridge (`civvis_orders`) to spare it from the envoy-reclaim
    /// peace. `None` with the gene off.
    pub fn city_state_opening_tile(&self) -> Option<Pos> {
        if !self.city_state_stone_stands() {
            return None;
        }
        self.city_state_stone.as_ref().map(|stone| stone.pos)
    }

    /// Whether our envoys skip `minor`: the city the opening is taking.
    pub(super) fn city_state_stone_spares_envoys(&self, minor: usize) -> bool {
        self.city_state_stone_target() == Some(minor)
    }

    /// Whether the captured city on `pos` is the stone, kept whatever else
    /// the disposition ranking reads.
    pub(super) fn city_state_stone_keeps(&self, pos: Pos) -> bool {
        self.city_state_stone_stands()
            && self
                .city_state_stone
                .as_ref()
                .is_some_and(|stone| stone.pos == pos)
    }

    /// A major civilization at war with us.
    fn city_state_stone_major_war(g: &Game, pid: usize) -> bool {
        g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && !player.is_free_city
                && g.is_at_war(pid, player.id)
        })
    }

    /// Breakers of ours standing on the board (not queued).
    fn city_state_stone_breakers_standing(g: &Game, pid: usize) -> usize {
        g.units
            .values()
            .filter(|unit| {
                unit.owner == pid && matches!(unit.kind.as_str(), "battering_ram" | "siege_tower")
            })
            .count()
    }

    fn city_state_stone_count(g: &mut Game, pid: usize, key: &str) {
        *g.players[pid]
            .counters
            .entry(format!("city_state_stone:{key}"))
            .or_insert(0) += 1;
    }

    /// ⭐ The scan: every met city-state's city, checked in the order the
    /// reasons are journalled.
    pub(crate) fn city_state_stone_scan(&self, g: &Game, pid: usize, home: Pos) -> StoneScan {
        let declarable: BTreeSet<usize> = g
            .legal_actions_within(pid, ActionFamilies::DIPLOMACY)
            .into_iter()
            .filter_map(|action| match action {
                Action::DeclareWar { player } => Some(player),
                _ => None,
            })
            .collect();
        let visible = self.battlefront_visibility(g, pid);
        let mut scan = StoneScan::default();
        for minor in g.players.iter().filter(|player| {
            player.id != pid
                && player.alive
                && player.is_minor
                && !player.is_barbarian
                && !player.is_free_city
                && g.has_met(pid, player.id)
        }) {
            scan.met += 1;
            let m = minor.id;
            for city in g.player_city_ids(m) {
                let pos = g.cities[&city].pos;
                let distance = g.wdist(home, pos);
                let refusal = if !g.players[pid].explored.contains(&pos) {
                    Some("its city is not charted")
                } else if distance > STONE_REACH_TILES {
                    Some("too far: past the 12 tiles a stepping stone reaches")
                } else if g.city_max_wall_hp(&g.cities[&city]) > STONE_WALL_CEILING {
                    Some("walled past Ancient Walls")
                } else if g.suzerain_of(m) == Some(pid) {
                    Some("we are its suzerain")
                } else if g.suzerain_of(m).is_some() {
                    Some("it has a suzerain")
                } else if !self.campaign_target_legal(g, pid, m) || !declarable.contains(&m) {
                    Some("we may not declare on it")
                } else if field_outclasses(g, pid, m, pos) {
                    Some("its field outclasses our army")
                } else {
                    None
                };
                match refusal {
                    Some(why) => {
                        if scan
                            .refused
                            .is_none_or(|(held, held_city, _)| (distance, city) < (held, held_city))
                        {
                            scan.refused = Some((distance, city, why));
                        }
                    }
                    None => {
                        let garrison =
                            Self::conquest_visible_garrison(g, pid, city, &visible) as i64;
                        let rank = (distance, garrison, city, m);
                        if scan.best.is_none_or(|held| rank < held) {
                            scan.best = Some(rank);
                        }
                    }
                }
            }
        }
        scan
    }

    /// Journal, once every few turns, why no stone is named.
    fn city_state_stone_note(&mut self, g: &Game, scan: &StoneScan, why: Option<&str>) {
        let note_due = self.city_state_stone_noted.is_none_or(|noted| {
            g.turn
                >= noted
                    + g.standard_duration(super::early_conquest::CONQUEST_FEASIBILITY_NOTE_TURNS)
        });
        if !note_due {
            return;
        }
        self.city_state_stone_noted = Some(g.turn);
        if let Some(why) = why {
            think!(self.journal(), Military, Strategy,
                   "City-state before the first capital: none named";
                   "{why}");
            return;
        }
        match scan.refused {
            _ if scan.met == 0 => {
                think!(self.journal(), Military, Strategy,
                       "City-state before the first capital: none named";
                       "no city-state is met");
            }
            None => {
                think!(self.journal(), Military, Strategy,
                       "City-state before the first capital: none named";
                       "no city of the {} met city-state(s) is on the board", scan.met);
            }
            Some((distance, city, why)) => {
                let walls = g.city_max_wall_hp(&g.cities[&city]);
                think!(self.journal(), Military, Strategy,
                       "City-state before the first capital: not {}", g.cities[&city].name;
                       "{} tiles from our capital, walls {}: {why}", distance, walls);
            }
        }
    }

    /// Name the stone. Called after the shipped `conquest_open` has named
    /// nothing, so a rival capital in hand always comes first.
    pub(super) fn city_state_stone_open(&mut self, g: &mut Game, pid: usize) {
        if !self.city_state_stone_active()
            || self.city_state_stone_named
            || self.conquest_closed
            || self.conquest_opening.is_some()
            || g.turn >= g.standard_duration(STONE_NAMING_DEADLINE)
            || g.player_city_ids(pid).len() < STONE_CITIES
        {
            return;
        }
        let Some(capital) = Self::conquest_capital(g, pid) else {
            return;
        };
        let home = g.cities[&capital].pos;
        let scan = self.city_state_stone_scan(g, pid, home);
        if Self::city_state_stone_major_war(g, pid) {
            self.city_state_stone_note(g, &scan, Some("a major is at war with us"));
            return;
        }
        if self.city_state_harvest_target.is_some() {
            self.city_state_stone_note(g, &scan, Some("city-state-harvest holds a name"));
            return;
        }
        let Some((distance, garrison, city, minor)) = scan.best else {
            self.city_state_stone_note(g, &scan, None);
            return;
        };
        let pos = g.cities[&city].pos;
        let Some(rally) = Self::conquest_rally_tile(g, pid, home, pos) else {
            self.city_state_stone_note(g, &scan, Some("no tile on its ring can hold a rally"));
            return;
        };
        self.city_state_stone_named = true;
        self.city_state_stone = Some(CityStateStone {
            pos,
            civ: g.players[minor].civ.clone(),
            seen_at_war: false,
            held: false,
            melee: STONE_MELEE,
        });
        self.conquest_opening = Some(ConquestOpening {
            target: minor,
            city,
            opened: g.turn,
            preparing_since: Some(g.turn),
            grace_until: None,
            rally,
            force: BTreeSet::new(),
            assembled: None,
            declared: None,
            kills_at_war: 0,
            losses: 0,
            taken: 0,
        });
        Self::city_state_stone_count(g, pid, "named");
        let walls = g.city_max_wall_hp(&g.cities[&city]);
        think!(self.journal(), Military, Strategy,
               "City-state before the first capital: {}", g.cities[&city].name;
               "{} tiles, walls {}, garrison {}, no suzerain; the capital reserves a breaker, \
                {} melee bodies and {} shooters on top of the city guards, rallies at the \
                ring and can wait until turn {}, or turn {} while its force is still coming",
               distance, walls, garrison, STONE_MELEE, STONE_RANGED,
               g.standard_duration(STONE_NAMING_DEADLINE)
                   .max(g.turn.saturating_add(self.conquest_preparation(g))),
               g.standard_duration(STONE_HOLD_DEADLINE);
               rally);
    }

    /// ⭐ Start of turn, before the shipped opening's own checks: re-read the
    /// stone from its tile and its civilization, and close it on a capture, a
    /// lost city, a refused or finished war, a suzerain or walls past
    /// Ancient. A capture or release never closes the game's capital
    /// opening (`conquest_release`). Exact no-op with no stone.
    pub(super) fn city_state_stone_maintain(&mut self, g: &mut Game, pid: usize) {
        let Some(stone) = self.city_state_stone.clone() else {
            return;
        };
        if !self.city_state_stone_active() || self.conquest_opening.is_none() {
            if self.conquest_opening.is_some() {
                self.conquest_release(g, "city-state-before-the-first-capital is off");
            }
            self.city_state_stone = None;
            return;
        }
        let Some(cid) = g.city_at(stone.pos) else {
            self.conquest_release(g, "the stepping stone's city is gone");
            return;
        };
        let owner = g.cities[&cid].owner;
        let theirs = g
            .players
            .get(owner)
            .is_some_and(|player| player.civ == stone.civ);
        if let Some(opening) = self.conquest_opening.as_mut() {
            opening.city = cid;
            if theirs {
                opening.target = owner;
            }
        }
        let Some(opening) = self.conquest_opening.clone() else {
            return;
        };
        let name = g.cities[&cid].name.clone();
        if owner == pid {
            Self::city_state_stone_count(g, pid, "captured");
            g.players[pid].counters.insert(
                "city_state_stone:captured_turn".to_string(),
                i64::from(g.turn),
            );
            think!(self.journal(), Military, Strategy,
                   "City-state before the first capital: took {}", name;
                   "kept; {} of the force survive{}; the opening on a rival capital is free \
                    to name its target",
                   opening.force.len(), if opening.force.len() == 1 { "s" } else { "" };
                   stone.pos);
            self.conquest_release(g, "the stepping stone is ours");
            return;
        }
        if !theirs {
            self.conquest_release(g, "the stepping stone changed hands to a third party");
            return;
        }
        let at_war = g.is_at_war(pid, owner);
        if let Some(declared) = opening.declared {
            if at_war {
                if let Some(stone) = self.city_state_stone.as_mut() {
                    stone.seen_at_war = true;
                }
            } else if g.turn > declared {
                // The host can refuse the declaration (`CanDeclareWarOn`
                // false: a city-state peace treaty the mirror does not
                // carry). Live `city-state-harvest` re-declared every frame
                // until it learned this (#4372).
                let why = if stone.seen_at_war {
                    "the war has ended"
                } else {
                    "the host refused the war"
                };
                self.conquest_release(g, why);
                return;
            }
            return;
        }
        if let Some(suzerain) = g.suzerain_of(owner) {
            let why = format!(
                "{} became its suzerain before the war",
                g.players[suzerain].civ
            );
            self.conquest_release(g, &why);
            return;
        }
        if g.city_max_wall_hp(&g.cities[&cid]) > STONE_WALL_CEILING {
            self.conquest_release(g, "its walls rose past Ancient before the war");
            return;
        }
        self.city_state_stone_sizes_the_force(g, &opening);
        let Some(opening) = self.conquest_opening.clone() else {
            return;
        };
        // The clock holds a force that is still being raised or walking.
        let held = opening.assembled.is_none()
            && g.turn < g.standard_duration(STONE_HOLD_DEADLINE)
            && !Self::city_state_stone_major_war(g, pid)
            && self.city_state_stone_in_progress(g, pid, &opening);
        if let Some(stone) = self.city_state_stone.as_mut() {
            stone.held = held;
        }
        self.city_state_stone_progress_note(g, pid, &opening, held);
    }

    /// The melee bodies the stone's force reserves now.
    pub(super) fn city_state_stone_melee(&self) -> usize {
        self.city_state_stone
            .as_ref()
            .map_or(STONE_MELEE, |stone| stone.melee)
    }

    /// The melee bodies the strike on the stone needs, with what one of them
    /// lands before it falls and what the centre takes: the kill-time
    /// arithmetic of [`Self::city_state_stone_kill_time`] for full-health
    /// bodies of the force's weakest melee kind (a Warrior while it has
    /// none), against the centre as it reads today behind at least Ancient
    /// walls (every live city-state had raised them by turn 30), plus
    /// [`STONE_SPARE_BODIES`]. `None` for a vanished city.
    pub(crate) fn city_state_stone_bodies_needed(
        g: &Game,
        opening: &ConquestOpening,
    ) -> Option<(usize, f64, f64)> {
        let city = g.cities.get(&opening.city)?;
        let defence = g.city_strength(city.id);
        let fielded = opening
            .force
            .iter()
            .filter_map(|uid| g.units.get(uid))
            .filter(|unit| Self::conquest_melee_body(&g.rules.units[unit.kind]))
            .map(|unit| g.unit_strength(unit, false))
            .min_by(f64::total_cmp);
        let attack = fielded.or_else(|| g.rules.units.get("warrior").map(|spec| spec.strength))?;
        let dealt = crate::game::expected_damage(attack, defence);
        let taken = crate::game::expected_damage(defence, attack);
        let lands = (STONE_BODY_HEALTH / taken)
            .ceil()
            .min(f64::from(STONE_STRIKE_TURNS));
        let each = dealt * lands;
        let needed = f64::from(city.hp.max(0))
            + f64::from(city.wall_hp.max(STONE_WALL_CEILING))
            + f64::from(STONE_CITY_HEAL * STONE_HEAL_TURNS);
        let bodies = (needed / each).floor() as usize + 1 + STONE_SPARE_BODIES;
        Some((bodies, each, needed))
    }

    /// Grow the reserved melee bodies to what the city asks for, between
    /// [`STONE_MELEE`] and [`STONE_MELEE_MAX`]. It only grows: a force is
    /// not stood down for a centre that lost a point. A grown force is no
    /// longer assembled, so the commit clock holds the stone while the new
    /// body is trained and walks. Before the declaration only.
    fn city_state_stone_sizes_the_force(&mut self, g: &Game, opening: &ConquestOpening) {
        let Some((bodies, each, needed)) = Self::city_state_stone_bodies_needed(g, opening) else {
            return;
        };
        let want = bodies.clamp(STONE_MELEE, STONE_MELEE_MAX);
        let had = self.city_state_stone_melee();
        if want <= had {
            return;
        }
        if let Some(stone) = self.city_state_stone.as_mut() {
            stone.melee = want;
        }
        if let Some(opening) = self.conquest_opening.as_mut() {
            opening.assembled = None;
        }
        let name = g
            .cities
            .get(&opening.city)
            .map(|city| city.name.clone())
            .unwrap_or_default();
        think!(self.journal(), Military, Strategy,
               "City-state before the first capital: the force for {} grows to {} melee bodies",
               name, want;
               "from {had}: one lands about {each:.0} before it falls, against {needed:.0} of \
                health, walls and healing, and {bodies} would finish it with one to spare");
    }

    /// Whether the stone's force is still coming: a reserved body is still
    /// missing, the walls stand and no breaker of ours does yet (reserved,
    /// queued, being trained or waiting on Masonry), or a named body is not
    /// yet at the rally.
    fn city_state_stone_in_progress(
        &self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
    ) -> bool {
        let counts = self.counts(g, pid);
        let (ranged, melee) = self.first_capital_shortfall(g, pid, &counts);
        let walled = g
            .cities
            .get(&opening.city)
            .is_some_and(|city| g.city_max_wall_hp(city) > 0);
        let breaker_coming = walled && Self::city_state_stone_breakers_standing(g, pid) == 0;
        let walking = opening.force.iter().any(|uid| {
            g.units.get(uid).is_some_and(|unit| {
                g.wdist(unit.pos, opening.rally) > super::early_conquest::CONQUEST_ASSEMBLY_RADIUS
            })
        });
        ranged + melee > 0 || breaker_coming || walking
    }

    /// The breaker's state, for the progress line.
    fn city_state_stone_breaker_state(g: &Game, pid: usize, opening: &ConquestOpening) -> String {
        let walled = g
            .cities
            .get(&opening.city)
            .is_some_and(|city| g.city_max_wall_hp(city) > 0);
        if !walled {
            return "not needed".to_string();
        }
        if let Some(ram) = g.units.values().find(|unit| {
            unit.owner == pid && matches!(unit.kind.as_str(), "battering_ram" | "siege_tower")
        }) {
            return format!(
                "standing {} from the rally",
                g.wdist(ram.pos, opening.rally)
            );
        }
        if Self::conquest_breakers_held(g, pid) > 0 {
            return "in training".to_string();
        }
        let masonry = g
            .rules
            .units
            .get("battering_ram")
            .and_then(|ram| ram.tech)
            .is_none_or(|tech| g.players[pid].techs.contains(&tech));
        if masonry {
            "not yet queued".to_string()
        } else {
            "waiting on Masonry".to_string()
        }
    }

    /// The bill preview's two sides for the stone, `(force strength, bill)`,
    /// or `None` with no force named.
    fn city_state_stone_bill(
        &self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
    ) -> Option<(f64, f64)> {
        let appraisal = self.city_state_stone_appraisal(g, pid, opening.target)?;
        let force: Vec<u32> = opening.force.iter().copied().collect();
        if force.is_empty() || !g.cities.contains_key(&opening.city) {
            return None;
        }
        let strength = Self::campaign_strength_of(g, &force);
        let requirement = self.campaign_city_requirement(
            g,
            pid,
            opening.city,
            &appraisal,
            strength / force.len() as f64,
        );
        Some((strength, requirement.strength))
    }

    /// Journal, once every few turns before the war, where the stone's force
    /// stands: bodies at the rally, the board rows holding the rest, the
    /// breaker and the bill.
    fn city_state_stone_progress_note(
        &mut self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
        held: bool,
    ) {
        let note_due = self.city_state_stone_noted.is_none_or(|noted| {
            g.turn
                >= noted
                    + g.standard_duration(super::early_conquest::CONQUEST_FEASIBILITY_NOTE_TURNS)
        });
        if !note_due || !self.journal().wants(crate::reasoning::Level::Strategy) {
            return;
        }
        self.city_state_stone_noted = Some(g.turn);
        let Some(city) = g.cities.get(&opening.city) else {
            return;
        };
        let reserved = self.conquest_force_size();
        let up = opening
            .force
            .iter()
            .filter_map(|uid| g.units.get(uid))
            .filter(|unit| {
                g.wdist(unit.pos, opening.rally) <= super::early_conquest::CONQUEST_ASSEMBLY_RADIUS
            })
            .count();
        // The board rows holding the named bodies that are not at the rally.
        let board = self.objective_board();
        let mut elsewhere: std::collections::BTreeMap<String, usize> =
            std::collections::BTreeMap::new();
        for uid in &opening.force {
            let near = g.units.get(uid).is_some_and(|unit| {
                g.wdist(unit.pos, opening.rally) <= super::early_conquest::CONQUEST_ASSEMBLY_RADIUS
            });
            if near {
                continue;
            }
            let row = board
                .forces
                .iter()
                .find(|force| force.units.contains(uid))
                .and_then(|force| {
                    board
                        .rows
                        .iter()
                        .find(|row| row.key == force.objective_key)
                        .map(|row| format!("{:?}", row.kind))
                })
                .unwrap_or_else(|| "walking".to_string());
            *elsewhere.entry(row).or_insert(0) += 1;
        }
        let elsewhere = if elsewhere.is_empty() {
            "none elsewhere".to_string()
        } else {
            elsewhere
                .iter()
                .map(|(row, n)| format!("{row} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let bill = match self.city_state_stone_bill(g, pid, opening) {
            Some((strength, need)) if strength + 1e-9 >= need => {
                format!("covered ({strength:.0} against {need:.0})")
            }
            Some((strength, need)) => format!(
                "short by {:.0} ({strength:.0} against {need:.0})",
                need - strength
            ),
            None => "not priced (no body named)".to_string(),
        };
        let clock = if held {
            format!(
                "the clock holds it to turn {} at most",
                g.standard_duration(STONE_HOLD_DEADLINE)
            )
        } else {
            format!(
                "the clock runs to turn {}",
                self.conquest_commit_due(g, opening)
            )
        };
        think!(self.journal(), Military, Strategy,
               "City-state before the first capital: {} of {} bodies at the rally for {}",
               up, reserved, city.name;
               "{} named, away: {elsewhere}; breaker {}; bill {bill}; {clock}",
               opening.force.len(), Self::city_state_stone_breaker_state(g, pid, opening);
               opening.rally);
    }

    /// Whether the assembled stone waits for its breaker rather than running
    /// down the shipped patience window: its bill is covered, the walls stand,
    /// no breaker of ours does yet but one is coming (queued, trainable, or
    /// its technology still to research), no major is at war with us, and it
    /// is before standard turn [`STONE_HOLD_DEADLINE`] (60 on Online). Live
    /// civvis-20261111T063232Z held Auckland's covered bill t25-t38 "waiting
    /// for the Battering Ram" and released it at t39 on the patience window.
    pub(super) fn city_state_stone_waits_for_its_breaker(
        &self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
    ) -> bool {
        if !self.city_state_stone_stands()
            || g.turn >= g.standard_duration(STONE_HOLD_DEADLINE)
            || Self::city_state_stone_major_war(g, pid)
            || Self::city_state_stone_breakers_standing(g, pid) > 0
            || !g
                .cities
                .get(&opening.city)
                .is_some_and(|city| g.city_max_wall_hp(city) > 0)
        {
            return false;
        }
        let covered = self
            .city_state_stone_bill(g, pid, opening)
            .is_some_and(|(strength, need)| strength + 1e-9 >= need);
        let ram = crate::game::Item::Unit {
            unit: crate::name!("battering_ram"),
        };
        let coming = Self::conquest_breakers_held(g, pid) > 0
            || self
                .city_state_stone_breaker_research_goal(g, pid)
                .is_some()
            || g.player_city_ids(pid)
                .into_iter()
                .any(|cid| g.can_produce(pid, cid, &ram));
        covered && coming
    }

    /// The patience-window release reason for a stone whose bill was covered
    /// and only the breaker never came; `None` otherwise.
    pub(super) fn city_state_stone_patience_reason(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<&'static str> {
        (self.city_state_stone_stands()
            && Self::city_state_stone_breakers_standing(g, pid) == 0
            && g.players[pid]
                .counters
                .contains_key("city_state_stone:bill_covered_turn"))
        .then_some("the force stood at the rally with the bill covered, but no breaker reached it")
    }

    /// `city-state-before-the-first-capital` under the armed
    /// `breaker-bought-for-the-siege` rule: before the war, once the stone's
    /// bill is covered and the walls stand with no breaker of ours on the
    /// board, buy a Battering Ram for Gold above the purchase upkeep floor in
    /// the city of ours nearest the rally that can train it (within the
    /// purchase reach). True when one was bought. `false` with either gene
    /// off.
    pub(super) fn city_state_stone_breaker_purchase(&mut self, g: &mut Game, pid: usize) -> bool {
        if !self.breaker_bought_for_the_siege || !self.city_state_stone_stands() {
            return false;
        }
        let Some(opening) = self.conquest_opening.clone() else {
            return false;
        };
        if opening.declared.is_some()
            || Self::city_state_stone_breakers_standing(g, pid) > 0
            || !g
                .cities
                .get(&opening.city)
                .is_some_and(|city| g.city_max_wall_hp(city) > 0)
            || !self
                .city_state_stone_bill(g, pid, &opening)
                .is_some_and(|(strength, need)| strength + 1e-9 >= need)
        {
            return false;
        }
        let budget = g.players[pid].gold - self.purchase_upkeep_floor(g, pid, 0.0);
        if budget <= 0.0 {
            return false;
        }
        let ram = crate::name!("battering_ram");
        let item = crate::game::Item::Unit { unit: ram };
        let offer = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| g.wdist(g.cities[cid].pos, opening.rally) <= STONE_BREAKER_PURCHASE_REACH)
            .filter(|cid| g.can_produce(pid, *cid, &item) && !g.purchase_is_blocked(*cid, &item))
            .filter_map(|cid| {
                let price = g.unit_purchase_cost(pid, cid, ram.as_str(), "gold")?;
                (price <= budget + 1e-9).then_some((
                    g.wdist(g.cities[&cid].pos, opening.rally),
                    cid,
                    price,
                ))
            })
            .min_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        let Some((_, cid, price)) = offer else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Buy {
                city: cid,
                unit: ram,
                formation: 0,
                currency: "gold".into(),
            },
        )
        .is_err()
        {
            return false;
        }
        Self::city_state_stone_count(g, pid, "breaker_bought");
        if let Some(target) = g.cities.get(&opening.city) {
            think!(self.journal(), Military, Decision,
                   "{} buys a battering ram for {}", g.cities[&cid].name, target.name;
                   "city-state before the first capital: the bill is covered and the walls \
                    wait on a breaker; {:.0} Gold", price);
        }
        true
    }

    /// `city-state-before-the-first-capital`: before the war, while a
    /// reserved melee body of the stone's force is still missing, buy one
    /// for Gold above the purchase upkeep floor: the strongest melee-class
    /// body a city of ours within the purchase reach of the rally can train
    /// (the class the Ram serves), in the nearest such city. One a turn,
    /// ahead of the peaceful purchases. True when one was bought; `false`
    /// with the gene off, without a stone, and once the war is open.
    ///
    /// The capital alone trains the force, about a Warrior every two turns
    /// once the Ram is done. Live Deity Gran Colombia
    /// `civvis-20261111T175804Z` named Geneva at turn 17 and stood five
    /// Warriors at the rally from turn 31, one short of finishing the city
    /// (327 in blows against 360); a suzerain took Geneva at 37. Over those
    /// turns the treasury bought a Trader (90 Gold, turn 26) and a Builder
    /// (80 Gold, turn 35).
    pub(super) fn city_state_stone_body_purchase(&mut self, g: &mut Game, pid: usize) -> bool {
        if !self.city_state_stone_stands() {
            return false;
        }
        let Some(opening) = self.conquest_opening.clone() else {
            return false;
        };
        if opening.declared.is_some() {
            return false;
        }
        let counts = self.counts(g, pid);
        let (_, missing) = self.first_capital_shortfall(g, pid, &counts);
        if missing == 0 {
            return false;
        }
        let budget = g.players[pid].gold - self.purchase_upkeep_floor(g, pid, 0.0);
        if budget <= 0.0 {
            return false;
        }
        // (strength, distance to the rally, city, unit, price): strongest
        // first, then the nearest city, then the lowest id.
        let mut offer: Option<(f64, i32, u32, Name, f64)> = None;
        for cid in g.player_city_ids(pid) {
            let reach = g.wdist(g.cities[&cid].pos, opening.rally);
            if reach > STONE_BREAKER_PURCHASE_REACH {
                continue;
            }
            for item in g.producible_items(pid, cid) {
                let crate::game::Item::Unit { unit } = &item else {
                    continue;
                };
                let spec = &g.rules.units[unit];
                if spec.promotion_class != "melee"
                    || !Self::conquest_melee_body(spec)
                    || g.purchase_is_blocked(cid, &item)
                {
                    continue;
                }
                let Some(price) = g.unit_purchase_cost(pid, cid, unit.as_str(), "gold") else {
                    continue;
                };
                if price > budget + 1e-9 {
                    continue;
                }
                let better = offer.as_ref().is_none_or(|best| {
                    spec.strength > best.0 + 1e-9
                        || ((spec.strength - best.0).abs() <= 1e-9
                            && (reach, cid) < (best.1, best.2))
                });
                if better {
                    offer = Some((spec.strength, reach, cid, *unit, price));
                }
            }
        }
        let Some((_, _, cid, unit, price)) = offer else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Buy {
                city: cid,
                unit,
                formation: 0,
                currency: "gold".into(),
            },
        )
        .is_err()
        {
            return false;
        }
        Self::city_state_stone_count(g, pid, "body_bought");
        if let Some(target) = g.cities.get(&opening.city) {
            think!(self.journal(), Military, Decision,
                   "{} buys a {} for {}", g.cities[&cid].name, unit.as_str(), target.name;
                   "city-state before the first capital: {} melee bod{} of the force still \
                    missing; {:.0} Gold",
                   missing, if missing == 1 { "y" } else { "ies" }, price);
        }
        true
    }

    /// The stone's commit clock floor: its own naming deadline (50 on
    /// Online), under the shipped approach grace. Zero with no stone.
    pub(super) fn city_state_stone_commit_floor(&self, g: &Game) -> u32 {
        if self.city_state_stone_stands() {
            g.standard_duration(STONE_NAMING_DEADLINE)
        } else {
            0
        }
    }

    /// The commit clock with the stone's hold: a held stone is never due
    /// this turn. `due` unchanged with no stone.
    pub(super) fn city_state_stone_commit_hold(&self, g: &Game, due: u32) -> u32 {
        let held = self.city_state_stone_stands()
            && self
                .city_state_stone
                .as_ref()
                .is_some_and(|stone| stone.held);
        if held {
            due.max(g.turn.saturating_add(1))
        } else {
            due
        }
    }

    /// What the stone's Battering Ram adds to the shipped breaker value, so
    /// it outranks every reserved body: its walls are known from the naming.
    /// Zero with no stone.
    pub(super) fn city_state_stone_breaker_lead(&self) -> f64 {
        if self.city_state_stone_stands() {
            super::early_conquest::CONQUEST_RESERVATION_PER_MISSING
                * self.conquest_force_size() as f64
        } else {
            0.0
        }
    }

    /// `city-state-before-the-first-capital`: the breaker's technology
    /// (Masonry) as the research picker's first forced goal, ahead of every
    /// other goal, from turn one while a stepping stone can still be named or
    /// stands. A tech already in progress finishes first. A `tech_value`
    /// credit is divided by the square root of the node's cost, so the
    /// opening's +90 moved Masonry about ten points: live
    /// civvis-20261111T061116Z researched Animal Husbandry (52) and Bronze
    /// Working (34) ahead of it, and 063232Z took six other techs first and
    /// fielded its first Ram at turn 58 while Auckland's bill stood covered
    /// from turn 25. `None` with the gene off and once the breaker's
    /// technology is known.
    pub(super) fn city_state_stone_breaker_research_goal(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<Name> {
        if !self.city_state_stone_window_open(g) {
            return None;
        }
        g.rules
            .units
            .get("battering_ram")
            .and_then(|ram| ram.tech)
            .filter(|tech| !g.players[pid].techs.contains(tech))
    }

    /// `city-state-before-the-first-capital`: the next forced research goal
    /// for the stone's force: Masonry while unknown (the arm above already
    /// takes it), then this civilization's own unique melee line when it
    /// out-fights a Warrior (Macedon's Hypaspist: Iron Working), behind the
    /// capital-walls and opening-Archery emergencies. `None` with the gene
    /// off.
    pub(super) fn city_state_stone_research_goal(&self, g: &Game, pid: usize) -> Option<Name> {
        if !self.city_state_stone_window_open(g) {
            return None;
        }
        Self::city_state_stone_research_goals(g, pid)
            .first()
            .copied()
    }

    /// Why the stone's assembled force holds at the rally, if it does: a
    /// major is at war with us, a suzerain appeared, the walls stand and no
    /// breaker of ours does, or the melee bodies at the rally would not
    /// finish the city before it kills them
    /// ([`Self::city_state_stone_kill_time`]).
    pub(super) fn city_state_stone_holds(
        &self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
    ) -> Option<&'static str> {
        if !self.city_state_stone_stands() {
            return None;
        }
        if Self::city_state_stone_major_war(g, pid) {
            return Some("a major is at war with us; the city-state waits");
        }
        if g.suzerain_of(opening.target).is_some() {
            return Some("it has a suzerain now");
        }
        let walled = g
            .cities
            .get(&opening.city)
            .is_some_and(|city| g.city_max_wall_hp(city) > 0);
        if walled && Self::city_state_stone_breakers_standing(g, pid) == 0 {
            return Some("waiting for the Battering Ram before the walls");
        }
        let (bodies, blows, needed) = Self::city_state_stone_kill_time(g, opening)?;
        if blows > needed {
            return None;
        }
        think!(self.journal(), Military, Detail,
               "City-state before the first capital: the {} melee bodies at the rally would not finish the city",
               bodies;
               "about {blows:.0} in blows before they fall, against {needed:.0} of health, walls and healing");
        Some("the melee bodies at the rally would not finish the city before they fall")
    }

    /// The kill-time preview of the stone's strike: the melee bodies of the
    /// force within the assembly radius of the rally, the blows they would
    /// land on the centre before it kills them (each body's mean blow, the
    /// engine's `expected_damage`, times the blows it survives to land, at
    /// most [`STONE_STRIKE_TURNS`]), and what the centre takes to fall: its
    /// health, its walls and [`STONE_HEAL_TURNS`] turns of healing. `None`
    /// for a vanished city. The bill preview prices strength against
    /// strength and read three Warriors as enough (live
    /// civvis-20261111T072641Z declared at "86 strength"); this prices the
    /// blows.
    pub(crate) fn city_state_stone_kill_time(
        g: &Game,
        opening: &ConquestOpening,
    ) -> Option<(usize, f64, f64)> {
        let city = g.cities.get(&opening.city)?;
        let defence = g.city_strength(city.id);
        let mut bodies = 0;
        let mut blows = 0.0;
        for unit in opening.force.iter().filter_map(|uid| g.units.get(uid)) {
            if g.wdist(unit.pos, opening.rally) > CONQUEST_ASSEMBLY_RADIUS
                || !Self::conquest_melee_body(&g.rules.units[unit.kind])
            {
                continue;
            }
            let attack = crate::game::effective_strength(g.unit_strength(unit, false), unit.hp);
            let dealt = crate::game::expected_damage(attack, defence);
            let taken = crate::game::expected_damage(defence, attack);
            let lands = (f64::from(unit.hp.max(1)) / taken)
                .ceil()
                .min(f64::from(STONE_STRIKE_TURNS));
            bodies += 1;
            blows += dealt * lands;
        }
        let needed = f64::from(city.hp.max(0))
            + f64::from(city.wall_hp.max(0))
            + f64::from(STONE_CITY_HEAL * STONE_HEAL_TURNS);
        Some((bodies, blows, needed))
    }

    /// Record and journal, once, the first turn the assembled force's bill
    /// preview covered the stone. No-op for anything but the standing stone.
    pub(super) fn city_state_stone_bill_covered(&mut self, g: &mut Game, pid: usize, share: f64) {
        if !self.city_state_stone_stands() {
            return;
        }
        let key = "city_state_stone:bill_covered_turn".to_string();
        if g.players[pid].counters.contains_key(&key) {
            return;
        }
        g.players[pid].counters.insert(key, i64::from(g.turn));
        let Some(opening) = self.conquest_opening.as_ref() else {
            return;
        };
        let force: Vec<u32> = opening.force.iter().copied().collect();
        let strength = Self::campaign_strength_of(g, &force);
        if let Some(city) = g.cities.get(&opening.city) {
            think!(self.journal(), Military, Strategy,
                   "City-state before the first capital: the bill for {} is covered", city.name;
                   "{:.0}% of the force is up at {:.0} strength, {} breaker(s) standing",
                   share * 100.0, strength, Self::city_state_stone_breakers_standing(g, pid);
                   city.pos);
        }
    }

    /// Whether the stone's research credit is open: the gene plays, the
    /// game's capital opening has not closed, and a stone can still be named
    /// or stands.
    fn city_state_stone_window_open(&self, g: &Game) -> bool {
        self.city_state_stone_active()
            && !self.conquest_closed
            && (self.city_state_stone_stands()
                || (!self.city_state_stone_named
                    && g.turn < g.standard_duration(STONE_NAMING_DEADLINE)))
    }

    /// The technologies the stone's force wants: the breaker's (Masonry) and
    /// the technology of this civilization's own unique melee line when that
    /// unit out-fights a Warrior (Macedon's Hypaspist: Iron Working). Read
    /// from the rules' `unique_to`/`replaces`, cheapest unique first.
    pub(crate) fn city_state_stone_research_goals(g: &Game, pid: usize) -> Vec<Name> {
        let mut goals: Vec<Name> = g
            .rules
            .units
            .get("battering_ram")
            .and_then(|ram| ram.tech)
            .into_iter()
            .collect();
        let civ = &g.players[pid].civ;
        let warrior = g
            .rules
            .units
            .get("warrior")
            .map_or(20.0, |spec| spec.strength);
        let unique = g
            .rules
            .units
            .values()
            .filter(|spec| {
                spec.buildable
                    && spec.unique_to.as_ref() == Some(civ)
                    && spec.replaces.is_some()
                    && spec.promotion_class == "melee"
                    && Self::conquest_melee_body(spec)
                    && spec.strength > warrior
            })
            .filter_map(|spec| spec.tech.map(|tech| (spec.cost, tech)))
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, tech)| tech);
        goals.extend(unique);
        goals.retain(|tech| !g.players[pid].techs.contains(tech));
        goals
    }

    /// `city-state-before-the-first-capital`: what `tech` is worth for
    /// leading to the stone force's technologies, from turn one until both are
    /// known, at the shipped opening's research weight per goal. Zero with the
    /// gene off and once the window has closed.
    pub(super) fn city_state_stone_research_value(&self, g: &Game, pid: usize, tech: &str) -> f64 {
        if !self.city_state_stone_window_open(g) {
            return 0.0;
        }
        Self::city_state_stone_research_goals(g, pid)
            .iter()
            .filter(|goal| self.tech_leads_to(g, tech, goal.as_str()))
            .count() as f64
            * super::early_conquest::CONQUEST_RESEARCH
    }

    /// Journal the declaration on the stone.
    pub(super) fn city_state_stone_declared(&mut self, g: &mut Game, pid: usize) {
        let Some(opening) = self.conquest_opening.as_ref() else {
            return;
        };
        let (city, bodies) = (opening.city, opening.force.len());
        Self::city_state_stone_count(g, pid, "declared");
        g.players[pid].counters.insert(
            "city_state_stone:declared_turn".to_string(),
            i64::from(g.turn),
        );
        if let Some(target) = g.cities.get(&city) {
            think!(self.journal(), Military, Strategy,
                   "City-state before the first capital: declaring on {}", target.name;
                   "{} bodies in the force, walls {}, {} breaker(s) standing; surprise war, \
                    production parity not asked",
                   bodies, g.city_max_wall_hp(target),
                   Self::city_state_stone_breakers_standing(g, pid);
                   target.pos);
        }
    }

    /// The bill preview's appraisal of the stone's city-state, which
    /// `appraise_neighbour` (majors only) does not give. `None` for anything
    /// but the standing stone.
    pub(super) fn city_state_stone_appraisal(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
    ) -> Option<NeighbourAppraisal> {
        if self.city_state_stone_target() != Some(target) {
            return None;
        }
        let player = g.players.get(target).filter(|player| player.alive)?;
        let mine = g.player_city_ids(pid);
        let theirs = g.player_city_ids(target);
        let distance = mine
            .iter()
            .flat_map(|a| {
                theirs
                    .iter()
                    .map(move |b| g.wdist(g.cities[a].pos, g.cities[b].pos))
            })
            .min()?;
        Some(NeighbourAppraisal {
            rival: target,
            power_ratio: g.military_power(pid) / g.military_power(target).max(1.0),
            tech_lead: g.players[pid].techs.len() as i64 - player.techs.len() as i64,
            distance,
        })
    }

    /// The overlay: while the stone's war runs, the plan's target is its
    /// city, and a Recovery with no threatened city and no major war keeps
    /// the lane's strategy instead (the `city-state-harvest` lesson). A
    /// timed-war appointment and a threatened Recovery keep their own plan.
    pub(crate) fn apply_city_state_stone_to_strategy(
        &self,
        g: &Game,
        pid: usize,
        plan: &mut StrategicPlan,
    ) {
        if !self.city_state_stone_stands() || self.war_plan.is_some() {
            return;
        }
        let Some(opening) = self.conquest_opening.as_ref() else {
            return;
        };
        if opening.declared.is_none()
            || !g.is_at_war(pid, opening.target)
            || !g
                .cities
                .get(&opening.city)
                .is_some_and(|city| city.owner == opening.target)
        {
            return;
        }
        if plan.strategy == GrandStrategy::Recovery {
            if plan.threatened_city.is_some() || Self::city_state_stone_major_war(g, pid) {
                return;
            }
            let Some(lane) = self.active_victory_target(g) else {
                return;
            };
            plan.strategy = lane.strategy();
            think!(self.journal(), Strategy, Strategy,
                   "City-state before the first capital keeps the {} lane", plan.strategy.as_str();
                   "the Recovery read a power gap to a major we are not fighting; no city of \
                    ours is threatened");
        }
        plan.target_player = Some(opening.target);
        plan.target_city = Some(opening.city);
    }

    /// A stone war that stopped paying asks for terms the way a city-state
    /// war ends: a direct peace, which the host takes unilaterally. Returns
    /// whether it handled `target` (only the standing stone's city-state).
    pub(super) fn city_state_stone_sues_for_peace(
        &mut self,
        g: &mut Game,
        pid: usize,
        target: usize,
    ) -> bool {
        if self.city_state_stone_target() != Some(target) {
            return false;
        }
        if g.is_at_war(pid, target)
            && g.peace_available_at(pid, target).is_none()
            && g.apply(pid, &Action::MakePeace { player: target }).is_ok()
        {
            think!(self.journal(), Diplomacy, Decision,
                   "Making peace with {}", g.players[target].civ;
                   "city-state-before-the-first-capital: the stepping stone stopped paying");
        }
        true
    }
}

#[cfg(test)]
mod tests;
