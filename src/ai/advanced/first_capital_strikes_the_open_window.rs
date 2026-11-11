//! `first-capital-strikes-the-open-window`: the first-capital opening keeps
//! its force and the campaign through its window, declares as soon as the
//! bodies at the rally cover the city's bill, opens its reservation a city
//! sooner on a soft capital, and is not released at the commit deadline
//! while its army was held away or its bill was already covered.
//!
//! ## Why
//!
//! Live Deity Macedon `civvis-20261111T040704Z` (four players, Pangaea,
//! Online, Domination) named Mikisiw-Wacîhk, the Cree capital 16 tiles out,
//! at turn 16 and never declared. The capital read a defence of 18-28 behind
//! Ancient Walls from turn 30 to 43, and at turn 40 our 226 power stood
//! against the whole Cree army's 104. Three things kept the window shut:
//!
//! - the reservation waited for the third city (turn 29), so the force was
//!   still being trained when the window was open;
//! - at turn 36 `war-target-prices-the-walls`, and at turn 44 the counter on
//!   Phoenicia's religion race, re-aimed the campaign at Tyre, 21 tiles
//!   away; at turn 46 a religious interception opened a war on Phoenicia,
//!   which stopped the opening's staging (any war did) and put the plan in
//!   Recovery;
//! - the declaration waited for 80% of the reserved six at the rally, and
//!   only two bodies ever stood there. At turn 57 the commit deadline
//!   released the opening.
//!
//! The chance is common. Over 109 retained live Deity games
//! (`soft_city_census.py`), a rival major's city within 12 tiles of ours
//! stood unwalled at defence 25 or less in 75, for a median 14 turns from a
//! median turn 26; in 25 games that city was an original capital. The window
//! closes around turn 45-50, when Deity rivals field Crossbowmen and walls.
//!
//! ## What the gene changes
//!
//! Only while `first-capital-before-the-walls` has named a capital and its
//! war is not yet open, inside the window (`conquest_reservation_open`):
//!
//! 1. **The opening keeps its force and the campaign**
//!    ([`AdvancedAi::first_capital_keeps_the_campaign`]). `assess` aims the
//!    campaign at the opening's target and city whatever the counter, the
//!    war-target pick or the prey rule would choose; only a home emergency or
//!    a front war still running outranks it. Elective wars (the plan's own
//!    declaration, second fronts, the opportunistic raid) are held
//!    ([`AdvancedAi::first_capital_holds_the_war`]), and the opening's
//!    declaration comes first in the turn's diplomacy. A defensive home war
//!    for the faith (the religious interception, `faith-war-before-the-flip`,
//!    the heretic strike) may still open, but it is no front
//!    ([`AdvancedAi::first_capital_faith_fronts`]): it does not own the
//!    campaign, `one-war-at-a-time` does not hold the opening's declaration
//!    for it, the opening's staging does not stop for it, and a force body
//!    walks on to the rally through it. A capital whose owner we already
//!    fight is not named at all (`first_capital_check`): live
//!    `civvis-20261111T063232Z` named Wak Kab'nal and released it in the same
//!    frame nine times over t53-58.
//! 2. **Go when the board says go**
//!    ([`AdvancedAi::first_capital_open_window_go`]). Once at least
//!    `CAMPAIGN_MIN_BODIES` force bodies stand within the assembly radius of
//!    the rally and they alone cover the city's bill
//!    (`campaign_city_requirement`, as `conquest_preview_takes_the_city`
//!    prices it), with a breaker at the rally or no walls standing, the
//!    opening counts as assembled and declares that turn. The shipped 80%
//!    share rule stays the fallback. Under either rule no declaration passes
//!    [`AdvancedAi::first_capital_outclassed`]: at least
//!    [`FIRST_CAPITAL_STRIKE_CITIES`] cities held,
//!    [`FIRST_CAPITAL_STRIKE_MELEE`] melee soldiers (never Scouts) at the
//!    rally, the owner's public military power at most
//!    [`FIRST_CAPITAL_POWER_RATIO`] times ours, a melee soldier within
//!    [`FIRST_CAPITAL_MELEE_MARGIN`] of the city's strength, the owner's
//!    visible army within [`FIRST_CAPITAL_DEFENDER_REACH`] no stronger than
//!    the bodies at the rally, and the melee soldiers' expected blows over
//!    [`FIRST_CAPITAL_STRIKE_TURNS`] turns above the city's health, walls and
//!    healing. Live 063232Z struck a defence-56 capital with Hypaspists of 38
//!    at t80 (no damage in six turns); live 071743Z struck Ethiopia at t7 with
//!    one city, one Warrior and two Scouts at 35 power against 113.
//! 3. **A soft capital starts sooner**
//!    ([`AdvancedAi::first_capital_read_the_softness`]). While the named
//!    capital is walled to Ancient or less and its strength reads
//!    [`FIRST_CAPITAL_SOFT_STRENGTH`] or less, the reservation opens at
//!    [`FIRST_CAPITAL_SOFT_SETTLER_CITIES`] cities instead of three (the
//!    capital still trains the first Settler). The force bodies already walk
//!    to the rally as they are trained (`conquest_staging_objective`); what
//!    was missing is the breaker, which the reservation trains but which was
//!    never a force body, so it stayed home
//!    ([`AdvancedAi::first_capital_breaker_walks`]).
//! 4. **The deadline does not release an opening that never had its army**
//!    ([`AdvancedAi::first_capital_window_grace`]). At the commit deadline,
//!    an unassembled opening whose force was held away by a home faith war,
//!    or whose whole force already covers the bill, gets the shipped grace
//!    (`CONQUEST_APPROACHING_GRACE_TURNS`) once. The wall stand-down
//!    (`first_capital_walled_out`) is unchanged.
//! 5. Each decision is journalled under "First capital before the walls".
//! 6. **The declared strike keeps its city and its posture**
//!    ([`AdvancedAi::first_capital_strike_objective`],
//!    [`AdvancedAi::first_capital_strike_running`]): `assess` keeps the
//!    opening's city as the campaign's over a siege committed elsewhere (live
//!    063232Z: "Campaign remains aimed at Palenque"), and the power-gap
//!    Recovery posture stays off while the strike's war runs; a home
//!    emergency and a threatened city still take them.
//!
//! Threatened-city and Recovery holds on the march, the tactics, the wall
//! ceilings, the reach and the naming deadline are the shipped ones. Inert
//! with the flag off, without `first-capital-before-the-walls` (and so
//! without `early-conquest-opening`), and on any lane but Domination.

use super::city_campaign::CAMPAIGN_MIN_BODIES;
use super::early_conquest::{
    ConquestOpening, CONQUEST_APPROACHING_GRACE_TURNS, CONQUEST_ASSEMBLY_RADIUS,
};
use super::first_capital_before_the_walls::FIRST_CAPITAL_WALL_CEILING;
use super::AdvancedAi;
use crate::game::Game;
use crate::think;

/// The most city strength a named capital may read and still be soft. The
/// live Cree capital read 16-28 from turn 28 to 43 and 38-47 from turn 44
/// on, once Crossbowmen and Swordsmen stood in it; the census's soft cities
/// read 25 or less.
pub(crate) const FIRST_CAPITAL_SOFT_STRENGTH: f64 = 30.0;

/// The cities the empire holds before the reservation opens on a soft
/// capital: two, so the capital still trains the first Settler and the
/// force is trained while the window is open.
pub(crate) const FIRST_CAPITAL_SOFT_SETTLER_CITIES: usize = 2;

/// The most city strength a named capital may read and be bare: a Warrior's
/// own, so the bodies the capital can train today out-trade the centre. Live
/// Deity Gran Colombia `civvis-20261111T174731Z` named Pasargadae 13 tiles
/// away at turn 16 reading 13 with no walls and still 13 at turn 38; the
/// seat held one city until turn 30, so the reservation opened then, the
/// force stood at the rally from turn 49 against a centre of 44, and the
/// opening was released at 63.
pub(crate) const FIRST_CAPITAL_BARE_STRENGTH: f64 = 20.0;

/// How far from our capital a bare capital may stand: a five-turn walk for
/// the first Warriors, well inside the shipped reach of twenty.
pub(crate) const FIRST_CAPITAL_BARE_REACH: i32 = 14;

/// The cities the empire holds before the reservation opens on a bare
/// capital, and before its strike: one. The capital's production goes to the
/// force before the first Settler; a rival capital is worth more cities.
pub(crate) const FIRST_CAPITAL_BARE_CITIES: usize = 1;

/// The military power the reserved force adds to ours once it is trained:
/// four Warriors and two Slingers or Archers. A capital is bare only when
/// its owner's public power is within [`FIRST_CAPITAL_POWER_RATIO`] of ours
/// plus this, because no strike passes that ratio later. The four bare
/// capitals the live seat has named, at the turn of the naming:
///
/// | Owner | Its power | Ours | Bare by this rule | What followed |
/// |---|---|---|---|---|
/// | Persia | 170 | 67 | yes (230) | still 163 to 234 at turns 30 to 40 |
/// | Scotland | 159 | 62 | yes (224) | 191 to 219, below ours by turn 40 |
/// | Mali | 113 | 34 | yes (187) | 164 to 285 |
/// | Arabia | 293 | 53 | no (212) | 399, then 485 at turn 40 |
///
/// Against Arabia (`civvis-20261111T194319Z`) the reservation held the
/// capital from turn 20 for a strike its power guard never allowed.
pub(crate) const FIRST_CAPITAL_BARE_FORCE: f64 = 110.0;

/// How much stronger than our best melee body at the rally the city may read
/// and still be struck. Live civvis-20261111T063232Z declared at t80 on Wak
/// Kab'nal, defence 47-56, with Warriors, Heavy Chariots and one or two
/// Hypaspists (38): in six turns the city took no damage and the army fell
/// from 504 to 347 power.
pub(crate) const FIRST_CAPITAL_MELEE_MARGIN: f64 = 10.0;

/// How far from the city the owner's visible army counts as its defence when
/// the strike is weighed against the bodies at the rally.
pub(crate) const FIRST_CAPITAL_DEFENDER_REACH: i32 = 3;

/// The cities we hold before any strike: two. The shipped opening never
/// defers the first Settler, and a war before its city is never the plan.
/// Live `civvis-20261111T071743Z` declared on Ethiopia at t7 with one city.
pub(crate) const FIRST_CAPITAL_STRIKE_CITIES: usize = 2;

/// Melee soldiers (land, melee-capable, not recon) that must stand at the
/// rally before any strike. `CAMPAIGN_MIN_BODIES` (3) counted every body of
/// the force, Scouts included: at t7 in 071743Z its three were one Warrior
/// and two Scouts.
pub(crate) const FIRST_CAPITAL_STRIKE_MELEE: usize = 3;

/// The most the owner's public military power may be, as a multiple of ours,
/// for a strike. The mirror carries it for every met rival, unseen units
/// included. The Cree window that motivated the gene read 104 against our
/// 226; 071743Z struck Ethiopia at 113 against our 35.
pub(crate) const FIRST_CAPITAL_POWER_RATIO: f64 = 1.3;

/// The most the owner's public military power may be, as a multiple of ours,
/// for the whole reserved force at the rally to strike without the campaign
/// bill's margin: no more than ours. The bill asks half again the strength
/// of every defender within six tiles and the centre together; the guards
/// of [`AdvancedAi::first_capital_outclassed`] have by then weighed the
/// owner's power, the centre, the army beside it and the kill time. Live
/// Deity Gran Colombia `civvis-20261111T182847Z`: six Warriors, a Battering
/// Ram and a Comandante stood whole at the rally of Stirling (defence 26)
/// from turn 35 to 45 on "the city's bill is not covered yet", at 194-239
/// power against Scotland's 199-224; Stirling read 40 behind walls at 46
/// and the opening was released.
pub(crate) const FIRST_CAPITAL_COMMIT_RATIO: f64 = 1.0;

/// Turns of blows the melee bodies at the rally must need, at most, to take
/// the city's health, walls and healing (the kill-time check).
pub(crate) const FIRST_CAPITAL_STRIKE_TURNS: u32 = 3;

/// What a city heals each turn in the engine (`game/turn.rs`: `city.hp + 20`).
pub(crate) const FIRST_CAPITAL_CITY_HEAL: i32 = 20;

/// What the bodies at the rally make of the city's bill.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OpenWindowGo {
    /// Force bodies within the assembly radius of the rally.
    pub(crate) bodies: usize,
    /// Their strength, wounds priced.
    pub(crate) strength: f64,
    /// The city's bill against them.
    pub(crate) requirement: f64,
}

impl AdvancedAi {
    /// Whether this seat plays the gene now: the flag and the first-capital
    /// opening it rides on (which carries the Domination lane and
    /// `early-conquest-opening`).
    pub(super) fn first_capital_strikes_active(&self) -> bool {
        self.first_capital_strikes_the_open_window && self.first_capital_active()
    }

    /// The opening this gene keeps: a major's capital named, its war not yet
    /// open, inside the window, the city still theirs. `None` with the gene
    /// off.
    pub(super) fn first_capital_window(&self, g: &Game) -> Option<&ConquestOpening> {
        if !self.first_capital_strikes_active() || !self.conquest_reservation_open(g) {
            return None;
        }
        let opening = self.conquest_opening.as_ref()?;
        let rival = g.players.get(opening.target)?;
        if !rival.alive || rival.is_minor || rival.is_barbarian {
            return None;
        }
        g.cities
            .get(&opening.city)
            .is_some_and(|city| city.owner == opening.target)
            .then_some(opening)
    }

    /// The home faith wars running beside an opening on `target`: the war
    /// `faith-war-before-the-flip` or the heretic strike opened, and the
    /// religious interception's, while we are at war with them and they are
    /// not `target` itself.
    fn first_capital_faith_wars_beside(&self, g: &Game, pid: usize, target: usize) -> Vec<usize> {
        let mut wars: Vec<usize> = Vec::new();
        if let Some(war) = self.faith_war_before_the_flip_war.as_ref() {
            wars.push(war.rival);
        }
        if let Some((rival, _)) = self.religious_interception_war {
            wars.push(rival);
        }
        wars.retain(|rival| *rival != target && g.is_at_war(pid, *rival));
        wars.sort_unstable();
        wars.dedup();
        wars
    }

    /// ⭐ The faith wars that are no front while the window stands. Empty
    /// with the gene off and outside the window. Read by `no_front_rivals`,
    /// so `assess`, `one_war_holds_declaration` and the plan's declaration
    /// treat them the way `faith-war-is-no-front` treats its own war.
    pub(super) fn first_capital_faith_fronts(&self, g: &Game, pid: usize) -> Vec<usize> {
        match self.first_capital_window(g) {
            Some(opening) => self.first_capital_faith_wars_beside(g, pid, opening.target),
            None => Vec::new(),
        }
    }

    /// ⭐ Item 1: the campaign target and city `assess` aims at while the
    /// window stands, in place of `shipped`. `None` with the gene off,
    /// outside the window, on a home emergency, and while a front war is
    /// running (`wartime_rivals` already leaves the no-front wars out).
    pub(super) fn first_capital_keeps_the_campaign(
        &self,
        g: &Game,
        pid: usize,
        shipped: Option<usize>,
        wartime_rivals: &[usize],
        emergency: bool,
        countering: Option<usize>,
    ) -> Option<(usize, u32)> {
        if emergency || !wartime_rivals.is_empty() {
            return None;
        }
        let opening = self.first_capital_window(g)?;
        if !self.campaign_target_legal(g, pid, opening.target) {
            return None;
        }
        let city = &g.cities[&opening.city];
        if let Some(other) = shipped.filter(|other| *other != opening.target) {
            let civ = &g.players[other].civ;
            let why = if countering == Some(other) {
                format!("Countering {civ}")
            } else {
                format!("the campaign's pick of {civ}")
            };
            think!(self.journal(), Strategy, Strategy,
                   "First capital before the walls: the opening keeps the campaign";
                   "{why} does not move the opening's force; the campaign stays on {} until \
                    its window closes", city.name;
                   city.pos);
        }
        for rival in self.first_capital_faith_wars_beside(g, pid, opening.target) {
            think!(self.journal(), Strategy, Detail,
                   "First capital before the walls: the faith war is no front";
                   "the war on {} asks only soldiers at home; the opening on {} keeps the \
                    campaign and its declaration", g.players[rival].civ, city.name);
        }
        Some((opening.target, opening.city))
    }

    /// ⭐ Item 1: whether the window holds an elective declaration on
    /// `target` (the plan's war, a second front): the opening owns the
    /// seat's declaration until its war opens or its window closes. `false`
    /// with the gene off and outside the window.
    pub(super) fn first_capital_holds_the_war(
        &self,
        g: &Game,
        target: usize,
        journal: bool,
    ) -> bool {
        let Some(opening) = self.first_capital_window(g) else {
            return false;
        };
        if journal {
            think!(self.journal(), Military, Strategy,
                   "First capital before the walls: holding off war with {}",
                   g.players[target].civ;
                   "the opening on {} owns this seat's declaration until its war opens or its \
                    window closes", g.cities[&opening.city].name);
        }
        true
    }

    /// Item 1: whether the opportunistic raid is held: the window stands and
    /// no raid war is running (one already running is still closed by its
    /// own desk). `false` with the gene off.
    pub(super) fn first_capital_holds_the_raid(&self, g: &Game) -> bool {
        self.raid_war.is_none() && self.first_capital_window(g).is_some()
    }

    /// Item 1: whether a running war on `other` stops the opening's staging.
    /// The shipped rule stops it for any war; under the gene a home faith war
    /// does not.
    pub(super) fn first_capital_war_stops_staging(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
    ) -> bool {
        !self.first_capital_faith_fronts(g, pid).contains(&other)
    }

    /// Item 1: whether `uid`, a body of the opening's force or its breaker,
    /// walks on to the rally through the turn's wars: every major we are at
    /// war with (`enemies`) is a home faith war, and no raider is among them.
    /// `false` with the gene off and outside the window.
    pub(super) fn first_capital_marches_through(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        enemies: &[usize],
    ) -> bool {
        let Some(opening) = self.first_capital_window(g) else {
            return false;
        };
        if !opening.force.contains(&uid) && !self.first_capital_breaker_walks(g, pid, uid) {
            return false;
        }
        let fronts = self.first_capital_faith_wars_beside(g, pid, opening.target);
        !enemies.is_empty() && enemies.iter().all(|enemy| fronts.contains(enemy))
    }

    /// Whether the opening on `target` reserves and walks a breaker although
    /// no walls stand there yet: under the gene, on a major's city. Live
    /// Deity Gran Colombia `civvis-20261111T185132Z` declared on an unwalled
    /// Niani (defence 26) at turn 45 with six bodies and no Ram; Niani's
    /// walls stood at 47, the opening was released ("the target walled up
    /// before the strike landed and no breaker is held"), Maya and Mali
    /// declared on us at 54 and 69 and we held one city of three at 90.
    pub(super) fn first_capital_brings_a_breaker(
        &self,
        g: &Game,
        target: &crate::game::City,
    ) -> bool {
        self.first_capital_strikes_active()
            && g.players
                .get(target.owner)
                .is_some_and(|owner| !owner.is_minor)
    }

    /// The breaker the opening walks with its force: our Battering Ram or
    /// Siege Tower nearest the rally (only a Siege Tower past Ancient Walls),
    /// while the target has walls or is a major's city under the gene
    /// ([`Self::first_capital_brings_a_breaker`]). `None` otherwise or with
    /// no breaker.
    pub(super) fn first_capital_breaker(
        &self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
    ) -> Option<u32> {
        let city = g.cities.get(&opening.city)?;
        let walls = g.city_max_wall_hp(city);
        if walls <= 0 && !self.first_capital_brings_a_breaker(g, city) {
            return None;
        }
        let tower_only = walls > FIRST_CAPITAL_WALL_CEILING;
        g.units
            .values()
            .filter(|unit| unit.owner == pid)
            .filter(|unit| match unit.kind.as_str() {
                "siege_tower" => true,
                "battering_ram" => !tower_only,
                _ => false,
            })
            .min_by_key(|unit| (g.wdist(unit.pos, opening.rally), unit.id))
            .map(|unit| unit.id)
    }

    /// Item 3: whether `uid` is the breaker the window walks to the rally
    /// with the force. `false` with the gene off and outside the window.
    pub(super) fn first_capital_breaker_walks(&self, g: &Game, pid: usize, uid: u32) -> bool {
        self.first_capital_window(g)
            .and_then(|opening| self.first_capital_breaker(g, pid, opening))
            == Some(uid)
    }

    /// The city's bill against `bodies` alone, as
    /// `conquest_preview_takes_the_city` prices it for the whole force:
    /// `(strength, requirement)`, or `None` when the capture would not hold
    /// or no body can take the centre.
    fn first_capital_bill(
        &self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
        bodies: &[u32],
    ) -> Option<(f64, f64)> {
        if bodies.is_empty() {
            return None;
        }
        let appraisal = self.appraise_neighbour(g, pid, opening.target)?;
        let strength = Self::campaign_strength_of(g, bodies);
        let average_body = strength / bodies.len() as f64;
        let requirement =
            self.campaign_city_requirement(g, pid, opening.city, &appraisal, average_body);
        let capturer = bodies
            .iter()
            .filter_map(|uid| g.units.get(uid))
            .any(|unit| g.rules.units[unit.kind].is_melee_capable());
        (requirement.holdable && capturer).then_some((strength, requirement.strength))
    }

    /// ⭐ Item 2: the board says go. At least `CAMPAIGN_MIN_BODIES` force
    /// bodies stand within `CONQUEST_ASSEMBLY_RADIUS` of the rally, they
    /// alone cover the city's bill (or the whole reserved force stands there
    /// and the owner's public power is at most
    /// [`FIRST_CAPITAL_COMMIT_RATIO`] times ours), and a breaker stands at
    /// the rally unless no walls stand. `None` with the gene off, once
    /// declared, and whenever any of those fails.
    pub(crate) fn first_capital_open_window_go(
        &self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
    ) -> Option<OpenWindowGo> {
        if !self.first_capital_strikes_active() || opening.declared.is_some() {
            return None;
        }
        let city = g
            .cities
            .get(&opening.city)
            .filter(|city| city.owner == opening.target)?;
        if g.players
            .get(opening.target)
            .is_none_or(|rival| rival.is_minor)
        {
            return None;
        }
        let bodies: Vec<u32> = opening
            .force
            .iter()
            .copied()
            .filter(|uid| {
                g.units.get(uid).is_some_and(|unit| {
                    g.wdist(unit.pos, opening.rally) <= CONQUEST_ASSEMBLY_RADIUS
                })
            })
            .collect();
        if bodies.len() < CAMPAIGN_MIN_BODIES {
            return None;
        }
        if city.wall_hp > 0
            && !self
                .first_capital_breaker(g, pid, opening)
                .is_some_and(|uid| {
                    g.wdist(g.units[&uid].pos, opening.rally) <= CONQUEST_ASSEMBLY_RADIUS
                })
        {
            return None;
        }
        if self.first_capital_outclassed(g, pid, opening).is_some() {
            return None;
        }
        let (strength, requirement) = self.first_capital_bill(g, pid, opening, &bodies)?;
        // The whole reserved force is here and the owner is no stronger than
        // we are: see `FIRST_CAPITAL_COMMIT_RATIO`.
        let committed = bodies.len() >= self.conquest_force_size()
            && g.military_power(city.owner)
                <= FIRST_CAPITAL_COMMIT_RATIO * g.military_power(pid) + 1e-9;
        (strength + 1e-9 >= requirement || committed).then_some(OpenWindowGo {
            bodies: bodies.len(),
            strength,
            requirement,
        })
    }

    /// The force bodies within the assembly radius of the rally.
    fn first_capital_bodies_at_the_rally(g: &Game, opening: &ConquestOpening) -> Vec<u32> {
        opening
            .force
            .iter()
            .copied()
            .filter(|uid| {
                g.units.get(uid).is_some_and(|unit| {
                    g.wdist(unit.pos, opening.rally) <= CONQUEST_ASSEMBLY_RADIUS
                })
            })
            .collect()
    }

    /// ⭐ Why no strike may open on this city now, or `None`. Under the gene
    /// no declaration passes it, whichever rule assembled the force (the
    /// board's go or the shipped share rule). In order:
    ///
    /// - we hold fewer than [`FIRST_CAPITAL_STRIKE_CITIES`] cities;
    /// - fewer than [`FIRST_CAPITAL_STRIKE_MELEE`] melee soldiers (not
    ///   Scouts) stand within the assembly radius of the rally;
    /// - no walls stand, we hold or have queued a breaker, and it is not
    ///   within the assembly radius of the rally yet;
    /// - the owner's public military power is more than
    ///   [`FIRST_CAPITAL_POWER_RATIO`] times ours;
    /// - the city's strength (as last seen) is more than
    ///   [`FIRST_CAPITAL_MELEE_MARGIN`] above our best melee soldier's;
    /// - the owner's visible army within [`FIRST_CAPITAL_DEFENDER_REACH`] of
    ///   the city outweighs the bodies at the rally;
    /// - the melee soldiers' expected blows (`game::expected_damage`, the
    ///   engine's own mean) over [`FIRST_CAPITAL_STRIKE_TURNS`] turns do not
    ///   exceed the city's health, its walls and its healing over those
    ///   turns.
    ///
    /// `None` with the gene off and once declared.
    pub(super) fn first_capital_outclassed(
        &self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
    ) -> Option<String> {
        if !self.first_capital_strikes_active() || opening.declared.is_some() {
            return None;
        }
        let city = g.cities.get(&opening.city)?;
        let cities = g.player_city_ids(pid).len();
        let before = if self.first_capital_bare == Some(city.pos) {
            FIRST_CAPITAL_BARE_CITIES
        } else {
            FIRST_CAPITAL_STRIKE_CITIES
        };
        if cities < before {
            return Some(format!(
                "we hold {cities} cit{} and strike no capital before {before}",
                if cities == 1 { "y" } else { "ies" }
            ));
        }
        let bodies = Self::first_capital_bodies_at_the_rally(g, opening);
        let melee: Vec<&crate::game::Unit> = bodies
            .iter()
            .filter_map(|uid| g.units.get(uid))
            .filter(|unit| Self::conquest_melee_body(&g.rules.units[unit.kind]))
            .collect();
        if melee.len() < FIRST_CAPITAL_STRIKE_MELEE {
            return Some(format!(
                "{} melee soldier{} at the rally for {} (Scouts do not count), fewer than {}",
                melee.len(),
                if melee.len() == 1 { "" } else { "s" },
                city.name,
                FIRST_CAPITAL_STRIKE_MELEE
            ));
        }
        // No walls stand, but they rise behind a declaration: a breaker we
        // hold or have queued stands at the rally first. With none held or
        // queued (no Masonry yet) the strike goes as before.
        if city.wall_hp <= 0 && Self::conquest_breakers_held(g, pid) > 0 {
            let at_the_rally = self
                .first_capital_breaker(g, pid, opening)
                .and_then(|uid| g.units.get(&uid))
                .is_some_and(|breaker| {
                    g.wdist(breaker.pos, opening.rally) <= CONQUEST_ASSEMBLY_RADIUS
                });
            if !at_the_rally {
                return Some(format!(
                    "our breaker is not at the rally yet, and {} would wall up behind the \
                     declaration",
                    city.name
                ));
            }
        }
        let (ours, theirs_public) = (g.military_power(pid), g.military_power(city.owner));
        if theirs_public > FIRST_CAPITAL_POWER_RATIO * ours {
            return Some(format!(
                "{}'s military {theirs_public:.0} is more than {FIRST_CAPITAL_POWER_RATIO:.1} times \
                 our {ours:.0}",
                g.players[city.owner].civ
            ));
        }
        let best_melee = melee
            .iter()
            .map(|unit| g.unit_strength(unit, false))
            .fold(f64::MIN, f64::max);
        let defence = g.city_strength(city.id);
        if defence > best_melee + FIRST_CAPITAL_MELEE_MARGIN {
            return Some(format!(
                "{} reads strength {defence:.0}, more than {FIRST_CAPITAL_MELEE_MARGIN:.0} above our \
                 best melee body at the rally ({best_melee:.0})",
                city.name
            ));
        }
        let visible = self.battlefront_visibility(g, pid);
        let theirs: f64 = g
            .units
            .values()
            .filter(|unit| unit.owner == city.owner)
            .filter(|unit| g.rules.units[unit.kind].class == "military")
            .filter(|unit| g.wdist(unit.pos, city.pos) <= FIRST_CAPITAL_DEFENDER_REACH)
            .filter(|unit| g.sees(&visible, unit.pos) && g.unit_visible_to(unit.id, pid))
            .map(|unit| crate::game::effective_strength(g.unit_strength(unit, true), unit.hp))
            .sum();
        let at_rally = Self::campaign_strength_of(g, &bodies);
        if theirs > at_rally {
            return Some(format!(
                "{}'s visible army within {FIRST_CAPITAL_DEFENDER_REACH} tiles ({theirs:.0}) \
                 outweighs the {} bodies at the rally ({at_rally:.0})",
                city.name,
                bodies.len()
            ));
        }
        let turns = f64::from(FIRST_CAPITAL_STRIKE_TURNS);
        let blows: f64 = melee
            .iter()
            .map(|unit| {
                let attack = crate::game::effective_strength(g.unit_strength(unit, false), unit.hp);
                crate::game::expected_damage(attack, defence)
            })
            .sum::<f64>()
            * turns;
        let needed = f64::from(city.hp.max(0))
            + f64::from(city.wall_hp.max(0))
            + f64::from(FIRST_CAPITAL_CITY_HEAL) * turns;
        (blows <= needed).then(|| {
            format!(
                "the {} melee soldiers at the rally would deal about {blows:.0} in \
                 {FIRST_CAPITAL_STRIKE_TURNS} turns against {}'s {} health, {} walls and {:.0} of \
                 healing",
                melee.len(),
                city.name,
                city.hp,
                city.wall_hp.max(0),
                f64::from(FIRST_CAPITAL_CITY_HEAL) * turns
            )
        })
    }

    /// ⭐ Once the opening has declared: its rival and city, which `assess`
    /// keeps as the campaign's target over a siege committed elsewhere (live
    /// 063232Z: "Campaign remains aimed at Palenque" while the strike stood
    /// on Wak Kab'nal). `None` with the gene off, before the declaration,
    /// after a capture or the city's loss, and on a home emergency.
    pub(super) fn first_capital_strike_objective(
        &self,
        g: &Game,
        pid: usize,
        shipped_city: Option<u32>,
        emergency: bool,
    ) -> Option<(usize, u32)> {
        if emergency || !self.first_capital_strike_running(g, pid) {
            return None;
        }
        let opening = self.conquest_opening.as_ref()?;
        if let Some(other) = shipped_city
            .filter(|other| *other != opening.city)
            .and_then(|other| g.cities.get(&other))
        {
            think!(self.journal(), Strategy, Strategy,
                   "First capital before the walls: the strike keeps the campaign";
                   "{} does not take the force while the strike on {} runs", other.name,
                   g.cities[&opening.city].name;
                   g.cities[&opening.city].pos);
        }
        Some((opening.target, opening.city))
    }

    /// Whether the opening's own war is running: declared, nothing taken yet,
    /// the city still the rival's, the war open. Holds the power-gap Recovery
    /// posture off (a threatened home city still takes it). `false` with the
    /// gene off.
    pub(super) fn first_capital_strike_running(&self, g: &Game, pid: usize) -> bool {
        self.first_capital_strikes_active()
            && self.conquest_opening.as_ref().is_some_and(|opening| {
                opening.declared.is_some()
                    && opening.taken == 0
                    && !opening.force.is_empty()
                    && g.players
                        .get(opening.target)
                        .is_some_and(|rival| rival.alive && !rival.is_minor)
                    && g.is_at_war(pid, opening.target)
                    && g.cities
                        .get(&opening.city)
                        .is_some_and(|city| city.owner == opening.target)
            })
    }

    /// Item 5: the journal line of a declaration the board's go opened.
    pub(super) fn first_capital_journal_the_go(
        &self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
        go: &OpenWindowGo,
    ) {
        let Some(city) = g.cities.get(&opening.city) else {
            return;
        };
        think!(self.journal(), Military, Decision,
               "First capital before the walls: the open window";
               "{} defence {:.0}, walls {}, their power {:.0} against our {:.0}; {} bodies at \
                the rally ({:.0} strength against a bill of {:.0}) cover the bill — declaring now",
               city.name, g.city_strength(city.id), city.wall_hp,
               g.military_power(opening.target), g.military_power(pid),
               go.bodies, go.strength, go.requirement;
               city.pos);
    }

    /// Whether `city` is soft right now: walled to Ancient or less and its
    /// strength, as last seen, at most [`FIRST_CAPITAL_SOFT_STRENGTH`].
    pub(crate) fn first_capital_is_soft(g: &Game, city: u32) -> bool {
        g.cities.get(&city).is_some_and(|target| {
            g.city_max_wall_hp(target) <= FIRST_CAPITAL_WALL_CEILING
                && g.city_strength(city) <= FIRST_CAPITAL_SOFT_STRENGTH
        })
    }

    /// Whether `city` is bare right now: no walls built, a strength (as last
    /// seen) of at most [`FIRST_CAPITAL_BARE_STRENGTH`], within
    /// [`FIRST_CAPITAL_BARE_REACH`] of our capital, and an owner whose public
    /// military power the reserved force would bring within the strike's
    /// ratio: at most [`FIRST_CAPITAL_POWER_RATIO`] times ours plus
    /// [`FIRST_CAPITAL_BARE_FORCE`].
    pub(crate) fn first_capital_is_bare(g: &Game, pid: usize, city: u32) -> bool {
        let Some(home) = Self::first_capital_home(g, pid) else {
            return false;
        };
        g.cities.get(&city).is_some_and(|target| {
            g.city_max_wall_hp(target) <= 0
                && g.city_strength(city) <= FIRST_CAPITAL_BARE_STRENGTH
                && g.wdist(home, target.pos) <= FIRST_CAPITAL_BARE_REACH
                && g.military_power(target.owner)
                    <= FIRST_CAPITAL_POWER_RATIO
                        * (g.military_power(pid) + FIRST_CAPITAL_BARE_FORCE)
        })
    }

    /// The bare capital's latch, start of turn: set the first turn the named
    /// capital reads bare and kept while that opening stands undeclared and
    /// the capital stays soft, so a garrison that walks in does not hand the
    /// half-trained force's queue back to the Settler.
    fn first_capital_read_the_bareness(&mut self, g: &Game, pid: usize, soft: bool) {
        // The latch is the city's tile: a live rebuild renumbers cities, and
        // an id latch dropped on most turns (live 185132Z journalled "is
        // bare" 14 times from turn 9 to 27 and the capital took up its
        // Settler again at 29, once Niani read 21).
        let named = self
            .conquest_opening
            .as_ref()
            .filter(|opening| soft && opening.declared.is_none())
            .and_then(|opening| g.cities.get(&opening.city));
        let tile = named.map(|city| city.pos);
        let kept = self.first_capital_bare.filter(|pos| tile == Some(*pos));
        let bare = kept.or_else(|| {
            named
                .filter(|city| Self::first_capital_is_bare(g, pid, city.id))
                .map(|city| city.pos)
        });
        if bare.is_some() && self.first_capital_bare != bare {
            if let Some(city) = named {
                think!(self.journal(), Military, Strategy,
                       "First capital before the walls: {} is bare", city.name;
                       "defence {:.0}, no walls, {} tiles from our capital: the reservation opens \
                        at {} city and the strike does not wait for a second",
                       g.city_strength(city.id),
                       Self::first_capital_home(g, pid).map_or(0, |home| g.wdist(home, city.pos)),
                       FIRST_CAPITAL_BARE_CITIES;
                       city.pos);
            }
        }
        self.first_capital_bare = bare;
    }

    /// ⭐ Item 3, start of turn: whether the named capital is soft, which
    /// `conquest_first_settler_cities` reads, journalled when it changes.
    /// Always `false` with the gene off.
    pub(super) fn first_capital_read_the_softness(&mut self, g: &Game, pid: usize) {
        let soft = self.first_capital_strikes_active()
            && self.conquest_opening.as_ref().is_some_and(|opening| {
                opening.declared.is_none()
                    && g.players
                        .get(opening.target)
                        .is_some_and(|rival| !rival.is_minor)
                    && Self::first_capital_is_soft(g, opening.city)
            });
        if soft != self.first_capital_soft_now {
            // The line for a capital that hardened, not for an opening that
            // declared or left.
            if let Some(city) = self
                .conquest_opening
                .as_ref()
                .filter(|opening| opening.declared.is_none())
                .and_then(|opening| g.cities.get(&opening.city))
            {
                if soft {
                    think!(self.journal(), Military, Strategy,
                           "First capital before the walls: {} is soft", city.name;
                           "defence {:.0}, walls {}: the reservation opens at {} cities",
                           g.city_strength(city.id), g.city_max_wall_hp(city),
                           FIRST_CAPITAL_SOFT_SETTLER_CITIES;
                           city.pos);
                } else {
                    think!(self.journal(), Military, Strategy,
                           "First capital before the walls: {} is no longer soft", city.name;
                           "defence {:.0}, walls {}: the reservation waits for the shipped city \
                            count again", g.city_strength(city.id), g.city_max_wall_hp(city);
                           city.pos);
                }
            }
        }
        self.first_capital_soft_now = soft;
        self.first_capital_read_the_bareness(g, pid, soft);
    }

    /// The cities a soft capital's reservation waits for (one for a bare
    /// one), or `None` while the named capital is not soft (or the gene is
    /// off).
    pub(super) fn first_capital_soft_settler_cities(&self) -> Option<usize> {
        (self.first_capital_soft_now && self.first_capital_strikes_active()).then_some(
            if self.first_capital_bare.is_some() {
                FIRST_CAPITAL_BARE_CITIES
            } else {
                FIRST_CAPITAL_SOFT_SETTLER_CITIES
            },
        )
    }

    /// ⭐ Item 4: at the commit deadline, an unassembled opening whose force
    /// was held away by a home faith war, or whose whole force already
    /// covers the bill, gets the shipped grace once instead of the release.
    /// No-op with the gene off, before the deadline, once assembled or
    /// declared, and once a grace was granted.
    pub(super) fn first_capital_window_grace(&mut self, g: &Game, pid: usize) {
        if !self.first_capital_strikes_active() {
            return;
        }
        let preparation = self.conquest_preparation(g);
        let Some(opening) = self.conquest_opening.as_ref() else {
            return;
        };
        if opening.declared.is_some()
            || opening.assembled.is_some()
            || opening.grace_until.is_some()
            || g.players
                .get(opening.target)
                .is_none_or(|rival| rival.is_minor || !rival.alive)
            || g.cities
                .get(&opening.city)
                .is_none_or(|city| city.owner != opening.target)
        {
            return;
        }
        let base = Self::conquest_commit_base(g, opening, preparation);
        if g.turn < base {
            return;
        }
        let held = self.first_capital_faith_wars_beside(g, pid, opening.target);
        let reason = if let Some(rival) = held.first() {
            format!(
                "the force was held away by the faith war on {}",
                g.players[*rival].civ
            )
        } else if self.conquest_preview_takes_the_city(g, pid, opening) {
            format!(
                "the bill is covered by the force of {} but too few stand at the rally",
                opening.force.len()
            )
        } else {
            return;
        };
        let grace = g.standard_duration(CONQUEST_APPROACHING_GRACE_TURNS);
        let city = g.cities[&opening.city].name.clone();
        if let Some(opening) = self.conquest_opening.as_mut() {
            opening.grace_until = Some(base.saturating_add(grace));
        }
        think!(self.journal(), Military, Strategy,
               "First capital before the walls: the window holds";
               "{city}: the commit deadline came and {reason}; the opening waits {grace} more \
                turns instead of releasing");
    }
}

#[cfg(test)]
mod tests;
