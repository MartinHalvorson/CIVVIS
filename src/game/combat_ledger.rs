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
}

impl CombatLedger {
    /// Nothing counted yet: such a ledger is left out of a save.
    pub fn is_empty(&self) -> bool {
        *self == CombatLedger::default()
    }
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

    fn ledgers(g: &Game) -> Vec<CombatLedger> {
        (0..g.players.len())
            .map(|pid| g.combat_ledger(pid))
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
        let text = serde_json::to_string(&g).expect("a game serializes");
        let back: Game = serde_json::from_str(&text).expect("a game parses");
        assert_eq!(back.combat_ledger(0), g.combat_ledger(0));
        assert_eq!(back.combat_ledger(1), CombatLedger::default());
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
    }
}
