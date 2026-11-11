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
//!    walks on to the rally through it.
//! 2. **Go when the board says go**
//!    ([`AdvancedAi::first_capital_open_window_go`]). Once at least
//!    `CAMPAIGN_MIN_BODIES` force bodies stand within the assembly radius of
//!    the rally and they alone cover the city's bill
//!    (`campaign_city_requirement`, as `conquest_preview_takes_the_city`
//!    prices it), with a breaker at the rally or no walls standing, the
//!    opening counts as assembled and declares that turn. The shipped 80%
//!    share rule stays the fallback.
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

    /// The breaker the opening walks with its force: our Battering Ram or
    /// Siege Tower nearest the rally (only a Siege Tower past Ancient Walls),
    /// while the target has walls. `None` with no walls or no breaker.
    pub(super) fn first_capital_breaker(
        &self,
        g: &Game,
        pid: usize,
        opening: &ConquestOpening,
    ) -> Option<u32> {
        let city = g.cities.get(&opening.city)?;
        let walls = g.city_max_wall_hp(city);
        if walls <= 0 {
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
    /// alone cover the city's bill, and a breaker stands at the rally unless
    /// no walls stand. `None` with the gene off, once declared, and whenever
    /// any of those fails.
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
        let (strength, requirement) = self.first_capital_bill(g, pid, opening, &bodies)?;
        (strength + 1e-9 >= requirement).then_some(OpenWindowGo {
            bodies: bodies.len(),
            strength,
            requirement,
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

    /// ⭐ Item 3, start of turn: whether the named capital is soft, which
    /// `conquest_first_settler_cities` reads, journalled when it changes.
    /// Always `false` with the gene off.
    pub(super) fn first_capital_read_the_softness(&mut self, g: &Game) {
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
    }

    /// The cities a soft capital's reservation waits for, or `None` while
    /// the named capital is not soft (or the gene is off).
    pub(super) fn first_capital_soft_settler_cities(&self) -> Option<usize> {
        (self.first_capital_soft_now && self.first_capital_strikes_active())
            .then_some(FIRST_CAPITAL_SOFT_SETTLER_CITIES)
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

// TEMPORARY CI scaffold; removed before ship.
#[cfg(test)]
mod replay_scaffold;
