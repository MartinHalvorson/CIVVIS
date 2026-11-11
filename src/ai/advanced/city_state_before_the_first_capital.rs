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
//!    until standard turn [`STONE_NAMING_DEADLINE`] (50 on Online), with the
//!    shipped preparation window, never while a major is at war with us and
//!    never while `city-state-harvest` holds a name.
//! 3. **The force**: [`STONE_MELEE`] melee bodies and [`STONE_RANGED`]
//!    shooters on top of each city's guard (the `first-capital-before-the-walls`
//!    guard rule), plus the shipped breaker reservation's Battering Ram
//!    against the walls. The reservation trains whatever melee body scores
//!    best now (a Warrior, a Spearman, the unique line once it is in); the
//!    bill preview, not a unit list, decides readiness. From turn one, while a
//!    stone can still be named, the opening's research credit
//!    (`conquest_research_value`) chases Masonry and the technology of this
//!    civilization's own unique melee line when it out-fights a Warrior
//!    (read from `unique_to`/`replaces`: Macedon's Hypaspist, Iron Working),
//!    at the shipped weight and never on top of the opening's own credit.
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
use super::early_conquest::ConquestOpening;
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

/// Shooters in the reserved force.
pub(crate) const STONE_RANGED: usize = 2;

/// Melee bodies in the reserved force: they strike the walls beside the Ram
/// and take the centre against about four Warriors.
pub(crate) const STONE_MELEE: usize = 3;

/// What keeping the captured stone is worth in the disposition ranking: it
/// outranks razing or liberating it under any strategy.
pub(crate) const STONE_KEEP_VALUE: f64 = 100_000.0;

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
               "{} tiles, walls {}, garrison {}, no suzerain; the capital reserves {} melee \
                bodies, {} shooters and a breaker on top of the city guards, rallies at the \
                ring and can wait until turn {}",
               distance, walls, garrison, STONE_MELEE, STONE_RANGED,
               g.turn.saturating_add(self.conquest_preparation(g));
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
        }
    }

    /// Why the stone's assembled force holds at the rally, if it does: a
    /// major is at war with us, a suzerain appeared, or the walls stand and
    /// no breaker of ours does.
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
        (walled && Self::city_state_stone_breakers_standing(g, pid) == 0)
            .then_some("waiting for the Battering Ram before the walls")
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
