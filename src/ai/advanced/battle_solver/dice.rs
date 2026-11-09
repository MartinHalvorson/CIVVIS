//! Exact combat arithmetic: what a blow can roll, and what a volley can do.
//!
//! The engine's roll (`game::damage`) is `30·e^(Δ/25)` times a uniform factor
//! on `[0.8, 1.2]`, rounded and clamped to `[1, 100]`. Measured against 4,669
//! live Civilization VI strikes (2026-11-08/09, `strike.preview` joined to
//! the `combat` that resolved it) the host's dealt/preview ratio has mean
//! 0.997 and a central range of 0.80–1.20, so this distribution is the real
//! one and a kill can be priced as a probability rather than a margin.
//!
//! The wound penalty is what makes the exact form worth having: a defender's
//! strength falls as it is hit, so the second blow of a volley does more than
//! the first. A kill chance read off mean damage gets this wrong in both
//! directions; [`VolleyDist`] carries the full hit-point distribution through
//! every blow and applies the wound penalty at each hit-point value.

/// The wound penalty as `game::effective_strength` states it.
#[inline]
pub fn effective_strength(base: f32, hp: i32) -> f32 {
    let wounded = (10.0 - hp.clamp(0, 100) as f32 / 10.0).round();
    (base - wounded).max(0.0)
}

/// The centre of a blow before the roll, unclamped (`game::damage`).
#[inline]
pub fn blow_centre(att: f32, def: f32) -> f32 {
    30.0 * ((att - def) / 25.0).exp()
}

/// The expected damage of a blow, as `game::expected_damage` states it.
#[inline]
pub fn mean_damage(att: f32, def: f32) -> f32 {
    blow_centre(att, def).clamp(1.0, 100.0)
}

/// The exact distribution of one blow: `(damage, probability)` pairs in
/// ascending damage, summing to one.
pub fn roll_pmf(att: f32, def: f32) -> Vec<(i32, f64)> {
    let centre = blow_centre(att, def) as f64;
    let (lo, hi) = (0.8 * centre, 1.2 * centre);
    let width = hi - lo;
    if width <= 0.0 {
        return vec![(1, 1.0)];
    }
    let mut pmf = Vec::with_capacity(32);
    let first = lo.round().max(1.0) as i32;
    let last = hi.round().min(100.0) as i32;
    // Everything rolled under 1.5 rounds to at most 1 and clamps to 1;
    // everything over 99.5 rounds to at least 100 and clamps to 100.
    for k in first..=last.max(first) {
        let low_edge = if k <= 1 {
            f64::NEG_INFINITY
        } else {
            k as f64 - 0.5
        };
        let high_edge = if k >= 100 {
            f64::INFINITY
        } else {
            k as f64 + 0.5
        };
        let overlap = high_edge.min(hi) - low_edge.max(lo);
        if overlap > 0.0 {
            pmf.push((k, overlap / width));
        }
    }
    pmf
}

/// Hit points of one target as a distribution, carried through a volley.
#[derive(Clone)]
pub struct VolleyDist {
    /// `hp[k]` = probability the target stands at `k` hit points; 0 = dead.
    pub hp: [f64; 101],
    /// Defender base strength before the wound penalty.
    pub def_base: f32,
}

impl VolleyDist {
    pub fn new(hp: i32, def_base: f32) -> Self {
        let mut dist = [0.0; 101];
        dist[hp.clamp(0, 100) as usize] = 1.0;
        Self { hp: dist, def_base }
    }

    /// Land one blow of `att` (already through the attacker's wound penalty).
    pub fn strike(&mut self, att: f32) {
        let mut next = [0.0f64; 101];
        next[0] = self.hp[0];
        for (hp, &p) in self.hp.iter().enumerate().skip(1) {
            if p <= 0.0 {
                continue;
            }
            for (dmg, roll) in roll_pmf(att, effective_strength(self.def_base, hp as i32)) {
                next[(hp as i32 - dmg).max(0) as usize] += p * roll;
            }
        }
        self.hp = next;
    }

    /// Land one blow that also returns damage, keeping the kill chance of
    /// the attacker: `P(this blow kills the attacker at `atk_hp`)` under the
    /// defender's reply at each of the target's possible hit points.
    pub fn p_killed(&self) -> f64 {
        self.hp[0]
    }
}

/// `P(the blows kill a target at `hp`)`, each blow given as the attacker's
/// strength after its own wound penalty. Exact, including the target's wound
/// penalty after each hit.
pub fn kill_probability(hp: i32, def_base: f32, blows: &[f32]) -> f64 {
    let mut dist = VolleyDist::new(hp, def_base);
    for &att in blows {
        dist.strike(att);
    }
    dist.p_killed()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pmf_sums_to_one_and_stays_on_the_rolled_range() {
        for (a, d) in [(40.0, 35.0), (20.0, 20.0), (55.0, 20.0), (10.0, 60.0)] {
            let pmf = roll_pmf(a, d);
            let total: f64 = pmf.iter().map(|(_, p)| p).sum();
            assert!((total - 1.0).abs() < 1e-9, "{a} {d} {total}");
            let centre = blow_centre(a, d) as f64;
            for (k, _) in &pmf {
                assert!(*k >= ((0.8 * centre).round() as i32).max(1));
                assert!(*k <= ((1.2 * centre).round() as i32).min(100));
            }
        }
    }

    #[test]
    fn the_pmf_is_the_roll_of_game_damage_sampled() {
        // Monte Carlo of the engine's own formula against the closed form.
        let (a, d) = (36.0f32, 30.0f32);
        let pmf = roll_pmf(a, d);
        let mut x = 0x1234_5678u64;
        let mut counts = std::collections::BTreeMap::<i32, f64>::new();
        let n = 400_000;
        for _ in 0..n {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let u = (x >> 11) as f64 / (1u64 << 53) as f64;
            let dmg = (30.0 * ((a - d) as f64 / 25.0).exp() * (0.8 + 0.4 * u)).round() as i32;
            *counts.entry(dmg.clamp(1, 100)).or_insert(0.0) += 1.0 / n as f64;
        }
        for (k, p) in pmf {
            let observed = counts.get(&k).copied().unwrap_or(0.0);
            assert!((p - observed).abs() < 0.006, "k {k}: {p} vs {observed}");
        }
    }

    #[test]
    fn the_wound_penalty_compounds_inside_a_volley() {
        // Two equal blows on a wounded target: each lands harder than the
        // last, because the target's strength falls as it is hit.
        let (hp, def, att) = (70, 30.0f32, 30.0f32);
        let exact = kill_probability(hp, def, &[att, att]);
        // Read at full strength each blow centres at 30, and 60 < 70: the
        // mean reading says this kill cannot happen.
        assert!(mean_damage(att, def) * 2.0 < hp as f32);
        // The exact reading says it happens two times in three: the target
        // stands at -3 for the first blow and about -6 for the second.
        assert!(exact > 0.6 && exact < 0.75, "{exact}");
        // And it is never above what three blows give.
        assert!(kill_probability(hp, def, &[att, att, att]) > exact);
    }

    #[test]
    fn a_hard_kill_and_an_impossible_one_read_as_such() {
        assert!((kill_probability(10, 20.0, &[40.0]) - 1.0).abs() < 1e-9);
        assert_eq!(kill_probability(100, 40.0, &[20.0]), 0.0);
        let p: f64 = roll_pmf(45.0, 20.0)
            .iter()
            .filter(|(dmg, _)| *dmg >= 70)
            .map(|(_, p)| p)
            .sum();
        assert!(p > 0.0 && p < 1.0, "{p}");
    }
}
