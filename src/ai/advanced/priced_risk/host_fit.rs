//! `risk-is-priced`: the host AI's fitted strike and target-choice odds, as
//! #5197 (`solver-plays-the-host`) fitted them on 17,669 live Deity enemy
//! unit-turns (`docs/HOST_ENEMY_MODEL.md` there). A verbatim copy of the
//! tables and the two functions this gene reads from
//! `battle_solver/host_enemy.rs`, held here only until #5197 is on main;
//! then this file goes and `priced_risk.rs` imports `host_enemy` instead.

/// The four kinds of land fighter the fit tells apart.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum HostClass {
    /// Melee, anti-cavalry, recon and anything else that closes to strike.
    #[default]
    Melee = 0,
    Ranged = 1,
    Siege = 2,
    /// Light and heavy cavalry.
    Cavalry = 3,
}

/// Who owns the unit.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Owner {
    #[default]
    Major = 0,
    /// A city-state, or the Free Cities.
    CityState = 1,
    Barbarian = 2,
}

/// P(strike) is a logistic: `STRIKE_INTERCEPT` plus one term from each table,
/// the first entry of each being the reference level.
pub const STRIKE_INTERCEPT: f32 = -4.1300;
/// By [`HostClass`].
pub const STRIKE_CLASS: [f32; 4] = [0.0000, 0.4178, -0.0926, 0.0615];
/// By [`Owner`].
pub const STRIKE_OWNER: [f32; 3] = [0.0000, 0.1859, 0.6539];
/// The striker's own hit points: 33 or less, 34 to 66, 67 or more.
pub const STRIKE_HP: [f32; 3] = [0.0000, 0.5406, 0.9565];
/// Standing in or beside a city of its own side.
pub const STRIKE_HOME: [f32; 2] = [0.0000, 0.3200];
/// The best strength edge (attack less defence, each after its wounds) over
/// the units in reach: under -10, -10 to -1, 0 to 9, 10 or more. The blow's
/// damage is `30·e^(edge/25)`, so a melee trade's dealt/taken ratio is
/// `e^(2·edge/25)`.
pub const STRIKE_EDGE: [f32; 4] = [0.0000, 0.3207, 0.6191, 0.8092];
/// Units of ours in reach: one, two, three or more.
pub const STRIKE_TARGETS: [f32; 3] = [0.0000, 0.2081, 0.2849];
/// One of them in reach without moving: adjacent, or in range of a shooter.
pub const STRIKE_STILL: [f32; 2] = [0.0000, 1.7895];

/// The target, given a strike: a conditional logit over the units in reach.
/// Weights on, in order: target hit points / 100, expected damage (capped at
/// the target's hit points) / 100, the blow is expected to kill, the target is
/// a ranged or siege unit, distance in tiles, the striker need not move.
pub const TARGET_WEIGHTS: [f32; 6] = [-1.3986, 2.3189, -0.0373, 0.2853, -1.1584, 1.6865];

/// What the strike decision is read from, each computable on the solver's
/// own board.
#[derive(Clone, Copy, Debug, Default)]
pub struct StrikeFeatures {
    pub class: HostClass,
    pub owner: Owner,
    pub hp: i16,
    pub home: bool,
    /// The best strength edge over the units in reach.
    pub edge: f32,
    /// Units of ours in reach, at least one.
    pub targets: usize,
    /// Some unit of ours is in reach without moving.
    pub still: bool,
}

#[inline]
fn hp_band(hp: i16) -> usize {
    if hp <= 33 {
        0
    } else if hp <= 66 {
        1
    } else {
        2
    }
}

#[inline]
fn edge_band(edge: f32) -> usize {
    if edge < -10.0 {
        0
    } else if edge < 0.0 {
        1
    } else if edge < 10.0 {
        2
    } else {
        3
    }
}

/// The chance the host's unit strikes one of ours this turn.
pub fn strike_probability(f: &StrikeFeatures) -> f32 {
    let z = STRIKE_INTERCEPT
        + STRIKE_CLASS[f.class as usize]
        + STRIKE_OWNER[f.owner as usize]
        + STRIKE_HP[hp_band(f.hp)]
        + STRIKE_HOME[f.home as usize]
        + STRIKE_EDGE[edge_band(f.edge)]
        + STRIKE_TARGETS[f.targets.clamp(1, 3) - 1]
        + STRIKE_STILL[f.still as usize];
    1.0 / (1.0 + (-z).exp())
}

/// One unit of ours the striker could hit.
#[derive(Clone, Copy, Debug, Default)]
pub struct TargetFeatures {
    pub hp: i16,
    /// Expected damage of the blow.
    pub dealt: f32,
    pub shooter: bool,
    pub distance: u8,
    /// The striker hits it from where it stands.
    pub still: bool,
}

/// The target's log-weight in the host's choice.
pub fn target_utility(t: &TargetFeatures) -> f32 {
    let hp = t.hp.max(1) as f32;
    let w = TARGET_WEIGHTS;
    w[0] * hp / 100.0
        + w[1] * t.dealt.min(hp) / 100.0
        + if t.dealt >= hp { w[2] } else { 0.0 }
        + if t.shooter { w[3] } else { 0.0 }
        + w[4] * t.distance as f32
        + if t.still { w[5] } else { 0.0 }
}
