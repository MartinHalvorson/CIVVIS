//! The per-player combat ledger: the units each player killed and lost in
//! combat, and the civilians it took and lost, counted at the line that
//! removes them.
//!
//! `Player::counters` has carried `kills` for a long time, but nothing counted
//! a unit LOST, so a seat record could set the cities an army took beside the
//! cities it lost and could not do the same for the army itself. Without that
//! a whole-game null cannot be split into "the tactics did not change the
//! exchange" and "the exchange did not change the game"
//! (`docs/BATTLE_SOLVER.md`).
//!
//! **What counts.** A unit is tallied once, by the code that removes it (or,
//! for a captured Settler or Builder, hands it over) because of an enemy's
//! blow: a melee or ranged kill either way, a city, encampment or district
//! strike, an air strike or interception, a support unit's priority-target
//! kill, the garrison of a taken encampment, the units standing in a city
//! taken by conquest, civilians captured or destroyed by a unit entering their
//! tile, and a nuclear blast. Religious units are not counted: theological
//! combat and condemnation are their own war and keep their own counters. Nor
//! is a unit removed by elimination, maintenance, fallout, a disaster, a
//! loyalty flip, a corps merge or a disband — none of those is a blow.
//!
//! **Who killed it.** The seat whose unit, city or district struck. A loss is
//! split by who that seat is: a major civilization, a city-state, or the
//! barbarians. Free Cities also carry `is_barbarian`, so a unit they take is
//! in `units_lost` and in none of the three.
//!
//! **Military only for the headline.** `units_killed` and `units_lost` count
//! units of class `military`; `civilians_taken` and `civilians_lost` count
//! `civilian` and `support` units (Settlers, Builders, Traders, Battering
//! Rams, Medics…), captured or destroyed.
//!
//! It observes only: nothing reads the ledger to decide anything. It is saved
//! with the player and left out of a save while it is empty, so a save from a
//! game nobody fought in is byte for byte what it was.
//!
//! **The edge ledger** (`EdgeLedger`, 2026-11-13) rides on the same record:
//! every unit-on-unit blow, binned by how far apart the two sides stood. The
//! **base gap** is the striker's attack strength (Bombard, else Ranged, else
//! Combat) minus the target's Combat strength, both off the ruleset; the
//! **total gap** is the two strengths the blow was resolved with
//! (`melee_exchange_strengths`, `ranged_strike_strengths`: every modifier and
//! the wound penalty in). A blow is booked on both seats: as taken, by the
//! striker's side, and as struck, by the target's side. Only blows between
//! two military units that are not aircraft count, and only between seats
//! that are a major, a city-state or the barbarians; a city, district or
//! encampment strike, an air strike and a Free City's blow count nowhere.
//! `tools/civ6_edge_ledger.py` reads the live host's blows on the same
//! definitions (`docs/EDGE_LEDGER.md`).

use super::*;

/// Units one player killed and lost in combat, and civilians it took and
/// lost. See the module header for exactly what counts.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CombatLedger {
    /// Enemy military units this player's units, cities and districts
    /// removed.
    #[serde(default)]
    pub units_killed: u32,
    /// This player's military units an enemy removed.
    #[serde(default)]
    pub units_lost: u32,
    /// `units_lost` to a major civilization.
    #[serde(default)]
    pub units_lost_to_majors: u32,
    /// `units_lost` to a city-state.
    #[serde(default)]
    pub units_lost_to_city_states: u32,
    /// `units_lost` to the barbarian seat (not the Free Cities).
    #[serde(default)]
    pub units_lost_to_barbarians: u32,
    /// Enemy civilian and support units this player captured or destroyed.
    #[serde(default)]
    pub civilians_taken: u32,
    /// This player's civilian and support units an enemy captured or
    /// destroyed.
    #[serde(default)]
    pub civilians_lost: u32,
    /// Every unit-on-unit blow this player's units took or struck, by gap.
    /// See the module header and [`EdgeLedger`].
    #[serde(default, skip_serializing_if = "EdgeLedger::is_empty")]
    pub edge: EdgeLedger,
}

impl CombatLedger {
    /// Nothing counted yet: such a ledger is left out of a save.
    pub fn is_empty(&self) -> bool {
        *self == CombatLedger::default()
    }
}

/// The gap bins, low to high: −10 or worse, −9 to −4, −3 to +3, +4 to +9,
/// +10 or better. `tools/civ6_edge_ledger.py` bins the live blows the same
/// way.
pub const EDGE_BINS: [&str; 5] = ["<=-10", "-9..-4", "-3..+3", "+4..+9", ">=+10"];
/// The other seat of a blow: a major civilization, a city-state, or the
/// barbarians (never the Free Cities).
pub const EDGE_SIDES: [&str; 3] = ["major", "city_state", "barbarian"];
/// The turn buckets of the majors' mean gap: 0-49, 50-99, 100-149, 150+.
pub const EDGE_TURN_BUCKETS: [&str; 4] = ["0-49", "50-99", "100-149", "150+"];

/// The bin of one gap, rounded to a whole strength point first.
pub fn edge_bin(gap: f64) -> usize {
    match gap.round() as i64 {
        i64::MIN..=-10 => 0,
        -9..=-4 => 1,
        -3..=3 => 2,
        4..=9 => 3,
        _ => 4,
    }
}

/// The turn bucket of a turn: 0-49, 50-99, 100-149, 150 and on.
pub fn edge_turn_bucket(turn: u32) -> usize {
    (turn / 50).min(3) as usize
}

/// The turn at Online speed that the same progress lands on: a Standard
/// turn 100 is Online turn 50, because Standard pays twice Online's costs.
pub fn online_equivalent_turn(turn: u32, speed: GameSpeed) -> u32 {
    (u64::from(turn) * 50 / u64::from(speed.cost_percent().max(1))) as u32
}

/// The strength a unit strikes with, off the ruleset alone: Bombard, else
/// Ranged, else Combat. The live ledger reads `data/units.json` the same way.
pub fn edge_attack_strength(spec: &crate::rules::UnitSpec) -> f64 {
    [spec.bombard_strength, spec.ranged_strength, spec.strength]
        .into_iter()
        .find(|strength| *strength > 0.0)
        .unwrap_or(0.0)
}

/// The blows that fell in one gap bin. Written as the array `[blows,
/// damage_to_defender, damage_to_attacker, defender_killed,
/// attacker_killed]`, the order the live ledger keeps.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(from = "[u32; 5]", into = "[u32; 5]")]
pub struct EdgeCell {
    pub blows: u32,
    /// Hit points the struck unit lost, at most what it had.
    pub damage_to_defender: u32,
    /// Hit points the striker lost to the return blow; zero for a shot.
    pub damage_to_attacker: u32,
    pub defender_killed: u32,
    pub attacker_killed: u32,
}

impl From<[u32; 5]> for EdgeCell {
    fn from(cell: [u32; 5]) -> Self {
        EdgeCell {
            blows: cell[0],
            damage_to_defender: cell[1],
            damage_to_attacker: cell[2],
            defender_killed: cell[3],
            attacker_killed: cell[4],
        }
    }
}

impl From<EdgeCell> for [u32; 5] {
    fn from(cell: EdgeCell) -> Self {
        [
            cell.blows,
            cell.damage_to_defender,
            cell.damage_to_attacker,
            cell.defender_killed,
            cell.attacker_killed,
        ]
    }
}

impl EdgeCell {
    pub fn absorb(&mut self, other: &EdgeCell) {
        self.blows += other.blows;
        self.damage_to_defender += other.damage_to_defender;
        self.damage_to_attacker += other.damage_to_attacker;
        self.defender_killed += other.defender_killed;
        self.attacker_killed += other.attacker_killed;
    }
}

/// One side's gap table: `[side][bin]`, sides as [`EDGE_SIDES`], bins as
/// [`EDGE_BINS`].
pub type EdgeTable = [[EdgeCell; 5]; 3];

/// Every unit-on-unit blow one player's units took and struck, by gap. See
/// the module header for what a gap is and which blows count.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EdgeLedger {
    /// Blows this player's units took, by the striker's side and base gap.
    #[serde(default)]
    pub taken_base: EdgeTable,
    /// The same blows by total gap.
    #[serde(default)]
    pub taken_total: EdgeTable,
    /// Blows this player's units struck, by the target's side and base gap.
    #[serde(default)]
    pub struck_base: EdgeTable,
    /// The same blows by total gap.
    #[serde(default)]
    pub struck_total: EdgeTable,
    /// A major's blows on this player's units, `[blows, sum of base gaps]`
    /// per [`EDGE_TURN_BUCKETS`] of the raw turn.
    #[serde(default)]
    pub major_gap_by_turn: [[i64; 2]; 4],
    /// The same, by [`online_equivalent_turn`].
    #[serde(default)]
    pub major_gap_by_online_turn: [[i64; 2]; 4],
}

impl EdgeLedger {
    pub fn is_empty(&self) -> bool {
        *self == EdgeLedger::default()
    }
}

/// One unit-on-unit blow as the resolver saw it: both units as they stood
/// before it, the two strengths it was resolved with, the rolled damage and
/// who died.
pub(crate) struct EdgeBlow<'a> {
    pub attacker: &'a Unit,
    pub defender: &'a Unit,
    pub attacker_strength: f64,
    pub defender_strength: f64,
    pub damage_to_defender: i32,
    pub damage_to_attacker: i32,
    pub defender_killed: bool,
    pub attacker_killed: bool,
}

impl Game {
    /// The combat ledger of one player; empty for a seat that does not exist.
    pub fn combat_ledger(&self, pid: usize) -> CombatLedger {
        self.players
            .get(pid)
            .map(|player| player.combat_ledger)
            .unwrap_or_default()
    }

    /// Tally the unit `uid`, which `killer`'s blow is about to remove. Call
    /// it before the removal: afterwards there is nothing left to read.
    pub(crate) fn tally_combat_death(&mut self, killer: usize, uid: u32) {
        if let Some(unit) = self.units.get(&uid) {
            let (owner, kind) = (unit.owner, unit.kind);
            self.tally_combat_removal(killer, owner, kind);
        }
    }

    /// Tally one unit of `owner` and `kind` that `killer` removed or took.
    /// A blow on one's own unit, and a religious unit, count nowhere.
    pub(crate) fn tally_combat_removal(&mut self, killer: usize, owner: usize, kind: Name) {
        if killer == owner || killer >= self.players.len() || owner >= self.players.len() {
            return;
        }
        let Some(spec) = self.rules.units.get(&kind) else {
            return;
        };
        let military = match spec.class.as_str() {
            "military" => true,
            "civilian" | "support" => false,
            _ => return,
        };
        let (barbarian, minor) = {
            let seat = &self.players[killer];
            (
                seat.is_barbarian && !seat.is_free_city,
                seat.is_minor && !seat.is_barbarian,
            )
        };
        let major = !self.players[killer].is_minor && !self.players[killer].is_barbarian;
        if military {
            self.players[killer].combat_ledger.units_killed += 1;
            let lost = &mut self.players[owner].combat_ledger;
            lost.units_lost += 1;
            if major {
                lost.units_lost_to_majors += 1;
            } else if minor {
                lost.units_lost_to_city_states += 1;
            } else if barbarian {
                lost.units_lost_to_barbarians += 1;
            }
        } else {
            self.players[killer].combat_ledger.civilians_taken += 1;
            self.players[owner].combat_ledger.civilians_lost += 1;
        }
    }

    /// Which [`EDGE_SIDES`] a seat sits on; `None` for the Free Cities and a
    /// seat that does not exist.
    fn edge_side(&self, pid: usize) -> Option<usize> {
        let seat = self.players.get(pid)?;
        match (seat.is_minor, seat.is_barbarian, seat.is_free_city) {
            (false, false, _) => Some(0),
            (true, false, _) => Some(1),
            (_, true, false) => Some(2),
            _ => None,
        }
    }

    /// Book one unit-on-unit blow on both seats. Reads the ruleset and the
    /// blow it is handed, and writes only the two ledgers.
    pub(crate) fn tally_edge_blow(&mut self, blow: EdgeBlow<'_>) {
        let (striker, target) = (blow.attacker.owner, blow.defender.owner);
        if striker == target {
            return;
        }
        let (Some(striker_side), Some(target_side)) =
            (self.edge_side(striker), self.edge_side(target))
        else {
            return;
        };
        let (Some(attacker), Some(defender)) = (
            self.rules.units.get(&blow.attacker.kind),
            self.rules.units.get(&blow.defender.kind),
        ) else {
            return;
        };
        let fighter = |spec: &crate::rules::UnitSpec| {
            spec.class == "military" && spec.domain.as_deref() != Some("air")
        };
        if !fighter(attacker) || !fighter(defender) {
            return;
        }
        let base = edge_attack_strength(attacker) - defender.strength;
        let total = blow.attacker_strength - blow.defender_strength;
        let lost = |rolled: i32, hp: i32| rolled.max(0).min(hp.max(0)) as u32;
        let cell = EdgeCell {
            blows: 1,
            damage_to_defender: lost(blow.damage_to_defender, blow.defender.hp),
            damage_to_attacker: lost(blow.damage_to_attacker, blow.attacker.hp),
            defender_killed: u32::from(blow.defender_killed),
            attacker_killed: u32::from(blow.attacker_killed),
        };
        let (base_bin, total_bin) = (edge_bin(base), edge_bin(total));
        let (turn, speed) = (self.turn, self.game_speed);
        let taken = &mut self.players[target].combat_ledger.edge;
        taken.taken_base[striker_side][base_bin].absorb(&cell);
        taken.taken_total[striker_side][total_bin].absorb(&cell);
        if striker_side == 0 {
            let gap = base.round() as i64;
            let raw = &mut taken.major_gap_by_turn[edge_turn_bucket(turn)];
            raw[0] += 1;
            raw[1] += gap;
            let online = &mut taken.major_gap_by_online_turn
                [edge_turn_bucket(online_equivalent_turn(turn, speed))];
            online[0] += 1;
            online[1] += gap;
        }
        let struck = &mut self.players[striker].combat_ledger.edge;
        struck.struck_base[target_side][base_bin].absorb(&cell);
        struck.struck_total[target_side][total_bin].absorb(&cell);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{run_game, BasicAi};

    /// Two majors at war on open grassland, each with one city, the barbarian
    /// seat present and nothing else on the board.
    fn board() -> Game {
        let mut g = Game::new_full(2, 40, 24, 3626, 400, 0, true);
        for uid in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(uid);
        }
        g.barb_camps.clear();
        g.barb_naval_camps.clear();
        for tile in g.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
        }
        g.found_city_for(0, (6, 12), None);
        g.found_city_for(1, (24, 12), None);
        g.record_contact(0, 1);
        g.at_war.insert((0, 1));
        g.turn = 100;
        g.current = 0;
        g
    }

    /// The headline counts of every seat; the edge ledger has its own tests.
    fn ledgers(g: &Game) -> Vec<CombatLedger> {
        (0..g.players.len())
            .map(|pid| CombatLedger {
                edge: EdgeLedger::default(),
                ..g.combat_ledger(pid)
            })
            .collect()
    }

    /// Charge one military loss to `ledger`, split by the kind of seat that
    /// struck it: the module header's rule, written out a second time.
    fn charge_loss(g: &Game, ledger: &mut CombatLedger, killer: usize) {
        ledger.units_lost += 1;
        let seat = &g.players[killer];
        if seat.is_barbarian && !seat.is_free_city {
            ledger.units_lost_to_barbarians += 1;
        } else if seat.is_minor && !seat.is_barbarian {
            ledger.units_lost_to_city_states += 1;
        } else if !seat.is_minor && !seat.is_barbarian {
            ledger.units_lost_to_majors += 1;
        }
    }

    /// Exactly these two seats moved, by exactly one military unit.
    fn one_unit_changed_hands(g: &Game, before: &[CombatLedger], killer: usize, victim: usize) {
        let after = ledgers(g);
        for pid in 0..g.players.len() {
            let mut expected = before[pid];
            if pid == killer {
                expected.units_killed += 1;
            }
            if pid == victim {
                charge_loss(g, &mut expected, killer);
            }
            assert_eq!(after[pid], expected, "seat {pid}: {after:?}");
        }
    }

    #[test]
    fn a_melee_kill_counts_once_on_each_side() {
        let mut g = board();
        let warrior = g.spawn_test_unit("warrior", 0, (10, 12));
        let victim = g.spawn_test_unit("warrior", 1, (11, 12));
        g.units.get_mut(&victim).unwrap().hp = 1;
        let before = ledgers(&g);
        g.apply(
            0,
            &Action::Attack {
                unit: warrior,
                target: (11, 12),
            },
        )
        .unwrap();
        assert!(!g.units.contains_key(&victim));
        assert!(g.units.contains_key(&warrior));
        one_unit_changed_hands(&g, &before, 0, 1);
        assert_eq!(g.players[0].counters.get("kills").copied(), Some(1));
    }

    #[test]
    fn a_melee_attacker_that_dies_is_the_defenders_kill() {
        let mut g = board();
        let warrior = g.spawn_test_unit("warrior", 0, (10, 12));
        g.units.get_mut(&warrior).unwrap().hp = 1;
        let defender = g.spawn_test_unit("warrior", 1, (11, 12));
        let before = ledgers(&g);
        g.apply(
            0,
            &Action::Attack {
                unit: warrior,
                target: (11, 12),
            },
        )
        .unwrap();
        assert!(!g.units.contains_key(&warrior));
        assert!(g.units.contains_key(&defender));
        one_unit_changed_hands(&g, &before, 1, 0);
    }

    #[test]
    fn a_ranged_kill_counts_once_on_each_side() {
        let mut g = board();
        let archer = g.spawn_test_unit("archer", 0, (10, 12));
        let victim = g.spawn_test_unit("warrior", 1, (11, 12));
        g.units.get_mut(&victim).unwrap().hp = 1;
        let before = ledgers(&g);
        g.apply(
            0,
            &Action::Ranged {
                unit: archer,
                target: (11, 12),
            },
        )
        .unwrap();
        assert!(!g.units.contains_key(&victim));
        one_unit_changed_hands(&g, &before, 0, 1);
    }

    #[test]
    fn a_city_strike_kill_counts_once_on_each_side() {
        let mut g = board();
        let cid = g.city_at((6, 12)).expect("our city");
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .push(crate::name!("walls"));
        g.cities.get_mut(&cid).unwrap().wall_hp = 100;
        let victim = g.spawn_test_unit("warrior", 1, (7, 12));
        g.units.get_mut(&victim).unwrap().hp = 1;
        let before = ledgers(&g);
        g.apply(
            0,
            &Action::CityStrike {
                city: cid,
                target: (7, 12),
            },
        )
        .unwrap();
        assert!(!g.units.contains_key(&victim));
        one_unit_changed_hands(&g, &before, 0, 1);
    }

    #[test]
    fn a_barbarian_kill_is_a_loss_to_the_barbarians() {
        let mut g = board();
        let barbarian = g.barb_pid.expect("the board seats barbarians");
        let raider = g.spawn_test_unit("warrior", barbarian, (10, 12));
        let victim = g.spawn_test_unit("warrior", 0, (11, 12));
        g.units.get_mut(&victim).unwrap().hp = 1;
        g.current = barbarian;
        let before = ledgers(&g);
        g.apply(
            barbarian,
            &Action::Attack {
                unit: raider,
                target: (11, 12),
            },
        )
        .unwrap();
        assert!(!g.units.contains_key(&victim));
        one_unit_changed_hands(&g, &before, barbarian, 0);
        assert_eq!(g.combat_ledger(0).units_lost_to_barbarians, 1);
        assert_eq!(g.players[0].counters.get("lost_to_barbarians"), Some(&1));
    }

    #[test]
    fn a_captured_builder_is_a_civilian_taken_and_lost() {
        let mut g = board();
        let warrior = g.spawn_test_unit("warrior", 0, (10, 12));
        let builder = g.spawn_test_unit("builder", 1, (11, 12));
        g.apply(
            0,
            &Action::Move {
                unit: warrior,
                to: (11, 12),
            },
        )
        .unwrap();
        assert_eq!(g.units[&builder].owner, 0, "the Builder is captured");
        assert_eq!(g.combat_ledger(0).civilians_taken, 1);
        assert_eq!(g.combat_ledger(1).civilians_lost, 1);
        assert_eq!(g.combat_ledger(0).units_killed, 0);
        assert_eq!(g.combat_ledger(1).units_lost, 0);
    }

    /// An empty ledger is not written, so a save from a game without a fight
    /// is what it was; a counted one round-trips.
    #[test]
    fn the_ledger_is_saved_only_once_it_counts_something() {
        let mut g = board();
        let quiet = serde_json::to_string(&g.players[0]).expect("a player serializes");
        assert!(!quiet.contains("combat_ledger"), "{quiet}");
        g.players[0].combat_ledger.units_killed = 3;
        g.players[0].combat_ledger.civilians_lost = 1;
        let headline = serde_json::to_string(&g.players[0]).expect("a player serializes");
        assert!(!headline.contains("\"edge\""), "{headline}");
        g.players[0].combat_ledger.edge.taken_base[0][4] = EdgeCell::from([2, 90, 10, 1, 0]);
        let player = serde_json::to_string(&g.players[0]).expect("a player serializes");
        assert!(
            player.contains("[2,90,10,1,0]"),
            "a cell is written as one array: {player}"
        );
        let text = serde_json::to_string(&g).expect("a game serializes");
        let back: Game = serde_json::from_str(&text).expect("a game parses");
        assert_eq!(back.combat_ledger(0), g.combat_ledger(0));
        assert_eq!(back.combat_ledger(1), CombatLedger::default());
    }

    #[test]
    fn the_gap_bins_and_turn_buckets_match_the_live_ledgers() {
        let bins: Vec<usize> = [
            -25.0, -10.0, -9.6, -4.0, -3.0, 0.0, 3.4, 4.0, 9.0, 9.5, 10.0, 60.0,
        ]
        .into_iter()
        .map(edge_bin)
        .collect();
        assert_eq!(bins, [0, 0, 0, 1, 2, 2, 2, 3, 3, 4, 4, 4]);
        assert_eq!(
            [0, 49, 50, 99, 100, 149, 150, 400].map(edge_turn_bucket),
            [0, 0, 1, 1, 2, 2, 3, 3]
        );
        assert_eq!(online_equivalent_turn(100, GameSpeed::Online), 100);
        assert_eq!(online_equivalent_turn(100, GameSpeed::Standard), 50);
        assert_eq!(online_equivalent_turn(300, GameSpeed::Marathon), 50);
        let rules = crate::rules::Rules::shared();
        assert_eq!(edge_attack_strength(&rules.units["catapult"]), 35.0);
        assert_eq!(edge_attack_strength(&rules.units["archer"]), 25.0);
        assert_eq!(edge_attack_strength(&rules.units["warrior"]), 20.0);
    }

    /// Every unit-on-unit blow `seat` struck or took, summed over the bins.
    fn blows(edge: &EdgeLedger) -> u32 {
        [&edge.taken_base, &edge.struck_base]
            .into_iter()
            .flatten()
            .flatten()
            .map(|cell| cell.blows)
            .sum()
    }

    #[test]
    fn a_melee_blow_is_booked_on_both_seats_by_base_and_total_gap() {
        let mut g = board();
        let swordsman = g.spawn_test_unit("swordsman", 0, (10, 12));
        let warrior = g.spawn_test_unit("warrior", 1, (11, 12));
        let (att, def) = g
            .melee_exchange_strengths(swordsman, warrior)
            .expect("both units stand");
        g.apply(
            0,
            &Action::Attack {
                unit: swordsman,
                target: (11, 12),
            },
        )
        .unwrap();
        let (ours, theirs) = (g.combat_ledger(0).edge, g.combat_ledger(1).edge);
        // Swordsman 35 against Warrior 20: a base gap of +15.
        let (base, total) = (edge_bin(15.0), edge_bin(att - def));
        let cell = ours.struck_base[0][base];
        assert_eq!(cell.blows, 1);
        let warrior_hp = g.units.get(&warrior).map_or(0, |unit| unit.hp.max(0));
        assert_eq!(cell.damage_to_defender, (100 - warrior_hp) as u32);
        assert_eq!(
            cell.damage_to_attacker,
            (100 - g.units[&swordsman].hp) as u32
        );
        assert_eq!(cell.defender_killed, u32::from(warrior_hp == 0));
        assert_eq!(ours.struck_total[0][total], cell);
        assert_eq!(theirs.taken_base[0][base], cell);
        assert_eq!(theirs.taken_total[0][total], cell);
        assert_eq!(blows(&ours), 1);
        assert_eq!(blows(&theirs), 1);
        assert_eq!(theirs.major_gap_by_turn[edge_turn_bucket(100)], [1, 15]);
        let online = edge_turn_bucket(online_equivalent_turn(100, g.game_speed));
        assert_eq!(theirs.major_gap_by_online_turn[online], [1, 15]);
        assert_eq!(ours.major_gap_by_turn, [[0, 0]; 4], "we were not struck");
    }

    #[test]
    fn a_shot_reads_the_ranged_strength_and_takes_no_return_blow() {
        let mut g = board();
        let archer = g.spawn_test_unit("archer", 0, (10, 12));
        let warrior = g.spawn_test_unit("warrior", 1, (11, 12));
        let (att, def) = g
            .ranged_strike_strengths(archer, warrior, (11, 12))
            .expect("both units stand");
        g.apply(
            0,
            &Action::Ranged {
                unit: archer,
                target: (11, 12),
            },
        )
        .unwrap();
        // The Archer's Ranged 25 against the Warrior's Combat 20: +5.
        let cell = g.combat_ledger(0).edge.struck_base[0][edge_bin(5.0)];
        assert_eq!(cell.blows, 1);
        assert_eq!(cell.damage_to_attacker, 0);
        assert_eq!(cell.attacker_killed, 0);
        assert_eq!(
            g.combat_ledger(0).edge.struck_total[0][edge_bin(att - def)],
            cell
        );
        assert_eq!(g.combat_ledger(1).edge.taken_base[0][edge_bin(5.0)], cell);
    }

    #[test]
    fn a_barbarian_blow_is_on_the_barbarian_side_and_out_of_the_majors_gap() {
        let mut g = board();
        let barbarian = g.barb_pid.expect("the board seats barbarians");
        let raider = g.spawn_test_unit("warrior", barbarian, (10, 12));
        g.spawn_test_unit("warrior", 0, (11, 12));
        g.current = barbarian;
        g.apply(
            barbarian,
            &Action::Attack {
                unit: raider,
                target: (11, 12),
            },
        )
        .unwrap();
        let ours = g.combat_ledger(0).edge;
        assert_eq!(ours.taken_base[2][edge_bin(0.0)].blows, 1);
        assert_eq!(blows(&ours), 1);
        assert_eq!(ours.major_gap_by_turn, [[0, 0]; 4]);
        let theirs = g.combat_ledger(barbarian).edge;
        assert_eq!(theirs.struck_base[0][edge_bin(0.0)].blows, 1);
    }

    /// Where a blow lands: the owner of the city or district there, else of
    /// the military unit there that the striker does not own.
    fn target_owner(g: &Game, seat: usize, target: Pos) -> Option<usize> {
        g.city_at(target)
            .or_else(|| g.defending_district_at(target))
            .map(|cid| g.cities[&cid].owner)
            .or_else(|| {
                g.unit_ids_at(target)
                    .iter()
                    .map(|id| &g.units[id])
                    .find(|unit| unit.owner != seat && g.rules.units[unit.kind].class == "military")
                    .map(|unit| unit.owner)
            })
    }

    /// ⭐ A whole game, recounted without the engine's hooks. Replays the
    /// action log through a plain `apply` loop and, around every action,
    /// reads which units disappeared or changed hands:
    ///
    /// * an enemy military unit gone during a strike is the striker's kill;
    /// * the striker's own military unit gone during its strike died
    ///   attacking, and is the kill of whoever held the target;
    /// * an enemy civilian or support unit gone or changing hands during a
    ///   move or a strike is a civilian taken;
    /// * a seat eliminated by the action loses everything it had left, and
    ///   only what stood on the target tile was struck;
    /// * nothing that disappears while a turn ends (maintenance, fallout,
    ///   levies going home) is combat.
    ///
    /// The ledger the game counted must agree, seat for seat.
    #[test]
    fn a_whole_game_ledger_matches_a_recount_from_its_action_log() {
        let params = || Game::new(3, 24, 16, 9, 100, 2);
        let mut g = params();
        let mut ais = BasicAi::fleet(&g);
        run_game(&mut g, &mut ais);
        let seats = g.players.len();
        let mut recount = vec![CombatLedger::default(); seats];
        let mut r = params();
        r.set_fog_memory(false);
        r.set_war_ledger(false);
        let rules = std::sync::Arc::clone(&r.rules);
        for (index, (seat, action)) in g.log.iter().enumerate() {
            let seat = *seat;
            let target = match action {
                Action::Attack { target, .. }
                | Action::Ranged { target, .. }
                | Action::AirStrike { target, .. }
                | Action::PriorityTarget { target, .. }
                | Action::CityStrike { target, .. }
                | Action::EncampmentStrike { target, .. }
                | Action::DistrictStrike { target, .. }
                | Action::WmdStrike { target, .. } => Some(*target),
                _ => None,
            };
            let struck = target.and_then(|at| target_owner(&r, seat, at));
            // Only a unit that walks in or strikes takes a civilian.
            let reaches =
                target.is_some() || matches!(action, Action::Move { .. } | Action::MoveTo { .. });
            let before: Vec<(u32, usize, Name, Pos)> = r
                .units
                .values()
                .map(|unit| (unit.id, unit.owner, unit.kind, unit.pos))
                .collect();
            let alive: Vec<bool> = r.players.iter().map(|player| player.alive).collect();
            r.apply(seat, action)
                .unwrap_or_else(|e| panic!("logged action {index} failed on replay: {e}"));
            if matches!(action, Action::EndTurn) {
                continue;
            }
            for (id, owner, kind, pos) in before {
                let gone = match r.units.get(&id) {
                    None => true,
                    Some(unit) if unit.owner != owner => false,
                    Some(_) => continue,
                };
                let class = rules.units[kind].class.as_str();
                if class == "religious" {
                    continue;
                }
                if owner != seat && alive[owner] && !r.players[owner].alive && target != Some(pos) {
                    continue;
                }
                let military = class == "military";
                if owner != seat {
                    if military && gone && target.is_some() {
                        recount[seat].units_killed += 1;
                        charge_loss(&r, &mut recount[owner], seat);
                    } else if matches!(class, "civilian" | "support") && reaches {
                        recount[seat].civilians_taken += 1;
                        recount[owner].civilians_lost += 1;
                    }
                } else if military && gone && target.is_some() {
                    let killer = struck.unwrap_or_else(|| {
                        panic!("action {index}: {action:?} lost its striker to nobody")
                    });
                    recount[killer].units_killed += 1;
                    charge_loss(&r, &mut recount[seat], killer);
                }
            }
        }
        let counted = ledgers(&g);
        let total: u32 = counted.iter().map(|ledger| ledger.units_lost).sum();
        assert!(total > 0, "the game has to have a fight in it: {counted:?}");
        for seat in 0..seats {
            assert_eq!(
                counted[seat], recount[seat],
                "seat {seat} ({}): counted, then recounted from the log",
                g.players[seat].civ
            );
        }
        assert_eq!(
            counted
                .iter()
                .map(|ledger| ledger.units_killed)
                .sum::<u32>(),
            total,
            "every military loss is somebody's kill"
        );
        // The edge ledger: every blow is booked once taken and once struck,
        // under both gaps, and its kills are a part of the kills.
        let sum = |pick: &dyn Fn(&EdgeLedger) -> &EdgeTable| {
            let mut all = EdgeCell::default();
            for pid in 0..seats {
                pick(&g.combat_ledger(pid).edge)
                    .iter()
                    .flatten()
                    .for_each(|cell| all.absorb(cell));
            }
            all
        };
        let taken = sum(&|edge| &edge.taken_base);
        assert!(taken.blows > 0, "the game has to have a blow in it");
        assert_eq!(taken, sum(&|edge| &edge.taken_total));
        assert_eq!(taken, sum(&|edge| &edge.struck_base));
        assert_eq!(taken, sum(&|edge| &edge.struck_total));
        for seat in 0..seats {
            let edge = g.combat_ledger(seat).edge;
            let majors: i64 = edge.taken_base[0]
                .iter()
                .map(|cell| i64::from(cell.blows))
                .sum();
            let bucketed = |buckets: &[[i64; 2]; 4]| buckets.iter().map(|b| b[0]).sum::<i64>();
            assert_eq!(bucketed(&edge.major_gap_by_turn), majors, "seat {seat}");
            assert_eq!(
                bucketed(&edge.major_gap_by_online_turn),
                majors,
                "seat {seat}"
            );
            let kills = edge
                .struck_base
                .iter()
                .flatten()
                .map(|cell| cell.defender_killed)
                .sum::<u32>()
                + edge
                    .taken_base
                    .iter()
                    .flatten()
                    .map(|cell| cell.attacker_killed)
                    .sum::<u32>();
            assert!(
                kills <= g.combat_ledger(seat).units_killed,
                "seat {seat}: {kills} unit-on-unit kills beside {:?}",
                g.combat_ledger(seat)
            );
        }
    }
}
