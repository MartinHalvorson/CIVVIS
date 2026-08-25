//! Defence against a rival religion, to the extent we care: the opt-in gene
//! `religious-veto-defence`.
//!
//! ## Why "to the extent we care" has an exact meaning here
//!
//! `Game::check_religious_victory` crowns a founder only when **every** living
//! major has more than half its cities in that faith. Each civilization is
//! therefore a veto: however far a rival religion has spread elsewhere, it
//! cannot win while we keep half our cities out of it. So the value of
//! defending is not "how many of our cities are slipping" — the question the
//! shipped defence asks through `city_needs_religious_support` — but how much
//! of the rival's victory is already done, and how close it is to taking
//! our half. A founder losing a city to a faith that holds nobody else loses
//! some belief yield; a founder or a non-founder losing its half to a faith
//! that already holds every other civilization loses the game.
//!
//! ## The stake
//!
//! For each rival founder, [`AdvancedAi::religious_veto_stakes`] reads the
//! civilizations its victory needs: the *others* (every living major but the
//! founder and us) and how many of them it already dominates, plus how far
//! it is toward half of our own cities. The stake is the share of that
//! requirement already met, `(dominated + our_progress) / (others + 1)`,
//! in `[0, 1]`; the rival with the highest stake is the threat. The shipped
//! defence is never withheld: the 60k founder study priced defence at about
//! a point, but early defence is the cheap kind — a city held costs a
//! Missionary, a city flipped back later costs the pressure that has piled
//! up since — so the gene only ever adds. Below
//! [`RELIGIOUS_VETO_STAKE_FLOOR`] it adds nothing. From the floor the free
//! levers engage:
//!
//! - a non-founder's threat is the stakes faith when the shipped warning
//!   (the first rival faith at 60% of a city's pressure) is silent;
//! - a spreader's target list scores our cities by the veto arithmetic: a
//!   city already in the threat faith is worth `stake × 160` (cheapest flip
//!   first), one the threat faith is closing on `stake × 100`.
//!
//! From [`RELIGIOUS_VETO_SPEND_STAKE`] — match point, the rival needs only
//! us or has most of our half — the levers that spend Faith scale in: the
//! defensive spreader cap grows by `ceil((stake − 0.5) × 4)` (one at 0.75,
//! two at 1.0), a non-founder's two-Missionary cap the same way, the
//! Inquisitor cap by one, and the purchase reserve is zero. The extra
//! spreaders wait while a founder's Apostle slot is still open
//! (`inquisition_on_threat`): every extra Missionary bought first is Faith
//! the 400-Faith Apostle waits for. The first probe of this gene spent the
//! extras from the floor, read fewer Inquisitions per founder and −9 pp on
//! 144 seats, and is the reason the spending levers start at match point.
//!
//! ## The Inquisitor repair, on with the gene at any stake
//!
//! `Action::RemoveHeresy` is legal in any own city centre with a charge left,
//! and `advanced_religious_step` took it wherever the unit stood — which is
//! the Holy City it was bought in, where there is no heresy. Three charges
//! went into the one city that never needed them. With the gene the
//! Inquisitor removes heresy only where a rival faith holds at least
//! [`HERESY_WORTH_A_CHARGE`] of the city's strongest pressure, and otherwise
//! walks (out of any raider's reach, see `missionary_field.rs`) to the own
//! city where the heresy is worst. That costs no Faith, so it is not scaled.
//!
//! ## What the record says, so nobody reads a fires probe as an effect
//!
//! The 60k founder study (`docs/eval/2026-08-21-the-founder-that-never-
//! launched-its-inquisition.md`) put defence at about a point, not the
//! twenty-five between keeping and losing the cities, because the founders
//! who lose them are the weaker empires — and its first cut lost 8 pp by
//! starving the steady Missionary corps for a late Apostle. This gene never
//! changes the purchase order and adds spreaders only in proportion to the
//! stake; the deployment genome pins it on after its +0.93 pp displayed pooled
//! Diff, while the standard screen can still price it like every other opt-in.

use super::AdvancedAi;
use crate::game::{Action, City, Game};
use crate::Pos;

/// Below this share of a rival's religious victory the gene stands aside.
/// In a six-player game with one holdout beside us this is "the others have
/// fallen and the rival has not yet reached our cities".
pub(super) const RELIGIOUS_VETO_STAKE_FLOOR: f64 = 0.5;

/// The stake from which the levers that spend Faith engage: match point.
pub(super) const RELIGIOUS_VETO_SPEND_STAKE: f64 = 0.75;

/// Extra defensive spreaders at a stake of one; the extras grow linearly
/// from the floor and round up, so match point buys one and a whole
/// victory two.
pub(super) const RELIGIOUS_VETO_EXTRA_SPREADERS: f64 = 2.0;

/// A rival faith worth a `RemoveHeresy` charge holds at least this share of
/// the city's strongest pressure. `RemoveHeresy` quarters every foreign
/// faith, so below this a charge buys a rounding error.
pub(super) const HERESY_WORTH_A_CHARGE: f64 = 0.3;

/// A city the threat faith holds already: the veto count moves when it flips
/// back, so it outranks everything but a slipping own-faith city.
const VETO_HELD_BONUS: f64 = 160.0;

/// A city the threat faith is closing on (at least 60% of the top pressure,
/// the shipped early-warning bar).
const VETO_THREATENED_BONUS: f64 = 100.0;

/// The rival founder whose religious victory we are a veto on.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ReligiousStakes {
    /// The threat faith.
    pub(super) religion: String,
    /// Its founder.
    pub(super) founder: usize,
    /// Living majors other than the founder and us that it already holds.
    pub(super) dominated: usize,
    /// Living majors other than the founder and us.
    pub(super) others: usize,
    /// Our cities, and how many of them follow the threat faith.
    pub(super) our_cities: usize,
    pub(super) our_converted: usize,
    /// The share of the rival's victory already done, in `[0, 1]`.
    pub(super) stake: f64,
}

impl AdvancedAi {
    /// Whether `religion` holds the majority of `owner`'s cities — the
    /// victory rule's own test, strictly more than half.
    fn converted_majority(g: &Game, owner: usize, religion: &str) -> bool {
        let cities = g.player_city_ids(owner);
        let following = cities
            .iter()
            .filter(|city| g.city_religion(&g.cities[city]) == Some(religion))
            .count();
        !cities.is_empty() && following * 2 > cities.len()
    }

    /// The rival religion nearest a victory we are a veto on, and how much of
    /// that victory is done. `None` when the gene is off, the lobby has no
    /// religious victory, we hold no cities, or no rival faith has any of
    /// the ground it needs.
    pub(super) fn religious_veto_stakes(&self, g: &Game, pid: usize) -> Option<ReligiousStakes> {
        if !self.religious_veto_defence || !g.victory_conditions.religious {
            return None;
        }
        let our_cities = g.player_city_ids(pid);
        if our_cities.is_empty() {
            return None;
        }
        let own = g.players[pid].religion.as_deref();
        let majors: Vec<usize> = g
            .players
            .iter()
            .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
            .map(|player| player.id)
            .collect();
        let mut best: Option<ReligiousStakes> = None;
        for &founder in &majors {
            if founder == pid {
                continue;
            }
            let Some(religion) = g.players[founder].religion.as_deref() else {
                continue;
            };
            if Some(religion) == own {
                continue;
            }
            let others: Vec<usize> = majors
                .iter()
                .copied()
                .filter(|major| *major != founder && *major != pid)
                .collect();
            let dominated = others
                .iter()
                .filter(|other| Self::converted_majority(g, **other, religion))
                .count();
            let our_converted = our_cities
                .iter()
                .filter(|city| g.city_religion(&g.cities[city]) == Some(religion))
                .count();
            let our_progress = (our_converted as f64 / our_cities.len() as f64 / 0.5).min(1.0);
            let stake = (dominated as f64 + our_progress) / (others.len() as f64 + 1.0);
            if stake <= 0.0 {
                continue;
            }
            let better = best.as_ref().is_none_or(|current| {
                stake > current.stake
                    || (stake == current.stake && religion < current.religion.as_str())
            });
            if better {
                best = Some(ReligiousStakes {
                    religion: religion.to_string(),
                    founder,
                    dominated,
                    others: others.len(),
                    our_cities: our_cities.len(),
                    our_converted,
                    stake,
                });
            }
        }
        best
    }

    /// The stakes at or above [`RELIGIOUS_VETO_STAKE_FLOOR`]: the gene's
    /// levers read this and nothing below it.
    pub(super) fn religious_veto_engaged(&self, g: &Game, pid: usize) -> Option<ReligiousStakes> {
        self.religious_veto_stakes(g, pid)
            .filter(|stakes| stakes.stake >= RELIGIOUS_VETO_STAKE_FLOOR)
    }

    /// Whether the stake is high enough to spend Faith on.
    pub(super) fn religious_veto_spends(stakes: Option<&ReligiousStakes>) -> bool {
        stakes.is_some_and(|stakes| stakes.stake >= RELIGIOUS_VETO_SPEND_STAKE)
    }

    /// The defensive spreaders the stake buys on top of the shipped cap:
    /// none below match point, one there, two for a whole victory.
    pub(super) fn religious_veto_extra_spreaders(stakes: Option<&ReligiousStakes>) -> usize {
        if !Self::religious_veto_spends(stakes) {
            return 0;
        }
        let stake = stakes.map_or(0.0, |stakes| stakes.stake);
        ((stake - RELIGIOUS_VETO_STAKE_FLOOR) / (1.0 - RELIGIOUS_VETO_STAKE_FLOOR)
            * RELIGIOUS_VETO_EXTRA_SPREADERS)
            .ceil()
            .max(0.0) as usize
    }

    /// The extra Inquisitor match point buys once the Inquisition is launched.
    pub(super) fn religious_veto_extra_inquisitors(stakes: Option<&ReligiousStakes>) -> usize {
        usize::from(Self::religious_veto_spends(stakes))
    }

    /// A non-founder's threat, with the gene: the shipped warning when it
    /// fires, else the stakes faith when the veto is engaged. Never less
    /// than the shipped answer.
    pub(super) fn religious_veto_threat(
        &self,
        g: &Game,
        pid: usize,
        shipped: Option<String>,
    ) -> Option<String> {
        if !self.religious_veto_defence || shipped.is_some() {
            return shipped;
        }
        self.religious_veto_engaged(g, pid)
            .map(|stakes| stakes.religion)
    }

    /// How much a spreader's target list adds for an own city under the
    /// veto arithmetic. Zero with the gene off, below the floor, or for a
    /// city the threat faith neither holds nor closes on.
    pub(super) fn religious_veto_target_bonus(
        pid: usize,
        city: &City,
        stakes: Option<&ReligiousStakes>,
        held: bool,
    ) -> i32 {
        let Some(stakes) = stakes else {
            return 0;
        };
        if city.owner != pid {
            return 0;
        }
        let threat = city.pressure.get(&stakes.religion).copied().unwrap_or(0.0);
        if threat <= 0.0 {
            return 0;
        }
        let top = city
            .pressure
            .values()
            .fold(0.0_f64, |best, pressure| best.max(*pressure));
        if held {
            // The cheapest flip first: the pressure the threat faith holds
            // over the strongest other faith is what a spreader must undo.
            let runner_up = city
                .pressure
                .iter()
                .filter(|(faith, _)| faith.as_str() != stakes.religion)
                .map(|(_, pressure)| *pressure)
                .fold(0.0_f64, f64::max);
            let flip_cost = ((threat - runner_up) / 40.0).clamp(0.0, 60.0);
            (stakes.stake * VETO_HELD_BONUS - flip_cost).round() as i32
        } else if threat >= top * 0.6 {
            (stakes.stake * VETO_THREATENED_BONUS).round() as i32
        } else {
            0
        }
    }

    /// The share of a city's strongest pressure held by faiths other than
    /// `religion`: what a `RemoveHeresy` charge would quarter.
    fn heresy_share(city: &City, religion: &str) -> f64 {
        let top = city
            .pressure
            .values()
            .fold(0.0_f64, |best, pressure| best.max(*pressure));
        if top <= 0.0 {
            return 0.0;
        }
        let heresy = city
            .pressure
            .iter()
            .filter(|(faith, _)| faith.as_str() != religion)
            .map(|(_, pressure)| *pressure)
            .fold(0.0_f64, f64::max);
        heresy / top
    }

    /// The Inquisitor's turn with the gene on: remove heresy where there is
    /// heresy, otherwise walk to the own city where it is worst. `None`
    /// when the gene is off or the unit is not an Inquisitor of a faith;
    /// `Some(false)` when nothing anywhere is worth a charge, which hands
    /// the unit on to the theological exchange and the last leg — never to
    /// the shipped charge-in-place.
    pub(super) fn inquisitor_veto_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        legal: &[Action],
    ) -> Option<bool> {
        if !self.religious_veto_defence {
            return None;
        }
        let unit = g.units.get(&uid)?;
        if unit.kind != "inquisitor" {
            return None;
        }
        let religion = unit
            .religion
            .clone()
            .or_else(|| g.players[pid].religion.clone())?;
        let here = unit.pos;
        let standing_in = g
            .city_at(here)
            .map(|city| &g.cities[&city])
            .filter(|city| city.owner == pid);
        if standing_in
            .is_some_and(|city| Self::heresy_share(city, &religion) >= HERESY_WORTH_A_CHARGE)
        {
            if let Some(action) = legal
                .iter()
                .find(|action| matches!(action, Action::RemoveHeresy { unit } if *unit == uid))
            {
                return Some(g.apply(pid, action).is_ok());
            }
        }
        let target: Option<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|city| &g.cities[&city])
            .filter(|city| city.pos != here)
            .map(|city| (Self::heresy_share(city, &religion), city.pos))
            .filter(|(share, _)| *share >= HERESY_WORTH_A_CHARGE)
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| g.wdist(here, right.1).cmp(&g.wdist(here, left.1)))
                    .then_with(|| right.1.cmp(&left.1))
            })
            .map(|(_, pos)| pos);
        let Some(target) = target else {
            return Some(false);
        };
        Some(self.religious_step_toward_range(g, pid, uid, target, 0))
    }
}
