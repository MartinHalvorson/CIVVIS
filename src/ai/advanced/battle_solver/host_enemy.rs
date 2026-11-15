//! The host AI's tactical habits, fitted to live games: when a visible enemy
//! strikes, whom it strikes, and whether it closes when it does not.
//!
//! The solver's stock enemy is a guess (greedy on blow value, standing still
//! half the time), and `docs/BATTLE_SOLVER.md` names that guess as what limits
//! the solver: the assumed enemy moved the bench from -100 to +262, and a
//! deeper horizon read worse because the model was wrong. These tables are
//! the host's own behaviour instead, fitted by `tools/fit_host_enemy.py` on
//! 30,869 enemy unit-turns from 116 live Deity games (2026-11-11..12) and
//! scored on games held out of the fit. `docs/HOST_ENEMY_MODEL.md` holds the
//! method, the approximations and the held-out likelihoods; the test below
//! reads that document back, so a refit that changes one and not the other
//! fails.
//!
//! Gene `solver-plays-the-host` makes the solver's enemy play by these
//! tables (`model::Sim::act_as_host`). The tables themselves are plain data
//! and need no gene.

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

/// P(closes on us | it struck nothing), by its hit points (33 or less, more)
/// and where it stood: a unit of ours in reach that it declined, or nothing in
/// reach and the nearest unit of ours 3 or fewer, 4 to 5, or 6 or more tiles
/// off. A unit that does not close holds.
pub const ADVANCE: [[f32; 4]; 2] = [
    [0.1905, 0.1948, 0.2208, 0.1725],
    [0.3708, 0.3115, 0.3640, 0.2926],
];

/// The plain rate at which a visible at-war enemy unit with one of ours in
/// reach struck one of ours on its turn, by owner. A summary for code that
/// prices the risk to a unit and has none of the features above.
pub const STRIKE_RATE_MAJOR: f32 = 0.2047;
pub const STRIKE_RATE_CITY_STATE: f32 = 0.3138;
pub const STRIKE_RATE_BARBARIAN: f32 = 0.2804;

/// [`STRIKE_RATE_MAJOR`] and its kin, by owner.
#[allow(dead_code)] // The summary other code prices unit risk with.
pub fn strike_rate(owner: Owner) -> f32 {
    match owner {
        Owner::Major => STRIKE_RATE_MAJOR,
        Owner::CityState => STRIKE_RATE_CITY_STATE,
        Owner::Barbarian => STRIKE_RATE_BARBARIAN,
    }
}

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

/// Where a unit that struck nothing stood.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Situation {
    /// A unit of ours was in reach and it did not strike.
    Declined,
    /// Nothing in reach; the nearest unit of ours is this many tiles off.
    Gap(u8),
}

/// The chance a unit that struck nothing closes on us rather than holding.
pub fn advance_probability(hp: i16, situation: Situation) -> f32 {
    let column = match situation {
        Situation::Declined => 0,
        Situation::Gap(gap) if gap <= 3 => 1,
        Situation::Gap(gap) if gap <= 5 => 2,
        Situation::Gap(_) => 3,
    };
    ADVANCE[(hp > 33) as usize][column]
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = include_str!("../../../../docs/HOST_ENEMY_MODEL.md");

    /// The number in the second column of the doc's `| name | value |` row.
    fn doc_value(name: &str) -> f32 {
        let row = DOC
            .lines()
            .find(|line| line.starts_with(&format!("| {name} |")))
            .unwrap_or_else(|| panic!("docs/HOST_ENEMY_MODEL.md has no row `{name}`"));
        let cell = row.split('|').nth(2).unwrap().trim();
        cell.split_whitespace().next().unwrap().parse().unwrap()
    }

    fn doc_row(name: &str) -> Vec<f32> {
        let row = DOC
            .lines()
            .find(|line| line.starts_with(&format!("| {name} |")))
            .unwrap_or_else(|| panic!("docs/HOST_ENEMY_MODEL.md has no row `{name}`"));
        row.split('|')
            .skip(2)
            .filter(|cell| !cell.trim().is_empty())
            .map(|cell| cell.split_whitespace().next().unwrap().parse().unwrap())
            .collect()
    }

    fn close(a: f32, b: f32, what: &str) {
        assert!((a - b).abs() < 5e-5, "{what}: code {a} doc {b}");
    }

    #[test]
    fn the_tables_are_the_documents_tables() {
        close(STRIKE_INTERCEPT, doc_value("intercept"), "intercept");
        let factors: [(&str, &[&str], &[f32]); 7] = [
            (
                "class",
                &["melee", "ranged", "siege", "cavalry"],
                &STRIKE_CLASS,
            ),
            (
                "owner",
                &["major", "city_state", "barbarian"],
                &STRIKE_OWNER,
            ),
            ("hp", &["hp<=33", "hp34-66", "hp67-100"], &STRIKE_HP),
            ("home", &["away", "home"], &STRIKE_HOME),
            (
                "edge",
                &["edge<-10", "edge-10..-1", "edge0..9", "edge>=10"],
                &STRIKE_EDGE,
            ),
            (
                "targets",
                &["1 target", "2 targets", "3+ targets"],
                &STRIKE_TARGETS,
            ),
            ("still", &["must move", "no move needed"], &STRIKE_STILL),
        ];
        for (name, levels, table) in factors {
            for (level, value) in levels.iter().zip(table) {
                let key = format!("{name}={level}");
                close(*value, doc_value(&key), &key);
            }
        }
        for (k, name) in ["hp", "dealt", "kills", "shooter", "distance", "no move"]
            .iter()
            .enumerate()
        {
            close(TARGET_WEIGHTS[k], doc_value(name), name);
        }
        for (k, band) in ["hp<=33", "hp>33"].iter().enumerate() {
            let row = doc_row(band);
            assert_eq!(row.len(), 4, "advance row {band}: {row:?}");
            for (c, value) in row.iter().enumerate() {
                close(ADVANCE[k][c], *value, band);
            }
        }
        for (owner, value) in [
            ("major", STRIKE_RATE_MAJOR),
            ("city_state", STRIKE_RATE_CITY_STATE),
            ("barbarian", STRIKE_RATE_BARBARIAN),
        ] {
            let row = doc_row(owner);
            close(value, *row.last().unwrap(), owner);
        }
    }

    #[test]
    fn the_host_strikes_more_when_it_need_not_move_and_trades_well() {
        let base = StrikeFeatures {
            hp: 100,
            edge: 12.0,
            targets: 1,
            ..Default::default()
        };
        let still = StrikeFeatures {
            still: true,
            ..base
        };
        let outmatched = StrikeFeatures {
            edge: -15.0,
            ..still
        };
        let wounded = StrikeFeatures { hp: 20, ..still };
        let p = strike_probability(&still);
        assert!(p > strike_probability(&base));
        assert!(p > strike_probability(&outmatched));
        assert!(p > strike_probability(&wounded));
        // The host never strikes as surely as a greedy model: no cell of the
        // fit reaches four in five.
        let best = StrikeFeatures {
            class: HostClass::Ranged,
            owner: Owner::Barbarian,
            home: true,
            targets: 3,
            ..still
        };
        assert!(strike_probability(&best) < 0.8);
        assert!(strike_rate(Owner::Major) < strike_rate(Owner::Barbarian));
    }

    #[test]
    fn the_target_is_the_one_it_hurts_from_where_it_stands() {
        let near = TargetFeatures {
            hp: 100,
            dealt: 30.0,
            distance: 1,
            still: true,
            ..Default::default()
        };
        let far = TargetFeatures {
            distance: 2,
            still: false,
            ..near
        };
        let wounded = TargetFeatures { hp: 40, ..near };
        assert!(target_utility(&near) > target_utility(&far));
        assert!(target_utility(&wounded) > target_utility(&near));
        assert!(
            advance_probability(20, Situation::Declined)
                < advance_probability(90, Situation::Declined)
        );
        assert_eq!(advance_probability(90, Situation::Gap(7)), ADVANCE[1][3]);
    }

    /// The gene's enemy on the solver's own board, against the tables.
    mod plays {
        use super::super::super::model::{
            Posture, Sim, State, Unit, Weights, F_CAPTURES, F_MELEE, F_ZOC, MP, OURS, THEIRS,
        };
        use super::super::super::search::tests::{plain_board, set_strengths};
        use super::super::*;

        fn soldier(tile: u16, side: u8, hp: i16) -> Unit {
            Unit {
                tile,
                hp,
                mp: 2 * MP,
                max_mp: 2 * MP,
                value: 80.0,
                side,
                attacks: 1,
                max_attacks: 1,
                fort: 0,
                flags: F_MELEE | F_ZOC | F_CAPTURES,
                range: 1,
                class: 0,
                acted: false,
            }
        }

        /// Plays the enemy's turn on `seeds` seeds and returns the end
        /// states.
        fn enemy_turns(units: &[Unit], w: i32, host: bool, seeds: u64) -> Vec<Vec<Unit>> {
            let mut b = plain_board(w, w, units, 0);
            for i in 0..units.len() {
                for j in 0..units.len() {
                    set_strengths(&mut b, i, j, 30.0, 30.0);
                }
            }
            let s = State::new(&b, units.to_vec(), vec![]);
            let weights = Weights {
                host_enemy: host,
                ..Weights::default()
            };
            let mut sim = Sim::new(
                &b,
                &s,
                weights,
                super::super::super::model::Objective::Field,
            );
            (0..seeds)
                .map(|seed| {
                    sim.reset(&s, seed);
                    sim.begin_side(THEIRS);
                    sim.side_turn(THEIRS, Posture::Press);
                    sim.s.units.clone()
                })
                .collect()
        }

        #[test]
        fn it_strikes_at_the_fitted_rate_not_half_the_time() {
            // One soldier of ours beside one of theirs, even strengths.
            let units = [soldier(55, OURS, 100), soldier(56, THEIRS, 100)];
            let rate = |host: bool| {
                let runs = enemy_turns(&units, 10, host, 2000);
                runs.iter().filter(|u| u[0].hp < 100).count() as f32 / runs.len() as f32
            };
            let expected = strike_probability(&StrikeFeatures {
                class: HostClass::Melee,
                owner: Owner::Major,
                hp: 100,
                home: false,
                edge: 0.0,
                targets: 1,
                still: true,
            });
            let (host, stock) = (rate(true), rate(false));
            assert!(
                (host - expected).abs() < 0.04,
                "host {host} expected {expected}"
            );
            assert!((stock - 0.5).abs() < 0.05, "stock {stock}");
        }

        #[test]
        fn it_takes_the_unit_in_front_of_it_not_the_best_trade() {
            // A healthy soldier beside it, a wounded one a step away. The
            // greedy enemy walks to the wounded one; the host hits what it
            // stands beside.
            let units = [
                soldier(77, OURS, 100),
                soldier(80, OURS, 40),
                soldier(78, THEIRS, 100),
            ];
            let share_beside = |host: bool| {
                let runs = enemy_turns(&units, 12, host, 2000);
                let struck: Vec<_> = runs
                    .iter()
                    .filter(|u| u[0].hp < 100 || u[1].hp < 40)
                    .collect();
                assert!(struck.len() > 200, "{host}: {} strikes", struck.len());
                struck.iter().filter(|u| u[0].hp < 100).count() as f32 / struck.len() as f32
            };
            let (host, stock) = (share_beside(true), share_beside(false));
            assert!(host > 0.7, "host {host}");
            assert!(stock < 0.1, "stock {stock}");
        }

        #[test]
        fn with_nothing_in_reach_it_closes_at_the_fitted_rate() {
            // Six tiles apart: nothing to strike this turn.
            let units = [soldier(62, OURS, 100), soldier(68, THEIRS, 100)];
            let rate = |host: bool| {
                let runs = enemy_turns(&units, 12, host, 2000);
                runs.iter().filter(|u| u[1].tile != 68).count() as f32 / runs.len() as f32
            };
            let expected = advance_probability(100, Situation::Gap(6));
            let (host, stock) = (rate(true), rate(false));
            assert!(
                (host - expected).abs() < 0.04,
                "host {host} expected {expected}"
            );
            assert!((stock - 0.5).abs() < 0.05, "stock {stock}");
        }
    }
}
