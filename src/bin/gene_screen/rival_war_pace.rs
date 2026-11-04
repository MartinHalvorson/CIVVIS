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
//!
//! **No friendships.** A chair turns down a measured seat's friendship,
//! alliance or defensive pact, and its own such offers to one are withdrawn.
//! The live Immortal AI almost never befriends our seat (91% of rivals were
//! Unfriendly by turn 10, and 0 of 555 rival-turns were allied). Without this
//! the seat's friendship offers bought peace from every chair, and a
//! friendship blocks any declaration until it expires.

use civvis::ai::{AdvancedAi, Ai, ExpansionCensus, PlanReport};
use civvis::game::{Action, DiplomaticDeal, Game, Item};
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
    /// The odds multiplier, in percent; 0 is no declarations.
    pub pct: u32,
    /// `--rival-walls`: the share of this chair's cities, in percent, that
    /// may raise walls; `None` leaves every city free to.
    pub walls_pct: Option<u32>,
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
    /// Friendships, alliances and pacts the knob turned down or withdrew.
    pub refused: u32,
    /// Wall builds `--rival-walls` turned into a soldier.
    pub vetoed_walls: u32,
}

impl PacedSeat {
    pub fn new(ai: AdvancedAi, pace: Option<Pace>) -> PacedSeat {
        PacedSeat {
            ai,
            pace,
            declared: Vec::new(),
            refused: 0,
            vetoed_walls: 0,
        }
    }
}

impl Ai for PacedSeat {
    fn uses_player_observation(&self) -> bool {
        Ai::uses_player_observation(&self.ai)
    }

    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        if let Some(pace) = &self.pace {
            // The live Firaxis AI at Immortal almost never befriends our seat
            // (91% of rivals Unfriendly by turn 10; 0 of 555 rival-turns
            // allied), while a CIVVIS chair accepts the seat's friendship and
            // the engine then refuses every war until it expires. The chair
            // turns down a friendship, alliance or defensive pact from a
            // measured seat, and its own such offers to one are withdrawn.
            let bonds: Vec<u32> = g
                .pending_deals
                .iter()
                .filter(|deal| {
                    pace.pct > 0
                        && deal.to == pid
                        && pace.targets.contains(&deal.from)
                        && binds(deal)
                })
                .map(|deal| deal.id)
                .collect();
            for deal in bonds {
                if g.apply(pid, &Action::RejectDeal { deal }).is_ok() {
                    self.refused += 1;
                }
            }
            // ⚠ Ahead of the chair's own turn: `AdvancedAi::take_turn` ends
            // the turn itself, so nothing after it can still act as the chair.
            if let Some(walls_pct) = pace.walls_pct {
                self.vetoed_walls += veto_walls(g, pid, pace.seed, walls_pct);
            }
            for &target in pace.targets.iter().filter(|_| pace.pct > 0) {
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
        // The chair's turn has ended here (`AdvancedAi::take_turn` sends
        // `EndTurn`); its pending offers to the seat are still on the table.
        if let Some(pace) = &self.pace {
            let before = g.pending_deals.len();
            g.pending_deals.retain(|deal| {
                pace.pct == 0
                    || !(deal.from == pid && pace.targets.contains(&deal.to) && binds(deal))
            });
            self.refused += (before - g.pending_deals.len()) as u32;
        }
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

/// ⭐ `--rival-walls PCT` (2026-11-04): only PCT percent of a chair's cities,
/// keyed by city, may build walls; the rest build their strongest land
/// soldier instead. The CIVVIS chairs wall far faster than the live Firaxis
/// field: 70% of their cities stood walled at turn 100, 84% at 150 and 86%
/// at 200 (4 games at `--rival-war-pace 200`), against 38%, 53% and 64% of
/// the live field's known cities. The measured seat took no city in 18
/// war-paced games (live 0.69 by turn 250), with sieges reading "walls
/// 100/100 … breakers: 0 gun(s) fit". Returns the builds it turned.
pub fn veto_walls(g: &mut Game, pid: usize, seed: u64, walls_pct: u32) -> u32 {
    let share = walls_pct.min(100) as f64 / 100.0;
    let mut vetoed = 0;
    for cid in g.player_city_ids(pid) {
        let walling = g.cities.get(&cid).is_some_and(|city| {
            matches!(city.queue.first(), Some(Item::Building { building }) if is_wall(building.as_str()))
        });
        if !walling || city_roll(seed, cid) < share {
            continue;
        }
        let soldier = g
            .rules
            .units
            .iter()
            .filter(|(_, def)| {
                def.strength > 0.0 && !matches!(def.domain.as_deref(), Some("sea" | "air"))
            })
            .map(|(name, def)| (*name, def.strength))
            .filter(|(name, _)| g.can_produce(pid, cid, &Item::Unit { unit: *name }))
            .max_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then_with(|| b.0.as_str().cmp(a.0.as_str()))
            });
        if let Some((unit, _)) = soldier {
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: Item::Unit { unit },
                },
            )
            .is_ok()
            {
                // The engine banks the paused wall's progress, and the chair
                // re-picks walls on its own turn and resumes it. Move the
                // banked wall progress into the soldier instead: the city
                // loses no production, and the wall never stands.
                if let Some(city) = g.cities.get_mut(&cid) {
                    let banked: Vec<String> = city
                        .production_progress
                        .keys()
                        .filter(|key| key.starts_with("building:") && is_wall(&key[9..]))
                        .cloned()
                        .collect();
                    for key in banked {
                        if let Some(progress) = city.production_progress.remove(&key) {
                            city.production += progress;
                        }
                    }
                }
                vetoed += 1;
            }
        }
    }
    vetoed
}

fn is_wall(building: &str) -> bool {
    matches!(building, "walls" | "medieval_walls" | "renaissance_walls")
}

/// A uniform draw in `[0, 1)` per city for `--rival-walls`.
fn city_roll(seed: u64, city: u32) -> f64 {
    roll(seed ^ 0x5741_4C4C_5300, u32::MAX, city as usize, 0)
}

/// A deal that would bind the two players out of war: a friendship, an
/// alliance or a defensive pact.
fn binds(deal: &DiplomaticDeal) -> bool {
    deal.friendship || deal.alliance.is_some() || deal.defensive_pact
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
    fn every_wall_tier_is_a_wall_and_the_city_draw_is_spread() {
        assert!(is_wall("walls") && is_wall("medieval_walls") && is_wall("renaissance_walls"));
        assert!(!is_wall("walls_of_benin") && !is_wall("monument"));
        let under = (0..10_000u32)
            .filter(|city| city_roll(44_700_000, *city) < 0.55)
            .count();
        assert!((5_300..5_700).contains(&under), "{under}");
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
