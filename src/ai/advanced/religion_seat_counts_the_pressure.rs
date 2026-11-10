//! `religion-seat-counts-the-pressure`: a Religion-target founder's spreaders
//! go where the host's own pressure rows say a rival's majority is cheapest.
//!
//! **Why (live Deity Russia, 2026-11-10).** A Religious victory counts cities:
//! a strict majority of them in every living major. Until #4921 the live board
//! held only each city's majority religion, so every rival city read alike
//! and the shipped target score (`advanced_missionary_step`) paid `pop x 12`
//! and `capital x 18`, the largest and oldest cities. Those are the dearest.
//! civvis-20261110T005727Z spent 93 verified charges on Brazil's nine cities
//! and held two; civvis-20261110T012017Z spent about 250 charges for four of
//! five majors.
//!
//! The banner rows the host now exports (`Game::observed_city_faiths`) say
//! what a city costs. In civvis-20261110T031523Z, Nubia's cities needed 2, 3,
//! 3, 4 and 5 Missionary charges at t60, so its majority of three cost 8; at
//! t110 its nine cities needed 2, 5, 6, 7, 7, 8, 9, 9 and 10, so a majority of
//! five cost 27 taking the cheapest and 43 taking the dearest. Rival lifetime
//! pressure in a rival city ran at a median 882 at t50, 1,944 at t75 and 3,527
//! at t116.
//!
//! **The arithmetic is the host's.** A spread adds the unit's pressure to its
//! faith and removes a share of every other faith's (`Game::do_spread`:
//! Missionary 200 and 10%, Apostle 220 and 25%, read back from the live rows
//! as +200 with the other faiths at x0.90 on 141 spreads and +220 at x0.75 on
//! 12; the religionless row does not move). Citizens are then seated by the
//! largest-remainder method and the faith with the most is the majority from
//! half of them up ([`FlipRows::holds`], 1,664 of 1,664 exported city-turns).
//!
//! **What the gene does**, for a seat aiming at Religion spreading its own
//! faith ([`AdvancedAi::religion_seat_pressure_plan`]). For every major whose
//! cities carry rows:
//! - its cities not yet ours are ranked by the charges an unpromoted Apostle
//!   would need, and the cheapest it still owes our majority, plus
//!   [`CAMPAIGN_SPARE`], are its *campaign*;
//! - a campaign city is paid [`CAMPAIGN_BONUS`], less [`CHARGE_COST`] for
//!   each charge this spreader would need there at its own strength, health
//!   and promotions (counted up to [`CHARGE_CAP`], so a campaign city never
//!   reads below one outside it), plus [`SOLO_FLIP_BONUS`] when its
//!   remaining charges finish the flip, plus [`ADVANTAGE_BONUS`] for each
//!   charge it saves there against an unpromoted Apostle. The host deals an
//!   Apostle one useful promotion of seven in turn (41 Apostles of
//!   civvis-20261110T075110Z: six each of Translator, Proselytizer and the
//!   rest), so a Translator, whose charge is worth three abroad, is one unit
//!   in seven and belongs on the entrenched cities;
//! - a city outside the campaign is charged [`OUTSIDE_CAMPAIGN`];
//! - the shipped population and capital terms are taken back for both.
//!
//! A city with no rows, a city-state, our own cities, another faith's spreader
//! and a seat aiming elsewhere keep the shipped score. Off, nothing changes.

use super::*;

/// Charges past which a city reads as out of one corps' reach.
pub(super) const CHARGE_CAP: u32 = 12;

/// The spreader a rival's cities are ranked for: an unpromoted Apostle at full
/// health (`data/units.json` `religious_spread` 220, eviction 25%).
const REFERENCE_SPREAD: f64 = 220.0;
const REFERENCE_EVICTION: f64 = 0.25;

/// Cities beyond the majority still owed that stay in a rival's campaign, so
/// one that slips back does not leave the corps without a target there.
pub(super) const CAMPAIGN_SPARE: usize = 1;

/// Paid to a campaign city.
pub(super) const CAMPAIGN_BONUS: i32 = 200;

/// Paid against each charge this spreader still needs in a campaign city.
pub(super) const CHARGE_COST: i32 = 30;

/// Paid when this spreader's own remaining charges finish the flip.
pub(super) const SOLO_FLIP_BONUS: i32 = 45;

/// Paid for each charge this spreader saves in a campaign city against an
/// unpromoted Apostle: a Translator or Proselytizer goes where its promotion
/// counts, and leaves the cheap cities to the rest.
pub(super) const ADVANTAGE_BONUS: i32 = 25;

/// Paid against a major's city outside its campaign.
pub(super) const OUTSIDE_CAMPAIGN: i32 = 180;

/// The shipped target score's weights on population and on a capital
/// (`advanced_missionary_step`), taken back for a city the plan prices.
const SHIPPED_POP_WEIGHT: i32 = 12;
const SHIPPED_CAPITAL_BONUS: i32 = 18;

/// A city's banner rows as a flip forecast reads them: lifetime pressure of
/// our faith, of each other faith, and of the Citizens following none.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct FlipRows {
    pub pop: i32,
    pub own: f64,
    pub others: Vec<f64>,
    pub none: f64,
}

impl FlipRows {
    /// The rows the host showed for `city`, split around `religion`.
    pub(super) fn observed(g: &Game, city: &crate::game::City, religion: &str) -> Option<Self> {
        let rows = g.observed_city_faiths.get(&city.id)?;
        if rows.is_empty() {
            return None;
        }
        let mut flip = FlipRows {
            pop: city.pop.max(0),
            own: 0.0,
            others: Vec::new(),
            none: 0.0,
        };
        for row in rows {
            match row.religion.as_deref() {
                Some(faith) if faith == religion => flip.own += row.pressure,
                Some(_) if row.pressure > 0.0 => flip.others.push(row.pressure),
                Some(_) => {}
                None => flip.none += row.pressure,
            }
        }
        Some(flip)
    }

    /// Citizens seated for our faith and for the religionless row by the
    /// largest-remainder method: whole parts of `pop x pressure / total`
    /// first, then one each to the largest fractional parts, a tie going to
    /// the greater pressure and then to a faith ahead of the religionless.
    fn seats(&self) -> (i32, i32) {
        let rows: Vec<f64> = std::iter::once(self.own)
            .chain(self.others.iter().copied())
            .chain(std::iter::once(self.none))
            .collect();
        let total: f64 = rows.iter().sum();
        if self.pop <= 0 || total <= f64::EPSILON {
            return (0, 0);
        }
        let quota: Vec<(f64, f64)> = rows
            .iter()
            .map(|pressure| {
                let quota = f64::from(self.pop) * pressure / total;
                (quota.floor(), quota - quota.floor())
            })
            .collect();
        let mut seats: Vec<i32> = quota.iter().map(|(whole, _)| *whole as i32).collect();
        let left = (self.pop - seats.iter().sum::<i32>()).max(0) as usize;
        let mut order: Vec<usize> = (0..rows.len()).filter(|i| rows[*i] > 0.0).collect();
        order.sort_by(|a, b| {
            quota[*b]
                .1
                .total_cmp(&quota[*a].1)
                .then(rows[*b].total_cmp(&rows[*a]))
                .then(a.cmp(b))
        });
        for index in order.into_iter().take(left) {
            seats[index] += 1;
        }
        (seats[0], seats[rows.len() - 1])
    }

    /// Whether our faith is the city's majority as the host decides it: no
    /// other faith out-presses it, and its followers are more than half the
    /// Citizens, or exactly half with the religionless half not out-pressing
    /// it. A level rival faith is read as not ours.
    pub(super) fn holds(&self) -> bool {
        if self.own <= 0.0 || self.others.iter().any(|other| *other >= self.own) {
            return false;
        }
        if self.pop <= 0 {
            let total = self.own + self.others.iter().sum::<f64>() + self.none;
            return self.own * 2.0 > total;
        }
        let (followers, religionless) = self.seats();
        followers * 2 > self.pop
            || (followers * 2 == self.pop && !(self.none > self.own && religionless == followers))
    }

    /// Spreads of `spread` pressure, each removing `eviction` of every other
    /// faith's, until our faith holds the city. `CHARGE_CAP + 1` when the cap
    /// does not reach it.
    pub(super) fn charges(&self, spread: f64, eviction: f64) -> u32 {
        let mut rows = self.clone();
        for used in 0..=CHARGE_CAP {
            if rows.holds() {
                return used;
            }
            rows.own += spread;
            for other in &mut rows.others {
                *other *= 1.0 - eviction;
            }
        }
        CHARGE_CAP + 1
    }
}

impl AdvancedAi {
    /// The charges the cities this seat can see still ask of its corps: for
    /// every rival major, the unpromoted-Apostle charges of the cheapest
    /// cities it still owes our majority (plus [`CAMPAIGN_SPARE`]), and for
    /// every city of ours not on our faith, a Missionary's. Each city counts
    /// up to [`CHARGE_CAP`]. `None` when the host has shown no rows, which is
    /// every simulated game.
    pub(super) fn campaign_charges_known(g: &Game, pid: usize, religion: &str) -> Option<u32> {
        if g.observed_city_faiths.is_empty() {
            return None;
        }
        let mut demand = 0;
        for rival in g.players.iter().filter(|rival| {
            rival.alive && !rival.is_minor && !rival.is_barbarian && !rival.is_free_city
        }) {
            let cities: Vec<&crate::game::City> = g
                .cities
                .values()
                .filter(|city| city.owner == rival.id)
                .collect();
            let mut open: Vec<u32> = cities
                .iter()
                .filter(|city| g.city_religion(city) != Some(religion))
                .filter_map(|city| FlipRows::observed(g, city, religion))
                .map(|rows| {
                    if rival.id == pid {
                        rows.charges(200.0, 0.10)
                    } else {
                        rows.charges(REFERENCE_SPREAD, REFERENCE_EVICTION)
                    }
                    .min(CHARGE_CAP)
                })
                .collect();
            if rival.id == pid {
                demand += open.iter().sum::<u32>();
                continue;
            }
            if g.same_team(pid, rival.id) {
                continue;
            }
            let held = cities
                .iter()
                .filter(|city| g.city_religion(city) == Some(religion))
                .count();
            let count = g
                .observed_public_empire_stats
                .get(&rival.id)
                .and_then(|stats| stats.city_count)
                .map_or(cities.len(), |count| count.max(cities.len()));
            let owed = (count / 2 + 1).saturating_sub(held);
            open.sort_unstable();
            demand += open.iter().take(owed + CAMPAIGN_SPARE).sum::<u32>();
        }
        Some(demand)
    }

    /// What one spread by `uid` adds to its faith in a foreign city and the
    /// share of every other faith's pressure it removes (`Game::do_spread`).
    fn spreader_strength(g: &Game, uid: u32) -> (f64, f64) {
        let unit = &g.units[&uid];
        let spread = g.rules.units[unit.kind].religious_spread
            * (1.0 + g.promotion_effect(unit, "foreign_spread_pct") / 100.0)
            * f64::from(unit.hp.clamp(0, 100))
            / 100.0;
        let eviction: f64 = match unit.kind.as_str() {
            "apostle" => 0.25,
            "missionary" => 0.10,
            _ => 0.0,
        };
        (
            spread,
            eviction.max(g.promotion_effect(unit, "rival_pressure_removed_pct") / 100.0),
        )
    }

    /// The adjustment to the spreader's target score for each rival major's
    /// city that carries the host's rows, by city. Empty with the gene off,
    /// on another target, or for another faith's spreader. See the module.
    pub(super) fn religion_seat_pressure_plan(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        religion: &str,
    ) -> BTreeMap<u32, i32> {
        let mut plan = BTreeMap::new();
        if !self.religion_seat_counts_the_pressure
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
            || g.players[pid].religion.as_deref() != Some(religion)
            || g.observed_city_faiths.is_empty()
        {
            return plan;
        }
        let (spread, eviction) = Self::spreader_strength(g, uid);
        if spread <= 0.0 {
            return plan;
        }
        let charges_left = g.units[&uid].charges.max(0) as u32;
        let mut cheapest: Option<(u32, String)> = None;
        for rival in g.players.iter().filter(|rival| {
            rival.id != pid
                && rival.alive
                && !rival.is_minor
                && !rival.is_barbarian
                && !rival.is_free_city
                && !g.same_team(pid, rival.id)
        }) {
            let cities: Vec<&crate::game::City> = g
                .cities
                .values()
                .filter(|city| city.owner == rival.id)
                .collect();
            let held = cities
                .iter()
                .filter(|city| g.city_religion(city) == Some(religion))
                .count();
            let count = g
                .observed_public_empire_stats
                .get(&rival.id)
                .and_then(|stats| stats.city_count)
                .map_or(cities.len(), |count| count.max(cities.len()));
            let owed = (count / 2 + 1).saturating_sub(held);
            let mut open: Vec<(u32, u32, FlipRows)> = cities
                .iter()
                .filter(|city| g.city_religion(city) != Some(religion))
                .filter_map(|city| {
                    let rows = FlipRows::observed(g, city, religion)?;
                    Some((
                        rows.charges(REFERENCE_SPREAD, REFERENCE_EVICTION),
                        city.id,
                        rows,
                    ))
                })
                .collect();
            open.sort_by_key(|row| (row.0, row.1));
            for (rank, (reference, cid, rows)) in open.iter().enumerate() {
                let city = &g.cities[cid];
                let shipped = if self.religion_seat_flips_the_cheapest {
                    // That gene already takes the shipped terms back.
                    0
                } else {
                    city.pop.max(0) * SHIPPED_POP_WEIGHT
                        + i32::from(city.is_capital) * SHIPPED_CAPITAL_BONUS
                };
                let adjustment = if rank < owed + CAMPAIGN_SPARE {
                    let needed = rows.charges(spread, eviction);
                    if cheapest.as_ref().is_none_or(|(least, _)| needed < *least) {
                        cheapest = Some((needed, city.name.clone()));
                    }
                    CAMPAIGN_BONUS - needed.min(CHARGE_CAP) as i32 * CHARGE_COST
                        + if (1..=charges_left).contains(&needed) {
                            SOLO_FLIP_BONUS
                        } else {
                            0
                        }
                        + reference.saturating_sub(needed) as i32 * ADVANTAGE_BONUS
                } else {
                    -OUTSIDE_CAMPAIGN
                };
                plan.insert(*cid, adjustment - shipped);
            }
        }
        if let Some((needed, name)) = cheapest {
            think!(self.journal(), Faith, Detail,
                "{} spreader prices {} rival cities", religion, plan.len();
                "religion-seat-counts-the-pressure: the host's rows put {} at {} of this unit's charges, the cheapest city a rival still owes our majority",
                name, needed);
        }
        plan
    }
}

#[cfg(test)]
mod tests;
