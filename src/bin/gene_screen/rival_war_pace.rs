//! ⭐ `--rival-war-pace PCT` (2026-11-04): the rival chairs open wars on the
//! measured seats at the rate the live Firaxis AI opens them on ours.
//!
//! **Why.** At Standard speed the calibrated harness (`--rivals
//! firaxis-faithful --handicap rivals --rival-rung -2 --rival-pace`) is a
//! nearly peaceful world. Paired reads on 11-04 took 0.00-0.06 cities per
//! measured seat by turn 220-250 and showed a war in 3 of 16 trajectories,
//! the first at turn 175. The live Gran Colombia seat at Standard Immortal
//! (26 games, 11-04 10:44Z-18:30Z) had a war declared on it by turn 60 in 19%
//! of games, by turn 100 in 58% and by turn 250 in 92%. It faced 2.1 such wars
//! per game, lost 0.46 cities by turn 100 and 1.54 by turn 250, and took 0.69.
//! So every Standard read priced defensive and domination genes in a world
//! where nobody attacks.
//!
//! **Model.** -5f's 10-turn declaration hazard, fitted on 68 live games and
//! 119 wars (AUC 0.83, PR #4344): the logit is
//!
//! ```text
//! −4.96 + 1.53·[their city within 7 tiles of ours] + 0.24·[within 8-10]
//!       + 1.15·min(n, 4)/4   (n = their land soldiers within 4 of our cities)
//!       + 0.24·[a denouncement either way] − 1.41·[friends]
//!       + 0.13·ln(their power / ours) + 0.79·[late game]
//! ```
//!
//! The plain power ratio barely predicted a declaration, so proximity and
//! soldiers carry the model. The hazard is per RAW turn: live wars landed at
//! the same raw turn at Online and Standard (-af, 11-04: t42.5 against t43).
//! The per-turn chance is `1 − (1 − p10)^(1/10)`, and PCT scales the odds,
//! which is the calibration dial (100 = the fitted model as it stands).
//!
//! **Harness only.** The roll is a hash of (seed, turn, rival, target), so it
//! never draws from the game's own RNG. With the flag absent no seat is
//! wrapped and a game is byte-for-byte what it was. A declaration is a plain
//! `Action::DeclareWar`, which the engine may refuse (unmet, a peace treaty
//! still running). After that the rival chair's own AI fights the war.

use civvis::ai::{AdvancedAi, Ai, ExpansionCensus, PlanReport};
use civvis::game::{Action, Game};
use civvis::reasoning::Journal;

/// The fitted logit's terms. See the module documentation.
const BIAS: f64 = -4.96;
const NEAR_CITY_TILES: i32 = 7;
const NEAR_CITY: f64 = 1.53;
const MID_CITY_TILES: i32 = 10;
const MID_CITY: f64 = 0.24;
const SOLDIER_TILES: i32 = 4;
const SOLDIER_CAP: usize = 4;
const SOLDIERS: f64 = 1.15;
const DENOUNCED: f64 = 0.24;
const FRIENDS: f64 = -1.41;
const POWER_RATIO: f64 = 0.13;
const LATE: f64 = 0.79;
/// Online turn 160 of 250 was the fitted "late" mark; on this game's own
/// clock that is Standard turn 320.
const LATE_STANDARD_TURN: u32 = 320;
/// The fitted window, in turns.
const WINDOW: f64 = 10.0;

/// One declaration the knob made: `(turn, rival, target)`.
pub type Declaration = (u32, usize, usize);

/// What one rival chair needs to roll its declarations.
#[derive(Clone, Debug)]
pub struct Pace {
    /// The odds multiplier, in percent.
    pub pct: u32,
    /// The game's seed, for the hash.
    pub seed: u64,
    /// The measured seats this chair may declare on.
    pub targets: Vec<usize>,
}

/// A seat's AI, with the knob's roll ahead of its turn when `pace` is set.
pub struct PacedSeat {
    pub ai: AdvancedAi,
    pub pace: Option<Pace>,
    pub declared: Vec<Declaration>,
}

impl PacedSeat {
    pub fn new(ai: AdvancedAi, pace: Option<Pace>) -> PacedSeat {
        PacedSeat {
            ai,
            pace,
            declared: Vec::new(),
        }
    }
}

impl Ai for PacedSeat {
    fn uses_player_observation(&self) -> bool {
        Ai::uses_player_observation(&self.ai)
    }

    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        if let Some(pace) = &self.pace {
            for &target in &pace.targets {
                if g.winner.is_some() {
                    break;
                }
                let Some(chance) = turn_chance(g, pid, target, pace.pct) else {
                    continue;
                };
                if roll(pace.seed, g.turn, pid, target) < chance
                    && g.apply(pid, &Action::DeclareWar { player: target }).is_ok()
                    && g.is_at_war(pid, target)
                {
                    self.declared.push((g.turn, pid, target));
                }
            }
        }
        Ai::take_turn(&mut self.ai, g, pid);
    }

    fn strategy_label(&self) -> Option<&'static str> {
        Ai::strategy_label(&self.ai)
    }

    fn plan_report(&self) -> Option<PlanReport> {
        Ai::plan_report(&self.ai)
    }

    fn review_census(&self) -> Option<civvis::strategic::ReviewCensus> {
        Ai::review_census(&self.ai)
    }

    fn expansion_census(&self) -> Option<ExpansionCensus> {
        Ai::expansion_census(&self.ai)
    }

    fn attach_journal(&mut self, journal: Journal) {
        Ai::attach_journal(&mut self.ai, journal);
    }
}

/// This turn's chance that `rival` declares on `target`, or `None` when a
/// declaration is not on the table at all (same seat, dead, unmet, at war).
pub fn turn_chance(g: &Game, rival: usize, target: usize, pct: u32) -> Option<f64> {
    if rival == target || pct == 0 {
        return None;
    }
    let (Some(us), Some(them)) = (g.players.get(target), g.players.get(rival)) else {
        return None;
    };
    if !us.alive || !them.alive || !g.has_met(rival, target) || g.is_at_war(rival, target) {
        return None;
    }
    let ours: Vec<_> = g
        .cities
        .values()
        .filter(|city| city.owner == target)
        .map(|city| city.pos)
        .collect();
    if ours.is_empty() {
        return None;
    }
    let nearest = g
        .cities
        .values()
        .filter(|city| city.owner == rival)
        .flat_map(|city| ours.iter().map(move |&pos| (city.pos, pos)))
        .map(|(a, b)| g.wdist(a, b))
        .min();
    let soldiers = g
        .units
        .values()
        .filter(|unit| {
            let def = &g.rules.units[unit.kind];
            unit.owner == rival
                && def.strength > 0.0
                && !matches!(def.domain.as_deref(), Some("sea" | "air"))
                && ours
                    .iter()
                    .any(|&pos| g.wdist(unit.pos, pos) <= SOLDIER_TILES)
        })
        .count();
    let denounced = them
        .denounced_until
        .get(&target)
        .is_some_and(|until| *until > g.turn)
        || us
            .denounced_until
            .get(&rival)
            .is_some_and(|until| *until > g.turn);
    let ratio = (g.military_power(rival).max(1.0) / g.military_power(target).max(1.0)).ln();
    let logit = hazard_logit(
        nearest,
        soldiers,
        denounced,
        g.are_friends(rival, target),
        ratio,
        g.turn >= g.standard_duration(LATE_STANDARD_TURN),
    ) + (pct as f64 / 100.0).ln();
    Some(per_turn(logit))
}

/// The fitted 10-turn logit for one rival and one target.
pub fn hazard_logit(
    nearest_city: Option<i32>,
    soldiers: usize,
    denounced: bool,
    friends: bool,
    ln_power_ratio: f64,
    late: bool,
) -> f64 {
    let mut logit = BIAS;
    match nearest_city {
        Some(d) if d <= NEAR_CITY_TILES => logit += NEAR_CITY,
        Some(d) if d <= MID_CITY_TILES => logit += MID_CITY,
        _ => {}
    }
    logit += SOLDIERS * soldiers.min(SOLDIER_CAP) as f64 / SOLDIER_CAP as f64;
    if denounced {
        logit += DENOUNCED;
    }
    if friends {
        logit += FRIENDS;
    }
    logit += POWER_RATIO * ln_power_ratio;
    if late {
        logit += LATE;
    }
    logit
}

/// The per-turn chance from a 10-turn logit.
pub fn per_turn(logit: f64) -> f64 {
    let window = 1.0 / (1.0 + (-logit).exp());
    1.0 - (1.0 - window).powf(1.0 / WINDOW)
}

/// A uniform draw in `[0, 1)` from the game's seed and this rival-target
/// turn, independent of the game's own RNG (splitmix64).
pub fn roll(seed: u64, turn: u32, rival: usize, target: usize) -> f64 {
    let mut z = seed
        ^ (turn as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ ((rival as u64) << 48)
        ^ ((target as u64) << 40)
        ^ 0x5741_5250_4143_45; // "WARPACE"
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fitted_baseline_is_a_rare_war() {
        // No city in reach, no soldiers, parity: 0.7% a window.
        let p = per_turn(hazard_logit(None, 0, false, false, 0.0, false));
        assert!((0.0006..0.0008).contains(&p), "{p}");
    }

    #[test]
    fn a_crowded_border_with_soldiers_is_a_likely_war() {
        let calm = per_turn(hazard_logit(Some(12), 0, false, false, 0.0, false));
        let crowded = per_turn(hazard_logit(Some(6), 4, false, false, 0.0, false));
        assert!(crowded > 10.0 * calm, "{crowded} against {calm}");
        // Friendship cuts it by about four.
        let friends = per_turn(hazard_logit(Some(6), 4, false, true, 0.0, false));
        assert!(friends < crowded / 3.0, "{friends} against {crowded}");
    }

    #[test]
    fn the_pace_scales_the_odds() {
        let base = hazard_logit(Some(6), 2, false, false, 0.5, false);
        let doubled = per_turn(base + 2.0f64.ln());
        let plain = per_turn(base);
        assert!(
            doubled > 1.8 * plain && doubled < 2.0 * plain,
            "{doubled} vs {plain}"
        );
    }

    #[test]
    fn the_roll_is_uniform_and_keyed() {
        let draws: Vec<f64> = (0..20_000).map(|t| roll(44_400_000, t, 1, 0)).collect();
        let mean = draws.iter().sum::<f64>() / draws.len() as f64;
        assert!((0.49..0.51).contains(&mean), "{mean}");
        assert!(draws.iter().all(|d| (0.0..1.0).contains(d)));
        assert_ne!(roll(1, 50, 1, 0), roll(1, 50, 2, 0));
        assert_ne!(roll(1, 50, 1, 0), roll(2, 50, 1, 0));
        assert_eq!(roll(7, 9, 3, 0), roll(7, 9, 3, 0));
    }
}
