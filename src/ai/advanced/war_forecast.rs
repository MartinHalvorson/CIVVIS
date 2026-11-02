//! War forecast: which neighbour is likely to declare on us, and whether its
//! army would beat ours.
//!
//! **The miss (live Lv 6 civvis-20261102T185013Z, Peter, 5 cities at t60,
//! 1 at t122).** Babylon declared a surprise war at turn 57 with a host
//! military of 215 against our 279. Every power test this controller runs
//! read that as safe, because `Game::military_power` adds unit strength
//! linearly. Our 279 was four Warriors (20) and six Archers (25); Babylon's
//! 215 was Pikemen (45) and a Heavy Chariot. Civilization VI's blow is
//! `30·e^{Δ/25}` (`game::expected_damage`): a Pikeman deals a Warrior
//! 2.7 times the damage it takes back. Babylon took Astrakhan and St
//! Petersburg in nine turns. After the peace, its military climbed 260 → 493
//! over twenty turns while ours stood at 240-290. It declared again at
//! turn 92 with Musketmen and Pike and Shot (55) and Knights (50) against our
//! Swordsmen (35), and took Moscow. `near-rival-deterrence` had stopped
//! looking at turn 80.
//!
//! **The census (68 live games, 2026-10-07 → 11-02, King to Deity;
//! 30,667 rival-turns at peace; 59 declarations on us, 14 of them costing a
//! city).** The ten-turn declaration hazard fits a logistic with these
//! factors (AUC 0.83, calibrated within ±0.03 in every decile):
//!
//! | signal | odds × |
//! |---|---:|
//! | their nearest city within 7 tiles of ours | 4.6 |
//! | within 8-10 tiles | 1.3 |
//! | 4+ of their soldiers within 4 tiles of our cities (scaled) | 3.2 |
//! | they denounced us | 1.3 |
//! | a declared friendship or alliance | 0.24 |
//! | ln(their quality power / ours) | 1.14 per e |
//! | live turn 160+ | 2.2 |
//!
//! Base rate 1.7% per rival per ten turns. A forecast that stands at a
//! hazard of [`FORECAST_HAZARD_FLOOR`] and a quality-weighted army at least
//! level with ours ([`FORECAST_EDGE_FLOOR`]) flags 9.3% of rival-turns. It
//! warned 3-20 turns ahead of 29 of the 59 declarations and of 12 of the 14
//! that cost a city. `near-rival-deterrence`'s own trigger (a city within 10
//! tiles, plain power level, live turns 30-80) flagged 3.6% and warned of 6 of
//! the 14. The coefficients were fit on the same games, so treat them as a
//! calibrated prior, not a holdout result.
//!
//! **Quality power.** A player's army is priced as its unit count times
//! `e^{(q-20)/25}`, `q` being the mean strength of its land units. The
//! count is `military_power / q`, so an army of Warriors counts its units.
//! This follows from the damage curve: with the same count, one side's
//! blows land `e^{Δ/25}` times harder and its units last `e^{Δ/25}` times
//! longer (Lanchester's square law with the quality factor). Our own `q` is
//! our land roster. A rival's is the mean of its land units in our sight
//! once [`SEEN_ARMY_FLOOR`] are visible. Otherwise it is
//! [`UNSEEN_ARMY_ERA_SHARE`] of the best land unit its technologies build.

use super::*;

/// Civilization VI's damage curve: a blow is `30·e^{Δ/25}`
/// (`game::expected_damage`), so 25 points of strength are a factor of e.
pub(crate) const STRENGTH_SCALE: f64 = 25.0;

/// The Warrior. An army of Warriors counts its units.
pub(crate) const REFERENCE_STRENGTH: f64 = 20.0;

/// A rival whose army is out of sight is assumed to field this share of the
/// best land unit its technologies build (the
/// `threatened_city_purchase::DEFENDER_ERA_SHARE` reading).
pub(crate) const UNSEEN_ARMY_ERA_SHARE: f64 = 0.8;

/// The fewest of a rival's land units in sight whose mean stands for its army.
pub(crate) const SEEN_ARMY_FLOOR: usize = 2;

/// A rival soldier within this many tiles of one of our cities is at our door.
pub(crate) const MASSED_TILES: i32 = 4;

/// Soldiers at our door beyond this many add nothing to the hazard.
pub(crate) const MASSED_CAP: usize = 4;

/// The forecast stands at this ten-turn hazard or more…
pub(crate) const FORECAST_HAZARD_FLOOR: f64 = 0.03;

/// …while the rival's quality power is at least this multiple of ours.
pub(crate) const FORECAST_EDGE_FLOOR: f64 = 1.0;

/// Live turn 160 at the seat's Quick speed, in standard turns.
pub(crate) const FORECAST_LATE_STANDARD: u32 = 240;

/// Logistic coefficients of the ten-turn declaration hazard (see the module).
const HAZARD_BIAS: f64 = -4.96;
const HAZARD_NEAR: f64 = 1.53;
const HAZARD_NEAR_RING: f64 = 0.24;
const HAZARD_MASSED: f64 = 1.15;
const HAZARD_DENOUNCED: f64 = 0.24;
const HAZARD_FRIEND: f64 = -1.41;
const HAZARD_EDGE: f64 = 0.13;
const HAZARD_LATE: f64 = 0.79;

/// The tiles inside which a neighbour's city is "near" (odds × 4.6), and the
/// outer ring (odds × 1.3).
const HAZARD_NEAR_TILES: i32 = 7;
const HAZARD_RING_TILES: i32 = 10;

/// What we expect of one rival at peace with us.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ThreatForecast {
    pub rival: usize,
    /// Chance the rival declares on us within ten live turns.
    pub hazard: f64,
    /// Our quality power.
    pub ours: f64,
    /// The rival's quality power.
    pub theirs: f64,
    /// Mean land strength behind each power.
    pub our_quality: f64,
    pub their_quality: f64,
    /// Tiles from their nearest known city to our nearest city.
    pub nearest: i32,
    /// Their soldiers in our sight within [`MASSED_TILES`] of our cities.
    pub massed: usize,
}

impl ThreatForecast {
    /// Their quality power over ours.
    pub(crate) fn edge(&self) -> f64 {
        self.theirs / self.ours.max(1.0)
    }

    /// The forecast calls for a counter: a likely declaration by an army at
    /// least level with ours.
    pub(crate) fn stands(&self) -> bool {
        self.hazard >= FORECAST_HAZARD_FLOOR && self.edge() >= FORECAST_EDGE_FLOOR
    }
}

/// A land soldier: military, not recon, not at sea or in the air.
fn land_soldier(spec: &crate::rules::UnitSpec) -> bool {
    spec.class == "military"
        && spec.promotion_class != "recon"
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
}

fn fighting_strength(spec: &crate::rules::UnitSpec) -> f64 {
    spec.strength.max(spec.ranged_attack_strength())
}

impl AdvancedAi {
    /// The strongest land soldier `owner`'s technologies and civics build,
    /// unique units aside. 0 when nothing is known.
    pub(crate) fn best_buildable_land_strength(g: &Game, owner: usize) -> f64 {
        let player = &g.players[owner];
        g.rules
            .units
            .values()
            .filter(|spec| {
                land_soldier(spec)
                    && !spec.siege
                    && spec.buildable
                    && spec.unique_to.is_none()
                    && spec
                        .tech
                        .as_ref()
                        .is_none_or(|tech| player.techs.contains(tech))
                    && spec
                        .civic
                        .as_ref()
                        .is_none_or(|civic| player.civics.contains(civic))
            })
            .map(fighting_strength)
            .fold(0.0, f64::max)
    }

    /// Mean strength of `owner`'s land army as `viewer` knows it: our own
    /// roster, a rival's soldiers in sight, or the era fallback.
    pub(crate) fn army_quality(g: &Game, viewer: usize, owner: usize) -> f64 {
        let visible = (viewer != owner).then(|| g.player_vision_frame(viewer));
        let (sum, count) = g
            .units
            .values()
            .filter(|unit| unit.owner == owner)
            .filter(|unit| visible.as_ref().is_none_or(|frame| g.sees(frame, unit.pos)))
            .filter_map(|unit| g.rules.units.get(&unit.kind))
            .filter(|spec| land_soldier(spec))
            .fold((0.0, 0usize), |(sum, count), spec| {
                (sum + fighting_strength(spec), count + 1)
            });
        let floor = if viewer == owner { 1 } else { SEEN_ARMY_FLOOR };
        if count >= floor {
            return sum / count as f64;
        }
        let era = UNSEEN_ARMY_ERA_SHARE * Self::best_buildable_land_strength(g, owner);
        if era > 0.0 {
            era
        } else {
            REFERENCE_STRENGTH
        }
    }

    /// `military_power / q` units at `e^{(q-20)/25}` each.
    pub(crate) fn quality_power_at(power: f64, quality: f64) -> f64 {
        let quality = quality.max(1.0);
        (power.max(0.0) / quality) * ((quality - REFERENCE_STRENGTH) / STRENGTH_SCALE).exp()
    }

    /// `owner`'s quality power as `viewer` knows it.
    pub(crate) fn quality_power(g: &Game, viewer: usize, owner: usize) -> f64 {
        Self::quality_power_at(
            g.military_power(owner),
            Self::army_quality(g, viewer, owner),
        )
    }

    /// The forecast for one met major at peace with us. `None` for anyone
    /// else, or before we hold a city.
    pub(crate) fn threat_forecast(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<ThreatForecast> {
        let other = g.players.get(rival)?;
        if rival == pid
            || !other.alive
            || other.is_minor
            || other.is_barbarian
            || other.is_free_city
            || !g.has_met(pid, rival)
            || g.is_at_war(pid, rival)
        {
            return None;
        }
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        if ours.is_empty() {
            return None;
        }
        let nearest = g
            .cities
            .values()
            .filter(|city| city.owner == rival)
            .flat_map(|city| ours.iter().map(|home| g.wdist(*home, city.pos)))
            .min()
            .unwrap_or(i32::MAX);
        let visible = g.player_vision_frame(pid);
        let massed = g
            .units
            .values()
            .filter(|unit| unit.owner == rival && g.sees(&visible, unit.pos))
            .filter(|unit| g.rules.units.get(&unit.kind).is_some_and(land_soldier))
            .filter(|unit| {
                ours.iter()
                    .any(|home| g.wdist(*home, unit.pos) <= MASSED_TILES)
            })
            .count();
        let our_quality = Self::army_quality(g, pid, pid);
        let their_quality = Self::army_quality(g, pid, rival);
        let ours_power = Self::quality_power_at(g.military_power(pid), our_quality);
        let theirs_power = Self::quality_power_at(g.military_power(rival), their_quality);
        let state = g.players[pid].observed_diplomatic_state.get(&rival);
        let state_is = |name: &str| state.is_some_and(|value| value == name);
        let denounced = state_is("DENOUNCED")
            || other
                .denounced_until
                .get(&pid)
                .is_some_and(|until| *until > g.turn);
        let friend = g.are_friends(pid, rival)
            || g.players[pid].alliances.contains_key(&rival)
            || state_is("DECLARED_FRIEND")
            || state_is("ALLIED");
        let edge = (theirs_power / ours_power.max(1.0)).clamp(0.05, 20.0);
        let late = g.turn >= g.standard_duration(FORECAST_LATE_STANDARD);
        let z = HAZARD_BIAS
            + if nearest <= HAZARD_NEAR_TILES {
                HAZARD_NEAR
            } else if nearest <= HAZARD_RING_TILES {
                HAZARD_NEAR_RING
            } else {
                0.0
            }
            + HAZARD_MASSED * massed.min(MASSED_CAP) as f64 / MASSED_CAP as f64
            + if denounced { HAZARD_DENOUNCED } else { 0.0 }
            + if friend { HAZARD_FRIEND } else { 0.0 }
            + HAZARD_EDGE * edge.ln()
            + if late { HAZARD_LATE } else { 0.0 };
        Some(ThreatForecast {
            rival,
            hazard: 1.0 / (1.0 + (-z).exp()),
            ours: ours_power,
            theirs: theirs_power,
            our_quality,
            their_quality,
            nearest,
            massed,
        })
    }

    /// Every met major's forecast, most dangerous first: a standing
    /// forecast before one that does not, then the larger hazard times edge.
    pub(crate) fn threat_forecasts(&self, g: &Game, pid: usize) -> Vec<ThreatForecast> {
        let mut out: Vec<ThreatForecast> = (0..g.players.len())
            .filter_map(|rival| self.threat_forecast(g, pid, rival))
            .collect();
        out.sort_by(|a, b| {
            b.stands()
                .cmp(&a.stands())
                .then_with(|| (b.hazard * b.edge()).total_cmp(&(a.hazard * a.edge())))
                .then(a.rival.cmp(&b.rival))
        });
        out
    }
}

#[cfg(test)]
mod tests;
